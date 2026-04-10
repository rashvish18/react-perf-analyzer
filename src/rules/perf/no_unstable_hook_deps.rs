//! rules/no_unstable_hook_deps.rs — Detect unstable values in hook dependency arrays.
//!
//! # What this detects
//!
//! Object literals, array literals, or inline functions used directly inside
//! the dependency array of `useEffect`, `useMemo`, `useCallback`, or
//! `useLayoutEffect`. These create a new reference on every render, making
//! the hook run on every render — defeating its purpose entirely.
//!
//! ```jsx
//! // ❌ New object every render → useEffect runs every render
//! useEffect(() => { fetchUser(filters) }, [{ id: userId }])
//!
//! // ❌ New array every render
//! useMemo(() => compute(a, b), [[a, b]])
//!
//! // ❌ Inline function in deps
//! useCallback(() => doThing(), [() => helper()])
//!
//! // ✅ Pass stable primitives or variables
//! useEffect(() => { fetchUser(filters) }, [userId])
//! ```
//!
//! # Why it's a problem
//!
//! React compares dependency array elements with `Object.is`. An object `{}`
//! or array `[]` literal creates a new reference on every render, so
//! `Object.is(prev, next)` is always `false` — the hook runs on every render.
//! This is one of the most common causes of infinite re-render loops.
//!
//! # AST traversal strategy
//!
//! 1. `visit_call_expression` — detect calls to the known hook names.
//! 2. Grab the last argument (the deps array).
//! 3. For each element in the deps `ArrayExpression`, check if it is an
//!    `ObjectExpression`, `ArrayExpression`, or function expression.
//! 4. Emit an issue for each unstable element found.

use std::path::Path;

use oxc_ast::ast::{ArrayExpressionElement, CallExpression, Expression};
use oxc_ast_visit::{walk, Visit};
use oxc_span::Span;

use crate::{
    rules::{Issue, RuleContext, Severity},
    utils::offset_to_line_col,
};

// ─── Rule struct ──────────────────────────────────────────────────────────────

pub struct NoUnstableHookDeps;

impl super::Rule for NoUnstableHookDeps {
    fn name(&self) -> &str {
        "no_unstable_hook_deps"
    }

    fn run(&self, ctx: &RuleContext<'_>) -> Vec<Issue> {
        let mut visitor = HookDepsVisitor {
            issues: Vec::new(),
            source_text: ctx.source_text,
            file_path: ctx.file_path,
        };
        visitor.visit_program(ctx.program);
        visitor.issues
    }
}

// ─── Visitor ──────────────────────────────────────────────────────────────────

struct HookDepsVisitor<'a> {
    issues: Vec<Issue>,
    source_text: &'a str,
    file_path: &'a Path,
}

/// Hooks whose last argument is a dependency array.
const DEP_HOOKS: &[&str] = &[
    "useEffect",
    "useMemo",
    "useCallback",
    "useLayoutEffect",
    "useInsertionEffect",
];

impl<'a> Visit<'a> for HookDepsVisitor<'_> {
    fn visit_call_expression(&mut self, expr: &CallExpression<'a>) {
        if let Some(hook_name) = hook_name(expr) {
            // The deps array is always the last argument for these hooks.
            if let Some(last_arg) = expr.arguments.last() {
                if let Some(Expression::ArrayExpression(deps_array)) = last_arg.as_expression() {
                    for element in &deps_array.elements {
                        self.check_dep(hook_name, element);
                    }
                }
            }
        }

        walk::walk_call_expression(self, expr);
    }
}

impl HookDepsVisitor<'_> {
    /// Check a single dependency element for instability.
    fn check_dep(&mut self, hook_name: &str, element: &ArrayExpressionElement<'_>) {
        match element {
            ArrayExpressionElement::ObjectExpression(obj) => {
                self.emit(
                    hook_name,
                    "object literal `{}`",
                    "Replace with individual stable primitive values or refs.",
                    obj.span,
                );
            }
            ArrayExpressionElement::ArrayExpression(arr) => {
                self.emit(
                    hook_name,
                    "nested array `[]`",
                    "Nested arrays in deps are always a new reference. Use individual stable values instead.",
                    arr.span,
                );
            }
            ArrayExpressionElement::ArrowFunctionExpression(arrow) => {
                self.emit(
                    hook_name,
                    "arrow function",
                    "Functions in deps create a new reference every render. Wrap with useCallback or move outside the component.",
                    arrow.span,
                );
            }
            ArrayExpressionElement::FunctionExpression(func) => {
                self.emit(
                    hook_name,
                    "function expression",
                    "Functions in deps create a new reference every render. Wrap with useCallback or move outside the component.",
                    func.span,
                );
            }
            _ => {}
        }
    }

    fn emit(&mut self, hook: &str, kind: &str, suggestion: &str, span: Span) {
        let (line, col) = offset_to_line_col(self.source_text, span.start);
        self.issues.push(Issue {
            rule: "no_unstable_hook_deps".to_string(),
            message: format!(
                "`{hook}` dependency contains a {kind} — new reference on every render causes \
                 the hook to run every render. {suggestion}"
            ),
            file: self.file_path.to_path_buf(),
            line,
            column: col,
            severity: Severity::Medium,
            source: crate::rules::IssueSource::ReactPerfAnalyzer,
            category: crate::rules::IssueCategory::Performance,
        });
    }
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

/// Returns the hook name if `expr` is a call to one of the known dep hooks.
fn hook_name<'a>(expr: &'a CallExpression<'_>) -> Option<&'a str> {
    match &expr.callee {
        Expression::Identifier(id) => {
            let name = id.name.as_str();
            if DEP_HOOKS.contains(&name) {
                Some(name)
            } else {
                None
            }
        }
        // React.useEffect(...) etc.
        Expression::StaticMemberExpression(member) => {
            let name = member.property.name.as_str();
            if DEP_HOOKS.contains(&name) {
                Some(name)
            } else {
                None
            }
        }
        _ => None,
    }
}
