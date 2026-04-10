//! rules/no_new_in_jsx_prop.rs — Detect `new` expressions used directly as JSX prop values.
//!
//! # What this detects
//!
//! A `new` expression passed directly as a JSX attribute value, creating a new
//! object instance on every render. Unlike OxLint's `jsx-no-new-object-as-prop`
//! (which catches `{}` literals), this rule catches constructor calls.
//!
//! ```jsx
//! // ❌ New Date instance on every render
//! <Chart date={new Date()} />
//!
//! // ❌ New Map on every render
//! <DataTable config={new Map(entries)} />
//!
//! // ❌ New class instance on every render
//! <Grid theme={new StyleSheet({ color: 'red' })} />
//!
//! // ✅ Hoist outside component or memoize
//! const today = useMemo(() => new Date(), []);
//! <Chart date={today} />
//! ```
//!
//! # Why it's a problem
//!
//! Every `new Foo()` call creates a fresh object reference. React compares
//! props with `Object.is`, so a new reference on every render always triggers
//! a child re-render, even if the constructed value is semantically identical.
//!
//! # AST traversal strategy
//!
//! `visit_jsx_opening_element` → for each JSX attribute whose value is an
//! expression container, check if the expression is a `NewExpression`.

use std::path::Path;

use oxc_ast::ast::{Expression, JSXAttributeItem, JSXAttributeValue, JSXOpeningElement};
use oxc_ast_visit::{walk, Visit};
use oxc_span::Span;

use crate::{
    rules::{Issue, RuleContext, Severity},
    utils::offset_to_line_col,
};

// ─── Rule struct ──────────────────────────────────────────────────────────────

pub struct NoNewInJsxProp;

impl super::Rule for NoNewInJsxProp {
    fn name(&self) -> &str {
        "no_new_in_jsx_prop"
    }

    fn run(&self, ctx: &RuleContext<'_>) -> Vec<Issue> {
        let mut visitor = NewInPropVisitor {
            issues: Vec::new(),
            source_text: ctx.source_text,
            file_path: ctx.file_path,
        };
        visitor.visit_program(ctx.program);
        visitor.issues
    }
}

// ─── Visitor ──────────────────────────────────────────────────────────────────

struct NewInPropVisitor<'a> {
    issues: Vec<Issue>,
    source_text: &'a str,
    file_path: &'a Path,
}

impl<'a> Visit<'a> for NewInPropVisitor<'_> {
    fn visit_jsx_opening_element(&mut self, elem: &JSXOpeningElement<'a>) {
        for attr_item in &elem.attributes {
            if let JSXAttributeItem::Attribute(attr) = attr_item {
                if let Some(JSXAttributeValue::ExpressionContainer(container)) = &attr.value {
                    if let Some(expr) = container.expression.as_expression() {
                        self.check_expr(expr);
                    }
                }
            }
        }
        walk::walk_jsx_opening_element(self, elem);
    }
}

impl NewInPropVisitor<'_> {
    fn check_expr(&mut self, expr: &Expression<'_>) {
        match expr {
            Expression::NewExpression(new_expr) => {
                let constructor = constructor_name(&new_expr.callee);
                self.emit(&constructor, new_expr.span);
            }
            // Follow ternary / logical / paren branches
            Expression::ConditionalExpression(cond) => {
                self.check_expr(&cond.consequent);
                self.check_expr(&cond.alternate);
            }
            Expression::LogicalExpression(logical) => {
                self.check_expr(&logical.left);
                self.check_expr(&logical.right);
            }
            Expression::ParenthesizedExpression(paren) => {
                self.check_expr(&paren.expression);
            }
            _ => {}
        }
    }

    fn emit(&mut self, constructor: &str, span: Span) {
        let (line, col) = offset_to_line_col(self.source_text, span.start);
        self.issues.push(Issue {
            rule: "no_new_in_jsx_prop".to_string(),
            message: format!(
                "`new {constructor}()` in a JSX prop creates a new instance on every render. \
                 Hoist outside the component or wrap with useMemo: \
                 `const value = useMemo(() => new {constructor}(...), [deps])`"
            ),
            file: self.file_path.to_path_buf(),
            line,
            column: col,
            severity: Severity::Warning,
        });
    }
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn constructor_name(callee: &Expression<'_>) -> String {
    match callee {
        Expression::Identifier(id) => id.name.to_string(),
        Expression::StaticMemberExpression(m) => {
            format!("{}.{}", constructor_name(&m.object), m.property.name)
        }
        _ => "Object".to_string(),
    }
}
