/// no_postmessage_wildcard — Detect postMessage calls with wildcard origin ("*").
///
/// # What this detects
///
/// `window.postMessage(data, "*")` sends the message to ANY receiving window
/// regardless of its origin. In micro-frontend architectures and large-scale
/// React apps, this can leak sensitive data to third-party iframes or parent
/// pages.
///
/// ```tsx
/// // BAD — sends to any origin
/// window.postMessage(sensitiveData, "*");
/// iframe.contentWindow.postMessage(authToken, "*");
///
/// // GOOD — restricted to known origin
/// window.postMessage(data, "https://app.example.com");
/// window.postMessage(data, window.location.origin);
/// ```
use std::path::Path;

use oxc_ast::ast::*;
use oxc_ast_visit::Visit;

use crate::rules::{Issue, IssueCategory, IssueSource, Rule, RuleContext, Severity};
use crate::utils::offset_to_line_col;

pub struct NoPostmessageWildcard;

impl Rule for NoPostmessageWildcard {
    fn name(&self) -> &str {
        "no_postmessage_wildcard"
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

impl<'a, 'b> Visit<'b> for Visitor<'a> {
    fn visit_call_expression(&mut self, expr: &CallExpression<'b>) {
        // Match any call where the method name is "postMessage"
        let is_post_message = matches!(
            &expr.callee,
            Expression::StaticMemberExpression(m)
                if m.property.name.as_str() == "postMessage"
        );

        if !is_post_message {
            return;
        }

        // postMessage(data, targetOrigin[, transfer])
        // The second argument is the targetOrigin.
        let Some(second_arg) = expr.arguments.get(1) else {
            return;
        };

        if let Some(Expression::StringLiteral(s)) = second_arg.as_expression() {
            if s.value.as_str() == "*" {
                let (line, col) = offset_to_line_col(self.source_text, expr.span.start);
                self.issues.push(Issue {
                    rule: "no_postmessage_wildcard".into(),
                    message: "postMessage() uses wildcard origin \"*\" — \
                              specify the target origin to prevent data leaks \
                              to untrusted windows"
                        .into(),
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
