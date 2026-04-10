/// cli.rs — CLI argument parsing using clap derive macros.
///
/// Defines the top-level `Cli` struct that clap populates from argv.
/// Kept intentionally minimal: path, output format, and rule thresholds.
use std::path::PathBuf;

use clap::{Parser, ValueEnum};

/// React performance static analysis tool.
///
/// Scans JS/TS/JSX files for React-specific performance anti-patterns
/// and outputs warnings to stdout.
///
/// Example usage:
///   react-perf-analyzer ./src
///   react-perf-analyzer ./src --format json
///   react-perf-analyzer ./src --format html --output report.html
///   react-perf-analyzer ./src --max-component-lines 150
#[derive(Parser, Debug)]
#[command(
    name = "react-perf-analyzer",
    version,
    about = "Static analysis for React performance anti-patterns",
    long_about = None
)]
pub struct Cli {
    /// Path to the file or directory to analyze.
    /// Directories are scanned recursively for .js/.jsx/.ts/.tsx files.
    pub path: PathBuf,

    /// Output format for lint results.
    ///
    /// - text: Human-readable columnar output (default)
    /// - json: Machine-readable JSON array
    /// - html: Self-contained HTML report with summary, charts, and issue table
    #[arg(long, default_value = "text", value_name = "FORMAT")]
    pub format: OutputFormat,

    /// Write output to a file instead of stdout.
    ///
    /// Required when using --format html (report can be very large).
    /// Optional for text and json — defaults to stdout when omitted.
    ///
    /// Example: --output report.html
    #[arg(long, value_name = "FILE")]
    pub output: Option<PathBuf>,

    /// Maximum number of lines allowed in a single React component.
    ///
    /// Components exceeding this threshold trigger the `large_component` warning.
    /// Default is 300 lines.
    #[arg(long, default_value_t = 300, value_name = "LINES")]
    pub max_component_lines: usize,

    /// Include test and Storybook files in the analysis.
    ///
    /// By default, the following file patterns are skipped because inline
    /// functions and object literals are idiomatic in tests/stories:
    ///   *.test.{js,ts,jsx,tsx}
    ///   *.spec.{js,ts,jsx,tsx}
    ///   *.stories.{js,ts,jsx,tsx}
    ///   *.story.{js,ts,jsx,tsx}
    ///   __tests__/ directories
    ///
    /// Pass this flag to lint those files anyway.
    #[arg(long, default_value_t = false)]
    pub include_tests: bool,
}

/// Supported output formats for lint results.
#[derive(ValueEnum, Clone, Debug, PartialEq)]
pub enum OutputFormat {
    /// Human-readable text output (default).
    Text,
    /// JSON array output, suitable for tooling integration.
    Json,
    /// Self-contained HTML report with summary stats and issue table.
    Html,
}
