/// rules/no_inline_jsx_fn.rs — Detect inline functions in JSX props.
///
/// # What this detects
///
/// Arrow functions or regular function expressions passed directly as JSX
/// attribute values — including cases wrapped in conditionals or logical
/// expressions:
///
/// ```jsx
/// // ❌ Direct inline arrow function
/// <Button onClick={() => handleClick()} />
///
/// // ❌ Direct inline function expression
/// <Input onChange={function(e) { setValue(e.target.value) }} />
///
/// // ❌ Inline function inside a ternary — still creates a new fn each render
/// <Button onClick={isDisabled ? () => {} : handleClick} />
///
/// // ❌ Inline function inside a logical expression
/// <Tooltip onShow={debug && (() => log("shown"))} />
///
/// // ✅ Stable reference — no warning
/// <Button onClick={handleClick} />
///
/// // ✅ Properly memoized with useCallback — no warning
/// <Button onClick={useCallback(() => doThing(id), [id])} />
///
/// // ✅ Also recognized: React.useCallback, useMemo
/// <Button onClick={React.useCallback(() => doThing(), [])} />
/// ```
///
/// # Why it's a problem
///
/// Every render of the parent component creates a *new* function object.
/// Child components wrapped in `React.memo` or implementing
/// `shouldComponentUpdate` always see a "changed" prop and re-render
/// unnecessarily, defeating the purpose of memoization.
///
/// # AST traversal strategy
///
/// 1. `visit_jsx_opening_element` — OXC calls this for every `<Tag ...>`.
/// 2. For each regular attribute (not a spread), extract the prop name and
///    expression value.
/// 3. Fast-path: if the expression is a `CallExpression` to `useCallback`
///    or `useMemo`, skip it — the function is intentionally memoized.
/// 4. Recursively scan the expression with `scan_for_inline_fn`:
///    - `ArrowFunctionExpression` / `FunctionExpression` → emit issue
///    - `ConditionalExpression` (ternary) → scan both branches
///    - `LogicalExpression` (`&&` / `||` / `??`) → scan both operands
///    - `ParenthesizedExpression` → unwrap and scan
///    - Everything else → no recursion (avoids false positives inside
///      call arguments, object literals, etc.)
///
/// # OXC API notes (v0.67)
///
/// - `Visit` trait + `walk` module: `oxc_ast_visit` crate (not `oxc_ast`)
/// - `JSXExpression::as_expression()` → `Option<&Expression<'a>>` (returns
///   `None` for the empty `attr={}` case)
/// - `Statement` and `JSXAttributeName` are flat inherited enums
use std::path::Path;

use oxc_ast::ast::{
    ConditionalExpression, Expression, JSXAttributeItem, JSXAttributeName, JSXAttributeValue,
    JSXOpeningElement, LogicalExpression, ParenthesizedExpression,
};
use oxc_ast_visit::{walk, Visit};
use oxc_span::Span;

use crate::{
    rules::{Issue, RuleContext, Severity},
    utils::offset_to_line_col,
};

// ─── Rule struct ──────────────────────────────────────────────────────────────

/// Zero-size marker struct — all per-file state lives in `InlineFnVisitor`.
pub struct NoInlineJsxFn;

impl super::Rule for NoInlineJsxFn {
    fn name(&self) -> &str {
        "no_inline_jsx_fn"
    }

    fn run(&self, ctx: &RuleContext<'_>) -> Vec<Issue> {
        let mut visitor = InlineFnVisitor {
            issues: Vec::new(),
            source_text: ctx.source_text,
            file_path: ctx.file_path,
        };
        // Walk the entire AST from the program root.
        // OXC's Visit trait handles recursive descent; we only override
        // the nodes we care about.
        visitor.visit_program(ctx.program);
        visitor.issues
    }
}

// ─── Visitor ──────────────────────────────────────────────────────────────────

/// Walks the AST and accumulates `no_inline_jsx_fn` issues.
struct InlineFnVisitor<'a> {
    issues: Vec<Issue>,
    source_text: &'a str,
    file_path: &'a Path,
}

