//! rules/no_regex_in_render.rs — Detect regex literals created inside component render bodies.
//!
//! # What this detects
//!
//! A regular expression literal (`/pattern/flags`) used inside a JSX prop
//! expression. Every render creates a new `RegExp` object, which breaks
//! memoized children and wastes memory.
//!
//! ```jsx
//! // ❌ New RegExp on every render
//! <Input pattern={/^\d{4}$/} />
//! <Filter test={/active|pending/i} />
//!
//! // ✅ Module-level constant — created once
//! const DIGIT_RE = /^\d{4}$/;
//! <Input pattern={DIGIT_RE} />
//! ```
//!
//! # Why it's a problem
//!
//! Regex literals are objects. A new object reference on every render means
//! React always sees a changed prop — child re-renders even if the pattern
//! hasn't changed. Module-level constants are created once and reused.
//!
//! # AST traversal strategy
//!
//! `visit_jsx_opening_element` → scan attribute expressions for `RegExpLiteral`.

use std::path::Path;

use oxc_ast::ast::{Expression, JSXAttributeItem, JSXAttributeValue, JSXOpeningElement};
use oxc_ast_visit::{walk, Visit};
use oxc_span::Span;

use crate::{
    rules::{Issue, RuleContext, Severity},
    utils::offset_to_line_col,
};

// ─── Rule struct ──────────────────────────────────────────────────────────────

pub struct NoRegexInRender;

impl super::Rule for NoRegexInRender {
    fn name(&self) -> &str {
        "no_regex_in_render"
    }

    fn run(&self, ctx: &RuleContext<'_>) -> Vec<Issue> {
        let mut visitor = RegexInRenderVisitor {
            issues: Vec::new(),
            source_text: ctx.source_text,
            file_path: ctx.file_path,
        };
        visitor.visit_program(ctx.program);
        visitor.issues
    }
}

// ─── Visitor ──────────────────────────────────────────────────────────────────

struct RegexInRenderVisitor<'a> {
    issues: Vec<Issue>,
    source_text: &'a str,
    file_path: &'a Path,
}

impl<'a> Visit<'a> for RegexInRenderVisitor<'_> {
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

impl RegexInRenderVisitor<'_> {
    fn scan_expr(&mut self, expr: &Expression<'_>) {
        match expr {
            Expression::RegExpLiteral(re) => {
                self.emit(re.span);
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

    fn emit(&mut self, span: Span) {
        let (line, col) = offset_to_line_col(self.source_text, span.start);
        self.issues.push(Issue {
            rule: "no_regex_in_render".to_string(),
            message: "Regex literal in a JSX prop creates a new RegExp object on every render. \
                 Move to a module-level constant: `const MY_RE = /pattern/;`"
                .to_string(),
            file: self.file_path.to_path_buf(),
            line,
            column: col,
            severity: Severity::Warning,
        });
    }
}
