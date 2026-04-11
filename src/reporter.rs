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
pub fn report_html(
    issues: &[Issue],
    scanned_path: &std::path::Path,
    file_count: usize,
    external_ran: bool,
) -> String {
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

    // Rule metadata: colour + friendly label (all rules).
    let rule_meta: &[(&str, &str, &str)] = &[
        // ── Perf rules ──────────────────────────────────────────────────────
        ("unstable_props", "#f97316", "Unstable Props"),
        ("no_inline_jsx_fn", "#ef4444", "Inline JSX Function"),
        ("no_array_index_key", "#eab308", "Array Index Key"),
        ("large_component", "#8b5cf6", "Large Component"),
        ("no_new_context_value", "#3b82f6", "New Context Value"),
        ("no_expensive_in_render", "#06b6d4", "Expensive in Render"),
        (
            "no_component_in_component",
            "#ec4899",
            "Component in Component",
        ),
        ("no_unstable_hook_deps", "#f59e0b", "Unstable Hook Deps"),
        ("no_new_in_jsx_prop", "#10b981", "New in JSX Prop"),
        (
            "no_use_state_lazy_init_missing",
            "#6366f1",
            "useState Lazy Init",
        ),
        ("no_json_in_render", "#14b8a6", "JSON in Render"),
        (
            "no_object_entries_in_render",
            "#f43f5e",
            "Object.entries in Render",
        ),
        ("no_regex_in_render", "#a855f7", "Regex in Render"),
        (
            "no_math_random_in_render",
            "#0ea5e9",
            "Math.random in Render",
        ),
        ("no_useless_memo", "#84cc16", "Useless Memo"),
        // ── Security rules ──────────────────────────────────────────────────
        ("no_unsafe_href", "#dc2626", "Unsafe href"),
        ("no_xss_via_jsx_prop", "#b91c1c", "XSS via JSX Prop"),
        ("no_hardcoded_secret_in_jsx", "#7c3aed", "Hardcoded Secret"),
        (
            "no_dangerously_set_inner_html_unescaped",
            "#be123c",
            "Unsafe innerHTML",
        ),
        ("no_postmessage_wildcard", "#0369a1", "postMessage Wildcard"),
    ];

    // Source metadata: colour + label for source badges.
    fn source_badge(source: &crate::rules::IssueSource) -> (&'static str, &'static str) {
        match source {
            crate::rules::IssueSource::ReactPerfAnalyzer => ("#f97316", "react-perf-analyzer"),
            crate::rules::IssueSource::OxcLinter => ("#3b82f6", "oxlint"),
            crate::rules::IssueSource::CargoAudit => ("#dc2626", "cargo-audit"),
        }
    }

    // Severity colour for badge in issue rows.
    fn severity_color(sev: &crate::rules::Severity) -> &'static str {
        match sev {
            crate::rules::Severity::Critical => "#dc2626",
            crate::rules::Severity::High => "#ea580c",
            crate::rules::Severity::Medium => "#ca8a04",
            crate::rules::Severity::Low => "#16a34a",
            crate::rules::Severity::Info => "#6b7280",
        }
    }

    // Per-source counts for the header stats.
    let our_count = issues
        .iter()
        .filter(|i| matches!(i.source, crate::rules::IssueSource::ReactPerfAnalyzer))
        .count();
    let oxlint_count = issues
        .iter()
        .filter(|i| matches!(i.source, crate::rules::IssueSource::OxcLinter))
        .count();
    let audit_count = issues
        .iter()
        .filter(|i| matches!(i.source, crate::rules::IssueSource::CargoAudit))
        .count();

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
            r#"<div class="card" data-rule="{rule}" style="border-left:4px solid {color}" onclick="filterByRule(this,'{rule}')">
              <div class="card-count" style="color:{color}">{count}</div>
              <div class="card-rule">{rule}</div>
              <div class="card-label">{label}</div>
              <div class="card-hint">click to filter</div>
            </div>"#
        ));
    }

    // ── Build bar chart HTML ───────────────────────────────────────────────────

    let mut bar_rows = String::new();
    for (idx, (file, count)) in top_files.iter().enumerate() {
        let pct = (*count as f64 / max_bar as f64 * 100.0) as usize;
        // Show only the last 3 path segments for readability.
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
        // Find this file's position in file_list so we can link to its section.
        let file_idx = file_list
            .iter()
            .position(|(f, _)| f.as_str() == *file)
            .unwrap_or(idx);
        bar_rows.push_str(&format!(
            r#"<div class="bar-row" onclick="scrollToFile({file_idx})" style="cursor:pointer" title="Jump to {file}">
              <div class="bar-label">{short}</div>
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

        // Collect unique rules for this file (used by JS filter).
        let mut file_rules: Vec<&str> = sorted_issues.iter().map(|i| i.rule.as_str()).collect();
        file_rules.dedup();
        file_rules.sort_unstable();
        file_rules.dedup();
        let data_rules = file_rules.join(" ");

        let mut rows = String::new();
        for issue in &sorted_issues {
            let color = rule_color(&issue.rule, rule_meta);
            let sev_color = severity_color(&issue.severity);
            let sev_label = issue.severity.to_string();
            let (src_color, src_label) = source_badge(&issue.source);
            let msg = html_escape(&issue.message);
            rows.push_str(&format!(
                r#"<tr data-rule="{rule}">
                  <td class="td-loc">{line}:{col}</td>
                  <td>
                    <span class="badge" style="background:{color}">{rule}</span>
                    <span class="badge sev-badge" style="background:{sev_color}">{sev_label}</span>
                  </td>
                  <td class="td-msg">{msg}
                    <span class="src-badge" style="border-color:{src_color};color:{src_color}">{src_label}</span>
                  </td>
                </tr>"#,
                rule = issue.rule,
                line = issue.line,
                col = issue.column,
            ));
        }

        let count = sorted_issues.len();
        let short_file = file
            .replace(scanned_path.to_string_lossy().as_ref(), ".")
            .trim_start_matches('/')
            .to_string();

        issue_sections.push_str(&format!(
            r#"<details id="file-{idx}" data-rules="{data_rules}" data-file="{file}">
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

    // Build oxlint / cargo-audit stat tiles: N/A when external tools didn't run.
    let oxlint_tile = if external_ran {
        format!(
            r#"<div class="stat"><div class="stat-num" style="color:#3b82f6">{oxlint_count}</div><div class="stat-lbl">oxlint</div></div>"#
        )
    } else {
        r#"<div class="stat"><div class="stat-num" style="color:#475569;font-size:22px">N/A</div><div class="stat-lbl">oxlint</div></div>"#.to_string()
    };

    let audit_tile = if external_ran {
        format!(
            r#"<div class="stat"><div class="stat-num" style="color:#dc2626">{audit_count}</div><div class="stat-lbl">cargo-audit</div></div>"#
        )
    } else {
        r#"<div class="stat"><div class="stat-num" style="color:#475569;font-size:22px">N/A</div><div class="stat-lbl">cargo-audit</div></div>"#.to_string()
    };

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

  /* Section headers */
  .section-header {{ display: flex; align-items: center; justify-content: space-between;
                     margin-bottom: 14px; flex-wrap: wrap; gap: 10px; }}
  h2 {{ font-size: 15px; font-weight: 600; color: #cbd5e1;
       text-transform: uppercase; letter-spacing: 0.05em; }}

  /* Rule cards */
  .cards {{ display: flex; gap: 12px; flex-wrap: wrap; margin-bottom: 32px; }}
  .card {{ background: #1e293b; border-radius: 10px; padding: 16px 20px; flex: 1;
           min-width: 160px; transition: transform .15s, box-shadow .15s, opacity .15s;
           cursor: pointer; position: relative; }}
  .card:hover {{ transform: translateY(-2px); box-shadow: 0 4px 16px rgba(0,0,0,.4); }}
  .card.active {{ box-shadow: 0 0 0 2px #38bdf8; transform: translateY(-2px); }}
  .card.dimmed {{ opacity: .35; }}
  .card-count {{ font-size: 28px; font-weight: 800; }}
  .card-rule {{ font-family: monospace; font-size: 11px; color: #94a3b8; margin-top: 4px; }}
  .card-label {{ font-size: 12px; color: #cbd5e1; margin-top: 2px; }}
  .card-hint {{ font-size: 10px; color: #475569; margin-top: 6px; }}

  /* Active filter banner */
  #filter-banner {{ display: none; align-items: center; gap: 10px; background: #0c2240;
                    border: 1px solid #1e40af; border-radius: 8px; padding: 8px 16px;
                    margin-bottom: 16px; font-size: 13px; color: #93c5fd; }}
  #filter-banner.visible {{ display: flex; }}
  #filter-banner strong {{ color: #bfdbfe; }}
  #clear-filter {{ margin-left: auto; background: #1e3a5f; border: 1px solid #2563eb;
                   color: #93c5fd; padding: 3px 10px; border-radius: 6px; cursor: pointer;
                   font-size: 12px; }}
  #clear-filter:hover {{ background: #1d4ed8; color: white; }}

  /* Bar chart */
  .bars {{ background: #1e293b; border-radius: 10px; padding: 20px 24px;
           margin-bottom: 32px; }}
  .bar-row {{ display: flex; align-items: center; gap: 12px; margin-bottom: 10px;
              border-radius: 6px; padding: 4px; transition: background .15s; }}
  .bar-row:last-child {{ margin-bottom: 0; }}
  .bar-row:hover {{ background: #263045; }}
  .bar-label {{ width: 260px; font-family: monospace; font-size: 11px; color: #94a3b8;
               white-space: nowrap; overflow: hidden; text-overflow: ellipsis;
               text-align: right; flex-shrink: 0; }}
  .bar-track {{ flex: 1; background: #0f172a; border-radius: 4px; height: 18px; overflow: hidden; }}
  .bar-fill {{ height: 100%; background: linear-gradient(90deg, #f97316, #ef4444);
              border-radius: 4px; transition: width .6s ease; }}
  .bar-count {{ width: 40px; text-align: right; font-weight: 600; color: #f8fafc;
               font-size: 12px; flex-shrink: 0; }}

  /* Issue section toolbar */
  .issues-toolbar {{ display: flex; align-items: center; gap: 10px; flex-wrap: wrap; }}
  .search-box {{ background: #1e293b; border: 1px solid #334155; color: #e2e8f0;
                border-radius: 6px; padding: 6px 12px; font-size: 12px; width: 260px;
                outline: none; }}
  .search-box:focus {{ border-color: #38bdf8; }}
  .search-box::placeholder {{ color: #475569; }}
  .btn {{ background: #1e293b; border: 1px solid #334155; color: #94a3b8;
          border-radius: 6px; padding: 5px 12px; cursor: pointer; font-size: 12px; }}
  .btn:hover {{ background: #273549; color: #e2e8f0; }}
  #visible-count {{ font-size: 12px; color: #64748b; margin-left: auto; }}

  /* Issue sections */
  .issue-sections {{ display: flex; flex-direction: column; gap: 8px; }}
  details {{ background: #1e293b; border: 1px solid #334155; border-radius: 8px;
             overflow: hidden; }}
  details[open] {{ border-color: #475569; }}
  details.hidden-section {{ display: none; }}
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
  .issue-table td {{ padding: 8px 12px; border-top: 1px solid #0f172a; vertical-align: top; }}
  .issue-table tr:hover td {{ background: #1e2a3a; }}
  .issue-table tr.row-hidden {{ display: none; }}
  .td-loc {{ font-family: monospace; color: #94a3b8; white-space: nowrap; width: 80px; }}
  .td-msg {{ color: #cbd5e1; }}
  .badge {{ display: inline-block; font-family: monospace; font-size: 10px; font-weight: 600;
           color: white; border-radius: 4px; padding: 2px 6px; white-space: nowrap; }}
  .sev-badge {{ margin-left: 4px; font-size: 10px; font-weight: 700; }}
  .src-badge {{ display: inline-block; font-size: 10px; font-weight: 600;
               border: 1px solid; border-radius: 4px; padding: 1px 5px;
               margin-left: 6px; white-space: nowrap; vertical-align: middle; }}

  /* No results */
  #no-results {{ display: none; text-align: center; padding: 40px; color: #475569;
                 font-size: 14px; }}
  #no-results.visible {{ display: block; }}

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
    <div class="stat"><div class="stat-num" style="color:#f97316">{our_count}</div><div class="stat-lbl">React Rules</div></div>
    {oxlint_tile}
    {audit_tile}
  </div>

  <!-- Rule breakdown -->
  <div class="section-header">
    <h2>Issues by Rule</h2>
    <span style="font-size:12px;color:#475569">Click a card to filter issues below</span>
  </div>
  <div class="cards">{rule_cards}</div>

  <!-- Active filter banner -->
  <div id="filter-banner">
    Showing issues for rule: <strong id="filter-rule-name"></strong>
    <button id="clear-filter" onclick="clearFilter()">✕ Show All</button>
  </div>

  <!-- Top 10 files -->
  <h2 style="margin-bottom:14px">Top 10 Most Affected Files <span style="font-size:11px;color:#475569;font-weight:400;text-transform:none">(click to jump)</span></h2>
  <div class="bars">{bar_rows}</div>

  <!-- Full issue list -->
  <div class="section-header">
    <h2>All Issues — {affected_files} files</h2>
    <div class="issues-toolbar">
      <input class="search-box" type="search" id="file-search" placeholder="🔍 Filter by filename…" oninput="filterBySearch(this.value)">
      <button class="btn" onclick="expandAll()">Expand All</button>
      <button class="btn" onclick="collapseAll()">Collapse All</button>
      <span id="visible-count"></span>
    </div>
  </div>
  <div class="issue-sections" id="issue-sections">{issue_sections}</div>
  <div id="no-results">No files match your filter.</div>

</div>

<div class="footer">
  Generated by <a href="https://crates.io/crates/react-perf-analyzer">react-perf-analyzer</a>
  &nbsp;·&nbsp;
  <a href="https://github.com/rashvish18/react-perf-analyzer">GitHub</a>
</div>

<script>
  let activeRule = null;
  let activeSearch = '';

  // ── Rule card filter ──────────────────────────────────────────────────────
  function filterByRule(card, rule) {{
    if (activeRule === rule) {{
      clearFilter();
      return;
    }}
    activeRule = rule;

    // Card visual state
    document.querySelectorAll('.card').forEach(c => {{
      c.classList.toggle('active', c.dataset.rule === rule);
      c.classList.toggle('dimmed', c.dataset.rule !== rule);
    }});

    // Banner
    document.getElementById('filter-rule-name').textContent = rule;
    document.getElementById('filter-banner').classList.add('visible');

    applyFilters();
  }}

  function clearFilter() {{
    activeRule = null;
    document.querySelectorAll('.card').forEach(c => {{
      c.classList.remove('active', 'dimmed');
    }});
    document.getElementById('filter-banner').classList.remove('visible');
    applyFilters();
  }}

  // ── Search filter ─────────────────────────────────────────────────────────
  function filterBySearch(q) {{
    activeSearch = q.toLowerCase().trim();
    applyFilters();
  }}

  // ── Combined filter logic ─────────────────────────────────────────────────
  function applyFilters() {{
    const sections = document.querySelectorAll('#issue-sections details');
    let visible = 0;

    sections.forEach(section => {{
      const rules = section.dataset.rules || '';
      const file  = (section.dataset.file  || '').toLowerCase();

      const ruleMatch  = !activeRule || rules.split(' ').includes(activeRule);
      const searchMatch = !activeSearch || file.includes(activeSearch);

      const show = ruleMatch && searchMatch;
      section.classList.toggle('hidden-section', !show);

      if (show) {{
        visible++;
        // Auto-open section when filtering by rule, close otherwise
        if (activeRule) {{
          section.open = true;
          // Hide rows that don't belong to the filtered rule
          section.querySelectorAll('tr[data-rule]').forEach(row => {{
            row.classList.toggle('row-hidden', row.dataset.rule !== activeRule);
          }});
        }} else {{
          // Restore all rows
          section.querySelectorAll('tr[data-rule]').forEach(row => {{
            row.classList.remove('row-hidden');
          }});
        }}
      }}
    }});

    document.getElementById('visible-count').textContent =
      visible === sections.length ? '' : `${{visible}} of ${{sections.length}} files`;
    document.getElementById('no-results').classList.toggle('visible', visible === 0);
  }}

  // ── Bar chart navigation ──────────────────────────────────────────────────
  function scrollToFile(idx) {{
    const el = document.getElementById('file-' + idx);
    if (!el) return;
    el.open = true;
    el.scrollIntoView({{ behavior: 'smooth', block: 'start' }});
  }}

  // ── Expand / Collapse ─────────────────────────────────────────────────────
  function expandAll() {{
    document.querySelectorAll('#issue-sections details:not(.hidden-section)')
      .forEach(d => d.open = true);
  }}
  function collapseAll() {{
    document.querySelectorAll('#issue-sections details')
      .forEach(d => d.open = false);
  }}
</script>
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

// ─── SARIF reporter ───────────────────────────────────────────────────────────

/// Generate a SARIF 2.1.0 JSON string for GitHub/GitLab/Azure DevOps.
///
/// SARIF (Static Analysis Results Interchange Format) is the standard that
/// lets CI platforms show inline annotations on PR diffs.
pub fn report_sarif(issues: &[Issue], version: &str) -> String {
    // Collect unique rule IDs for the tool.driver.rules array.
    let mut seen_rules: Vec<&str> = vec![];
    for issue in issues {
        if !seen_rules.contains(&issue.rule.as_str()) {
            seen_rules.push(issue.rule.as_str());
        }
    }

    let rules_json: String = seen_rules
        .iter()
        .map(|r| format!(r#"{{"id":"{r}","shortDescription":{{"text":"{r}"}}}}"#,))
        .collect::<Vec<_>>()
        .join(",");

    let results_json: String = issues
        .iter()
        .map(|issue| {
            let level = match issue.severity {
                crate::rules::Severity::Critical | crate::rules::Severity::High => "error",
                crate::rules::Severity::Medium => "warning",
                crate::rules::Severity::Low | crate::rules::Severity::Info => "note",
            };
            let msg = issue.message.replace('"', "\\\"");
            let file = issue.file.display().to_string().replace('\\', "/");
            format!(
                r#"{{"ruleId":"{rule}","level":"{level}","message":{{"text":"{msg}"}},
"locations":[{{"physicalLocation":{{"artifactLocation":{{"uri":"{file}","uriBaseId":"%SRCROOT%"}},
"region":{{"startLine":{line},"startColumn":{col}}}}}}}]}}"#,
                rule = issue.rule,
                line = issue.line,
                col = issue.column,
            )
        })
        .collect::<Vec<_>>()
        .join(",");

    format!(
        r#"{{"version":"2.1.0","$schema":"https://json.schemastore.org/sarif-2.1.0.json",
"runs":[{{"tool":{{"driver":{{"name":"react-perf-analyzer","version":"{version}",
"informationUri":"https://github.com/rashvish18/react-perf-analyzer",
"rules":[{rules_json}]}}}},"results":[{results_json}]}}]}}"#
    )
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
