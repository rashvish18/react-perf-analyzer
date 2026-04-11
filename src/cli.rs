/// cli.rs — CLI argument parsing using clap derive macros.
use std::path::PathBuf;

use clap::{Parser, ValueEnum};

use crate::rules::Severity;

/// React performance + security static analysis tool.
///
/// Scans JS/TS/JSX files for React-specific performance anti-patterns
/// and security vulnerabilities.
///
/// Example usage:
///   react-perf-analyzer ./src
///   react-perf-analyzer ./src --category security
///   react-perf-analyzer ./src --format sarif --output results.sarif
///   react-perf-analyzer ./src --fail-on high
#[derive(Parser, Debug)]
#[command(
    name = "react-perf-analyzer",
    version,
    about = "React performance + security scanner. Single binary. Zero config.",
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
    /// - html: Self-contained HTML report
    /// - sarif: SARIF 2.1.0 for GitHub/GitLab/Azure DevOps inline annotations
    #[arg(long, default_value = "text", value_name = "FORMAT")]
    pub format: OutputFormat,

    /// Write output to a file instead of stdout.
    #[arg(long, value_name = "FILE")]
    pub output: Option<PathBuf>,

    /// Rule category to run.
    ///
    /// - all: Run all rules (default)
    /// - perf: Only React performance rules (existing 15)
    /// - security: Only React security rules
    #[arg(long, value_enum, default_value = "all", value_name = "CATEGORY")]
    pub category: Category,

    /// Minimum severity level that causes a non-zero exit code.
    ///
    /// Useful for CI gates. Example: --fail-on high exits 1 if any
    /// high or critical issues are found.
    ///
    /// - none: Never fail on severity (default — exit 1 only if any issues)
    /// - low | medium | high | critical: fail if issues at that level or above
    #[arg(long, value_enum, default_value = "none", value_name = "LEVEL")]
    pub fail_on: FailOn,

    /// Maximum number of lines allowed in a single React component.
    #[arg(long, default_value_t = 300, value_name = "LINES")]
    pub max_component_lines: usize,

    /// Include test and Storybook files in the analysis.
    #[arg(long, default_value_t = false)]
    pub include_tests: bool,

    /// Only analyze files that have changed in git (staged + unstaged).
    ///
    /// Designed for pre-commit hooks — typically completes in <10 ms because
    /// only modified files are parsed and analyzed. Ignored if the current
    /// directory is not inside a git repository (falls back to full scan).
    ///
    /// Example pre-commit hook:
    ///   react-perf-analyzer ./src --only-changed --fail-on high
    #[arg(long, default_value_t = false)]
    pub only_changed: bool,

    /// Path to a baseline JSON file produced by a previous run.
    ///
    /// Issues already present in the baseline are suppressed so CI only
    /// fails on *new* regressions. Generate a baseline with:
    ///   react-perf-analyzer ./src --format json --output .sast-baseline.json
    #[arg(long, value_name = "FILE")]
    pub baseline: Option<PathBuf>,

    /// Skip running external tools (oxlint, cargo-audit).
    ///
    /// By default react-perf-analyzer runs oxlint and cargo-audit as
    /// subprocesses to give a unified view. Use this flag to skip them and
    /// run only the built-in React rules — useful when you just want a fast
    /// scan without waiting for npm/Node.js startup.
    #[arg(long, default_value_t = false)]
    pub no_external: bool,

    /// Path to a TOML file containing custom lint rules.
    ///
    /// If not specified, the tool auto-discovers `react-perf-rules.toml`
    /// by walking up from the scan path. Set to an empty string to disable
    /// custom rules entirely.
    ///
    /// Example rule file:
    ///
    ///   [[rule]]
    ///   id       = "no-console-log"
    ///   message  = "Remove console.log before merging"
    ///   severity = "medium"
    ///   pattern  = "console\\.log\\s*\\("
    #[arg(long, value_name = "FILE")]
    pub rules: Option<PathBuf>,
}

/// Supported output formats.
#[derive(ValueEnum, Clone, Debug, PartialEq)]
pub enum OutputFormat {
    /// Human-readable text output (default).
    Text,
    /// JSON array, suitable for tooling integration.
    Json,
    /// Self-contained HTML report.
    Html,
    /// SARIF 2.1.0 — GitHub/GitLab/Azure DevOps inline annotations.
    Sarif,
}

/// Rule category filter.
#[derive(ValueEnum, Clone, Debug, PartialEq, Default)]
pub enum Category {
    /// Run all rules (perf + security).
    #[default]
    All,
    /// Only React performance rules.
    Perf,
    /// Only React security rules.
    Security,
}

/// Severity threshold for CI exit code.
#[derive(ValueEnum, Clone, Debug, PartialEq)]
pub enum FailOn {
    /// Never fail based on severity (exit 1 if any issues found).
    None,
    Low,
    Medium,
    High,
    Critical,
}

impl FailOn {
    /// Convert to the corresponding Severity threshold.
    /// Returns None when FailOn::None (use default "any issue" logic).
    pub fn as_severity(&self) -> Option<Severity> {
        match self {
            FailOn::None => None,
            FailOn::Low => Some(Severity::Low),
            FailOn::Medium => Some(Severity::Medium),
            FailOn::High => Some(Severity::High),
            FailOn::Critical => Some(Severity::Critical),
        }
    }
}
