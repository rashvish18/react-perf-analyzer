/// custom_rules.rs — TOML-based custom rule DSL engine.
///
/// Lets teams define their own lint rules in a TOML file with **no Rust
/// required**. The engine does text-level pattern matching (regex) over
/// the raw source of each file — intentionally simpler than the AST-based
/// built-in rules, but powerful enough for enforcing team conventions.
///
/// # Rule file format
///
/// Place a `react-perf-rules.toml` (or pass `--rules <file>`) in your
/// project root. Each `[[rule]]` entry specifies:
///
/// ```toml
/// [[rule]]
/// id       = "no-console-log"
/// message  = "Remove console.log before merging"
/// severity = "medium"            # info | low | medium | high | critical
/// category = "perf"              # perf | security
/// pattern  = "console\\.log\\s*\\("   # regex (Rust syntax)
///
/// # Optional: only run on files matching this glob pattern
/// file_glob = "**/*.{ts,tsx}"
///
/// # Optional: ignore lines that also match this pattern
/// ignore_if  = "#\\s*nolint"
/// ```
///
/// # Engine behaviour
///
/// For each file the engine:
/// 1. Loads custom rules from TOML (or `react-perf-rules.toml` by default)
/// 2. Filters rules by `file_glob` if specified
/// 3. Scans each line for `pattern` (case-sensitive regex)
/// 4. If `ignore_if` is set and the line also matches, the hit is skipped
/// 5. Emits an `Issue` for every match
///
/// # Limitations
///
/// - Patterns are line-by-line (no multi-line spans)
/// - No AST context — the engine cannot distinguish e.g. a comment from code
/// - For complex structural checks, write a built-in Rust rule instead
use std::path::Path;

use crate::rules::{Issue, IssueCategory, IssueSource, Severity};

// ─── TOML schema ──────────────────────────────────────────────────────────────

#[derive(serde::Deserialize, Debug)]
struct RuleFile {
    #[serde(rename = "rule", default)]
    rules: Vec<CustomRuleDef>,
}

#[derive(serde::Deserialize, Debug, Clone)]
pub struct CustomRuleDef {
    /// Unique rule identifier (used in issue output and HTML report).
    pub id: String,
    /// Human-readable message shown to developers.
    pub message: String,
    /// Regex pattern to search for (Rust regex syntax).
    pub pattern: String,
    /// Severity level: info | low | medium | high | critical.
    #[serde(default = "default_severity")]
    pub severity: String,
    /// Category: perf | security.
    #[serde(default = "default_category")]
    pub category: String,
    /// Optional glob pattern for file filtering (e.g. `"**/*.tsx"`).
    pub file_glob: Option<String>,
    /// Optional regex: if a line also matches this, the hit is suppressed.
    pub ignore_if: Option<String>,
}

fn default_severity() -> String {
    "medium".to_string()
}
fn default_category() -> String {
    "perf".to_string()
}

// ─── Compiled rule ────────────────────────────────────────────────────────────

/// A parsed + compiled custom rule, ready to scan files.
pub struct CompiledRule {
    pub def: CustomRuleDef,
    pattern: regex_lite::Regex,
    ignore_if: Option<regex_lite::Regex>,
}

// ─── Public API ───────────────────────────────────────────────────────────────

/// Load custom rules from `path`. Returns `(rules, errors)`.
///
/// Parse errors for individual regex patterns are collected and returned
/// rather than aborting so that a bad pattern in one rule doesn't silence
/// all other rules.
pub fn load_custom_rules(path: &Path) -> (Vec<CompiledRule>, Vec<String>) {
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) => {
            return (
                vec![],
                vec![format!("could not read '{}': {e}", path.display())],
            );
        }
    };

    let rule_file: RuleFile = match basic_toml::from_str(&text) {
        Ok(f) => f,
        Err(e) => {
            return (
                vec![],
                vec![format!("could not parse '{}': {e}", path.display())],
            );
        }
    };

    let mut compiled = Vec::new();
    let mut errors = Vec::new();

    for def in rule_file.rules {
        let pattern = match regex_lite::Regex::new(&def.pattern) {
            Ok(r) => r,
            Err(e) => {
                errors.push(format!(
                    "rule '{}': invalid pattern '{}': {e}",
                    def.id, def.pattern
                ));
                continue;
            }
        };

        let ignore_if = match def.ignore_if.as_deref() {
            Some(ign) => match regex_lite::Regex::new(ign) {
                Ok(r) => Some(r),
                Err(e) => {
                    errors.push(format!(
                        "rule '{}': invalid ignore_if '{}': {e}",
                        def.id, ign
                    ));
                    None
                }
            },
            None => None,
        };

        compiled.push(CompiledRule {
            def,
            pattern,
            ignore_if,
        });
    }

    (compiled, errors)
}

