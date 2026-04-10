//! rules/no_useState_lazy_init_missing.rs — Detect expensive computations in useState initializer.
//!
//! # What this detects
//!
//! A call expression (function call) passed directly as the argument to
//! `useState()` when that argument is a known-expensive operation. React only
//! uses the initial value once (on mount), but the expression is evaluated on
//! every render.
//!
//! ```jsx
//! // ❌ JSON.parse runs on every render — result only used on first render
//! const [data, setData] = useState(JSON.parse(stored))
//!
//! // ❌ Expensive function call evaluated every render
//! const [list, setList] = useState(computeInitialList(props.items))
//! const [config, setConfig] = useState(buildConfig(rawConfig))
//!
//! // ✅ Lazy initializer — the function is only called on mount
//! const [data, setData] = useState(() => JSON.parse(stored))
//! const [list, setList] = useState(() => computeInitialList(props.items))
//! ```
//!
//! # Why it's a problem
//!
//! React evaluates the `useState(value)` argument on every render, but only
//! uses it during initial mount. Using the lazy initializer form
//! `useState(() => value)` defers evaluation to mount-time only — critical for
//! expensive computations like JSON parsing, large array operations, or DOM reads.
//!
//! # AST traversal strategy
//!
//! `visit_call_expression` → detect `useState(callExpr)` or `React.useState(callExpr)`
//! where the single argument is a `CallExpression` (not an arrow/function, which
//! would already be the lazy form). Emit when the argument is a call to a known-
//! expensive function or any function that receives arguments.

use std::path::Path;

use oxc_ast::ast::{CallExpression, Expression};
use oxc_ast_visit::{walk, Visit};
use oxc_span::Span;

use crate::{
    rules::{Issue, RuleContext, Severity},
    utils::offset_to_line_col,
};

// ─── Rule struct ──────────────────────────────────────────────────────────────

pub struct NoUseStateLazyInitMissing;

impl super::Rule for NoUseStateLazyInitMissing {
    fn name(&self) -> &str {
        "no_useState_lazy_init_missing"
    }

    fn run(&self, ctx: &RuleContext<'_>) -> Vec<Issue> {
        let mut visitor = LazyInitVisitor {
            issues: Vec::new(),
            source_text: ctx.source_text,
            file_path: ctx.file_path,
        };
        visitor.visit_program(ctx.program);
        visitor.issues
    }
}

// ─── Visitor ──────────────────────────────────────────────────────────────────

struct LazyInitVisitor<'a> {
    issues: Vec<Issue>,
    source_text: &'a str,
    file_path: &'a Path,
}

/// Always-expensive patterns regardless of arguments.
const EXPENSIVE_CALLS: &[&str] = &[
    "JSON.parse",
    "JSON.stringify",
    "structuredClone",
    "localStorage.getItem",
    "sessionStorage.getItem",
    "JSON",
];

impl<'a> Visit<'a> for LazyInitVisitor<'_> {
    fn visit_call_expression(&mut self, expr: &CallExpression<'a>) {
        if is_use_state(expr) {
            // useState takes exactly one argument.
            if let Some(arg) = expr.arguments.first() {
                if let Some(init_expr) = arg.as_expression() {
                    // Already using lazy form — arrow or function expression is fine.
                    let already_lazy = matches!(
                        init_expr,
                        Expression::ArrowFunctionExpression(_) | Expression::FunctionExpression(_)
                    );

                    if !already_lazy {
                        if let Expression::CallExpression(inner_call) = init_expr {
                            let fn_name = callee_name(&inner_call.callee);
                            // Flag if: known-expensive name OR has arguments (parameterized call)
                            let is_expensive =
                                EXPENSIVE_CALLS.iter().any(|&e| fn_name.starts_with(e))
                                    || !inner_call.arguments.is_empty();

                            if is_expensive {
                                self.emit(&fn_name, inner_call.span);
                            }
                        }
                    }
                }
            }
        }

        walk::walk_call_expression(self, expr);
    }
}

impl LazyInitVisitor<'_> {
    fn emit(&mut self, fn_name: &str, span: Span) {
        let (line, col) = offset_to_line_col(self.source_text, span.start);
        self.issues.push(Issue {
            rule: "no_useState_lazy_init_missing".to_string(),
            message: format!(
                "`useState({fn_name}(...))` evaluates `{fn_name}` on every render, \
                 but React only uses the initial value on mount. \
                 Use the lazy initializer form: `useState(() => {fn_name}(...))`"
            ),
            file: self.file_path.to_path_buf(),
            line,
            column: col,
            severity: Severity::Warning,
        });
    }
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn is_use_state(expr: &CallExpression<'_>) -> bool {
    match &expr.callee {
        Expression::Identifier(id) => id.name.as_str() == "useState",
        Expression::StaticMemberExpression(m) => m.property.name.as_str() == "useState",
        _ => false,
    }
}

fn callee_name(callee: &Expression<'_>) -> String {
    match callee {
        Expression::Identifier(id) => id.name.to_string(),
        Expression::StaticMemberExpression(m) => {
            format!("{}.{}", callee_name(&m.object), m.property.name)
        }
        _ => "fn".to_string(),
    }
}
