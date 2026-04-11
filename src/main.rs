/// main.rs — react-perf-analyzer entry point.
///
/// Pipeline:
///   1. Parse CLI arguments
///   2. Collect JS/TS/JSX files
///   3. Parse + analyze files in parallel (rayon + oxc)
///   4. Report results (text / json / html / sarif)
///   5. Exit with code based on --fail-on threshold
///
/// Exit codes:
///   0 — no issues found (or all below --fail-on threshold)
///   1 — issues found at or above --fail-on threshold
///   2 — fatal error (path not found, write error)
mod analyzer;
mod baseline;
mod changed_files;
mod cli;
mod file_loader;
mod orchestrator;
mod parser;
mod reporter;
mod rules;
mod utils;

use std::fs;
use std::time::Instant;

use clap::Parser as ClapParser;
use oxc_allocator::Allocator;
use rayon::prelude::*;

use crate::{
    analyzer::analyze,
    baseline::{filter_baseline, load_baseline},
    changed_files::get_changed_files,
    cli::{Cli, FailOn, OutputFormat},
    file_loader::collect_files,
    orchestrator::run_external_tools,
    parser::parse_file,
    reporter::{report_html, report_json, report_sarif, report_text},
    rules::Issue,
};

fn main() {
    // ── Step 1: Parse CLI arguments ───────────────────────────────────────────
    let cli = Cli::parse();

    // ── Step 2: Collect files ─────────────────────────────────────────────────
    let start = Instant::now();
    let files = collect_files(&cli.path, cli.include_tests);

    if files.is_empty() {
        eprintln!("No JS/TS/JSX files found under '{}'.", cli.path.display());
        std::process::exit(0);
    }

    // ── Step 2b: Filter to changed files if --only-changed ────────────────────
    let files = if cli.only_changed {
        let changed = get_changed_files(&cli.path);
        if changed.is_empty() {
            // Either not a git repo (warning already printed) or zero changes.
            eprintln!("✓ No changed JS/TS/JSX files — nothing to analyze.");
            std::process::exit(0);
        }
        let changed_set: std::collections::HashSet<_> = changed.into_iter().collect();
        let filtered: Vec<_> = files
            .into_iter()
            .filter(|f| changed_set.contains(f.as_path()))
            .collect();
        if filtered.is_empty() {
            eprintln!("✓ No changed JS/TS/JSX files in scope — nothing to analyze.");
            std::process::exit(0);
        }
        eprintln!(
            "⚡ --only-changed: analyzing {} changed file(s)",
            filtered.len()
        );
        filtered
    } else {
        files
    };

    let file_count = files.len();
    let max_lines = cli.max_component_lines;
    let category = cli.category.clone();

    // ── Step 3: Parallel parse + analyze ─────────────────────────────────────
    let all_issues: Vec<Issue> = files
        .par_iter()
        .flat_map(|path| {
            let source_text = match fs::read_to_string(path) {
                Ok(s) => s,
                Err(err) => {
                    eprintln!("Warning: could not read '{}': {err}", path.display());
                    return vec![];
                }
            };

            let allocator = Allocator::default();

            let program = match parse_file(&allocator, path, &source_text) {
                Ok(p) => p,
                Err(err) => {
                    eprintln!(
                        "Warning: failed to parse '{}': {}",
                        err.file,
                        err.messages.join("; ")
                    );
                    return vec![];
                }
            };

            analyze(&program, &source_text, path, max_lines, &category)
        })
        .collect();

    // ── Step 3b: Run external tools (oxlint, cargo-audit) ────────────────────
    let ext = run_external_tools(&cli.path);

    // Print tool status hints to stderr.
    for tool in &ext.tools_run {
        eprintln!("  ✅ {tool}");
    }
    for (tool, reason) in &ext.tools_skipped {
        eprintln!("  ⚠  {tool}: {reason}");
    }
    if !ext.tools_run.is_empty() || !ext.tools_skipped.is_empty() {
        eprintln!();
    }

    // Merge external issues with our own.
    let mut all_issues = all_issues;
    all_issues.extend(ext.issues);

    // ── Step 3c: Apply baseline (suppress known issues) ───────────────────────
    let all_issues = if let Some(ref baseline_path) = cli.baseline {
        let entries = load_baseline(baseline_path);
        filter_baseline(all_issues, &entries)
    } else {
        all_issues
    };

    // ── Step 4: Report ────────────────────────────────────────────────────────
    let issue_count = match cli.format {
        OutputFormat::Text => {
            let count = report_text(&all_issues);
            if let Some(ref out_path) = cli.output {
                let _ = fs::write(out_path, "");
            }
            count
        }
        OutputFormat::Json => {
            let count = report_json(&all_issues);
            reporter::print_summary(&all_issues);
            count
        }
        OutputFormat::Html => {
            let html = report_html(&all_issues, &cli.path, file_count);
            let out_path = cli
                .output
                .clone()
                .unwrap_or_else(|| std::path::PathBuf::from("react-perf-report.html"));

            match fs::write(&out_path, &html) {
                Ok(_) => {
                    let abs_path =
                        std::fs::canonicalize(&out_path).unwrap_or_else(|_| out_path.clone());
                    eprintln!("✅ HTML report written to: {}", out_path.display());
                    #[cfg(target_os = "macos")]
                    let _ = std::process::Command::new("open").arg(&abs_path).spawn();
                    #[cfg(target_os = "linux")]
                    let _ = std::process::Command::new("xdg-open")
                        .arg(&abs_path)
                        .spawn();
                }
                Err(e) => {
                    eprintln!("Error writing HTML report to '{}': {e}", out_path.display());
                    std::process::exit(2);
                }
            }
            all_issues.len()
        }
        OutputFormat::Sarif => {
            let sarif = report_sarif(&all_issues, env!("CARGO_PKG_VERSION"));
            let out_path = cli
                .output
                .clone()
                .unwrap_or_else(|| std::path::PathBuf::from("results.sarif"));
            match fs::write(&out_path, &sarif) {
                Ok(_) => eprintln!("✅ SARIF report written to: {}", out_path.display()),
                Err(e) => {
                    eprintln!("Error writing SARIF to '{}': {e}", out_path.display());
                    std::process::exit(2);
                }
            }
            all_issues.len()
        }
    };

    // ── Step 5: Elapsed time ──────────────────────────────────────────────────
    let elapsed = start.elapsed();
    let elapsed_str = if elapsed.as_secs() >= 1 {
        format!("{:.2}s", elapsed.as_secs_f64())
    } else {
        format!("{}ms", elapsed.as_millis())
    };
    eprintln!(
        "\nScanned {file_count} file(s){} in {elapsed_str}.",
        if issue_count > 0 {
            format!(", found {issue_count} issue(s)")
        } else {
            String::new()
        }
    );

    // ── Step 6: Exit code ─────────────────────────────────────────────────────
    let should_fail = match cli.fail_on {
        FailOn::None => issue_count > 0,
        ref threshold => {
            let min_sev = threshold.as_severity().unwrap();
            all_issues.iter().any(|i| i.severity >= min_sev)
        }
    };

    std::process::exit(if should_fail { 1 } else { 0 });
}
