/// no_dangerously_set_inner_html_unescaped — Detect unsafe sanitisers passed
/// to `dangerouslySetInnerHTML`.
///
/// # What this detects
///
/// oxc_linter already flags all `dangerouslySetInnerHTML` usage generically.
/// This rule is smarter: it allows well-known safe sanitisers (DOMPurify,
/// the `xss` package) but flags sanitisers that are commonly misused or
/// bypassed (`marked`, `showdown`, regex-based "sanitisers").
///
/// ```tsx
/// // OK — well-known safe sanitiser
/// <div dangerouslySetInnerHTML={{ __html: DOMPurify.sanitize(html) }} />
///
/// // BAD — marked() returns raw HTML with script tags intact
/// <div dangerouslySetInnerHTML={{ __html: marked(userInput) }} />
///
/// // BAD — regex replacement is bypassable (nested tags, encoding)
/// <div dangerouslySetInnerHTML={{ __html: html.replace(/<script>/g, '') }} />
///
/// // BAD — raw string variable, no sanitisation
/// <div dangerouslySetInnerHTML={{ __html: content }} />
/// ```
use std::path::Path;

use oxc_ast::ast::*;
use oxc_ast_visit::Visit;
use oxc_span::GetSpan;

use crate::rules::{Issue, IssueCategory, IssueSource, Rule, RuleContext, Severity};
use crate::utils::offset_to_line_col;

pub struct NoDangerouslySetInnerHtmlUnescaped;

impl Rule for NoDangerouslySetInnerHtmlUnescaped {
    fn name(&self) -> &str {
        "no_dangerously_set_inner_html_unescaped"
    }

    fn run(&self, ctx: &RuleContext<'_>) -> Vec<Issue> {
        let mut visitor = Visitor {
            source_text: ctx.source_text,
            file_path: ctx.file_path,
            issues: vec![],
        };
        visitor.visit_program(ctx.program);
        visitor.issues
    }
}

struct Visitor<'a> {
    source_text: &'a str,
    file_path: &'a Path,
    issues: Vec<Issue>,
}

/// Functions known to produce UNSAFE HTML (not real sanitisers).
const UNSAFE_SANITIZERS: &[&str] = &[
    "marked",
    "marked.parse",
    "showdown",
    "sanitizeHtml", // often called with default (unsafe) config
    "micromark",
    "snarkdown",
    "commonmark",
];

/// Functions known to produce SAFE HTML — allowlist.
const SAFE_SANITIZERS: &[&str] = &[
    "DOMPurify.sanitize",
    "sanitize", // generic name from the `xss` package when imported as sanitize
    "xss",
    "escapeHtml",
    "escape",
];

impl<'a, 'b> Visit<'b> for Visitor<'a> {
    fn visit_jsx_attribute(&mut self, attr: &JSXAttribute<'b>) {
        // Only interested in dangerouslySetInnerHTML
        let JSXAttributeName::Identifier(name) = &attr.name else {
            return;
        };
        if name.name.as_str() != "dangerouslySetInnerHTML" {
            return;
        }

        let Some(JSXAttributeValue::ExpressionContainer(container)) = &attr.value else {
            return;
        };
        let Some(expr) = container.expression.as_expression() else {
            return;
        };

        // The value must be an object { __html: <expr> }
        let Expression::ObjectExpression(obj) = expr else {
            return;
        };

        for prop in &obj.properties {
            let ObjectPropertyKind::ObjectProperty(kv) = prop else {
                continue;
            };

            // Look for the __html key
            let is_html_key = match &kv.key {
                PropertyKey::StaticIdentifier(id) => id.name.as_str() == "__html",
                PropertyKey::StringLiteral(s) => s.value.as_str() == "__html",
                _ => false,
            };
            if !is_html_key {
                continue;
            }

            self.check_html_value(&kv.value);
        }
    }
}

impl<'a> Visitor<'a> {
    fn check_html_value<'b>(&mut self, value: &Expression<'b>) {
        match value {
            // Static string literal → safe (no user input)
            Expression::StringLiteral(_) => {}

            // Call expression — check if it's a safe or unsafe sanitiser
            Expression::CallExpression(call) => {
                let callee_name = callee_as_string(&call.callee);

                // Safe sanitisers — don't flag
                if SAFE_SANITIZERS.iter().any(|s| callee_name.contains(s)) {
                    return;
                }

                // Unsafe sanitisers — flag with specific message
                if let Some(unsafe_name) =
                    UNSAFE_SANITIZERS.iter().find(|s| callee_name.contains(*s))
                {
                    let (line, col) = offset_to_line_col(self.source_text, call.span.start);
                    self.issues.push(Issue {
                        rule: "no_dangerously_set_inner_html_unescaped".into(),
                        message: format!(
                            "`{unsafe_name}()` does not sanitise HTML — \
                             use DOMPurify.sanitize() to prevent XSS"
                        ),
                        file: self.file_path.to_path_buf(),
                        line,
                        column: col,
                        severity: Severity::Critical,
                        source: IssueSource::ReactPerfAnalyzer,
                        category: IssueCategory::Security,
                    });
                } else {
                    // Unknown call — flag as medium, may or may not be safe
                    let (line, col) = offset_to_line_col(self.source_text, call.span.start);
                    self.issues.push(Issue {
                        rule: "no_dangerously_set_inner_html_unescaped".into(),
                        message: format!(
                            "`__html: {callee_name}(...)` — verify this function \
                             sanitises HTML with DOMPurify to prevent XSS"
                        ),
                        file: self.file_path.to_path_buf(),
                        line,
                        column: col,
                        severity: Severity::High,
                        source: IssueSource::ReactPerfAnalyzer,
                        category: IssueCategory::Security,
                    });
                }
            }

            // Any other expression (variable, member, template) — flag as raw HTML
            _ => {
                let (line, col) = offset_to_line_col(self.source_text, value.span().start);
                self.issues.push(Issue {
                    rule: "no_dangerously_set_inner_html_unescaped".into(),
                    message: "Raw expression in `dangerouslySetInnerHTML.__html` — \
                              sanitise with DOMPurify.sanitize() to prevent XSS"
                        .into(),
                    file: self.file_path.to_path_buf(),
                    line,
                    column: col,
                    severity: Severity::Critical,
                    source: IssueSource::ReactPerfAnalyzer,
                    category: IssueCategory::Security,
                });
            }
        }
    }
}

/// Convert a callee expression to a dotted string (e.g. "DOMPurify.sanitize").
fn callee_as_string(callee: &Expression<'_>) -> String {
    match callee {
        Expression::Identifier(id) => id.name.as_str().to_string(),
        Expression::StaticMemberExpression(mem) => {
            let obj = callee_as_string(&mem.object);
            let prop = mem.property.name.as_str();
            format!("{obj}.{prop}")
        }
        _ => String::new(),
    }
}
