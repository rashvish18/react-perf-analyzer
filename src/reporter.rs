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

// ─── Terminal stats box ───────────────────────────────────────────────────────

/// Print a compact stats box to stderr — mirrors the HTML summary tiles.
///
/// ```text
/// ╭────────────────┬────────────────┬────────────────┬────────────────┬────────────────┬────────────────╮
/// │      168       │     54178      │      312       │      168       │      N/A       │      N/A       │
/// │  Total Issues  │ Files Scanned  │ Files w/Issues │  React Rules   │    oxlint      │  cargo-audit   │
/// ╰────────────────┴────────────────┴────────────────┴────────────────┴────────────────┴────────────────╯
/// ```
pub fn print_stats_box(
    total: usize,
    file_count: usize,
    affected_files: usize,
    our_count: usize,
    external_ran: bool,
    oxlint_count: usize,
    audit_count: usize,
) {
    // ANSI colours — numbers only; labels stay plain.
    const RST: &str = "\x1b[0m";
    const ORANGE: &str = "\x1b[38;5;208m"; // Total Issues
    const CYAN: &str = "\x1b[36m"; // Files Scanned
    const PURPLE: &str = "\x1b[35m"; // Files w/ Issues
    const AMBER: &str = "\x1b[33m"; // React Rules
    const BLUE: &str = "\x1b[34m"; // oxlint
    const RED: &str = "\x1b[31m"; // cargo-audit
    const DIM: &str = "\x1b[90m"; // N/A

    // Store owned strings so we can borrow them as &str below.
    let total_s = total.to_string();
    let file_count_s = file_count.to_string();
    let affected_s = affected_files.to_string();
    let our_s = our_count.to_string();
    let oxlint_s = if external_ran {
        oxlint_count.to_string()
    } else {
        "N/A".to_string()
    };
    let audit_s = if external_ran {
        audit_count.to_string()
    } else {
        "N/A".to_string()
    };

    // (plain_value, label, ansi_color)
    let cells: [(&str, &str, &str); 6] = [
        (&total_s, "Total Issues", ORANGE),
        (&file_count_s, "Files Scanned", CYAN),
        (&affected_s, "Files w/Issues", PURPLE),
        (&our_s, "React Rules", AMBER),
        (&oxlint_s, "oxlint", if external_ran { BLUE } else { DIM }),
        (
            &audit_s,
            "cargo-audit",
            if external_ran { RED } else { DIM },
        ),
    ];

    const W: usize = 16; // inner visible width per cell

    fn pad_center(s: &str, width: usize) -> String {
        let len = s.chars().count();
        if len >= width {
            return s.to_string();
        }
        let total_pad = width - len;
        let left = total_pad / 2;
        let right = total_pad - left;
        format!("{}{}{}", " ".repeat(left), s, " ".repeat(right))
    }

    let bar = "─".repeat(W);

    // Top border
    let top: String = std::iter::once(format!("╭{bar}"))
        .chain((1..cells.len()).map(|_| format!("┬{bar}")))
        .collect::<String>()
        + "╮";
    eprintln!("{top}");

    // Numbers row (coloured)
    let num_row: String = cells
        .iter()
        .map(|(val, _, color)| {
            let centered = pad_center(val, W);
            // Re-insert colour around the number itself (already centered with spaces).
            let colored = centered.replacen(val.trim(), &format!("{color}{val}{RST}"), 1);
            format!("│{colored}")
        })
        .collect::<String>()
        + "│";
    eprintln!("{num_row}");

    // Label row
    let lbl_row: String = cells
        .iter()
        .map(|(_, lbl, _)| format!("│{}", pad_center(lbl, W)))
        .collect::<String>()
        + "│";
    eprintln!("{lbl_row}");

    // Bottom border
    let bot: String = std::iter::once(format!("╰{bar}"))
        .chain((1..cells.len()).map(|_| format!("┴{bar}")))
        .collect::<String>()
        + "╯";
    eprintln!("{bot}");
}

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

// ─── AI Prompt reporter ───────────────────────────────────────────────────────

