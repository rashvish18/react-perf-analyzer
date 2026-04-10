//! rules/no_new_context_value.rs — Detect unstable values in Context.Provider.
//!
//! # What this detects
//!
//! Object literals, array literals, or inline functions passed as the `value`
//! prop to a React Context Provider component:
//!
//! ```jsx
//! // ❌ Object literal — new object every render → ALL consumers re-render
//! <ThemeContext.Provider value={{ theme, toggleTheme }} />
//!
//! // ❌ Array literal — same problem
//! <UserContext.Provider value={[user, setUser]} />
//!
//! // ❌ Inline function — new function reference every render
//! <CallbackContext.Provider value={() => doSomething()} />
//!
//! // ❌ Object in a ternary branch
//! <AuthContext.Provider value={isAdmin ? { role: "admin" } : defaults} />
//!
//! // ✅ Stable reference — no warning
//! const contextValue = useMemo(() => ({ theme, toggleTheme }), [theme]);
//! <ThemeContext.Provider value={contextValue} />
//!
//! // ✅ useMemo-wrapped directly — no warning
//! <ThemeContext.Provider value={useMemo(() => ({ theme }), [theme])} />
//! ```
//!
//! # Why it's a problem
//!
//! Every render of the Provider's parent creates a new object/array/function
//! reference. React uses referential equality (`Object.is`) to decide whether
//! context consumers need to re-render. A new reference on every render means
//! EVERY context consumer re-renders on EVERY parent render — even if the
//! actual data hasn't changed. This silently defeats all memoization in the
//! consumer tree.
//!
//! # AST traversal strategy
//!
//! 1. `visit_jsx_opening_element` — called for every `<Tag ...>`.
//! 2. Check if the element name is `X.Provider` (JSXMemberExpression where
//!    property is `"Provider"`).
//! 3. Find the `value` attribute.
//! 4. Fast-path: skip if wrapped in `useMemo` / `React.useMemo`.
//! 5. `scan_value` recursively checks for object/array/function literals,
//!    following the same conditional/logical/paren patterns as `unstable_props`.

use std::path::Path;

use oxc_ast::ast::{
    Expression, JSXAttributeItem, JSXAttributeName, JSXAttributeValue, JSXElementName,
    JSXOpeningElement,
};
use oxc_ast_visit::{walk, Visit};
use oxc_span::Span;

use crate::{
    rules::{Issue, RuleContext, Severity},
    utils::offset_to_line_col,
};

// ─── Rule struct ──────────────────────────────────────────────────────────────

/// Zero-size rule struct — all per-file state lives in the visitor.
pub struct NoNewContextValue;

impl super::Rule for NoNewContextValue {
    fn name(&self) -> &str {
        "no_new_context_value"
    }

    fn run(&self, ctx: &RuleContext<'_>) -> Vec<Issue> {
        let mut visitor = ContextValueVisitor {
            issues: Vec::new(),
            source_text: ctx.source_text,
            file_path: ctx.file_path,
        };
        visitor.visit_program(ctx.program);
        visitor.issues
    }
}

// ─── Visitor ──────────────────────────────────────────────────────────────────

/// Walks the AST and accumulates `no_new_context_value` issues.
struct ContextValueVisitor<'a> {
    issues: Vec<Issue>,
    source_text: &'a str,
    file_path: &'a Path,
}

impl<'a> Visit<'a> for ContextValueVisitor<'_> {
    /// Check every JSX opening element for the `X.Provider value={...}` pattern.
    fn visit_jsx_opening_element(&mut self, elem: &JSXOpeningElement<'a>) {
        if is_context_provider(&elem.name) {
            for attr_item in &elem.attributes {
                if let JSXAttributeItem::Attribute(attr) = attr_item {
                    // Only care about the `value` prop.
                    if extract_prop_name(&attr.name) != "value" {
                        continue;
                    }

                    let Some(JSXAttributeValue::ExpressionContainer(container)) = &attr.value
                    else {
                        continue;
                    };

                    let Some(expr) = container.expression.as_expression() else {
                        continue;
                    };

                    // If already wrapped in useMemo, the developer has handled it.
                    if is_memo_wrapped(expr) {
                        continue;
                    }

                    self.scan_value(expr);
                }
            }
        }

        // Always walk children so nested Providers are also checked.
        walk::walk_jsx_opening_element(self, elem);
    }
}

