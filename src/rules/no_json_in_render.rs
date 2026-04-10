//! rules/no_json_in_render.rs — Detect JSON.parse / JSON.stringify in JSX props.
//!
//! # What this detects
//!
//! Calls to `JSON.parse()` or `JSON.stringify()` used directly as JSX prop
//! values. JSON operations are O(n) and run synchronously on the main thread
//! on every render.
//!
//! ```jsx
//! // ❌ JSON.parse on every render
//! <DataGrid config={JSON.parse(configString)} />
//!
//! // ❌ JSON.stringify in a prop
//! <Debug value={JSON.stringify(state)} />
//!
//! // ✅ Memoize
//! const config = useMemo(() => JSON.parse(configString), [configString]);
//! <DataGrid config={config} />
//! ```
//!
//! # AST traversal strategy
//!
//! `visit_jsx_opening_element` → scan each attribute expression for a
//! `CallExpression` whose callee is `JSON.parse` or `JSON.stringify`.

use std::path::Path;

use oxc_ast::ast::{Expression, JSXAttributeItem, JSXAttributeValue, JSXOpeningElement};
use oxc_ast_visit::{walk, Visit};
use oxc_span::Span;

use crate::{
    rules::{Issue, RuleContext, Severity},
    utils::offset_to_line_col,
};

// ─── Rule struct ──────────────────────────────────────────────────────────────

pub struct NoJsonInRender;

impl super::Rule for NoJsonInRender {
    fn name(&self) -> &str {
        "no_json_in_render"
    }

    fn run(&self, ctx: &RuleContext<'_>) -> Vec<Issue> {
        let mut visitor = JsonInRenderVisitor {
            issues: Vec::new(),
            source_text: ctx.source_text,
            file_path: ctx.file_path,
        };
        visitor.visit_program(ctx.program);
        visitor.issues
    }
}

// ─── Visitor ──────────────────────────────────────────────────────────────────

struct JsonInRenderVisitor<'a> {
    issues: Vec<Issue>,
    source_text: &'a str,
    file_path: &'a Path,
}

impl<'a> Visit<'a> for JsonInRenderVisitor<'_> {
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

impl JsonInRenderVisitor<'_> {
    fn scan_expr(&mut self, expr: &Expression<'_>) {
        match expr {
            Expression::CallExpression(call) => {
                if let Some(method) = json_method(&call.callee) {
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
            rule: "no_json_in_render".to_string(),
            message: format!(
                "`{method}` in a JSX prop runs on every render — JSON operations are O(n) and \
                 block the main thread. Wrap with useMemo: \
                 `const value = useMemo(() => {method}(...), [deps])`"
            ),
            file: self.file_path.to_path_buf(),
            line,
            column: col,
            severity: Severity::Warning,
        });
    }
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

/// Returns `"JSON.parse"` or `"JSON.stringify"` if the callee matches.
fn json_method(callee: &Expression<'_>) -> Option<&'static str> {
    if let Expression::StaticMemberExpression(m) = callee {
        if let Expression::Identifier(obj) = &m.object {
            if obj.name.as_str() == "JSON" {
                return match m.property.name.as_str() {
                    "parse" => Some("JSON.parse"),
                    "stringify" => Some("JSON.stringify"),
                    _ => None,
                };
            }
        }
    }
    None
}
