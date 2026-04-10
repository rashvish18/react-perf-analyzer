/// no_xss_via_jsx_prop — Detect request data passed directly into JSX props.
///
/// # What this detects
///
/// In SSR environments (Next.js pages router, Remix loaders, Express + React),
/// `req.query`, `req.body`, and `req.params` are user-controlled. Passing these
/// directly into JSX props can cause reflected XSS when the component is
/// server-rendered and the HTML is sent to the browser without escaping.
///
/// ```tsx
/// // BAD — req.query is user-controlled, SSR reflects it into HTML
/// <div title={req.query.msg} />
/// <input placeholder={req.body.name} />
/// <meta content={req.params.id} />
///
/// // GOOD — sanitised or static
/// <div title={sanitize(req.query.msg)} />
/// <div title="static title" />
/// ```
use std::path::Path;

use oxc_ast::ast::*;
use oxc_ast_visit::Visit;
use oxc_span::GetSpan;

use crate::rules::{Issue, IssueCategory, IssueSource, Rule, RuleContext, Severity};
use crate::utils::offset_to_line_col;

pub struct NoXssViaJsxProp;

impl Rule for NoXssViaJsxProp {
    fn name(&self) -> &str {
        "no_xss_via_jsx_prop"
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

/// Root objects whose properties are treated as user-controlled in SSR.
const TAINTED_ROOTS: &[&str] = &["req", "request", "ctx"];

/// Sub-properties of tainted roots that carry user input.
const TAINTED_PROPS: &[&str] = &[
    "query",
    "body",
    "params",
    "headers",
    "cookies",
    "searchParams",
];

impl<'a, 'b> Visit<'b> for Visitor<'a> {
    fn visit_jsx_attribute(&mut self, attr: &JSXAttribute<'b>) {
        let Some(JSXAttributeValue::ExpressionContainer(container)) = &attr.value else {
            return;
        };
        let Some(expr) = container.expression.as_expression() else {
            return;
        };

        if let Some((root, prop)) = extract_member_root(expr) {
            if TAINTED_ROOTS.contains(&root) && TAINTED_PROPS.contains(&prop) {
                let prop_name = match &attr.name {
                    JSXAttributeName::Identifier(id) => id.name.as_str().to_string(),
                    JSXAttributeName::NamespacedName(nn) => nn.name.name.as_str().to_string(),
                };
                let (line, col) = offset_to_line_col(self.source_text, expr.span().start);
                self.issues.push(Issue {
                    rule: "no_xss_via_jsx_prop".into(),
                    message: format!(
                        "JSX prop `{prop_name}` receives `{root}.{prop}` directly — \
                         user-controlled data in SSR can cause reflected XSS"
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
    }
}

/// Extract (root_name, property_name) from `root.prop.anything` or `root.prop`.
fn extract_member_root<'b>(expr: &Expression<'b>) -> Option<(&'b str, &'b str)> {
    match expr {
        // root.prop  (two-level: req.query)
        Expression::StaticMemberExpression(mem) => {
            let prop_name = mem.property.name.as_str();
            match &mem.object {
                Expression::Identifier(id) => Some((id.name.as_str(), prop_name)),
                // root.prop.sub — return root + prop (ignore sub-property)
                Expression::StaticMemberExpression(inner) => {
                    let inner_prop = inner.property.name.as_str();
                    if let Expression::Identifier(root_id) = &inner.object {
                        Some((root_id.name.as_str(), inner_prop))
                    } else {
                        None
                    }
                }
                _ => None,
            }
        }
        _ => None,
    }
}
