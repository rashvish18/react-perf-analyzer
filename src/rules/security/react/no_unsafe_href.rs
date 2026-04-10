/// no_unsafe_href — Detect unsafe href/to/src JSX props that could enable XSS.
///
/// # What this detects
///
/// The `javascript:` URL protocol in `href`, `to`, or `src` props causes the
/// browser to execute arbitrary JS when the user clicks the link. Even without
/// an explicit `javascript:` prefix, passing user-controlled data into `href`
/// without sanitisation is an open redirect / XSS risk, especially in SSR
/// environments like Next.js or Remix.
///
/// ```tsx
/// // BAD — href from user input, open redirect + potential XSS
/// <a href={props.url}>click</a>
/// <a href={`javascript:${handler}`}>click</a>
/// <Link to={router.query.returnUrl}>back</Link>
///
/// // GOOD — static string literal, no risk
/// <a href="/dashboard">dashboard</a>
/// <a href="https://example.com">external</a>
/// ```
use std::path::Path;

use oxc_ast::ast::*;
use oxc_ast_visit::Visit;
use oxc_span::GetSpan;

use crate::rules::{Issue, IssueCategory, IssueSource, Rule, RuleContext, Severity};
use crate::utils::offset_to_line_col;

pub struct NoUnsafeHref;

impl Rule for NoUnsafeHref {
    fn name(&self) -> &str {
        "no_unsafe_href"
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

/// JSX prop names that carry URL values and are dangerous when user-controlled.
const DANGEROUS_PROPS: &[&str] = &["href", "to", "src", "action", "formAction"];

impl<'a, 'b> Visit<'b> for Visitor<'a> {
    fn visit_jsx_attribute(&mut self, attr: &JSXAttribute<'b>) {
        let name = match &attr.name {
            JSXAttributeName::Identifier(id) => id.name.as_str(),
            JSXAttributeName::NamespacedName(nn) => nn.name.name.as_str(),
        };

        if !DANGEROUS_PROPS.contains(&name) {
            return;
        }

        let Some(JSXAttributeValue::ExpressionContainer(container)) = &attr.value else {
            return;
        };

        let Some(expr) = container.expression.as_expression() else {
            return;
        };

        match expr {
            // Static string literals — only flag if it starts with javascript:
            Expression::StringLiteral(s) => {
                let val = s.value.as_str().trim().to_lowercase();
                if val.starts_with("javascript:") {
                    let (line, col) = offset_to_line_col(self.source_text, s.span.start);
                    self.issues.push(Issue {
                        rule: "no_unsafe_href".into(),
                        message: format!(
                            "`{name}` prop contains `javascript:` URL — this enables XSS"
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
            // Template literals containing javascript: prefix
            Expression::TemplateLiteral(tpl) => {
                let raw_start = tpl
                    .quasis
                    .first()
                    .map(|q| q.value.raw.as_str().trim().to_lowercase())
                    .unwrap_or_default();
                if raw_start.starts_with("javascript:") {
                    let (line, col) = offset_to_line_col(self.source_text, tpl.span.start);
                    self.issues.push(Issue {
                        rule: "no_unsafe_href".into(),
                        message: format!(
                            "`{name}` template literal starts with `javascript:` — XSS risk"
                        ),
                        file: self.file_path.to_path_buf(),
                        line,
                        column: col,
                        severity: Severity::Critical,
                        source: IssueSource::ReactPerfAnalyzer,
                        category: IssueCategory::Security,
                    });
                }
            }
            // Non-literal expressions in href/to — flag as potential open redirect
            Expression::Identifier(_)
            | Expression::StaticMemberExpression(_)
            | Expression::CallExpression(_) => {
                let (line, col) = offset_to_line_col(self.source_text, expr.span().start);
                self.issues.push(Issue {
                    rule: "no_unsafe_href".into(),
                    message: format!(
                        "`{name}` prop is set from a dynamic expression — \
                         validate the URL to prevent open redirect or XSS"
                    ),
                    file: self.file_path.to_path_buf(),
                    line,
                    column: col,
                    severity: Severity::Medium,
                    source: IssueSource::ReactPerfAnalyzer,
                    category: IssueCategory::Security,
                });
            }
            _ => {}
        }
    }
}