/// Generate a structured markdown file with one AI-ready prompt section per
/// affected file.
///
/// Each section is self-contained: it lists every issue with its line, column,
/// rule name, severity, and message, then embeds the full numbered source code,
/// and finally gives the AI clear instructions so the developer can paste the
/// section directly into Claude, Copilot Chat, Cursor, or any other assistant.
///
/// # Output layout
///
/// ```text
/// # Fix React Issues: `src/components/UserCard.tsx`
///
/// > **2 issue(s) found.** Fix all of them …
///
/// ## Issues
/// ### Issue 1 — Line 12, Col 18 | `no_inline_jsx_fn` | Medium
/// **Problem**: Inline arrow function …
///
/// ## Full Source Code
/// ```tsx
/// 1 | import React …
/// …
/// ```
///
/// ## Instructions for AI
/// …
///
/// ---
///
/// # Fix React Issues: `src/hooks/useData.ts`
/// …
/// ```
///
/// Returns the total number of issues across all files (used for exit code logic).
///
/// Single-file mode: all file sections concatenated into one `.md`.
/// Use for small codebases (< ~100K estimated tokens).
/// For large monorepos use `report_ai_prompt_dir` instead.
pub fn report_ai_prompt(issues: &[Issue], output_path: Option<&std::path::Path>) -> usize {
    use std::collections::BTreeMap;
    use std::fs;
    use std::path::PathBuf;

    if issues.is_empty() {
        let msg = "✓ No issues found — nothing to fix.\n";
        match output_path {
            Some(p) => { let _ = fs::write(p, msg); }
            None    => print!("{msg}"),
        }
        return 0;
    }

    // ── Sort + group ──────────────────────────────────────────────────────────
    let mut sorted = issues.to_vec();
    sorted.sort_by(|a, b| {
        a.file.cmp(&b.file).then(a.line.cmp(&b.line)).then(a.column.cmp(&b.column))
    });
    let mut by_file: BTreeMap<PathBuf, Vec<&Issue>> = BTreeMap::new();
    for issue in &sorted {
        by_file.entry(issue.file.clone()).or_default().push(issue);
    }

    // ── Build one prompt block per file using the shared helper ───────────────
    let mut blocks: Vec<String> = Vec::with_capacity(by_file.len());
    for (file_path, file_issues) in &by_file {
        let source = fs::read_to_string(file_path)
            .unwrap_or_else(|err| format!("(could not read file: {err})"));
        blocks.push(build_file_prompt_block(file_path, file_issues, &source));
    }

    let output = blocks.join("\n---\n\n");

    match output_path {
        Some(p) => {
            if let Err(e) = fs::write(p, &output) {
                eprintln!("Error writing AI prompt to '{}': {e}", p.display());
            } else {
                let est_tokens = output.len() / 4;
                let file_count = by_file.len();
                eprintln!(
                    "   {file_count} file section(s) | {} issues | ~{} tokens estimated",
                    issues.len(),
                    fmt_token_count(est_tokens),
                );
                if est_tokens > 180_000 {
                    eprintln!("   ⚠  Prompt is very large — consider splitting by file or using --category perf");
                } else if est_tokens > 90_000 {
                    eprintln!("   ⚠  Prompt is large — may exceed some AI tool context windows (GPT-4 Turbo: 128K)");
                }
            }
        }
        None => println!("{output}"),
    }

    issues.len()
}

// ─── Directory mode ───────────────────────────────────────────────────────────