impl<'a> Visit<'a> for InlineFnVisitor<'_> {
    /// Entry point for every JSX opening element: `<Tag attr1={...} attr2=... />`.
    ///
    /// We override at the *element* level (not attribute level) so we can
    /// access both the attribute name and its value together — the name is
    /// included in the warning message for better developer ergonomics.
    fn visit_jsx_opening_element(&mut self, elem: &JSXOpeningElement<'a>) {
        for attr_item in &elem.attributes {
            match attr_item {
                // Regular attribute: `onClick={...}` or `disabled`
                JSXAttributeItem::Attribute(attr) => {
                    // Attributes without a value are boolean flags (`<Comp disabled />`).
                    // They can never hold a function, so skip them.
                    let Some(JSXAttributeValue::ExpressionContainer(container)) = &attr.value
                    else {
                        continue;
                    };

                    // `as_expression()` returns None only for the empty `attr={}`
                    // case (JSXEmptyExpression) — filter that out.
                    let Some(expr) = container.expression.as_expression() else {
                        continue;
                    };

                    // Extract the human-readable prop name for the warning message.
                    // Most props are plain identifiers (`onClick`, `onChange`), but
                    // namespaced props like `xlink:href` also exist in SVG/MathML.
                    let prop_name = extract_prop_name(&attr.name);

                    // Fast-path: if the expression is `useCallback(...)` or
                    // `useMemo(...)` / `React.useCallback(...)`, the developer
                    // has already memoized the function — no issue to report.
                    if is_memoized(expr) {
                        continue;
                    }

                    // Recursively scan the expression for inline functions.
                    // This handles direct functions AND functions nested inside
                    // ternaries, logical expressions, and parentheses.
                    self.scan_for_inline_fn(expr, &prop_name);
                }

                // Spread attribute: `{...props}` — skip, not a named prop.
                JSXAttributeItem::SpreadAttribute(_) => {}
            }
        }

        // IMPORTANT: always walk into child elements so nested JSX like
        // `<Parent><Child onClick={...} /></Parent>` is also analyzed.
        walk::walk_jsx_opening_element(self, elem);
    }
}

