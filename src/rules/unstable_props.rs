/// rules/unstable_props.rs — Detect object/array literals in JSX props.
///
/// # What this detects
///
/// Object literal or array literal expressions passed directly as JSX
/// attribute values — including those hidden inside ternaries, logical
/// expressions, and parentheses:
///
/// ```jsx
/// // ❌ Direct object literal — new object every render
/// <UserCard style={{ color: "red", fontSize: 14 }} />
///
/// // ❌ Direct array literal — new array every render
/// <DataTable columns={["id", "name", "email"]} />
///
/// // ❌ Object literal in ternary branch
/// <Chart options={isBar ? { type: "bar" } : defaults} />
///
/// // ❌ Object in logical expression
/// <Grid config={enabled && { dense: true }} />
///
/// // ✅ Stable reference — extracted outside the component
/// const COLUMNS = ["id", "name", "email"];
/// <DataTable columns={COLUMNS} />
///
/// // ✅ useMemo-wrapped — developer has intentionally memoized
/// <UserCard style={useMemo(() => ({ color }), [color])} />
/// ```
///
/// # Why it's a problem
///
/// In JavaScript, `{a: 1} === {a: 1}` is `false` — two object literals are
/// never referentially equal even with identical contents. On every render a
/// new object/array is allocated, so child components wrapped in `React.memo`
/// or `shouldComponentUpdate` always see a changed prop and re-render
/// unnecessarily.
///
/// # AST traversal strategy
///
/// 1. `visit_jsx_opening_element` — called for every `<Tag ...>`.
/// 2. For each regular attribute, extract the prop name and expression value.
/// 3. Fast-path: `useMemo(...)` / `React.useMemo(...)` — developer has already
///    memoized the value; suppress the warning.
/// 4. `scan_for_unstable` recursively inspects the expression:
///    - `ObjectExpression` / `ArrayExpression` → emit issue
///    - `ConditionalExpression` → scan both consequent and alternate
///    - `LogicalExpression` (`&&` / `||` / `??`) → scan both operands
///    - `ParenthesizedExpression` → unwrap and scan
///    - Everything else → stop (avoids false positives)
///
/// # OXC API notes (v0.67)
///
/// - `Visit` trait + `walk` module live in the `oxc_ast_visit` crate.
/// - `container.expression.as_expression()` returns `Option<&Expression<'a>>`
///   (returns `None` for empty `attr={}` — `JSXEmptyExpression`).
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

/// Zero-size rule struct — all per-file state lives in `UnstablePropsVisitor`.
pub struct UnstableProps;

impl super::Rule for UnstableProps {
    fn name(&self) -> &str {
        "unstable_props"
    }