/// Directory mode: one self-contained `.md` per affected file + an `index.md`
/// dashboard, written into `output_dir`.
///
/// Designed for large monorepos where a single-file prompt would be millions of
/// tokens. Each per-file prompt is guaranteed to fit in any AI context window.
///
/// # Output layout
/// ```text
/// output_dir/
/// ├── index.md                       ← priority-ranked dashboard with checkboxes
/// ├── libs_item_breadcrumb_src_lib_breadcrumb-container.tsx.md
/// ├── libs_item_buy-box_src_lib_buy-box.tsx.md
/// └── …
/// ```
///
/// `index.md` sorts files by priority score (critical×100 + high×10 + medium×1)
/// so developers always tackle the highest-impact files first.
pub fn report_ai_prompt_dir(
    issues: &[Issue],
    output_dir: &std::path::Path,
    scan_root: &std::path::Path,
) -> usize {
    use std::collections::BTreeMap;
    use std::fs;
    use std::path::PathBuf;

    if issues.is_empty() {
        eprintln!("✓ No issues found — nothing to fix.");
        return 0;
    }

    // ── Create output directory ───────────────────────────────────────────────
    if let Err(e) = fs::create_dir_all(output_dir) {
        eprintln!("Error creating output directory '{}': {e}", output_dir.display());
        return 0;
    }

    // ── Sort + group by file ──────────────────────────────────────────────────
    let mut sorted = issues.to_vec();
    sorted.sort_by(|a, b| {
        a.file.cmp(&b.file).then(a.line.cmp(&b.line)).then(a.column.cmp(&b.column))
    });
    let mut by_file: BTreeMap<PathBuf, Vec<&Issue>> = BTreeMap::new();
    for issue in &sorted {
        by_file.entry(issue.file.clone()).or_default().push(issue);
    }

    // ── Write per-file prompts + collect metadata for index ──────────────────
    let mut records: Vec<FileRecord> = Vec::with_capacity(by_file.len());
    let mut total_tokens: usize = 0;

    for (file_path, file_issues) in &by_file {
        let source = fs::read_to_string(file_path)
            .unwrap_or_else(|err| format!("(could not read file: {err})"));

        let block = build_file_prompt_block(file_path, file_issues, &source);
        let est_tokens = block.len() / 4;
        total_tokens += est_tokens;

        // ── Safe output filename (relative path with / replaced by _) ─────────
        let safe_name = make_safe_filename(file_path, scan_root);
        let out_path = output_dir.join(&safe_name);

        if let Err(e) = fs::write(&out_path, &block) {
            eprintln!("  ⚠  Could not write '{}': {e}", out_path.display());
            continue;
        }

        // ── Severity breakdown for the index ──────────────────────────────────
        let mut sev = SevCounts::default();
        for issue in file_issues {
            match issue.severity {
                crate::rules::Severity::Critical => sev.critical += 1,
                crate::rules::Severity::High     => sev.high += 1,
                crate::rules::Severity::Medium   => sev.medium += 1,
                crate::rules::Severity::Low      => sev.low += 1,
                crate::rules::Severity::Info     => sev.info += 1,
            }
        }

        // Relative display path (falls back to absolute if stripping fails).
        let rel_display = file_path
            .strip_prefix(scan_root)
            .unwrap_or(file_path)
            .display()
            .to_string();

        records.push(FileRecord {
            rel_display,
            safe_name,
            issue_count: file_issues.len(),
            sev,
            est_tokens,
        });
    }

    // ── Sort records by priority score descending ─────────────────────────────
    records.sort_by(|a, b| {
        b.sev.priority_score()
            .partial_cmp(&a.sev.priority_score())
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(b.issue_count.cmp(&a.issue_count))
    });

    // ── Write index.md ────────────────────────────────────────────────────────
    let index = build_index_md(&records, issues.len(), total_tokens, output_dir, scan_root);
    let index_path = output_dir.join("index.md");
    if let Err(e) = fs::write(&index_path, &index) {
        eprintln!("Error writing index.md: {e}");
    }

    // ── Stderr summary ────────────────────────────────────────────────────────
    let top = records.first();
    eprintln!(
        "   {} file prompt(s) | {} issues | ~{} total tokens",
        records.len(),
        issues.len(),
        fmt_token_count(total_tokens),
    );
    if let Some(t) = top {
        eprintln!(
            "   🔥 Highest priority: {} ({} issues, ~{} tokens)",
            t.rel_display,
            t.issue_count,
            fmt_token_count(t.est_tokens),
        );
    }
    let dir_display = output_dir.to_string_lossy();
    let dir_display = dir_display.trim_end_matches('/').trim_end_matches('\\');
    eprintln!("   📋 Open {dir_display}/index.md to start fixing.");

    issues.len()
}

// ─── Shared per-file block builder ───────────────────────────────────────────

/// Build the full markdown prompt block for a single source file.
///
/// Used by both `report_ai_prompt` (single-file mode) and
/// `report_ai_prompt_dir` (directory mode) so the output format is identical.
fn build_file_prompt_block(
    file_path: &std::path::Path,
    file_issues: &[&Issue],
    source: &str,
) -> String {
    use std::collections::BTreeMap;

    let lang = match file_path.extension().and_then(|e| e.to_str()).unwrap_or("") {
        "tsx" => "tsx",
        "jsx" => "jsx",
        "ts"  => "ts",
        _     => "js",
    };

    let file_display = file_path.display();
    let n = file_issues.len();
    let issue_label = if n == 1 { "issue" } else { "issues" };

    // ── Header ────────────────────────────────────────────────────────────────
    let mut block = format!(
        "# Fix React Issues: `{file_display}`\n\n\
         > **{n} {issue_label} found.** \
         Fix all of them without changing component logic or render output.\n\n\
         ## Issues\n\n"
    );

    // ── Issue list with "why it hurts" blurb ─────────────────────────────────
    for (idx, issue) in file_issues.iter().enumerate() {
        let why = rule_why_blurb(&issue.rule);
        block.push_str(&format!(
            "### Issue {num} — Line {line}, Col {col} | `{rule}` | {sev}\n\
             **Why it hurts**: {why}\n\
             **Problem**: {msg}\n\n",
            num  = idx + 1,
            line = issue.line,
            col  = issue.column,
            rule = issue.rule,
            sev  = issue.severity,
            msg  = issue.message,
        ));
    }

    // ── Line → issue number lookup for inline markers ─────────────────────────
    let mut line_to_issues: BTreeMap<u32, Vec<usize>> = BTreeMap::new();
    for (idx, issue) in file_issues.iter().enumerate() {
        line_to_issues.entry(issue.line).or_default().push(idx + 1);
    }

    // ── Numbered source with inline markers ───────────────────────────────────
    block.push_str("## Full Source Code\n\n");
    block.push_str("> Lines marked with `// ← ⚠ Issue N` are the exact locations to fix.\n\n");
    block.push_str(&format!("```{lang}\n"));
    for (i, src_line) in source.lines().enumerate() {
        let line_no = (i + 1) as u32;
        if let Some(nums) = line_to_issues.get(&line_no) {
            let labels: Vec<String> = nums.iter().map(|n| format!("Issue {n}")).collect();
            block.push_str(&format!(
                "{:>4} | {src_line}  // ← ⚠ {}\n",
                line_no,
                labels.join(", ")
            ));
        } else {
            block.push_str(&format!("{:>4} | {src_line}\n", line_no));
        }
    }
    block.push_str("```\n\n");

    // ── AI instructions ───────────────────────────────────────────────────────
    block.push_str(
        "## Instructions for AI\n\n\
         You are an expert React developer. Fix **ALL** the issues listed above \
         in the source code shown.\n\n\
         Rules:\n\
         - Do **NOT** change any component logic, business logic, or render output\n\
         - Preserve all imports, exports, and TypeScript types exactly as-is\n\
         - Maintain the existing code style and formatting\n\
         - Fix **only** the specific patterns described above — do not refactor anything else\n\
         - Return the **complete corrected file** — no explanation, no markdown fences, \
         just the raw source code\n",
    );

    block
}

