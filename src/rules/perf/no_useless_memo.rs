//! rules/no_useless_memo.rs — Detect useMemo/useCallback with empty [] deps.
//!
//! # What this detects
//!
//! `useMemo` or `useCallback` calls with an empty dependency array `[]` where
//! the wrapped value never changes and should be a module-level constant instead.
//! This adds hook overhead and memory cost for no benefit.
//!
//! ```jsx
//! // ❌ Never changes — wastes hook overhead every render and holds a closure
//! const config = useMemo(() => ({ theme: 'dark', lang: 'en' }), []);
//! const noop = useCallback(() => {}, []);
//! const BASE_URL = useMemo(() => 'https://api.example.com', []);
//!
//! // ✅ Module-level constant — zero overhead, created once
//! const CONFIG = { theme: 'dark', lang: 'en' };
//! const noop = () => {};
//! const BASE_URL = 'https://api.example.com';
//! ```
//!
//! # Why it's a problem
//!
//! `useMemo(() => x, [])` with empty deps means the value is computed once on
//! mount and never recomputed — identical to a module-level constant. The hook
//! adds memory overhead (closure + dep array), React internal bookkeeping, and
//! a function call on every render to check if the deps changed.
//!
//! # AST traversal strategy
//!
//! `visit_call_expression` → detect `useMemo(fn, [])` or `useCallback(fn, [])`
//! where the second argument is an empty `ArrayExpression`.

use std::path::Path;

use oxc_ast::ast::{CallExpression, Expression};
use oxc_ast_visit::{walk, Visit};
use oxc_span::Span;

use crate::{
    rules::{Issue, RuleContext, Severity},
    utils::offset_to_line_col,
};

// ─── Rule struct ──────────────────────────────────────────────────────────────

pub struct NoUselessMemo;

impl super::Rule for NoUselessMemo {
    fn name(&self) -> &str {
        "no_useless_memo"
    }

    fn run(&self, ctx: &RuleContext<'_>) -> Vec<Issue> {
        let mut visitor = UselessMemoVisitor {
            issues: Vec::new(),
            source_text: ctx.source_text,
            file_path: ctx.file_path,
        };
        visitor.visit_program(ctx.program);
        visitor.issues
    }
}

// ─── Visitor ──────────────────────────────────────────────────────────────────

struct UselessMemoVisitor<'a> {
    issues: Vec<Issue>,
    source_text: &'a str,
    file_path: &'a Path,
}

impl<'a> Visit<'a> for UselessMemoVisitor<'_> {
    fn visit_call_expression(&mut self, expr: &CallExpression<'a>) {
        if let Some(hook_name) = memo_hook_name(expr) {
            // Must have exactly 2 arguments: the factory/callback and the deps array.
            if expr.arguments.len() == 2 {
                if let Some(Expression::ArrayExpression(arr)) = expr.arguments[1].as_expression() {
                    if arr.elements.is_empty() {
                        self.emit(hook_name, expr.span);
                    }
                }
            }
        }
        walk::walk_call_expression(self, expr);
    }
}

impl UselessMemoVisitor<'_> {
    fn emit(&mut self, hook: &str, span: Span) {
        let (line, col) = offset_to_line_col(self.source_text, span.start);
        let alt = if hook == "useMemo" {
            "a module-level constant or variable"
        } else {
            "a module-level function"
        };
        self.issues.push(Issue {
            rule: "no_useless_memo".to_string(),
            message: format!(
                "`{hook}` with empty `[]` deps never recomputes — equivalent to {alt}. \
                 Move the value outside the component to eliminate hook overhead."
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

fn memo_hook_name<'a>(expr: &'a CallExpression<'_>) -> Option<&'a str> {
    match &expr.callee {
        Expression::Identifier(id) => match id.name.as_str() {
            "useMemo" | "useCallback" => Some(id.name.as_str()),
            _ => None,
        },
        Expression::StaticMemberExpression(m) => match m.property.name.as_str() {
            "useMemo" | "useCallback" => Some(m.property.name.as_str()),
            _ => None,
        },
        _ => None,
    }
}
