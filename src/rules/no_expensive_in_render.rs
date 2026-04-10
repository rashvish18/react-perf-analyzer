//! rules/no_expensive_in_render.rs — Detect expensive array operations in JSX props.
//!
//! # What this detects
//!
//! Calls to expensive array methods (`.sort()`, `.filter()`, `.reduce()`,
//! `.find()`, `.findIndex()`, `.flatMap()`, `.reduceRight()`) used directly
//! as JSX attribute values without `useMemo` memoization:
//!
//! ```jsx
//! // ❌ .filter() directly in a prop — recomputed every render
//! <UserList users={allUsers.filter(u => u.active)} />
//!
//! // ❌ .sort() directly in a prop — mutates + recomputes every render
//! <Leaderboard scores={scores.sort((a, b) => b - a)} />
//!
//! // ❌ .reduce() in a prop
//! <Summary total={items.reduce((acc, i) => acc + i.price, 0)} />
//!
//! // ❌ .find() in a prop
//! <Editor doc={docs.find(d => d.id === activeId)} />
//!
//! // ❌ Chained — the outer .sort() is flagged
//! <List items={data.filter(Boolean).sort()} />
//!
//! // ❌ Inside a ternary branch
//! <List items={loaded ? items.filter(isActive) : []} />
//!
//! // ✅ Wrapped in useMemo — no warning
//! const active = useMemo(() => allUsers.filter(u => u.active), [allUsers]);
//! <UserList users={active} />
//!
//! // ✅ useMemo directly in prop — no warning
//! <UserList users={useMemo(() => allUsers.filter(u => u.active), [allUsers])} />
//! ```
//!
//! # Why it's a problem
//!
//! Array methods like `.sort()` and `.filter()` iterate over every element.
//! When called directly in a JSX prop, they run on *every* render of the parent
//! component — including renders triggered by unrelated state changes. For lists
//! with hundreds or thousands of items this causes measurable jank.
//!
//! Additionally, `.sort()` mutates the array in place, which can cause subtle
//! bugs when the same array reference is used elsewhere.
//!
//! # AST traversal strategy
//!
//! 1. `visit_jsx_opening_element` — called for every `<Tag ...>`.
//! 2. For each regular attribute, extract the expression value.
//! 3. Fast-path: skip if the top-level expression is `useMemo(...)` /
//!    `React.useMemo(...)`.
//! 4. `scan_for_expensive` recursively checks the expression:
//!    - `CallExpression` with an expensive method name → emit issue
//!    - `ConditionalExpression` → scan both branches
//!    - `LogicalExpression` → scan both operands
//!    - `ParenthesizedExpression` → unwrap and scan
//!    - Everything else → stop (no false positives from helper functions)

use std::path::Path;

use oxc_ast::ast::{Expression, JSXAttributeItem, JSXAttributeValue, JSXOpeningElement};
use oxc_ast_visit::{walk, Visit};
use oxc_span::Span;

use crate::{
    rules::{Issue, RuleContext, Severity},
    utils::offset_to_line_col,
};

// ─── Constants ────────────────────────────────────────────────────────────────

/// Array methods that are expensive to call on every render.
///
/// `map` is intentionally excluded here because it is commonly used for
/// rendering JSX lists (`items.map(item => <li key={item.id}>...`)`, which
/// is correct and expected. The other methods perform data computation that
/// should be memoized.
const EXPENSIVE_METHODS: &[&str] = &[
    "sort",
    "filter",
    "reduce",
    "reduceRight",
    "find",
    "findIndex",
    "flatMap",
];

// ─── Rule struct ──────────────────────────────────────────────────────────────

/// Zero-size rule struct — all per-file state lives in the visitor.
pub struct NoExpensiveInRender;

impl super::Rule for NoExpensiveInRender {
    fn name(&self) -> &str {
        "no_expensive_in_render"
    }

    fn run(&self, ctx: &RuleContext<'_>) -> Vec<Issue> {
        let mut visitor = ExpensiveInRenderVisitor {
            issues: Vec::new(),
            source_text: ctx.source_text,
            file_path: ctx.file_path,
        };
        visitor.visit_program(ctx.program);
        visitor.issues
    }
}