impl ContextValueVisitor<'_> {
    /// Recursively scan `expr` for unstable value types.
    ///
    /// Follows ternary / logical / paren branches the same way `unstable_props`
    /// does, because a literal in either branch of a ternary is still unstable.
    fn scan_value(&mut self, expr: &Expression<'_>) {
        match expr {
            // Object literal: `value={{ theme, toggle }}`
            Expression::ObjectExpression(obj) => {
                self.emit(
                    "object literal",
                    "Extract to a stable variable or wrap with useMemo: \
                           `const value = useMemo(() => ({ ... }), [deps])`",
                    obj.span,
                );
            }

            // Array literal: `value={[user, setUser]}`
            Expression::ArrayExpression(arr) => {
                self.emit(
                    "array literal",
                    "Extract to a stable variable or wrap with useMemo: \
                           `const value = useMemo(() => [...], [deps])`",
                    arr.span,
                );
            }

            // Arrow function: `value={() => handleLogin()}`
            Expression::ArrowFunctionExpression(arrow) => {
                self.emit(
                    "arrow function",
                    "Wrap with useCallback: \
                           `const value = useCallback(() => { ... }, [deps])`",
                    arrow.span,
                );
            }

            // Function expression: `value={function() { ... }}`
            Expression::FunctionExpression(func) => {
                self.emit(
                    "function expression",
                    "Wrap with useCallback: \
                           `const value = useCallback(function() { ... }, [deps])`",
                    func.span,
                );
            }

            // Ternary: scan both branches
            Expression::ConditionalExpression(cond) => {
                self.scan_value(&cond.consequent);
                self.scan_value(&cond.alternate);
            }

            // Logical (&&, ||, ??): scan both operands
            Expression::LogicalExpression(logical) => {
                self.scan_value(&logical.left);
                self.scan_value(&logical.right);
            }

            // Parenthesized: unwrap and scan
            Expression::ParenthesizedExpression(paren) => {
                self.scan_value(&paren.expression);
            }

            _ => {}
        }
    }

    /// Emit a `no_new_context_value` issue with a context-specific suggestion.
    fn emit(&mut self, kind: &str, suggestion: &str, span: Span) {
        let (line, col) = offset_to_line_col(self.source_text, span.start);
        self.issues.push(Issue {
            rule: "no_new_context_value".to_string(),
            message: format!(
                "Context Provider 'value' receives a new {kind} on every render — \
                 all consumers will re-render. {suggestion}"
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

// ─── Helper functions ─────────────────────────────────────────────────────────

/// Returns `true` if the JSX element name ends with `.Provider`.
///
/// Matches `ThemeContext.Provider`, `AuthContext.Provider`, etc.
/// Ignores plain identifiers (`<Provider>`) to reduce false positives,
/// since React context providers are always accessed as `SomeContext.Provider`.
fn is_context_provider(name: &JSXElementName<'_>) -> bool {
    match name {
        JSXElementName::MemberExpression(member) => member.property.name.as_str() == "Provider",
        _ => false,
    }
}

/// Extract the human-readable JSX attribute name.
fn extract_prop_name(name: &JSXAttributeName<'_>) -> String {
    match name {
        JSXAttributeName::Identifier(id) => id.name.to_string(),
        JSXAttributeName::NamespacedName(ns) => {
            format!("{}:{}", ns.namespace.name, ns.name.name)
        }
    }
}

/// Returns `true` if `expr` is a direct call to `useMemo` or `React.useMemo`.
fn is_memo_wrapped(expr: &Expression<'_>) -> bool {
    if let Expression::CallExpression(call) = expr {
        match &call.callee {
            Expression::Identifier(id) => return id.name.as_str() == "useMemo",
            Expression::StaticMemberExpression(member) => {
                return member.property.name.as_str() == "useMemo";
            }
            _ => {}
        }
    }
    false
}