    fn run(&self, ctx: &RuleContext<'_>) -> Vec<Issue> {
        let mut visitor = UnstablePropsVisitor {
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

/// Walks the AST and accumulates `unstable_props` issues.
struct UnstablePropsVisitor<'a> {
    issues: Vec<Issue>,
    source_text: &'a str,
    file_path: &'a Path,
}

impl<'a> Visit<'a> for UnstablePropsVisitor<'_> {
    /// Entry point for every JSX opening element: `<Tag attr1={...} attr2=... />`.
    ///
    /// We override at the *element* level (not attribute level) so we can
    /// access both the attribute name and its value together — the name is
    /// included in the warning message for better developer ergonomics.
    fn visit_jsx_opening_element(&mut self, elem: &JSXOpeningElement<'a>) {
        for attr_item in &elem.attributes {
            match attr_item {
                // Regular attribute: `style={{...}}` or `columns={[...]}`
                JSXAttributeItem::Attribute(attr) => {
                    // Attributes without a value are boolean flags (`<Comp disabled />`).
                    // They can never hold an object/array, so skip them.
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
                    let prop_name = extract_prop_name(&attr.name);

                    // Fast-path: if the top-level expression is `useMemo(...)` or
                    // `React.useMemo(...)`, the developer has already memoized the
                    // value — suppress the warning entirely.
                    if is_memo_wrapped(expr) {
                        continue;
                    }

                    // Recursively scan for object/array literals.
                    self.scan_for_unstable(expr, &prop_name);
                }

                // Spread attribute: `{...props}` — skip, not a named prop.
                JSXAttributeItem::SpreadAttribute(_) => {}
            }
        }

        // IMPORTANT: always walk into child elements so nested JSX like
        // `<Parent><Child style={{...}} /></Parent>` is also analyzed.
        walk::walk_jsx_opening_element(self, elem);
    }
}

impl UnstablePropsVisitor<'_> {
    /// Recursively inspect `expr` for unstable object/array literal expressions.
    ///
    /// We recurse into conditional and logical expressions because a literal in
    /// either branch still allocates a new value on every render:
    ///
    /// ```jsx
    /// // Still a problem — new object allocated for the `true` branch every render
    /// <Chart options={isBar ? { type: "bar" } : stableDefaults} />
    /// ```
    ///
    /// We do NOT recurse into:
    /// - Call expression arguments (the return value may be stable)
    /// - Object/array contents themselves (only the top-level literal matters)
    fn scan_for_unstable(&mut self, expr: &Expression<'_>, prop_name: &str) {
        match expr {
            // ── Direct object literal ─────────────────────────────────────
            // `style={{ color: "red" }}`, `config={{ dense: true }}`
            Expression::ObjectExpression(obj) => {
                self.emit(UnstableKind::Object, prop_name, obj.span);
            }

            // ── Direct array literal ──────────────────────────────────────
            // `columns={["id", "name"]}`, `items={[1, 2, 3]}`
            Expression::ArrayExpression(arr) => {
                self.emit(UnstableKind::Array, prop_name, arr.span);
            }

            // ── Ternary: scan both branches ───────────────────────────────
            // `options={isBar ? { type: "bar" } : defaults}`
            //                    ^^^^^^^^^^^^^^  ← warn if this is a literal
            Expression::ConditionalExpression(cond) => {
                self.scan_conditional(cond, prop_name);
            }

            // ── Logical (&&, ||, ??): scan both operands ──────────────────
            // `config={enabled && { dense: true }}`
            Expression::LogicalExpression(logical) => {
                self.scan_logical(logical, prop_name);
            }

            // ── Parenthesized: unwrap and scan ────────────────────────────
            // `style={({ color: "red" })}`
            Expression::ParenthesizedExpression(paren) => {
                self.scan_parenthesized(paren, prop_name);
            }

            // All other expression types (identifiers, call expressions,
            // member expressions, etc.) are not unstable literal definitions
            // and do not need recursive scanning.
            _ => {}
        }
    }

    // ── Conditional expression (`? :`) ────────────────────────────────────────

    /// Scan both branches of a ternary for unstable literals.
    ///
    /// The test expression (`condition` in `condition ? a : b`) is skipped —
    /// a literal there would be used as a boolean, not stored as a prop value.
    fn scan_conditional(&mut self, cond: &ConditionalExpression<'_>, prop_name: &str) {
        self.scan_for_unstable(&cond.consequent, prop_name);
        self.scan_for_unstable(&cond.alternate, prop_name);
    }

    // ── Logical expression (`&&`, `||`, `??`) ────────────────────────────────

    /// Scan both sides of a logical expression for unstable literals.
    ///
    /// ```jsx
    /// <Grid config={enabled && { dense: true }} />
    /// <Grid config={overrides || { gap: 4 }} />
    /// ```
    fn scan_logical(&mut self, logical: &LogicalExpression<'_>, prop_name: &str) {
        self.scan_for_unstable(&logical.left, prop_name);
        self.scan_for_unstable(&logical.right, prop_name);
    }

    // ── Parenthesized expression ──────────────────────────────────────────────

    /// Unwrap parentheses and scan the inner expression.
    ///
    /// ```jsx
    /// <Card style={({ color: "red" })} />
    /// ```
    fn scan_parenthesized(&mut self, paren: &ParenthesizedExpression<'_>, prop_name: &str) {
        self.scan_for_unstable(&paren.expression, prop_name);
    }

    // ── Issue emission ────────────────────────────────────────────────────────

