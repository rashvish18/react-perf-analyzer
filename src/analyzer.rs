/// analyzer.rs — Rule dispatch and issue collection.
///
/// The analyzer is intentionally thin: it constructs a `RuleContext` from the
/// parsed file data, then calls every registered rule's `run` method.
///
/// # Design choices
///
/// - **No shared mutable state**: Each rule creates its own visitor internally.
///   Rules only communicate with the analyzer by returning `Vec<Issue>`.
///
/// - **Single AST pass per rule**: Each rule gets a reference to the *same*
///   `Program`. Parsing happens exactly once per file in `main.rs`; the
///   analyzer does not re-parse.
///
/// - **Extensible**: Adding a new rule requires only adding it to `rules::all_rules()`.
///   The analyzer loop here never needs to change.
use std::path::Path;

use oxc_ast::ast::Program;

use crate::cli::Category;
use crate::rules::{all_rules, Issue, Rule, RuleContext};

/// Run all registered lint rules against a single parsed file.
///
/// # Arguments
/// * `program`             — The parsed OXC AST for this file.
/// * `source_text`         — Raw source code (used for line/col conversion).
/// * `file_path`           — Path of the file (embedded in each `Issue`).
/// * `max_component_lines` — Passed to `large_component` rule.
///
/// # Returns
/// All `Issue`s found across all rules, in the order the rules are registered.
pub fn analyze(
    program: &Program<'_>,
    source_text: &str,
    file_path: &Path,
    max_component_lines: usize,
    category: &Category,
) -> Vec<Issue> {
    analyze_with_rules(
        program,
        source_text,
        file_path,
        max_component_lines,
        &all_rules(category),
    )
}

/// Run a custom set of rules — used in tests to exercise individual rules.
///
/// Production callers should use `analyze()` which uses the full registry.
pub fn analyze_with_rules(
    program: &Program<'_>,
    source_text: &str,
    file_path: &Path,
    max_component_lines: usize,
    rules: &[Box<dyn Rule>],
) -> Vec<Issue> {
    let ctx = RuleContext {
        program,
        source_text,
        file_path,
        max_component_lines,
    };

    // Run every rule and flatten the results into a single Vec.
    // Rules are independent — order here is display order in the report.
    rules.iter().flat_map(|rule| rule.run(&ctx)).collect()
}
