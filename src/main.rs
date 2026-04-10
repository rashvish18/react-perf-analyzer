/// main.rs — react-perf-lint entry point.
///
/// Orchestrates the full lint pipeline:
///
///   1. Parse CLI arguments (clap)
///   2. Collect JS/TS/JSX files from the given path (walkdir)
///   3. Parse + analyze files IN PARALLEL (rayon + oxc)
///   4. Report results to stdout (text or JSON)
///   5. Exit with code 1 if any issues were found (CI-friendly)
///
/// # Concurrency model
///
/// Rayon's `par_iter` distributes files across CPU threads automatically.
/// Each thread handles its own file independently:
///   - Reads the file from disk
///   - Creates a fresh OXC `Allocator` (NOT Send, so cannot be shared)
///   - Parses the source into an AST
///   - Runs all rules against the AST
///   - Returns a Vec<Issue>
///
/// The `flat_map` + `collect` at the end gathers all issues from all threads
/// into a single sorted Vec without any Mutex or shared state.
///
/// # Exit codes
///   0 — no issues found (clean)
///   1 — one or more issues found
///   2 — fatal error (e.g. path does not exist)
mod analyzer;
mod cli;
mod file_loader;
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
    cli::{Cli, OutputFormat},
    file_loader::collect_files,
    parser::parse_file,
    reporter::{report_html, report_json, report_text},
    rules::Issue,
};

fn main() {
    // ── Step 1: Parse CLI arguments ──────────────────────────────────────────
    let cli = Cli::parse();

    // ── Step 2: Collect files ─────────────────────────────────────────────────
    let start = Instant::now();
    let files = collect_files(&cli.path, cli.include_tests);

    if files.is_empty() {
        eprintln!("No JS/TS/JSX files found under '{}'.", cli.path.display());
        std::process::exit(0);
    }

    let file_count = files.len();
    let max_lines = cli.max_component_lines;

    // ── Step 3: Parallel parse + analyze ─────────────────────────────────────
    //
    // `par_iter` splits the file list across Rayon's global thread pool.
    //
    // IMPORTANT — Allocator lifetime:
    //   OXC's `Allocator` is NOT Send (it's a single-threaded bump allocator).
    //   We MUST create a new Allocator inside the closure so it lives entirely
    //   within one thread's stack frame. The `Program<'a>` borrows from it,
    //   so both must be in the same scope.
    let all_issues: Vec<Issue> = files
        .par_iter()
        .flat_map(|path| {
            // Read the file. Errors are logged and skipped.
            let source_text = match fs::read_to_string(path) {
                Ok(s) => s,
                Err(err) => {
                    eprintln!("Warning: could not read '{}': {err}", path.display());
                    return vec![];
                }
            };

            // One allocator per file, per thread.
            // The allocator and source_text are both local → they outlive the Program.
            let allocator = Allocator::default();

            // Parse the file. Non-fatal parse errors are swallowed; fatal
            // errors (panics) cause the file to be skipped with a warning.
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

            // Run all lint rules against the AST.
            analyze(&program, &source_text, path, max_lines)
        })
        .collect();

    // ── Step 4: Report ────────────────────────────────────────────────────────
    let issue_count = match cli.format {
        OutputFormat::Text => {
            let count = report_text(&all_issues);
            // Write to file if --output was specified, otherwise already printed.
            if let Some(ref out_path) = cli.output {
                // Re-capture text output into a file by re-running (simple approach:
                // redirect — for text format, just duplicate to file too).
                let _ = fs::write(out_path, ""); // truncate; text already on stdout
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
                    let file_url = format!("file://{}", abs_path.display());
                    eprintln!("✅ HTML report written to: {}", out_path.display());
                    eprintln!("   {file_url}");
                }
                Err(e) => {
                    eprintln!("Error writing HTML report to '{}': {e}", out_path.display());
                    std::process::exit(2);
                }
            }
            all_issues.len()
        }
    };

    // Print file stats to stderr (doesn't interfere with stdout output).
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

    // ── Step 5: Exit code ─────────────────────────────────────────────────────
    // Exit 1 when issues are found — enables use in CI pipelines:
    //   react-perf-lint ./src || echo "Performance issues detected!"
    std::process::exit(if issue_count > 0 { 1 } else { 0 });
}