    /// Build and push an `Issue` for a detected unstable literal.
    ///
    /// # Arguments
    /// * `kind`      — Whether this is an object or array literal.
    /// * `prop_name` — The JSX attribute name (e.g. `"style"`, `"columns"`).
    /// * `span`      — OXC source span of the literal node itself.
    fn emit(&mut self, kind: UnstableKind, prop_name: &str, span: Span) {
        let (line, col) = offset_to_line_col(self.source_text, span.start);

        let suggestion = build_suggestion(kind, prop_name);

        let kind_desc = match kind {
            UnstableKind::Object => "Object literal",
            UnstableKind::Array => "Array literal",
        };

        self.issues.push(Issue {
            rule: "unstable_props".to_string(),
            message: format!(
                "{kind_desc} in '{prop_name}' prop creates a new reference on every render. {suggestion}"
            ),
            file: self.file_path.to_path_buf(),
            line,
            column: col,
            severity: Severity::Warning,
        });
    }
}

// ─── Helper types ─────────────────────────────────────────────────────────────

/// Distinguishes object literals from array literals.
///
/// Used to generate more specific warning messages and suggestions.
#[derive(Copy, Clone)]
enum UnstableKind {
    Object,
    Array,
}

// ─── Free helper functions ────────────────────────────────────────────────────

/// Extract the human-readable attribute name as a `String`.
///
/// ```jsx
/// style         → "style"         (JSXAttributeName::Identifier)
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

/// Returns `true` if `expr` is a direct call to `useMemo` or `React.useMemo`.
///
/// When the developer has already wrapped the value in `useMemo`, we treat it
/// as intentional memoization and suppress the warning.
///
/// Recognized patterns:
/// - `useMemo(() => ({...}), [deps])`
/// - `React.useMemo(() => ({...}), [deps])`
fn is_memo_wrapped(expr: &Expression<'_>) -> bool {
    if let Expression::CallExpression(call) = expr {
        return is_use_memo_callee(&call.callee);
    }
    false
}

/// Returns `true` if the callee of a call expression is `useMemo` or `React.useMemo`.
fn is_use_memo_callee(callee: &Expression<'_>) -> bool {
    match callee {
        // Plain identifier: `useMemo(...)`
        Expression::Identifier(id) => id.name.as_str() == "useMemo",

        // Member expression: `React.useMemo(...)`
        // We check only the property name, not the object, so this also
        // matches custom React-compatible runtimes (Preact, etc.).
        Expression::StaticMemberExpression(member) => member.property.name.as_str() == "useMemo",

        _ => false,
    }
}

/// Build an actionable fix suggestion tailored to the literal kind and prop name.
///
/// Provides context-aware advice:
/// - Style props (`style`, `sx`, `css`, `theme`) → `useMemo` with style pattern
/// - Arrays → extract to module-level constant
/// - Other objects → `useMemo` or stable variable
fn build_suggestion(kind: UnstableKind, prop_name: &str) -> String {
    // Common style-related prop names that almost always hold inline objects.
    let is_style_prop = matches!(prop_name, "style" | "sx" | "css" | "theme" | "wrapperStyle");

    // Derive a PascalCase suffix for the suggested variable name.
    // e.g. "columns" → "Columns", "onClick" → "OnClick"
    let pascal_name = capitalize(prop_name);

    match kind {
        UnstableKind::Object if is_style_prop => {
            format!(
                "Extract to a module-level constant or wrap with useMemo: \
                 `const {pascal_name} = useMemo(() => ({{ ... }}), [deps])`"
            )
        }
        UnstableKind::Object => {
            format!(
                "Extract to a stable variable outside the component or wrap with useMemo: \
                 `const {pascal_name} = useMemo(() => ({{ ... }}), [deps])`"
            )
        }
        UnstableKind::Array => {
            format!(
                "Extract to a module-level constant or wrap with useMemo: \
                 `const {pascal_name} = useMemo(() => [...], [deps])`"
            )
        }
    }
}

/// Capitalize the first ASCII character of a string slice.
///
/// `"style"` → `"Style"`, `"columns"` → `"Columns"`
fn capitalize(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(c) => c.to_uppercase().to_string() + chars.as_str(),
    }
}