// ─── Directory-mode helpers ───────────────────────────────────────────────────

/// Per-severity counts for one file — used to build the index table.
#[derive(Default)]
struct SevCounts {
    critical: usize,
    high:     usize,
    medium:   usize,
    low:      usize,
    info:     usize,
}

impl SevCounts {
    /// Priority score: higher = fix this file first.
    /// Security/critical issues float to the top of the index.
    fn priority_score(&self) -> f64 {
        self.critical as f64 * 100.0
            + self.high   as f64 * 10.0
            + self.medium as f64 * 1.0
            + self.low    as f64 * 0.1
    }
}

/// Metadata about one affected file, collected while writing per-file prompts.
struct FileRecord {
    rel_display: String,  // e.g. "libs/item/buy-box/src/lib/buy-box.tsx"
    safe_name:   String,  // e.g. "libs_item_buy-box_src_lib_buy-box.tsx.md"
    issue_count: usize,
    sev:         SevCounts,
    est_tokens:  usize,
}

/// Convert an absolute file path to a safe output filename.
///
/// Strips the `scan_root` prefix, then replaces `/`, `\`, `:`, and spaces
/// with `_` so the name is safe on all file systems.
///
/// Example: `/users/dev/walmart/libs/item/buy-box/src/lib/buy-box.tsx`
///   scan_root = `/users/dev/walmart`
///   → `libs_item_buy-box_src_lib_buy-box.tsx.md`
fn make_safe_filename(path: &std::path::Path, scan_root: &std::path::Path) -> String {
    let rel = path.strip_prefix(scan_root).unwrap_or(path);
    let s = rel.to_string_lossy();
    let safe: String = s.chars().map(|c| match c {
        '/' | '\\' | ':' | ' ' => '_',
        c => c,
    }).collect();
    // Strip any leading underscore that results from an absolute path fallback.
    let safe = safe.trim_start_matches('_');
    format!("{safe}.md")
}

