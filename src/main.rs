/// main.rs — react-perf-analyzer entry point.
///
/// Pipeline:
///   1. Parse CLI arguments
///   2. Collect JS/TS/JSX files
///   3. Parse + analyze files in parallel (rayon + oxc)  ← live progress
///   4. Run external tools (oxlint, cargo-audit)          ← live progress
///   5. Apply baseline / custom rules
///   6. Report results (text / json / html / sarif)
///   7. Exit with code based on --fail-on threshold
///
/// Exit codes:
///   0 — no issues found (or all below --fail-on threshold)
///   1 — issues found at or above --fail-on threshold
///   2 — fatal error (path not found, write error)
mod analyzer;
mod baseline;
mod changed_files;
mod cli;
mod custom_rules;
mod file_loader;
mod orchestrator;
mod parser;
mod reporter;
mod rules;
mod utils;

use std::fs;
use std::io::Write as _;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Instant;

use clap::Parser as ClapParser;
use oxc_allocator::Allocator;
use rayon::prelude::*;

use crate::{
    analyzer::analyze,
    baseline::{filter_baseline, load_baseline},
    changed_files::get_changed_files,
    cli::{Cli, FailOn, OutputFormat},
    custom_rules::{find_default_rules_file, load_custom_rules, run_custom_rules},
    file_loader::collect_files,
    orchestrator::run_external_tools,
    parser::parse_file,
    reporter::{report_html, report_json, report_sarif, report_text},
    rules::Issue,
};

fn fmt_ms(ms: u128) -> String {
    if ms >= 1000 {
        format!("{:.2}s", ms as f64 / 1000.0)
    } else {
        format!("{ms}ms")
    }
}

