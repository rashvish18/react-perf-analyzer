/// orchestrator.rs — Run external tools and merge results into our Issue type.
///
/// We own React-specific rules. Everything else is delegated:
///   - oxlint  → general JS/TS lint rules (400+)
///   - cargo-audit → Rust dependency CVEs
///
/// Both are invoked as subprocesses with JSON output, then parsed into
/// the same `Issue` type used by our own rules so the HTML report can
/// show everything in one unified view.
///
/// Design principles:
///   - Print "Running <tool>..." before each subprocess so the user sees progress
///   - If a tool is not in PATH → silently skip, print hint to stderr
///   - If a tool fails → print warning, return empty Vec (never crash)
///   - All subprocess output is captured; nothing bleeds to stdout
use std::io::Write as _;
use std::path::Path;
use std::process::Command;
use std::time::Instant;

use crate::rules::{Issue, IssueCategory, IssueSource, Severity};

// ─── Orchestrator result ──────────────────────────────────────────────────────

pub struct OrchestratorResult {
    /// All issues from all external tools combined.
    pub issues: Vec<Issue>,
    /// Tools that were skipped (not installed) or failed.
    pub tools_skipped: Vec<(&'static str, String)>,
}

/// Run all available external tools against `path` and return merged results.
pub fn run_external_tools(path: &Path) -> OrchestratorResult {
    let mut all_issues: Vec<Issue> = vec![];
    let mut tools_skipped: Vec<(&'static str, String)> = vec![];

    // ── oxlint ────────────────────────────────────────────────────────────────
    eprint!("  🔍 Running oxlint...");
    let _ = std::io::stderr().flush();
    let t = Instant::now();

    match run_oxlint(path) {
        ToolResult::Ok(issues) => {
            let elapsed_ms = t.elapsed().as_millis();
            let count = issues.len();
            eprint!(
                "\r  ✅ oxlint — {count} issue(s) in {elapsed_ms}ms{}\n",
                " ".repeat(20)
            );
            all_issues.extend(issues);
        }
        ToolResult::NotInstalled => {
            eprint!("\r  ⚠  oxlint not found{}\n", " ".repeat(30));
            tools_skipped.push(("oxlint", "not found — install: npm i -g oxlint".into()));
        }
        ToolResult::Failed(msg) => {
            eprint!("\r  ⚠  oxlint failed{}\n", " ".repeat(30));
            tools_skipped.push(("oxlint", format!("failed: {msg}")));
        }
    }

    // ── cargo-audit ───────────────────────────────────────────────────────────
    // Only runs if a Cargo.lock file exists in the scanned path.
    if path.join("Cargo.lock").exists() {
        eprint!("  🔍 Running cargo-audit...");
        let _ = std::io::stderr().flush();
        let t = Instant::now();

        match run_cargo_audit(path) {
            ToolResult::Ok(issues) => {
                let elapsed_ms = t.elapsed().as_millis();
                let count = issues.len();
                eprint!(
                    "\r  ✅ cargo-audit — {count} issue(s) in {elapsed_ms}ms{}\n",
                    " ".repeat(10)
                );
                all_issues.extend(issues);
            }
            ToolResult::NotInstalled => {
                eprint!("\r  ⚠  cargo-audit not found{}\n", " ".repeat(20));
                tools_skipped.push((
                    "cargo-audit",
                    "not found — install: cargo install cargo-audit".into(),
                ));
            }
            ToolResult::Failed(msg) => {
                eprint!("\r  ⚠  cargo-audit failed{}\n", " ".repeat(20));
                tools_skipped.push(("cargo-audit", format!("failed: {msg}")));
            }
        }
    }

    OrchestratorResult {
        issues: all_issues,
        tools_skipped,
    }
}

// ─── Internal result type ─────────────────────────────────────────────────────

enum ToolResult {
    Ok(Vec<Issue>),
    NotInstalled,
    Failed(String),
}

// ─── oxlint ───────────────────────────────────────────────────────────────────

/// Run `oxlint --format json <path>` and parse the output.
///
/// oxlint JSON schema (relevant fields):
/// ```json
/// {
///   "diagnostics": [
///     {
///       "message": "...",
///       "code": "eslint(no-unused-vars)",
///       "severity": "warning",
///       "filename": "src/foo.ts",
///       "labels": [{ "span": { "line": 12, "column": 5 } }]
///     }
///   ]
/// }
/// ```
fn run_oxlint(path: &Path) -> ToolResult {
    let path_str = match path.to_str() {
        Some(s) => s,
        None => return ToolResult::Failed("invalid path".into()),
    };

    let output = Command::new("oxlint")
        .args(["--format", "json", path_str])
        .output();

    let output = match output {
        Ok(o) => o,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            // Try npx fallback
            let npx_args = ["oxlint", "--format", "json", path_str];
            match Command::new("npx").args(npx_args).output() {
                Ok(o) => o,
                Err(_) => return ToolResult::NotInstalled,
            }
        }
        Err(e) => return ToolResult::Failed(e.to_string()),
    };

    // oxlint exits 1 when issues are found — that's normal, not an error.
    let stdout = String::from_utf8_lossy(&output.stdout);

    if stdout.trim().is_empty() {
        return ToolResult::Ok(vec![]);
    }

    parse_oxlint_json(&stdout, path)
}

fn parse_oxlint_json(json: &str, base_path: &Path) -> ToolResult {
    // Parse with serde_json into a flexible Value first.
    let value: serde_json::Value = match serde_json::from_str(json) {
        Ok(v) => v,
        Err(e) => return ToolResult::Failed(format!("JSON parse error: {e}")),
    };

    let diagnostics = match value.get("diagnostics").and_then(|d| d.as_array()) {
        Some(d) => d,
        None => return ToolResult::Ok(vec![]),
    };

    let mut issues = vec![];

    for diag in diagnostics {
        let message = diag
            .get("message")
            .and_then(|m| m.as_str())
            .unwrap_or("")
            .to_string();

        let rule = diag
            .get("code")
            .and_then(|c| c.as_str())
            .unwrap_or("oxlint")
            // Strip the "eslint(...)" wrapper → keep just the rule name
            .trim_start_matches("eslint(")
            .trim_start_matches("oxc(")
            .trim_end_matches(')')
            .to_string();

        let severity_str = diag
            .get("severity")
            .and_then(|s| s.as_str())
            .unwrap_or("warning");

        let severity = match severity_str {
            "error" => Severity::High,
            _ => Severity::Low,
        };

        let filename = diag.get("filename").and_then(|f| f.as_str()).unwrap_or("");

        // Resolve file path relative to base_path if needed.
        let file_path = if std::path::Path::new(filename).is_absolute() {
            std::path::PathBuf::from(filename)
        } else {
            base_path.join(filename)
        };

        // Location from first label's span.
        let (line, column) = diag
            .get("labels")
            .and_then(|l| l.as_array())
            .and_then(|l| l.first())
            .and_then(|l| l.get("span"))
            .map(|span| {
                let line = span.get("line").and_then(|v| v.as_u64()).unwrap_or(1) as u32;
                let col = span.get("column").and_then(|v| v.as_u64()).unwrap_or(1) as u32;
                (line, col)
            })
            .unwrap_or((1, 1));

        if message.is_empty() || filename.is_empty() {
            continue;
        }

        issues.push(Issue {
            rule,
            message,
            file: file_path,
            line,
            column,
            severity,
            source: IssueSource::OxcLinter,
            category: IssueCategory::Performance, // oxlint mixes perf + style
        });
    }

    ToolResult::Ok(issues)
}

// ─── cargo-audit ──────────────────────────────────────────────────────────────

/// Run `cargo audit --json` and parse RUSTSEC advisories.
///
/// cargo-audit JSON schema (relevant fields):
/// ```json
/// {
///   "vulnerabilities": {
///     "list": [
///       {
///         "advisory": {
///           "id": "RUSTSEC-2024-0001",
///           "title": "...",
///           "description": "...",
///           "cvss": "CVSS:3.1/AV:N/.../7.5"
///         },
///         "package": {
///           "name": "...",
///           "version": "...",
///           "manifest_path": "/path/to/Cargo.toml"
///         }
///       }
///     ]
///   }
/// }
/// ```
fn run_cargo_audit(path: &Path) -> ToolResult {
    let output = Command::new("cargo")
        .args(["audit", "--json"])
        .current_dir(path)
        .output();

    let output = match output {
        Ok(o) => o,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return ToolResult::NotInstalled,
        Err(e) => return ToolResult::Failed(e.to_string()),
    };

    // cargo audit exits 1 when vulnerabilities are found — that's normal.
    let stdout = String::from_utf8_lossy(&output.stdout);

    if stdout.trim().is_empty() {
        return ToolResult::Ok(vec![]);
    }

    // Check for "no such command: audit" error (cargo-audit not installed).
    let stderr = String::from_utf8_lossy(&output.stderr);
    if stderr.contains("no such command") || stderr.contains("unknown subcommand") {
        return ToolResult::NotInstalled;
    }

    parse_cargo_audit_json(&stdout, path)
}

fn parse_cargo_audit_json(json: &str, base_path: &Path) -> ToolResult {
    let value: serde_json::Value = match serde_json::from_str(json) {
        Ok(v) => v,
        Err(e) => return ToolResult::Failed(format!("JSON parse error: {e}")),
    };

    let vuln_list = value
        .pointer("/vulnerabilities/list")
        .and_then(|l| l.as_array());

    let Some(vulns) = vuln_list else {
        return ToolResult::Ok(vec![]);
    };

    let mut issues = vec![];

    for vuln in vulns {
        let advisory = match vuln.get("advisory") {
            Some(a) => a,
            None => continue,
        };

        let id = advisory
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or("RUSTSEC-UNKNOWN");

        let title = advisory
            .get("title")
            .and_then(|v| v.as_str())
            .unwrap_or("Unknown vulnerability");

        let pkg_name = vuln
            .pointer("/package/name")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");

        let pkg_version = vuln
            .pointer("/package/version")
            .and_then(|v| v.as_str())
            .unwrap_or("?");

        // Derive severity from CVSS score if available.
        let severity = advisory
            .get("cvss")
            .and_then(|c| c.as_str())
            .and_then(extract_cvss_score)
            .map(|score| {
                if score >= 9.0 {
                    Severity::Critical
                } else if score >= 7.0 {
                    Severity::High
                } else if score >= 4.0 {
                    Severity::Medium
                } else {
                    Severity::Low
                }
            })
            .unwrap_or(Severity::High); // default High when no CVSS

        // Point to Cargo.lock as the file location.
        let cargo_lock = base_path.join("Cargo.lock");

        issues.push(Issue {
            rule: id.to_string(),
            message: format!("{id}: {title} in {pkg_name} v{pkg_version}"),
            file: cargo_lock,
            line: 1,
            column: 1,
            severity,
            source: IssueSource::CargoAudit,
            category: IssueCategory::Dependency,
        });
    }

    ToolResult::Ok(issues)
}

/// Extract the numeric CVSS base score from a CVSS vector string.
/// Example: "CVSS:3.1/AV:N/AC:L/PR:N/UI:N/S:U/C:H/I:H/A:H" → None
/// (We look for the /score suffix some tools append, or default to None.)
fn extract_cvss_score(cvss: &str) -> Option<f64> {
    // Some tools append the score as the last component: ".../7.5"
    cvss.rsplit('/')
        .next()
        .and_then(|s| s.parse::<f64>().ok())
        .filter(|&s| s > 0.0 && s <= 10.0)
}