impl InlineFnVisitor<'_> {
    /// Recursively inspect `expr` for inline function expressions.
    ///
    /// We recurse into conditional and logical expressions because a function
    /// in either branch still creates a new reference on every render:
    ///
    /// ```jsx
    /// // Still a problem — new fn created for the `true` branch every render
    /// <Button onClick={disabled ? () => {} : handleClick} />
    /// ```
    ///
    /// We do NOT recurse into:
    /// - Call expression arguments (e.g. `doSomething(() => {})` as a prop
    ///   value is ambiguous — the callback might be stable)
    /// - Object/array expressions (handled by `unstable_props` rule)
    fn scan_for_inline_fn(&mut self, expr: &Expression<'_>, prop_name: &str) {
        match expr {
            // ── Direct inline arrow function ──────────────────────────────
            // `onClick={() => doSomething()}`
            // `onSubmit={async (e) => { await submit(e); }}`
            Expression::ArrowFunctionExpression(arrow) => {
                self.emit(
                    FnKind::Arrow,
                    prop_name,
                    arrow.span,
                    arrow.params.items.len(),
                );
            }

            // ── Direct inline function expression ─────────────────────────
            // `onChange={function(e) { setValue(e.target.value); }}`
            Expression::FunctionExpression(func) => {
                self.emit(
                    FnKind::Regular {
                        is_async: func.r#async,
                        is_generator: func.generator,
                    },
                    prop_name,
                    func.span,
                    func.params.items.len(),
                );
            }

            // ── Ternary: scan both branches ───────────────────────────────
            // `onClick={isDisabled ? () => {} : handleClick}`
            //                         ^^^^^^^^^  ← warn if this is an inline fn
            Expression::ConditionalExpression(cond) => {
                self.scan_conditional(cond, prop_name);
            }

            // ── Logical (&&, ||, ??): scan both operands ──────────────────
            // `onShow={debug && (() => logger.log('shown'))}`
            Expression::LogicalExpression(logical) => {
                self.scan_logical(logical, prop_name);
            }

            // ── Parenthesized: unwrap and scan ────────────────────────────
            // `onClick={(() => fn())}`
            Expression::ParenthesizedExpression(paren) => {
                self.scan_parenthesized(paren, prop_name);
            }

            // All other expression types (identifiers, call expressions,
            // member expressions, etc.) are not inline function definitions
            // and do not need recursive scanning for this rule.
            _ => {}
        }
    }

    // ── Conditional expression (`? :`) ────────────────────────────────────────

    /// Scan both branches of a ternary for inline functions.
    ///
    /// The test expression (`condition` in `condition ? a : b`) is skipped —
    /// a function there would be called immediately, not stored as a prop.
    fn scan_conditional(&mut self, cond: &ConditionalExpression<'_>, prop_name: &str) {
        // `onClick={flag ? () => handleTrue() : handleFalse}`
        //                  ^^^^^^^^^^^^^^^^^^  ← scan this
        self.scan_for_inline_fn(&cond.consequent, prop_name);
        // `onClick={flag ? handleTrue : () => handleFalse()}`
        //                               ^^^^^^^^^^^^^^^^^^^  ← and this
        self.scan_for_inline_fn(&cond.alternate, prop_name);
    }

    // ── Logical expression (`&&`, `||`, `??`) ────────────────────────────────

    /// Scan both sides of a logical expression for inline functions.
    ///
    /// ```jsx
    /// // Both sides can contain inline functions:
    /// <Comp onShow={debug && (() => log("show"))} />
    /// <Comp onShow={fallback || (() => defaultHandler())} />
    /// ```
    fn scan_logical(&mut self, logical: &LogicalExpression<'_>, prop_name: &str) {
        self.scan_for_inline_fn(&logical.left, prop_name);
        self.scan_for_inline_fn(&logical.right, prop_name);
    }

    // ── Parenthesized expression ──────────────────────────────────────────────

    /// Unwrap parentheses and scan the inner expression.
    ///
    /// ```jsx
    /// <Button onClick={(() => doThing())} />
    ///                   ^^^^^^^^^^^^^^^^  ← inner inline fn
    /// ```
    fn scan_parenthesized(&mut self, paren: &ParenthesizedExpression<'_>, prop_name: &str) {
        self.scan_for_inline_fn(&paren.expression, prop_name);
    }

    // ── Issue emission ────────────────────────────────────────────────────────

    /// Build and push an `Issue` for a detected inline function.
    ///
    /// # Arguments
    /// * `kind`       — Whether this is an arrow fn or a regular function expression.
    /// * `prop_name`  — The JSX attribute name (e.g. `"onClick"`).
    /// * `span`       — OXC source span of the function node itself.
    /// * `param_count`— Number of parameters (used to tailor the suggestion).
    fn emit(&mut self, kind: FnKind, prop_name: &str, span: Span, param_count: usize) {
        let (line, col) = offset_to_line_col(self.source_text, span.start);

        // Build a context-aware fix suggestion.
        let suggestion = build_suggestion(kind, prop_name, param_count);

        let fn_desc = match kind {
            FnKind::Arrow => "Inline arrow function",
            FnKind::Regular { .. } => "Inline function expression",
        };

        self.issues.push(Issue {
            rule: "no_inline_jsx_fn".to_string(),
            message: format!(
                "{fn_desc} in '{prop_name}' prop creates a new reference on every render. {suggestion}"
            ),
            file: self.file_path.to_path_buf(),
            line,
            column: col,
            severity: Severity::Warning,
        });
    }
}

// ─── Helper types ─────────────────────────────────────────────────────────────

/// Distinguishes arrow functions from regular function expressions.
///
/// Used to generate more specific warning messages and suggestions.
#[derive(Copy, Clone)]
enum FnKind {
    Arrow,
    Regular { is_async: bool, is_generator: bool },
}

