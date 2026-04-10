/// reporter.rs — Output formatting for lint results.
///
/// Supports three output modes:
///
/// 1. **Text** (default): Human-readable columnar format, modelled after ESLint:
///
///    ```
///    src/App.tsx:12:5   warning  no_inline_jsx_fn   Inline function in JSX...
///    src/Dashboard.tsx:1:1   warning  large_component   Component is 420 lines...
///
///    ✖ 2 warnings found
///    ```
///
/// 2. **JSON**: Machine-readable array of issue objects, suitable for CI
///    tooling, editors, or piping into `jq`:
///
///    ```json
///    [
///      {
///        "rule": "no_inline_jsx_fn",
///        "message": "...",
///        "file": "src/App.tsx",
///        "line": 12,
///        "column": 5,
///        "severity": "warning"
///      }
///    ]
///    ```
///
/// 3. **HTML**: Self-contained dark-mode report with summary stats, per-rule
///    breakdown cards, a top-10 files bar chart, and a collapsible issue table
///    grouped by file. Written to a file via `--output` (defaults to
///    `react-perf-report.html`). No CDN dependencies — one portable `.html` file.
use std::path::Path;

use crate::rules::Issue;

// ─── Text reporter ────────────────────────────────────────────────────────────

/// Print issues to stdout in human-readable columnar format.
///
/// Issues are sorted by file path then line number so the output is
/// predictable and easy to scan.
///
/// Returns the total number of issues printed (used for the summary line).
pub fn report_text(issues: &[Issue]) -> usize {
    if issues.is_empty() {
        println!("✓ No performance issues found.");
        return 0;
    }

    // Sort a local copy: by file path, then line, then column.
    let mut sorted = issues.to_vec();
    sorted.sort_by(|a, b| {
        a.file
            .cmp(&b.file)
            .then(a.line.cmp(&b.line))
            .then(a.column.cmp(&b.column))
    });

    let mut current_file: Option<&Path> = None;

    for issue in &sorted {
        // Print a blank-line-separated file header on first occurrence.
        if current_file != Some(&issue.file) {
            if current_file.is_some() {
                println!(); // blank line between files
            }
            current_file = Some(&issue.file);
        }

        // Format:  path:line:col   severity   rule_name   message
        // Column widths chosen to mirror ESLint's default output.
        println!(
            "{file}:{line}:{col}  {severity}  {rule:<22}  {message}",
            file = issue.file.display(),
            line = issue.line,
            col = issue.column,
            severity = issue.severity,
            rule = issue.rule,
            message = issue.message,
        );
    }

    // Summary line.
    println!();
    let count = sorted.len();
    let label = if count == 1 { "issue" } else { "issues" };
    println!("✖ {count} {label} found");

    count
}

// ─── JSON reporter ────────────────────────────────────────────────────────────

/// Serialize issues to a pretty-printed JSON array on stdout.
///
/// Uses `serde_json` for serialization. File paths are serialized as
/// their display strings (forward-slash on Unix, backslash on Windows).
pub fn report_json(issues: &[Issue]) -> usize {
    // We need to serialize the file path as a string, not a PathBuf.
    // Build a simple wrapper struct for serde.
    #[derive(serde::Serialize)]
    struct JsonIssue<'a> {
        rule: &'a str,
        message: &'a str,
        file: String,
        line: u32,
        column: u32,
        severity: &'a crate::rules::Severity,
    }

    let json_issues: Vec<JsonIssue<'_>> = issues
        .iter()
        .map(|i| JsonIssue {
            rule: &i.rule,
            message: &i.message,
            file: i.file.display().to_string(),
            line: i.line,
            column: i.column,
            severity: &i.severity,
        })
        .collect();

    match serde_json::to_string_pretty(&json_issues) {
        Ok(json) => println!("{json}"),
        Err(err) => eprintln!("Error serializing JSON output: {err}"),
    }

    issues.len()
}

// ─── HTML reporter ────────────────────────────────────────────────────────────

