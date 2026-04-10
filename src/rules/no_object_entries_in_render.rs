//! rules/no_object_entries_in_render.rs — Detect Object.keys/values/entries in JSX props.
//!
//! # What this detects
//!
//! Calls to `Object.keys()`, `Object.values()`, or `Object.entries()` used
//! directly as JSX prop values. These always return a new array reference,
//! causing unnecessary child re-renders.
//!
//! ```jsx
//! // ❌ New array on every render → child always re-renders
//! <Select options={Object.entries(countryMap)} />
//! <List items={Object.keys(config)} />
//! <Table rows={Object.values(dataMap)} />
//!
//! // ✅ Memoize
//! const options = useMemo(() => Object.entries(countryMap), [countryMap]);
//! <Select options={options} />
//! ```
//!
//! # Why it's a problem
//!
//! `Object.keys/values/entries` always returns a brand-new array. Even if the
//! object hasn't changed, the child receives a new array reference on every
//! render, so `React.memo` and `PureComponent` comparisons always fail.
//!
//! # AST traversal strategy
//!
//! `visit_jsx_opening_element` → scan each attribute expression for a
//! `CallExpression` with callee `Object.keys`, `Object.values`, or `Object.entries`.

use std::path::Path;

use oxc_ast::ast::{Expression, JSXAttributeItem, JSXAttributeValue, JSXOpeningElement};
use oxc_ast_visit::{walk, Visit};
use oxc_span::Span;

use crate::{
    rules::{Issue, RuleContext, Severity},
    utils::offset_to_line_col,
};

// ─── Rule struct ──────────────────────────────────────────────────────────────

pub struct NoObjectEntriesInRender;

impl super::Rule for NoObjectEntriesInRender {
    fn name(&self) -> &str {
        "no_object_entries_in_render"
    }

    fn run(&self, ctx: &RuleContext<'_>) -> Vec<Issue> {
        let mut visitor = ObjectEntriesVisitor {
            issues: Vec::new(),
            source_text: ctx.source_text,
            file_path: ctx.file_path,
        };
        visitor.visit_program(ctx.program);
        visitor.issues
    }
}

// ─── Visitor ──────────────────────────────────────────────────────────────────

struct ObjectEntriesVisitor<'a> {
    issues: Vec<Issue>,
    source_text: &'a str,
    file_path: &'a Path,
}

impl<'a> Visit<'a> for ObjectEntriesVisitor<'_> {
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

impl ObjectEntriesVisitor<'_> {
    fn scan_expr(&mut self, expr: &Expression<'_>) {
        match expr {
            Expression::CallExpression(call) => {
                if let Some(method) = object_static_method(&call.callee) {
                    self.emit(method, call.span);
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

    fn emit(&mut self, method: &str, span: Span) {
        let (line, col) = offset_to_line_col(self.source_text, span.start);
        self.issues.push(Issue {
            rule: "no_object_entries_in_render".to_string(),
            message: format!(
                "`{method}()` in a JSX prop always returns a new array reference, causing \
                 the child to re-render even when the object hasn't changed. \
                 Wrap with useMemo: `const items = useMemo(() => {method}(obj), [obj])`"
            ),
            file: self.file_path.to_path_buf(),
            line,
            column: col,
            severity: Severity::Warning,
        });
    }
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

/// Returns the full method name if callee is `Object.keys`, `.values`, or `.entries`.
fn object_static_method(callee: &Expression<'_>) -> Option<&'static str> {
    if let Expression::StaticMemberExpression(m) = callee {
        if let Expression::Identifier(obj) = &m.object {
            if obj.name.as_str() == "Object" {
                return match m.property.name.as_str() {
                    "keys" => Some("Object.keys"),
                    "values" => Some("Object.values"),
                    "entries" => Some("Object.entries"),
                    _ => None,
                };
            }
        }
    }
    None
}
