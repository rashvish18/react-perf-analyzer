/// rules/mod.rs — Core rule engine types + rule registry.
///
/// Architecture:
///   rules/perf/     — 15 React performance rules (existing)
///   rules/security/ — React-specific security rules (new)
///
/// External tools (oxc_linter, cargo-audit) are orchestrated separately
/// and their results are merged into the same Issue type.
pub mod perf;
pub mod security;

use std::path::{Path, PathBuf};

use oxc_ast::ast::Program;

use crate::cli::Category;

// ─── Issue ────────────────────────────────────────────────────────────────────

/// A single lint finding from any source (our rules, oxc_linter, cargo-audit).
#[derive(Debug, Clone, serde::Serialize)]
pub struct Issue {
    /// The rule name that produced this issue (e.g. "no_inline_jsx_fn").
    pub rule: String,
    /// Human-readable description of the problem.
    pub message: String,
    /// Path to the file containing the issue.
    pub file: PathBuf,
    /// 1-indexed line number of the issue.
    pub line: u32,
    /// 1-indexed column number of the issue.
    pub column: u32,
    /// Severity level.
    pub severity: Severity,
    /// Which tool produced this issue.
    pub source: IssueSource,
    /// Rule category: performance, security, or dependency.
    pub category: IssueCategory,
}

// ─── Severity ─────────────────────────────────────────────────────────────────

/// Ordered severity levels (Info < Low < Medium < High < Critical).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, serde::Serialize)]
#[serde(rename_all = "lowercase")]
#[allow(dead_code)] // Info/Low used by Phase 5 --fail-on, OxcLinter/CargoAudit used by Phase 4
pub enum Severity {
    Info,
    Low,
    /// Default for existing perf rules.
    Medium,
    High,
    Critical,
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Severity::Info => write!(f, "info"),
            Severity::Low => write!(f, "low"),
            Severity::Medium => write!(f, "medium"),
            Severity::High => write!(f, "high"),
            Severity::Critical => write!(f, "critical"),
        }
    }
}

// ─── IssueSource ──────────────────────────────────────────────────────────────

/// Which tool produced the issue — used for grouped display in the HTML report.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
#[allow(dead_code)]
pub enum IssueSource {
    /// Our own oxc-based rules (perf + React security).
    ReactPerfAnalyzer,
    /// Results ingested from oxc_linter (general JS/TS rules).
    OxcLinter,
    /// Results ingested from cargo-audit (Rust dependency CVEs).
    CargoAudit,
}

impl std::fmt::Display for IssueSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IssueSource::ReactPerfAnalyzer => write!(f, "react-perf-analyzer"),
            IssueSource::OxcLinter => write!(f, "oxc-linter"),
            IssueSource::CargoAudit => write!(f, "cargo-audit"),
        }
    }
}

// ─── IssueCategory ────────────────────────────────────────────────────────────

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "lowercase")]
#[allow(dead_code)]
pub enum IssueCategory {
    Performance,
    Security,
    Dependency,
}

impl std::fmt::Display for IssueCategory {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            IssueCategory::Performance => write!(f, "performance"),
            IssueCategory::Security => write!(f, "security"),
            IssueCategory::Dependency => write!(f, "dependency"),
        }
    }
}

// ─── RuleContext ──────────────────────────────────────────────────────────────

/// Context passed to every rule's `run` method.
pub struct RuleContext<'a> {
    pub program: &'a Program<'a>,
    pub source_text: &'a str,
    pub file_path: &'a Path,
    pub max_component_lines: usize,
}

// ─── Rule trait ───────────────────────────────────────────────────────────────

/// The trait every lint rule must implement.
pub trait Rule: Send + Sync {
    #[allow(dead_code)]
    fn name(&self) -> &str;
    fn run(&self, ctx: &RuleContext<'_>) -> Vec<Issue>;
}

// ─── Registry ─────────────────────────────────────────────────────────────────

/// Returns rules filtered by category.
pub fn all_rules(category: &Category) -> Vec<Box<dyn Rule>> {
    match category {
        Category::Perf => perf::perf_rules(),
        Category::Security => security::security_rules(),
        Category::All => {
            let mut r = perf::perf_rules();
            r.extend(security::security_rules());
            r
        }
    }
}