// ─── Visitor ──────────────────────────────────────────────────────────────────

/// Walks the AST and accumulates `no_expensive_in_render` issues.
struct ExpensiveInRenderVisitor<'a> {
    issues: Vec<Issue>,
    source_text: &'a str,
    file_path: &'a Path,
}

impl<'a> Visit<'a> for ExpensiveInRenderVisitor<'_> {
    fn visit_jsx_opening_element(&mut self, elem: &JSXOpeningElement<'a>) {
        for attr_item in &elem.attributes {
            if let JSXAttributeItem::Attribute(attr) = attr_item {
                let Some(JSXAttributeValue::ExpressionContainer(container)) = &attr.value else {
                    continue;
                };

                let Some(expr) = container.expression.as_expression() else {
                    continue;
                };

                // Fast-path: developer has already memoized — skip entirely.
                if is_memo_wrapped(expr) {
                    continue;
                }

                self.scan_for_expensive(expr);
            }
        }

        // Always walk children so nested JSX elements are also analyzed.
        walk::walk_jsx_opening_element(self, elem);
    }
}

impl ExpensiveInRenderVisitor<'_> {
    /// Recursively inspect `expr` for expensive array method calls.
    ///
    /// We recurse into conditional and logical expressions because a call in
    /// either branch still executes on every render:
    ///
    /// ```jsx
    /// // Still expensive — filter runs every render when `loaded` is true
    /// <List items={loaded ? items.filter(isActive) : []} />
    /// ```
    ///
    /// We do NOT recurse into:
    /// - The arguments of arbitrary function calls (avoids flagging helper
    ///   functions like `transform(items.filter(...))` — the helper may cache)
    /// - Object values or array elements
    fn scan_for_expensive(&mut self, expr: &Expression<'_>) {
        match expr {
            // Direct method call on something: `items.filter(...)`, `arr.sort()`
            Expression::CallExpression(call) => {
                if let Some((method_name, span)) = get_expensive_call(call) {
                    self.emit(method_name, span);
                }
                // Do NOT recurse into the call's arguments — too aggressive.
            }

            // Ternary: scan both result branches
            Expression::ConditionalExpression(cond) => {
                self.scan_for_expensive(&cond.consequent);
                self.scan_for_expensive(&cond.alternate);
            }

            // Logical (&&, ||, ??): scan both operands
            Expression::LogicalExpression(logical) => {
                self.scan_for_expensive(&logical.left);
                self.scan_for_expensive(&logical.right);
            }

            // Parenthesized: unwrap and scan
            Expression::ParenthesizedExpression(paren) => {
                self.scan_for_expensive(&paren.expression);
            }

            _ => {}
        }
    }

    fn emit(&mut self, method_name: &str, span: Span) {
        let (line, col) = offset_to_line_col(self.source_text, span.start);
        self.issues.push(Issue {
            rule: "no_expensive_in_render".to_string(),
            message: format!(
                "`.{method_name}()` called directly in render is recomputed on every re-render. \
                 Wrap with useMemo: `const result = useMemo(() => arr.{method_name}(...), [arr])`"
            ),
            file: self.file_path.to_path_buf(),
            line,
            column: col,
            severity: Severity::Warning,
        });
    }
}

// ─── Helper functions ─────────────────────────────────────────────────────────

/// If `call` is a member-expression call to an expensive array method, return
/// the method name and the span of the method identifier.
///
/// Returns `None` for non-member calls (e.g. `filter(items)`) or calls to
/// methods not in `EXPENSIVE_METHODS`.
fn get_expensive_call<'a>(call: &oxc_ast::ast::CallExpression<'a>) -> Option<(&'static str, Span)> {
    if let Expression::StaticMemberExpression(member) = &call.callee {
        let method_name = member.property.name.as_str();
        for &name in EXPENSIVE_METHODS {
            if method_name == name {
                return Some((name, member.property.span));
            }
        }
    }
    None
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