// ─── Free helper functions ────────────────────────────────────────────────────

/// Extract the human-readable attribute name as a `String`.
///
/// ```jsx
/// onClick       → "onClick"       (JSXAttributeName::Identifier)
/// xlink:href    → "xlink:href"    (JSXAttributeName::NamespacedName)
/// ```
fn extract_prop_name(name: &JSXAttributeName<'_>) -> String {
    match name {
        JSXAttributeName::Identifier(id) => id.name.to_string(),
        JSXAttributeName::NamespacedName(ns) => {
            format!("{}:{}", ns.namespace.name, ns.name.name)
        }
    }
}

/// Returns `true` if `expr` is a direct call to a memoization hook.
///
/// Recognized patterns:
/// - `useCallback(fn, deps)` — memoizes a function
/// - `useMemo(fn, deps)`     — memoizes a computed value
/// - `React.useCallback(fn, deps)` — same, via React namespace
/// - `React.useMemo(fn, deps)`
///
/// If the developer has already wrapped the inline function in `useCallback`,
/// we treat it as intentional and suppress the warning.
fn is_memoized(expr: &Expression<'_>) -> bool {
    if let Expression::CallExpression(call) = expr {
        return is_memo_callee(&call.callee);
    }
    false
}

/// Returns `true` if the callee of a call expression is a known memoization hook.
///
/// Handles both bare (`useCallback`) and namespaced (`React.useCallback`) forms.
fn is_memo_callee(callee: &Expression<'_>) -> bool {
    const MEMO_HOOKS: &[&str] = &["useCallback", "useMemo"];

    match callee {
        // Plain identifier: `useCallback(...)`, `useMemo(...)`
        Expression::Identifier(id) => MEMO_HOOKS.contains(&id.name.as_str()),

        // Member expression: `React.useCallback(...)`, `React.useMemo(...)`
        // We check only the property name, not the object name, to also
        // match custom React-compatible runtimes (Preact, etc.).
        Expression::StaticMemberExpression(member) => {
            MEMO_HOOKS.contains(&member.property.name.as_str())
        }

        _ => false,
    }
}

/// Build an actionable fix suggestion tailored to the function kind and prop.
///
/// The suggestion guides the developer toward the correct fix:
/// - Event handlers → `useCallback`
/// - Other props    → extract to a stable variable or `useCallback`
fn build_suggestion(kind: FnKind, prop_name: &str, param_count: usize) -> String {
    // Detect common event handler patterns (on* prefix).
    let is_event_handler = prop_name.starts_with("on")
        && prop_name.len() > 2
        && prop_name
            .chars()
            .nth(2)
            .map(|c| c.is_uppercase())
            .unwrap_or(false);

    match kind {
        FnKind::Arrow if is_event_handler => {
            if param_count == 0 {
                // `onClick={() => doThing()}`  → extract named handler
                format!(
                    "Extract to a named handler or wrap with useCallback: \
                     `const handle{} = useCallback(() => {{ ... }}, [deps])`",
                    capitalize(prop_name.trim_start_matches("on"))
                )
            } else {
                // `onChange={(e) => setValue(e.target.value)}`
                format!(
                    "Wrap with useCallback to stabilize the reference: \
                     `const handle{} = useCallback((e) => {{ ... }}, [deps])`",
                    capitalize(prop_name.trim_start_matches("on"))
                )
            }
        }
        FnKind::Arrow => {
            "Extract to a stable variable outside the component or wrap with useCallback"
                .to_string()
        }
        FnKind::Regular {
            is_async,
            is_generator,
        } => {
            let prefix = match (is_async, is_generator) {
                (true, _) => "async ",
                (_, true) => "function* ",
                _ => "",
            };
            format!(
                "Convert to an arrow function and wrap with useCallback, \
                 or extract as a {prefix}named function outside the component"
            )
        }
    }
}

/// Capitalize the first ASCII character of a string slice.
///
/// `"click"` → `"Click"`, `"change"` → `"Change"`
fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => c.to_uppercase().to_string() + chars.as_str(),
    }
}