/// Generate a self-contained HTML report string.
///
/// The report includes:
/// - Header with scan path, timestamp, and summary stats
/// - Rule-breakdown cards with issue counts and colour coding
/// - Top 10 most-affected files bar chart (pure CSS)
/// - Full issue table grouped by file with collapsible sections
///
/// No external CDN dependencies — the returned string is a complete `.html` file.
pub fn report_html(issues: &[Issue], scanned_path: &std::path::Path, file_count: usize) -> String {
    use std::collections::HashMap;

    // ── Aggregate stats ───────────────────────────────────────────────────────

    let total = issues.len();
    let mut by_rule: HashMap<&str, usize> = HashMap::new();
    let mut by_file: HashMap<String, Vec<&Issue>> = HashMap::new();

    for issue in issues {
        *by_rule.entry(issue.rule.as_str()).or_default() += 1;
        by_file
            .entry(issue.file.display().to_string())
            .or_default()
            .push(issue);
    }

    // Sort files by descending issue count.
    let mut file_list: Vec<(String, Vec<&Issue>)> = by_file.into_iter().collect();
    file_list.sort_by(|a, b| b.1.len().cmp(&a.1.len()).then(a.0.cmp(&b.0)));

    // Top 10 files for the bar chart.
    let top_files: Vec<(&str, usize)> = file_list
        .iter()
        .take(10)
        .map(|(f, issues)| (f.as_str(), issues.len()))
        .collect();

    let max_bar = top_files.first().map(|(_, n)| *n).unwrap_or(1);

    // Rule metadata: colour + friendly label.
    let rule_meta: &[(&str, &str, &str)] = &[
        ("unstable_props", "#f97316", "Unstable Props"),
        ("no_inline_jsx_fn", "#ef4444", "Inline JSX Function"),
        ("no_array_index_key", "#eab308", "Array Index Key"),
        ("large_component", "#8b5cf6", "Large Component"),
        ("no_new_context_value", "#3b82f6", "New Context Value"),
        ("no_expensive_in_render", "#06b6d4", "Expensive in Render"),
    ];

    fn rule_color<'a>(rule: &str, meta: &[(&'a str, &'a str, &'a str)]) -> &'a str {
        meta.iter()
            .find(|(r, _, _)| *r == rule)
            .map(|(_, c, _)| *c)
            .unwrap_or("#6b7280")
    }

    fn rule_label<'a>(rule: &'a str, meta: &[(&'a str, &'a str, &'a str)]) -> &'a str {
        meta.iter()
            .find(|(r, _, _)| *r == rule)
            .map(|(_, _, l)| *l)
            .unwrap_or(rule)
    }

    // ── Build rule cards HTML ──────────────────────────────────────────────────

    let mut rule_cards = String::new();
    let mut rule_rows: Vec<(&str, usize)> = by_rule.iter().map(|(r, c)| (*r, *c)).collect();
    rule_rows.sort_by(|a, b| b.1.cmp(&a.1));

    for (rule, count) in &rule_rows {
        let color = rule_color(rule, rule_meta);
        let label = rule_label(rule, rule_meta);
        rule_cards.push_str(&format!(
            r#"<div class="card" style="border-left:4px solid {color}">
              <div class="card-count" style="color:{color}">{count}</div>
              <div class="card-rule">{rule}</div>
              <div class="card-label">{label}</div>
            </div>"#
        ));
    }

    // ── Build bar chart HTML ───────────────────────────────────────────────────

    let mut bar_rows = String::new();
    for (file, count) in &top_files {
        let pct = (*count as f64 / max_bar as f64 * 100.0) as usize;
        // Show only the last 2 path segments for readability.
        let short: String = std::path::Path::new(file)
            .components()
            .rev()
            .take(3)
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .map(|c| c.as_os_str().to_string_lossy().into_owned())
            .collect::<Vec<_>>()
            .join("/");
        bar_rows.push_str(&format!(
            r#"<div class="bar-row">
              <div class="bar-label" title="{file}">{short}</div>
              <div class="bar-track">
                <div class="bar-fill" style="width:{pct}%"></div>
              </div>
              <div class="bar-count">{count}</div>
            </div>"#
        ));
    }

    // ── Build issue table HTML ─────────────────────────────────────────────────

    let mut issue_sections = String::new();
    for (idx, (file, file_issues)) in file_list.iter().enumerate() {
        let mut sorted_issues = file_issues.clone();
        sorted_issues.sort_by_key(|i| (i.line, i.column));

        let mut rows = String::new();
        for issue in &sorted_issues {
            let color = rule_color(&issue.rule, rule_meta);
            let msg = html_escape(&issue.message);
            rows.push_str(&format!(
                r#"<tr>
                  <td class="td-loc">{line}:{col}</td>
                  <td><span class="badge" style="background:{color}">{rule}</span></td>
                  <td class="td-msg">{msg}</td>
                </tr>"#,
                line = issue.line,
                col = issue.column,
                rule = issue.rule,
            ));
        }

        let count = sorted_issues.len();
        let short_file = file
            .replace(scanned_path.to_string_lossy().as_ref(), ".")
            .trim_start_matches('/')
            .to_string();

        issue_sections.push_str(&format!(
            r#"<details id="file-{idx}">
              <summary>
                <span class="file-path">{short_file}</span>
                <span class="file-badge">{count}</span>
              </summary>
              <table class="issue-table">
                <thead><tr><th>Line</th><th>Rule</th><th>Message</th></tr></thead>
                <tbody>{rows}</tbody>
              </table>
            </details>"#
        ));
    }

    // ── Timestamp ─────────────────────────────────────────────────────────────

    let scan_path_str = scanned_path.display();
    let affected_files = file_list.len();

    // ── Assemble full HTML ────────────────────────────────────────────────────

    format!(
        r#"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="UTF-8">
<meta name="viewport" content="width=device-width, initial-scale=1.0">
<title>react-perf-analyzer Report</title>
<style>
  *, *::before, *::after {{ box-sizing: border-box; margin: 0; padding: 0; }}
  body {{ font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif;
         background: #0f172a; color: #e2e8f0; line-height: 1.5; font-size: 14px; }}
  .header {{ background: #1e293b; border-bottom: 1px solid #334155; padding: 24px 32px; }}
  .header h1 {{ font-size: 22px; font-weight: 700; color: #f1f5f9; }}
  .header h1 span {{ color: #38bdf8; }}
  .header .meta {{ margin-top: 6px; color: #94a3b8; font-size: 12px; }}
  .header .path {{ font-family: monospace; color: #7dd3fc; }}
  .main {{ padding: 24px 32px; max-width: 1400px; margin: 0 auto; }}

  /* Summary numbers */
  .summary {{ display: flex; gap: 16px; margin-bottom: 28px; flex-wrap: wrap; }}
  .stat {{ background: #1e293b; border: 1px solid #334155; border-radius: 10px;
           padding: 16px 24px; flex: 1; min-width: 140px; text-align: center; }}
  .stat-num {{ font-size: 36px; font-weight: 800; color: #f97316; }}
  .stat-lbl {{ font-size: 12px; color: #94a3b8; margin-top: 4px; text-transform: uppercase;
               letter-spacing: 0.05em; }}

  /* Rule cards */
  h2 {{ font-size: 15px; font-weight: 600; color: #cbd5e1; margin-bottom: 14px;
       text-transform: uppercase; letter-spacing: 0.05em; }}
  .cards {{ display: flex; gap: 12px; flex-wrap: wrap; margin-bottom: 32px; }}
  .card {{ background: #1e293b; border-radius: 10px; padding: 16px 20px; flex: 1;
           min-width: 160px; transition: transform .15s; cursor: default; }}
  .card:hover {{ transform: translateY(-2px); }}
  .card-count {{ font-size: 28px; font-weight: 800; }}
  .card-rule {{ font-family: monospace; font-size: 11px; color: #94a3b8; margin-top: 4px; }}
  .card-label {{ font-size: 12px; color: #cbd5e1; margin-top: 2px; }}

  /* Bar chart */
  .bars {{ background: #1e293b; border-radius: 10px; padding: 20px 24px;
           margin-bottom: 32px; }}
  .bar-row {{ display: flex; align-items: center; gap: 12px; margin-bottom: 10px; }}
  .bar-row:last-child {{ margin-bottom: 0; }}
  .bar-label {{ width: 260px; font-family: monospace; font-size: 11px; color: #94a3b8;
               white-space: nowrap; overflow: hidden; text-overflow: ellipsis;
               text-align: right; flex-shrink: 0; }}
  .bar-track {{ flex: 1; background: #0f172a; border-radius: 4px; height: 18px; overflow: hidden; }}
  .bar-fill {{ height: 100%; background: linear-gradient(90deg, #f97316, #ef4444);
              border-radius: 4px; transition: width .6s ease; }}
  .bar-count {{ width: 40px; text-align: right; font-weight: 600; color: #f8fafc;
               font-size: 12px; flex-shrink: 0; }}

  /* Issue sections */
  .issue-sections {{ display: flex; flex-direction: column; gap: 8px; }}
  details {{ background: #1e293b; border: 1px solid #334155; border-radius: 8px;
             overflow: hidden; }}
  details[open] {{ border-color: #475569; }}
  summary {{ display: flex; align-items: center; gap: 10px; padding: 12px 16px;
             cursor: pointer; list-style: none; user-select: none; }}
  summary::-webkit-details-marker {{ display: none; }}
  summary::before {{ content: '▶'; font-size: 10px; color: #64748b; transition: transform .2s;
                    flex-shrink: 0; }}
  details[open] summary::before {{ transform: rotate(90deg); }}
  .file-path {{ font-family: monospace; font-size: 12px; color: #7dd3fc; flex: 1;
               overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }}
  .file-badge {{ background: #f97316; color: white; font-size: 11px; font-weight: 700;
                border-radius: 999px; padding: 1px 8px; flex-shrink: 0; }}

  .issue-table {{ width: 100%; border-collapse: collapse; font-size: 12px; }}
  .issue-table th {{ background: #0f172a; color: #64748b; font-weight: 600;
                    padding: 8px 12px; text-align: left; font-size: 11px;
                    text-transform: uppercase; letter-spacing: 0.05em; }}
  .issue-table td {{ padding: 8px 12px; border-top: 1px solid #1e293b; vertical-align: top; }}
  .issue-table tr:hover td {{ background: #1e2a3a; }}
  .td-loc {{ font-family: monospace; color: #94a3b8; white-space: nowrap; width: 80px; }}
  .td-msg {{ color: #cbd5e1; }}
  .badge {{ display: inline-block; font-family: monospace; font-size: 10px; font-weight: 600;
           color: white; border-radius: 4px; padding: 2px 6px; white-space: nowrap; }}

  /* Footer */
  .footer {{ text-align: center; padding: 24px; color: #475569; font-size: 11px; }}
  .footer a {{ color: #38bdf8; text-decoration: none; }}
</style>
</head>
<body>
<div class="header">
  <h1>⚛️ <span>react-perf-analyzer</span> Report</h1>
  <div class="meta">
    Scanned: <span class="path">{scan_path_str}</span>
  </div>
</div>

<div class="main">

  <!-- Summary stats -->
  <div class="summary">
    <div class="stat"><div class="stat-num">{total}</div><div class="stat-lbl">Total Issues</div></div>
    <div class="stat"><div class="stat-num" style="color:#38bdf8">{file_count}</div><div class="stat-lbl">Files Scanned</div></div>
    <div class="stat"><div class="stat-num" style="color:#a78bfa">{affected_files}</div><div class="stat-lbl">Files with Issues</div></div>
  </div>

  <!-- Rule breakdown -->
  <h2>Issues by Rule</h2>
  <div class="cards">{rule_cards}</div>

  <!-- Top 10 files -->
  <h2>Top 10 Most Affected Files</h2>
  <div class="bars">{bar_rows}</div>

  <!-- Full issue list -->
  <h2>All Issues — {affected_files} files</h2>
  <div class="issue-sections">{issue_sections}</div>

</div>

<div class="footer">
  Generated by <a href="https://crates.io/crates/react-perf-analyzer">react-perf-analyzer</a>
  &nbsp;·&nbsp;
  <a href="https://github.com/rashvish18/react-perf-analyzer">GitHub</a>
</div>
</body>
</html>"#
    )
}

/// Escape characters that have special meaning in HTML.
fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

// ─── Summary helpers ──────────────────────────────────────────────────────────

/// Print a concise summary of per-rule issue counts to stderr.
///
/// Useful when `--format json` is used and you still want a human summary.
pub fn print_summary(issues: &[Issue]) {
    use std::collections::HashMap;

    if issues.is_empty() {
        return;
    }

    let mut counts: HashMap<&str, usize> = HashMap::new();
    for issue in issues {
        *counts.entry(issue.rule.as_str()).or_insert(0) += 1;
    }

    let mut pairs: Vec<(&&str, &usize)> = counts.iter().collect();
    pairs.sort_by_key(|(rule, _)| **rule);

    eprintln!("\nSummary:");
    for (rule, count) in pairs {
        eprintln!("  {rule:<24} {count} issue(s)");
    }
}
