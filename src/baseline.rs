/// baseline.rs — Suppress previously-known issues so CI only fails on regressions.
///
/// # Workflow
///
/// 1. **Generate baseline** (run once, commit the file):
///    ```sh
///    react-perf-analyzer ./src --format json --output .sast-baseline.json
///    ```
///
/// 2. **Use baseline in CI**:
///    ```sh
///    react-perf-analyzer ./src --baseline .sast-baseline.json --fail-on high
///    ```
///    Only issues that are *not* in the baseline cause a non-zero exit code.
///
/// # Matching strategy
///
/// An issue is considered "in baseline" when **all four** of these match:
/// - `rule` — exact string match
/// - `file` — path suffix match (handles absolute vs relative path differences)
/// - `line` — exact line number
/// - `column` — exact column number
///
/// This is intentionally strict to avoid silencing new issues that happen to
/// share a rule name with a baseline entry. Users can widen the path match
/// by editing the baseline JSON.
use std::path::Path;

use crate::rules::Issue;

/// A single entry in the baseline file (subset of Issue fields).
#[derive(serde::Deserialize, serde::Serialize, Clone, Debug)]
pub struct BaselineEntry {
    pub rule: String,
    pub file: String,
    pub line: u32,
    pub column: u32,
}

/// Load a baseline JSON file and return its entries.
///
/// Returns an empty `Vec` (and prints a warning) on any parse error so that
/// a corrupt baseline never silently suppresses all issues.
pub fn load_baseline(path: &Path) -> Vec<BaselineEntry> {
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("⚠  --baseline: could not read '{}': {e}", path.display());
            return vec![];
        }
    };

    // The baseline file is produced by --format json, which emits an array
    // of objects. We tolerate both the full Issue JSON shape and the minimal
    // BaselineEntry shape.
    match serde_json::from_str::<Vec<BaselineEntry>>(&text) {
        Ok(entries) => {
            eprintln!(
                "  📋 Baseline loaded: {} known issue(s) suppressed",
                entries.len()
            );
            entries
        }
        Err(e) => {
            eprintln!(
                "⚠  --baseline: could not parse '{}' as JSON: {e}. \
                 Ignoring baseline.",
                path.display()
            );
            vec![]
        }
    }
}

/// Returns `true` when `issue_path` and `entry_path` refer to the same file.
///
/// Handles absolute vs relative path mismatches by checking whether one path
/// ends with the other (component-wise, not just string-suffix). Non-path
/// components (root `/`, prefix `C:`) are stripped before comparison.
fn paths_match(issue_path: &Path, entry_path: &str) -> bool {
    use std::path::Component;
    let normalise = |p: &Path| -> Vec<String> {
        p.components()
            .filter(|c| !matches!(c, Component::Prefix(_) | Component::RootDir))
            .map(|c| c.as_os_str().to_string_lossy().into_owned())
            .collect()
    };
    let a = normalise(issue_path);
    let b = normalise(Path::new(entry_path));
    // Match if one is a path-component suffix of the other.
    let (longer, shorter) = if a.len() >= b.len() {
        (&a, &b)
    } else {
        (&b, &a)
    };
    let offset = longer.len() - shorter.len();
    longer[offset..] == shorter[..]
}

/// Returns `true` when `issue` matches `entry` on rule, file, line, and column.
fn entry_matches(issue: &Issue, entry: &BaselineEntry) -> bool {
    issue.rule == entry.rule
        && issue.line == entry.line
        && issue.column == entry.column
        && paths_match(&issue.file, &entry.file)
}

/// Filter `issues`, removing any that appear in the baseline.
///
/// Returns a new `Vec` containing only issues that are **not** in the baseline
/// (i.e. regressions / new issues). The count of suppressed issues is printed
/// to stderr so developers know the baseline is active.
///
/// Performance: O(n × m) where n = issue count, m = baseline size.
/// For typical codebases (< 5 000 issues, < 5 000 baseline entries) this is
/// well under 1 ms. If needed, the lookup can be indexed by rule in the future.
pub fn filter_baseline(issues: Vec<Issue>, entries: &[BaselineEntry]) -> Vec<Issue> {
    if entries.is_empty() {
        return issues;
    }
    let before = issues.len();
    let new_issues: Vec<Issue> = issues
        .into_iter()
        .filter(|i| !entries.iter().any(|e| entry_matches(i, e)))
        .collect();
    let suppressed = before - new_issues.len();
    if suppressed > 0 {
        eprintln!("  🔕 Baseline suppressed {suppressed} known issue(s).");
    }
    new_issues
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::{IssueCategory, IssueSource, Severity};
    use std::path::PathBuf;

    fn make_issue(rule: &str, file: &str, line: u32, col: u32) -> Issue {
        Issue {
            rule: rule.to_string(),
            message: "test".to_string(),
            file: PathBuf::from(file),
            line,
            column: col,
            severity: Severity::Medium,
            source: IssueSource::ReactPerfAnalyzer,
            category: IssueCategory::Performance,
        }
    }

    fn make_entry(rule: &str, file: &str, line: u32, col: u32) -> BaselineEntry {
        BaselineEntry {
            rule: rule.to_string(),
            file: file.to_string(),
            line,
            column: col,
        }
    }

    #[test]
    fn known_issue_is_suppressed() {
        let issues = vec![make_issue("no_inline_jsx_fn", "src/App.tsx", 10, 5)];
        let baseline = vec![make_entry("no_inline_jsx_fn", "src/App.tsx", 10, 5)];
        let result = filter_baseline(issues, &baseline);
        assert!(result.is_empty(), "known issue should be suppressed");
    }

    #[test]
    fn new_issue_survives() {
        let issues = vec![
            make_issue("no_inline_jsx_fn", "src/App.tsx", 10, 5),
            make_issue("no_inline_jsx_fn", "src/App.tsx", 99, 1), // new line
        ];
        let baseline = vec![make_entry("no_inline_jsx_fn", "src/App.tsx", 10, 5)];
        let result = filter_baseline(issues, &baseline);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].line, 99);
    }

    #[test]
    fn absolute_vs_relative_path_match() {
        let issues = vec![make_issue(
            "large_component",
            "/home/user/project/src/Dashboard.tsx",
            1,
            1,
        )];
        let baseline = vec![make_entry("large_component", "src/Dashboard.tsx", 1, 1)];
        let result = filter_baseline(issues, &baseline);
        assert!(
            result.is_empty(),
            "absolute and relative paths should match via suffix"
        );
    }

    #[test]
    fn empty_baseline_returns_all() {
        let issues = vec![make_issue("some_rule", "x.tsx", 1, 1)];
        let result = filter_baseline(issues, &[]);
        assert_eq!(result.len(), 1);
    }
}
