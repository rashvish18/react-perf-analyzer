//! rules/no_math_random_in_render.rs — Detect Math.random() / Date.now() in JSX props.
//!
//! # What this detects
//!
//! Calls to `Math.random()` or `Date.now()` used directly as JSX prop values.
//! These return a different value on every call, guaranteeing a re-render of
//! the child on every parent render.
//!
//! ```jsx
//! // ❌ Different value every render — child always re-renders
//! <Avatar seed={Math.random()} />
//! <Timestamp value={Date.now()} />
//!
//! // ✅ Generate once with useMemo or useState
//! const seed = useMemo(() => Math.random(), []);
//! <Avatar seed={seed} />
//! ```
//!
//! # Why it's a problem
//!
//! Non-deterministic values change on every render. React's reconciler uses
//! prop equality to skip child re-renders — a value that always changes defeats
//! this entirely and can cause infinite loops if the child triggers parent state.
//!
//! # AST traversal strategy
//!
//! `visit_jsx_opening_element` → scan attribute expressions for calls to
//! `Math.random` or `Date.now`.

use std::path::Path;

use oxc_ast::ast::{Expression, JSXAttributeItem, JSXAttributeValue, JSXOpeningElement};
use oxc_ast_visit::{walk, Visit};
use oxc_span::Span;

use crate::{
    rules::{Issue, RuleContext, Severity},
    utils::offset_to_line_col,
};

// ─── Rule struct ──────────────────────────────────────────────────────────────

pub struct NoMathRandomInRender;

impl super::Rule for NoMathRandomInRender {
    fn name(&self) -> &str {
        "no_math_random_in_render"
    }

    fn run(&self, ctx: &RuleContext<'_>) -> Vec<Issue> {
        let mut visitor = MathRandomVisitor {
            issues: Vec::new(),
            source_text: ctx.source_text,
            file_path: ctx.file_path,
        };
        visitor.visit_program(ctx.program);
        visitor.issues
    }
}

// ─── Visitor ──────────────────────────────────────────────────────────────────

struct MathRandomVisitor<'a> {
    issues: Vec<Issue>,
    source_text: &'a str,
    file_path: &'a Path,
}

impl<'a> Visit<'a> for MathRandomVisitor<'_> {
    fn visit_jsx_opening_element(&mut self, elem: &JSXOpeningElement<'a>) {
        for attr_item in &elem.attributes {
            if let JSXAttributeItem::Attribute(attr) = attr_item {
                if let Some(JSXAttributeValue::ExpressionContainer(container)) = &attr.value {
                    if let Some(expr) = container.expression.as_expression() {
                        self.scan_expr(expr);
                    }
                }
            }
        }
        walk::walk_jsx_opening_element(self, elem);
    }
}

impl MathRandomVisitor<'_> {
    fn scan_expr(&mut self, expr: &Expression<'_>) {
        match expr {
            Expression::CallExpression(call) => {
                if let Some(fn_name) = nondeterministic_call(&call.callee) {
                    self.emit(fn_name, call.span);
                }
            }
            Expression::ConditionalExpression(cond) => {
                self.scan_expr(&cond.consequent);
                self.scan_expr(&cond.alternate);
            }
            Expression::LogicalExpression(logical) => {
                self.scan_expr(&logical.left);
                self.scan_expr(&logical.right);
            }
            Expression::ParenthesizedExpression(paren) => {
                self.scan_expr(&paren.expression);
            }
            _ => {}
        }
    }

    fn emit(&mut self, fn_name: &str, span: Span) {
        let (line, col) = offset_to_line_col(self.source_text, span.start);
        self.issues.push(Issue {
            rule: "no_math_random_in_render".to_string(),
            message: format!(
                "`{fn_name}()` in a JSX prop returns a different value on every render, \
                 guaranteeing the child re-renders every time. \
                 Generate once with useMemo or useState: \
                 `const value = useMemo(() => {fn_name}(), [])`"
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

/// Returns the full name if callee is `Math.random` or `Date.now`.
fn nondeterministic_call(callee: &Expression<'_>) -> Option<&'static str> {
    if let Expression::StaticMemberExpression(m) = callee {
        if let Expression::Identifier(obj) = &m.object {
            let obj_name = obj.name.as_str();
            let prop_name = m.property.name.as_str();
            return match (obj_name, prop_name) {
                ("Math", "random") => Some("Math.random"),
                ("Date", "now") => Some("Date.now"),
                _ => None,
            };
        }
    }
    None
}