/// Build the `index.md` dashboard — action-oriented, step-based layout.
///
/// Design goals:
/// - Readable in raw text (VS Code editor) AND rendered markdown
/// - No complex tables — simple bullet lists
/// - 3 clear steps: security first → high-impact → rest by module
/// - Module grouping avoids overwhelming the user with 600+ individual files
fn build_index_md(
    records: &[FileRecord],
    total_issues: usize,
    total_tokens: usize,
    prompt_dir: &std::path::Path,
    scan_root: &std::path::Path,
) -> String {
    use std::collections::BTreeMap;

    let file_count = records.len();
    let tokens_str = fmt_token_count(total_tokens);
    let prompt_dir_str = prompt_dir.to_string_lossy().trim_end_matches('/').to_string();
    let scan_root_str = scan_root.to_string_lossy().trim_end_matches('/').to_string();
    // ── Split into tiers ──────────────────────────────────────────────────────
    // Tier 1: security (critical or high severity)
    let security: Vec<&FileRecord> = records
        .iter()
        .filter(|r| r.sev.critical > 0 || r.sev.high > 0)
        .collect();

    // Tier 2: top 15 by issue count (excluding security files already shown)
    let security_names: std::collections::HashSet<&str> =
        security.iter().map(|r| r.rel_display.as_str()).collect();

    let mut top_files: Vec<&FileRecord> = records
        .iter()
        .filter(|r| !security_names.contains(r.rel_display.as_str()))
        .collect();
    top_files.sort_by(|a, b| b.issue_count.cmp(&a.issue_count));
    let top_files: Vec<&FileRecord> = top_files.into_iter().take(15).collect();

    // Tier 3: everything else — grouped by top-level module (first path segment)
    let shown: std::collections::HashSet<&str> = security
        .iter()
        .chain(top_files.iter())
        .map(|r| r.rel_display.as_str())
        .collect();

    let mut modules: BTreeMap<String, (usize, usize)> = BTreeMap::new(); // module → (files, issues)
    for rec in records.iter().filter(|r| !shown.contains(r.rel_display.as_str())) {
        let module = rec
            .rel_display
            .split('/')
            .next()
            .unwrap_or("other")
            .to_string();
        let entry = modules.entry(module).or_default();
        entry.0 += 1;       // file count
        entry.1 += rec.issue_count; // issue count
    }

    // ── Sort modules by issue count descending ───────────────────────────────
    let remaining_total: usize = modules.values().map(|(_, i)| i).sum();
    let remaining_files: usize = modules.values().map(|(f, _)| f).sum();
    let mut module_list: Vec<(&String, &(usize, usize))> = modules.iter().collect();
    module_list.sort_by(|a, b| b.1.1.cmp(&a.1.1));

    // ── Count totals for intake message ──────────────────────────────────────
    let sec_count  = security.len();
    let top_count  = top_files.len();

    // ══════════════════════════════════════════════════════════════════════════
    // ORCHESTRATOR PROMPT — paste the whole file into GitHub Copilot Chat, Cursor, or Claude
    // The AI reads the instructions, interviews the user, fixes code, updates
    // the checkboxes, and optionally raises a PR with attribution.
    // ══════════════════════════════════════════════════════════════════════════

    let mut md = String::new();

    // ── Paste hint (HTML comment — not rendered in Markdown previews) ─────────
    md.push_str(
        "<!--\n  ╔══════════════════════════════════════════════════════════════╗\n\
           ║  REACT PERF ORCHESTRATOR — paste this ENTIRE file into        ║\n\
           ║  Claude, GitHub Copilot Chat, or Cursor to start the workflow. ║\n\
           ║  The AI will guide you, fix files, and track progress here.    ║\n\
           ╚══════════════════════════════════════════════════════════════════╝\n\
         -->\n\n"
    );

    // ── Title ─────────────────────────────────────────────────────────────────
    md.push_str(&format!(
        "# 🤖 React Perf Fix Orchestrator\n\n\
         > **{total_issues} issues · {file_count} files · ~{tokens_str} tokens**  \n\
         > Powered by [react-perf-analyzer](https://github.com/rashvish18/react-perf-analyzer) · [crates.io](https://crates.io/crates/react-perf-analyzer)\n\n\
         ---\n\n"
    ));

    // ── AI System Instructions ────────────────────────────────────────────────
    md.push_str(&format!(
        "## 🧠 Instructions for AI (read this first)\n\n\
         You are a **senior React performance & security engineer** acting as a guided fix assistant.\n\
         Your job:\n\
         1. **Read this entire document** before doing anything\n\
         2. **Run Phase 1** — ask the user the intake questions below\n\
         3. **Run Phase 2** — fix files in the order the user chose, one at a time\n\
         4. **Run Phase 3** — mark each fixed file `[x] ✅` in this document\n\
         5. **Run Phase 4** — if the user wants a PR, create it with attribution\n\n\
         > ⚠️ **Do NOT start fixing any code until Phase 1 is complete.**\n\n\
         ### 📁 File Locations\n\n\
         | | Path |\n\
         |---|---|\n\
         | **Source code root** | `{scan_root_str}` |\n\
         | **Prompt files directory** | `{prompt_dir_str}` |\n\n\
         To read a prompt file, use its full path: `{prompt_dir_str}/<filename>.md`  \n\
         To edit a source file, look it up under: `{scan_root_str}/<relative-path>`  \n\
         Example: link `detail-page_src_lib_foo.tsx.md` → read `{prompt_dir_str}/detail-page_src_lib_foo.tsx.md`  \n\
         then edit `{scan_root_str}/detail-page/src/lib/foo.tsx`\n\n\
         ---\n\n"
    ));

    // ── Phase 1: Intake ───────────────────────────────────────────────────────
    md.push_str(&format!(
        "## Phase 1 — 🎤 Intake\n\n\
         Send the user **exactly** this message, then wait for their reply:\n\n\
         ---\n\n\
         👋 I've scanned your codebase with **react-perf-analyzer** and found \
         **{total_issues} issues across {file_count} files**. \
         Answer the questions below — reply with the **numbers** (e.g. `1, 1, 2, 1`).\n\n\
         **Q1 — Scope** — How many files to fix today?\n\
         `1` 🔴 Security only — {sec_count} file{sec_s} with critical/high severity *(recommended)*\n\
         `2` 🟡 Top 15 files — biggest performance wins\n\
         `3` 📦 All {file_count} files — full cleanup\n\
         `4` 🔢 Custom — specify how many\n\n\
         **Q2 — Severity** — Which severity levels?\n\
         `1` 🔴 Critical + High only — security & serious perf issues\n\
         `2` 🌐 All — including medium, low, informational\n\n\
         **Q3 — Pull Request** — Open a PR when done?\n\
         `1` ✅ Yes — create a PR with fix summary\n\
         `2` ❌ No — fix locally only\n\n\
         **Q4 — Skip** — Any files or modules to leave untouched?\n\
         `1` ⏩ None — fix everything in scope\n\
         `2` 🚫 Yes — I'll tell you which ones\n\n\
         ---\n\n\
         Once the user replies, map their numbers to choices, confirm with a summary:\n\n\
         > ✅ **All set!** Starting with:\n\
         > - 📂 **Scope:** [answer]\n\
         > - 🔍 **Severity:** [answer]\n\
         > - 🚀 **PR:** [answer]\n\
         > - ⏩ **Skip:** [answer]\n\n\
         Then proceed immediately to **Phase 2**.\n\n\
         ---\n\n",
        sec_s = if sec_count == 1 { "" } else { "s" }
    ));

    // ── Phase 2: Fix Queue ────────────────────────────────────────────────────
    md.push_str(
        "## Phase 2 — 🔧 Fix Queue\n\n\
         > ⚠️ **Fix ONE file at a time. Do NOT narrate steps — the user sees ONLY the summary.**\n\n\
         **For each file, silently run this full loop:**\n\n\
         **Step A — Fix**\n\
         - Read the `.md` prompt file for this file\n\
         - Read the actual source file\n\
         - Apply every fix listed in the prompt\n\n\
         **Step B — Validate (run ALL, fix errors automatically)**\n\
         - Run `react-perf-analyzer <file> --format text` — fix any remaining issues\n\
         - Run `npx tsc --noEmit` (or the project's TS check) — fix any type errors introduced\n\
         - Run `npx eslint <file> --fix` — fix any lint errors introduced\n\
         - If any check fails: fix the error, then re-run that check until it passes\n\
         - Repeat until ALL three checks are green\n\n\
         **Step C — Mark done**\n\
         - Update the checkbox: `[ ]` → `[x] ✅ fixed YYYY-MM-DD`\n\n\
         **Step D — Show the user ONLY this summary:**\n\n\
         > ✅ **Fixed `<filename>`**\n\
         > | Check | Result |\n\
         > |-------|--------|\n\
         > | react-perf-analyzer | ✅ 0 issues |\n\
         > | TypeScript | ✅ no errors |\n\
         > | ESLint | ✅ no errors |\n\
         >\n\
         > **Changes made:**\n\
         > - [rule] — what changed\n\
         > - [rule] — what changed\n\n\
         **After ALL files in scope are done — Step E (once, at the end):**\n\n\
         First, always ask before doing anything with git:\n\n\
         > 📋 **All fixes validated!** Before I commit or raise a PR, a few quick questions:\n\
         >\n\
         > `1` ✅ **Commit changes** — stage & commit the fixed files\n\
         > `2` 🚀 **Commit + raise PR** — commit and open a Pull Request\n\
         > `3` 👀 **Review first** — show me a diff of all changes before committing\n\
         > `4` ⏸ **Skip for now** — I'll commit manually later\n\n\
         - If `1` or `2`: stage only the fixed files, commit with message:\n\
           `fix: resolve React perf & security issues (react-perf-analyzer)`\n\
         - If `2`: also proceed to **Phase 4** to raise the PR\n\
         - If `3`: show a concise diff summary, then ask again\n\
         - If `4`: skip git entirely\n\n\
         Finally, always show:\n\
         > 🔁 **Ready for the next batch?** Type `Fix next` to continue with the remaining files.\n\n"
    );

    // ── Fix Queue — Security tier ─────────────────────────────────────────────
    if security.is_empty() {
        md.push_str("### ✅ Security Tier — No critical/high issues found\n\n");
    } else {
        md.push_str(&format!(
            "### 🔴 Security Tier — Fix These First ({sec_count} file{sec_s})\n\n\
             *XSS, unsafe hrefs, and other vulnerabilities — highest priority.*\n\n",
            sec_s = if sec_count == 1 { "" } else { "s" }
        ));
        for rec in &security {
            let badges = severity_badges(&rec.sev);
            md.push_str(&format!(
                "- [ ] [{file}]({link})  {badges} · {n} issue{s}\n",
                file  = short_name(&rec.rel_display),
                link  = rec.safe_name,
                n     = rec.issue_count,
                s     = if rec.issue_count == 1 { "" } else { "s" },
            ));
        }
        md.push('\n');
    }

    // ── Fix Queue — High-impact tier ──────────────────────────────────────────
    md.push_str(&format!(
        "### 🟡 High-Impact Tier — Top {top_count} Files by Issue Count\n\n\
         *Most issues per file — biggest performance wins.*\n\n"
    ));
    for (i, rec) in top_files.iter().enumerate() {
        md.push_str(&format!(
            "{}. [ ] [{file}]({link}) — **{n} issue{s}**\n",
            i + 1,
            file = short_name(&rec.rel_display),
            link = rec.safe_name,
            n    = rec.issue_count,
            s    = if rec.issue_count == 1 { "" } else { "s" },
        ));
    }
    md.push('\n');

    // ── Fix Queue — Module tier ───────────────────────────────────────────────
    if !modules.is_empty() {
        md.push_str(&format!(
            "### 📦 Remaining by Module ({remaining_files} files · {remaining_total} issues)\n\n\
             *Fix module by module — great for delegating to different teams.*\n\n"
        ));
        for (module, (files, issues)) in &module_list {
            md.push_str(&format!(
                "- [ ] **{module}** — {issues} issue{is} in {files} file{fs} \
                 *(look for `{module}_*.md` in this directory)*\n",
                is = if *issues == 1 { "" } else { "s" },
                fs = if *files == 1 { "" } else { "s" },
            ));
        }
        md.push('\n');
    }

    md.push_str("---\n\n");

    // ── Phase 3: Status Tracking ──────────────────────────────────────────────
    md.push_str(&format!(
        "## Phase 3 — ✅ Mark Fixed\n\n\
         After verifying each fix, update the checkbox in **Phase 2** from:\n\
         ````\n\
         - [ ] some/file.tsx\n\
         ````\n\
         to:\n\
         ````\n\
         - [x] ✅ some/file.tsx  — fixed YYYY-MM-DD\n\
         ````\n\n\
         This file is your **live progress tracker** — update it as you go \
         so you can resume later without re-running the scan.\n\n\
         ### 📊 Progress Summary *(update as you go)*\n\n\
         | | Count |\n\
         |---|---|\n\
         | Total files | {file_count} |\n\
         | ✅ Fixed | 0 |\n\
         | ⏳ Remaining | {file_count} |\n\n\
         ---\n\n"
    ));

    // ── Phase 4: Pull Request ─────────────────────────────────────────────────
    md.push_str(&format!(
        "## Phase 4 — 🚀 Pull Request\n\n\
         *Only run this phase if the user answered `yes` to Q3.*\n\n\
         ### Step 4a — Ask the user\n\n\
         Ask the user:\n\n\
         - **Branch name?** *(default: `fix/react-perf-analyzer-fixes`)*\n\
         - **PR title?** *(default: `fix: resolve React performance & security issues`)*\n\
         - **Squash all commits into one?** yes / no\n\n\
         ### Step 4b — Create the PR\n\n\
         1. Create branch with the name the user chose\n\
         2. Stage only the files you fixed (`git add -p` or list them explicitly)\n\
         3. Commit with message: `fix: resolve React performance & security issues — {total_issues} issues in {file_count} files. Powered by react-perf-analyzer`\n\
         4. Open the PR with title and a body that includes:\n\
            - A summary table: issues found ({total_issues}), files fixed, critical/security fixed ({sec_count})\n\
            - The list of ✅ fixed files copied from Phase 2\n\
            - Verification note: each file was checked with `react-perf-analyzer <file> --format text`\n\
            - Footer: `Generated by [react-perf-analyzer](https://github.com/rashvish18/react-perf-analyzer)`\n\n\
         ---\n\n"
    ));

    // ── Footer ────────────────────────────────────────────────────────────────
    md.push_str(&format!(
        "*Generated by [react-perf-analyzer](https://github.com/rashvish18/react-perf-analyzer) · [crates.io](https://crates.io/crates/react-perf-analyzer) \
         · {total_issues} issues · {file_count} files · ~{tokens_str} tokens*\n"
    ));

    md
}

