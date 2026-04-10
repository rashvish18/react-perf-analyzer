/// rules/mod.rs — Core rule engine types.
///
/// Defines:
///   - `Issue`       — a single lint finding (location + message)
///   - `Severity`    — warning vs error
///   - `RuleContext` — shared context passed to every rule
///   - `Rule` trait  — the interface every lint rule implements
///   - `all_rules()` — returns the full registry of active rules
pub mod large_component;
pub mod no_array_index_key;
pub mod no_component_in_component;
pub mod no_expensive_in_render;
pub mod no_inline_jsx_fn;
pub mod no_json_in_render;
pub mod no_math_random_in_render;
pub mod no_new_context_value;
pub mod no_new_in_jsx_prop;
pub mod no_object_entries_in_render;
pub mod no_regex_in_render;
pub mod no_unstable_hook_deps;
pub mod no_use_state_lazy_init_missing;
pub mod no_useless_memo;
pub mod unstable_props;

use std::path::{Path, PathBuf};

use oxc_ast::ast::Program;

use crate::rules::large_component::LargeComponent;
use crate::rules::no_array_index_key::NoArrayIndexKey;
use crate::rules::no_component_in_component::NoComponentInComponent;
use crate::rules::no_expensive_in_render::NoExpensiveInRender;
use crate::rules::no_inline_jsx_fn::NoInlineJsxFn;
use crate::rules::no_json_in_render::NoJsonInRender;
use crate::rules::no_math_random_in_render::NoMathRandomInRender;
use crate::rules::no_new_context_value::NoNewContextValue;
use crate::rules::no_new_in_jsx_prop::NoNewInJsxProp;
use crate::rules::no_object_entries_in_render::NoObjectEntriesInRender;
use crate::rules::no_regex_in_render::NoRegexInRender;
use crate::rules::no_unstable_hook_deps::NoUnstableHookDeps;
use crate::rules::no_use_state_lazy_init_missing::NoUseStateLazyInitMissing;
use crate::rules::no_useless_memo::NoUselessMemo;
use crate::rules::unstable_props::UnstableProps;

// ─── Issue ───────────────────────────────────────────────────────────────────

/// A single lint finding reported by a rule.
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
}

/// Severity level for a lint issue.
#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Warning,
    #[allow(dead_code)] // Reserved for future rules (e.g. security-critical patterns).
    Error,
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Severity::Warning => write!(f, "warning"),
            Severity::Error => write!(f, "error"),
        }
    }
}

// ─── RuleContext ──────────────────────────────────────────────────────────────

/// Context passed to every rule's `run` method.
///
/// Bundles together everything a rule needs to analyze a file:
/// the parsed AST, the raw source text for span-to-line conversion,
/// the file path for issue location, and configurable thresholds.
pub struct RuleContext<'a> {
    /// The fully parsed OXC AST for this file.
    pub program: &'a Program<'a>,

    /// Raw source text of the file. Used for line/column conversion
    /// and for the `large_component` line-count calculation.
    pub source_text: &'a str,

    /// Path of the file being analyzed (used in Issue reports).
    pub file_path: &'a Path,

    /// Maximum lines a component may have before `large_component` fires.
    pub max_component_lines: usize,
}

// ─── Rule trait ───────────────────────────────────────────────────────────────

/// The trait every lint rule must implement.
///
/// Rules are stateless — all per-file state lives inside the visitor struct
/// created inside `run`. The `Rule` objects themselves are created once and
/// shared across all Rayon threads (hence `Send + Sync`).
pub trait Rule: Send + Sync {
    /// Short snake_case name used in issue reports (e.g. `"no_inline_jsx_fn"`).
    /// Called by the reporter when formatting output.
    #[allow(dead_code)]
    fn name(&self) -> &str;

    /// Analyze the file represented by `ctx` and return any issues found.
    ///
    /// Called once per file. Implementations typically create a visitor
    /// struct, walk the AST with `visitor.visit_program(ctx.program)`, and
    /// return the collected issues.
    fn run(&self, ctx: &RuleContext<'_>) -> Vec<Issue>;
}

// ─── Registry ─────────────────────────────────────────────────────────────────

/// Returns all registered lint rules, boxed as trait objects.
///
/// To add a new rule: implement `Rule`, add it to this Vec.
pub fn all_rules() -> Vec<Box<dyn Rule>> {
    vec![
        Box::new(NoInlineJsxFn),
        Box::new(UnstableProps),
        Box::new(LargeComponent),
        Box::new(NoNewContextValue),
        Box::new(NoArrayIndexKey),
        Box::new(NoExpensiveInRender),
        Box::new(NoComponentInComponent),
        Box::new(NoUnstableHookDeps),
        Box::new(NoNewInJsxProp),
        Box::new(NoUseStateLazyInitMissing),
        Box::new(NoJsonInRender),
        Box::new(NoObjectEntriesInRender),
        Box::new(NoRegexInRender),
        Box::new(NoMathRandomInRender),
        Box::new(NoUselessMemo),
    ]
}