/// Scan a single file's source text against all compiled custom rules.
///
/// Returns issues for every line that matches a rule's `pattern` (and does
/// not match the rule's `ignore_if` pattern if one is set).
pub fn run_custom_rules(rules: &[CompiledRule], source: &str, file_path: &Path) -> Vec<Issue> {
    let mut issues = Vec::new();

    // Filter rules to those applicable to this file (via file_glob).
    let applicable: Vec<&CompiledRule> = rules
        .iter()
        .filter(|r| file_matches_glob(file_path, r.def.file_glob.as_deref()))
        .collect();

    if applicable.is_empty() {
        return issues;
    }

    for (line_idx, line) in source.lines().enumerate() {
        let line_no = (line_idx + 1) as u32;

        for rule in &applicable {
            if !rule.pattern.is_match(line) {
                continue;
            }
            // Check ignore_if.
            if let Some(ref ign) = rule.ignore_if {
                if ign.is_match(line) {
                    continue;
                }
            }
            // Find column of first match.
            let col = rule
                .pattern
                .find(line)
                .map(|m| m.start() as u32 + 1)
                .unwrap_or(1);

            issues.push(Issue {
                rule: rule.def.id.clone(),
                message: rule.def.message.clone(),
                file: file_path.to_path_buf(),
                line: line_no,
                column: col,
                severity: parse_severity(&rule.def.severity),
                source: IssueSource::ReactPerfAnalyzer,
                category: parse_category(&rule.def.category),
            });
        }
    }

    issues
}

/// Try to locate the default custom rules file in `base` or any ancestor.
///
/// Looks for `react-perf-rules.toml` walking up from `base` toward the
/// filesystem root. Returns `None` if not found.
pub fn find_default_rules_file(base: &Path) -> Option<std::path::PathBuf> {
    let filename = "react-perf-rules.toml";
    let mut dir = if base.is_file() {
        base.parent()?.to_path_buf()
    } else {
        base.to_path_buf()
    };
    loop {
        let candidate = dir.join(filename);
        if candidate.exists() {
            return Some(candidate);
        }
        if !dir.pop() {
            return None;
        }
    }
}

// ─── Helpers ──────────────────────────────────────────────────────────────────

fn parse_severity(s: &str) -> Severity {
    match s.to_lowercase().as_str() {
        "critical" => Severity::Critical,
        "high" => Severity::High,
        "low" => Severity::Low,
        "info" => Severity::Info,
        _ => Severity::Medium,
    }
}

fn parse_category(s: &str) -> IssueCategory {
    match s.to_lowercase().as_str() {
        "security" => IssueCategory::Security,
        _ => IssueCategory::Performance,
    }
}

/// Returns `true` when `file` matches `glob_pattern` (or when `glob_pattern` is `None`).
fn file_matches_glob(file: &Path, glob_pattern: Option<&str>) -> bool {
    let pattern = match glob_pattern {
        Some(p) => p,
        None => return true, // No glob = apply to all files.
    };
    // Build a glob matcher. Fall back to "apply to all" on invalid patterns.
    let matcher = match glob::Pattern::new(pattern) {
        Ok(m) => m,
        Err(_) => return true,
    };
    // Match against the file's display string.
    matcher.matches_path_with(
        file,
        glob::MatchOptions {
            case_sensitive: true,
            require_literal_separator: false,
            require_literal_leading_dot: false,
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn make_rule(pattern: &str, ignore_if: Option<&str>) -> CompiledRule {
        CompiledRule {
            def: CustomRuleDef {
                id: "test-rule".to_string(),
                message: "test message".to_string(),
                pattern: pattern.to_string(),
                severity: "medium".to_string(),
                category: "perf".to_string(),
                file_glob: None,
                ignore_if: ignore_if.map(str::to_string),
            },
            pattern: regex_lite::Regex::new(pattern).unwrap(),
            ignore_if: ignore_if.map(|p| regex_lite::Regex::new(p).unwrap()),
        }
    }

    #[test]
    fn detects_pattern_match() {
        let rules = vec![make_rule(r"console\.log\s*\(", None)];
        let src = "const x = 1;\nconsole.log(x);\nreturn x;";
        let issues = run_custom_rules(&rules, src, Path::new("test.tsx"));
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].line, 2);
    }

    #[test]
    fn ignore_if_suppresses_hit() {
        let rules = vec![make_rule(r"console\.log\s*\(", Some(r"// nolint"))];
        let src = "console.log(x); // nolint\nconsole.log(y);";
        let issues = run_custom_rules(&rules, src, Path::new("test.tsx"));
        assert_eq!(issues.len(), 1);
        assert_eq!(issues[0].line, 2);
    }

    #[test]
    fn no_match_returns_empty() {
        let rules = vec![make_rule(r"eval\s*\(", None)];
        let src = "const x = 1 + 2;";
        let issues = run_custom_rules(&rules, src, Path::new("test.tsx"));
        assert!(issues.is_empty());
    }

    #[test]
    fn file_glob_filters_correctly() {
        let mut rule = make_rule(r"TODO", None);
        rule.def.file_glob = Some("**/*.test.tsx".to_string());

        // Should match
        let issues = run_custom_rules(&[rule], "// TODO: fix this", Path::new("src/App.test.tsx"));
        assert_eq!(issues.len(), 1);
    }

    #[test]
    fn toml_roundtrip() {
        let toml_src = r#"
[[rule]]
id       = "no-console"
message  = "No console.log in production code"
pattern  = "console\\.log\\s*\\("
severity = "high"
category = "perf"
"#;
        let rule_file: RuleFile = basic_toml::from_str(toml_src).unwrap();
        assert_eq!(rule_file.rules.len(), 1);
        assert_eq!(rule_file.rules[0].id, "no-console");
        assert_eq!(rule_file.rules[0].severity, "high");
    }
}