/// Extract a short display name from a relative path (last 2 segments).
///
/// `detail-page/src/lib/detail-page-head/detail-page-head.tsx`
///   → `detail-page / detail-page-head.tsx`
fn short_name(rel: &str) -> String {
    let parts: Vec<&str> = rel.split('/').collect();
    match parts.len() {
        0 => rel.to_string(),
        1 => rel.to_string(),
        2 => rel.to_string(),
        n => format!("{} / {}", parts[0], parts[n - 1]),
    }
}

/// Build compact severity badge string for a file record.
fn severity_badges(sev: &SevCounts) -> String {
    let mut badges = Vec::new();
    if sev.critical > 0 {
        badges.push(format!("🔴 {} critical", sev.critical));
    }
    if sev.high > 0 {
        badges.push(format!("🟠 {} high", sev.high));
    }
    if badges.is_empty() {
        String::new()
    } else {
        badges.join(" · ")
    }
}

// ─── Rule explanation blurbs ──────────────────────────────────────────────────

/// Returns a concise one-liner explaining *why* a given rule is a performance
/// or security problem. Used in the AI prompt to give the model semantic context
/// beyond just the issue message.
fn rule_why_blurb(rule: &str) -> &'static str {
    match rule {
        "no_inline_jsx_fn" =>
            "A new function object is created on every render. Child components wrapped in \
             `React.memo` always see a changed prop and re-render unnecessarily.",
        "unstable_props" =>
            "Object/array literals create a new reference each render. \
             `React.memo` and `shouldComponentUpdate` comparisons always fail, causing wasted re-renders.",
        "no_array_index_key" =>
            "Using array index as `key` breaks React's reconciliation when items are reordered \
             or inserted, causing incorrect DOM updates and lost component state.",
        "large_component" =>
            "Large components are hard to memoize effectively and force React to diff more nodes \
             per render. Splitting into smaller components enables finer-grained memoization.",
        "no_new_context_value" =>
            "An inline object/array as Context value is recreated every render, causing ALL \
             context consumers to re-render even when the data hasn't changed.",
        "no_expensive_in_render" =>
            "Heavy computation runs synchronously on every render, blocking the main thread \
             and causing frame drops. Wrap with `useMemo` to cache the result.",
        "no_component_in_component" =>
            "Defining a component inside another component creates a new component type each render. \
             React unmounts and remounts the inner component every time, losing its state.",
        "no_unstable_hook_deps" =>
            "Object/array/function literals in deps arrays are always a new reference. \
             `Object.is` comparison always returns false → hook runs on every render.",
        "no_new_in_jsx_prop" =>
            "`new` expressions in JSX props create a new instance every render, \
             defeating memoization and causing unnecessary child re-renders.",
        "no_use_state_lazy_init_missing" =>
            "Passing a function call (not a function reference) to `useState` re-runs the \
             expensive initializer on every render instead of only on mount.",
        "no_json_in_render" =>
            "`JSON.parse`/`JSON.stringify` are expensive operations. Running them in the \
             render path blocks the main thread and can cause janky UI.",
        "no_object_entries_in_render" =>
            "`Object.entries/keys/values` creates a new array on every render. \
             Cache the result with `useMemo` to avoid allocating on each render cycle.",
        "no_regex_in_render" =>
            "Regex literals in render are recompiled on every call. \
             Move to module scope or cache with `useMemo`.",
        "no_math_random_in_render" =>
            "`Math.random()` in render produces a different value every render, \
             making output non-deterministic and breaking React's reconciliation.",
        "no_useless_memo" =>
            "`useCallback`/`useMemo` with an empty `[]` deps array never recomputes — \
             it adds hook overhead with no benefit. Extract to a module-level constant instead.",
        // Security rules
        "no_dangerously_set_inner_html_unescaped" =>
            "Unescaped HTML injected via `dangerouslySetInnerHTML` can execute arbitrary \
             scripts (XSS). Sanitize with DOMPurify or use a safe rendering API.",
        "no_hardcoded_secret_in_jsx" =>
            "Hardcoded secrets/tokens in JSX are bundled into the client and exposed publicly. \
             Use environment variables or a secrets manager instead.",
        "no_unsafe_href" =>
            "`javascript:` URLs in `href` can execute scripts when clicked (XSS). \
             Validate and sanitize all href values.",
        "no_xss_via_jsx_prop" =>
            "Unescaped user-controlled values in JSX props can inject malicious attributes. \
             Sanitize all external input before rendering.",
        "no_postmessage_wildcard" =>
            "`postMessage` with `\"*\"` as the target origin sends messages to any window. \
             Always specify the exact expected origin.",
        _ => "See the issue message above for context and the suggested fix.",
    }
}

// ─── Token count formatter ────────────────────────────────────────────────────

/// Format a token count with K suffix for readability.
///
/// `4200`  → `"4,200"`
/// `14200` → `"14.2K"`
fn fmt_token_count(n: usize) -> String {
    if n >= 10_000 {
        format!("{:.1}K", n as f64 / 1_000.0)
    } else {
        // Add thousands separator manually for small numbers.
        let s = n.to_string();
        if s.len() > 3 {
            let (head, tail) = s.split_at(s.len() - 3);
            format!("{head},{tail}")
        } else {
            s
        }
    }
}