fn main() {
    // ── Step 1: Parse CLI arguments ───────────────────────────────────────────
    let cli = Cli::parse();
    let total_start = Instant::now();

    // Banner
    eprintln!("react-perf-analyzer v{}", env!("CARGO_PKG_VERSION"));
    eprintln!("Scanning: {}", cli.path.display());
    eprintln!();

    // ── Step 2: Collect files ─────────────────────────────────────────────────
    eprint!("  📂 Discovering files...");
    let _ = std::io::stderr().flush();
    let t = Instant::now();
    let files = collect_files(&cli.path, cli.include_tests);
    let discover_ms = t.elapsed().as_millis();

    if files.is_empty() {
        eprintln!(
            "\r  ⚠  No JS/TS/JSX files found under '{}'.",
            cli.path.display()
        );
        std::process::exit(0);
    }
    eprintln!(
        "\r  📂 Found {} file(s) in {}{}",
        files.len(),
        fmt_ms(discover_ms),
        " ".repeat(20)
    );

    // ── Step 2b: Filter to changed files if --only-changed ────────────────────
    let files = if cli.only_changed {
        let changed = get_changed_files(&cli.path);
        if changed.is_empty() {
            eprintln!("  ✓ No changed JS/TS/JSX files — nothing to analyze.");
            std::process::exit(0);
        }
        let changed_set: std::collections::HashSet<_> = changed.into_iter().collect();
        let filtered: Vec<_> = files
            .into_iter()
            .filter(|f| changed_set.contains(f.as_path()))
            .collect();
        if filtered.is_empty() {
            eprintln!("  ✓ No changed JS/TS/JSX files in scope — nothing to analyze.");
            std::process::exit(0);
        }
        eprintln!(
            "  ⚡ --only-changed: {} changed file(s) to analyze",
            filtered.len()
        );
        filtered
    } else {
        files
    };

    let file_count = files.len();
    let max_lines = cli.max_component_lines;
    let category = cli.category.clone();

    // ── Step 2c: Load custom rules (TOML DSL) ────────────────────────────────
    let custom_rule_set: Vec<custom_rules::CompiledRule> = {
        let rules_path = cli
            .rules
            .clone()
            .or_else(|| find_default_rules_file(&cli.path));
        match rules_path {
            Some(ref p) => {
                let (compiled, errors) = load_custom_rules(p);
                for err in &errors {
                    eprintln!("  ⚠  custom rule: {err}");
                }
                if !compiled.is_empty() {
                    eprintln!(
                        "  📏 Custom rules: {} rule(s) from '{}'",
                        compiled.len(),
                        p.display()
                    );
                }
                compiled
            }
            None => vec![],
        }
    };

    // ── Step 3: Parallel parse + analyze ─────────────────────────────────────
    // Spin up a progress thread that writes live "X/N" counts with \r.
    let processed = Arc::new(AtomicUsize::new(0));
    let done_flag = Arc::new(AtomicBool::new(false));

    let prog_count = processed.clone();
    let prog_done = done_flag.clone();

    let progress_thread = std::thread::spawn(move || loop {
        if prog_done.load(Ordering::Relaxed) {
            break;
        }
        let n = prog_count.load(Ordering::Relaxed);
        eprint!("\r  🔬 Analyzing files  {n}/{file_count}");
        let _ = std::io::stderr().flush();
        std::thread::sleep(std::time::Duration::from_millis(80));
    });

    let t = Instant::now();
    let all_issues: Vec<Issue> = files
        .par_iter()
        .flat_map(|path| {
            let source_text = match fs::read_to_string(path) {
                Ok(s) => s,
                Err(err) => {
                    eprintln!("\n  Warning: could not read '{}': {err}", path.display());
                    processed.fetch_add(1, Ordering::Relaxed);
                    return vec![];
                }
            };

            let allocator = Allocator::default();

            let program = match parse_file(&allocator, path, &source_text) {
                Ok(p) => p,
                Err(err) => {
                    eprintln!(
                        "\n  Warning: failed to parse '{}': {}",
                        err.file,
                        err.messages.join("; ")
                    );
                    processed.fetch_add(1, Ordering::Relaxed);
                    return vec![];
                }
            };

            let mut file_issues = analyze(&program, &source_text, path, max_lines, &category);
            if !custom_rule_set.is_empty() {
                file_issues.extend(run_custom_rules(&custom_rule_set, &source_text, path));
            }
            processed.fetch_add(1, Ordering::Relaxed);
            file_issues
        })
        .collect();

    // Stop progress thread and clear line.
    done_flag.store(true, Ordering::Relaxed);
    let _ = progress_thread.join();
    let analyze_ms = t.elapsed().as_millis();
    eprint!(
        "\r  ✅ Analyzed {file_count} file(s) — {} issue(s) in {}{}",
        all_issues.len(),
        fmt_ms(analyze_ms),
        " ".repeat(20)
    );
    eprintln!();

    // ── Step 3b: Run external tools (oxlint, cargo-audit) ────────────────────
    let mut all_issues = all_issues;
    let external_ran = cli.external;

    if cli.external {
        let ext = run_external_tools(&cli.path);
        for (tool, reason) in &ext.tools_skipped {
            eprintln!("  ⚠  {tool}: {reason}");
        }
        all_issues.extend(ext.issues);
    } else {
        eprintln!("  ⏭  External tools not enabled (pass --external to run oxlint + cargo-audit)");
    }

    // ── Step 3c: Apply baseline (suppress known issues) ───────────────────────
    let all_issues = if let Some(ref baseline_path) = cli.baseline {
        let entries = load_baseline(baseline_path);
        filter_baseline(all_issues, &entries)
    } else {
        all_issues
    };

    eprintln!();

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
            let html = report_html(&all_issues, &cli.path, file_count, external_ran);
            let out_path = cli
                .output
                .clone()
                .unwrap_or_else(|| std::path::PathBuf::from("react-perf-report.html"));

            match fs::write(&out_path, &html) {
                Ok(_) => {
                    let abs_path =
                        std::fs::canonicalize(&out_path).unwrap_or_else(|_| out_path.clone());
                    eprintln!("✅ HTML report → {}", out_path.display());
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
                Ok(_) => eprintln!("✅ SARIF report → {}", out_path.display()),
                Err(e) => {
                    eprintln!("Error writing SARIF to '{}': {e}", out_path.display());
                    std::process::exit(2);
                }
            }
            all_issues.len()
        }
    };

    // ── Step 5: Summary line ──────────────────────────────────────────────────
    let total_ms = total_start.elapsed().as_millis();
    eprintln!(
        "\n{} issue(s) found across {file_count} file(s) — total {}",
        issue_count,
        fmt_ms(total_ms),
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
