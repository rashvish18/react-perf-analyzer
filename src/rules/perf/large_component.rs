/// rules/large_component.rs — Detect React components that are too large.
///
/// # What this detects
///
/// React function components whose body exceeds the configured line threshold
/// (default 300, configurable via `--max-component-lines`).
///
/// Detected declaration forms:
///
/// ```jsx
/// // ❌ Function declaration
/// function Dashboard() { /* 350 lines */ }
///
/// // ❌ Arrow function variable
/// const Dashboard = () => { /* 400 lines */ };
///
/// // ❌ memo()-wrapped component (common in real codebases!)
/// const Dashboard = memo(() => { /* 320 lines */ });
/// const Dashboard = React.memo(() => { /* 310 lines */ });
///
/// // ❌ forwardRef()-wrapped component
/// const Input = forwardRef((props, ref) => { /* 330 lines */ });
///
/// // ❌ Named export
/// export function ProfilePage() { /* 360 lines */ }
///
/// // ❌ Default export
/// export default function App() { /* 400 lines */ }
/// ```
///
/// # Why it matters
///
/// Large components:
/// - Make selective memoization impractical (wrapping 300 lines in `React.memo`
///   still re-renders the whole thing when any of many state slices change)
/// - Force React to reconcile large virtual-DOM subtrees on every update
/// - Make it hard to reason about which state change triggers which update
/// - Often indicate that unrelated responsibilities are co-located
///
/// # Component detection heuristic
///
/// A function qualifies as a React component if:
///   1. The binding name starts with an uppercase letter (PascalCase).
///   2. The function body contains at least one JSX element or JSX fragment.
///
/// # Complexity metrics (reported in warning)
///
/// Beyond raw line count we collect two fast proxy metrics:
///
/// - **JSX element count** — how many distinct `<Element>` nodes are in the
///   render output. High count → large render tree → harder to split.
///
/// - **Hook call count** — how many `useXxx(...)` calls are at the top level.
///   High count → many state slices → component is doing too much.
///
/// These metrics are reported alongside line counts so developers have context
/// for *why* the component is large, not just *that* it is large.
///
/// # Line counting
///
/// We count three line categories within the component span:
/// - **total**   — every line in the span
/// - **blank**   — empty or whitespace-only lines
/// - **comment** — lines whose first non-whitespace chars are `//`, `*`, or `/*`
/// - **logical** — total − blank − comment (actual code lines)
///
/// The warning shows all three so developers can tell whether their component
/// is inflated by comments/blank lines or by genuine logic.
///
/// # OXC Statement enum (flat/inherited structure)
///
/// In OXC 0.67, `Statement` is a flat enum that inherits from `Declaration`
/// and `ModuleDeclaration` via the `@inherit` macro. This means:
///   - `Statement::FunctionDeclaration(...)` — direct variant (no wrapping)
///   - `Statement::VariableDeclaration(...)` — direct variant
///   - `Statement::ExportNamedDeclaration(...)` — inherited from ModuleDeclaration
///   - `ExportNamedDeclaration.declaration: Option<Declaration<'a>>` — uses
///     the separate `Declaration` enum for the actual declaration inside
use std::path::Path;

use oxc_ast::ast::{
    ArrowFunctionExpression, BindingPatternKind, CallExpression, Declaration, Expression, Function,
    FunctionBody, Program, Statement, VariableDeclarator,
};
use oxc_ast_visit::{walk, Visit};
use oxc_span::Span;

use crate::{
    rules::{Issue, RuleContext, Severity},
    utils::offset_to_line_col,
};

// ─── Rule struct ──────────────────────────────────────────────────────────────

/// Zero-size marker struct — all state is local to each `run()` call.
pub struct LargeComponent;

impl super::Rule for LargeComponent {
    fn name(&self) -> &str {
        "large_component"
    }

    fn run(&self, ctx: &RuleContext<'_>) -> Vec<Issue> {
        find_large_components(
            ctx.program,
            ctx.source_text,
            ctx.file_path,
            ctx.max_component_lines,
        )
    }
}

// ─── Line metrics ─────────────────────────────────────────────────────────────

/// Breakdown of line categories within a component span.
#[derive(Debug, Default)]
struct LineMetrics {
    total: usize,
    blank: usize,
    comment: usize,
}

impl LineMetrics {
    /// Logical lines = total − blank − comment (pure code lines).
    fn logical(&self) -> usize {
        self.total.saturating_sub(self.blank + self.comment)
    }
}

/// Count line categories in `source[start..end]`.
///
/// Lines are categorised as:
/// - **blank**   — empty or whitespace-only
/// - **comment** — first non-whitespace token is `//`, `/*`, or `*` (JSDoc continuation)
/// - **logical** — everything else
fn measure_lines(source: &str, start: u32, end: u32) -> LineMetrics {
    let start = (start as usize).min(source.len());
    let end = (end as usize).min(source.len());
    if start >= end {
        return LineMetrics::default();
    }

    let mut m = LineMetrics::default();
    for raw_line in source[start..end].lines() {
        m.total += 1;
        let trimmed = raw_line.trim();
        if trimmed.is_empty() {
            m.blank += 1;
        } else if trimmed.starts_with("//") || trimmed.starts_with("/*") || trimmed.starts_with('*')
        {
            m.comment += 1;
        }
    }
    m
}

// ─── Complexity metrics ───────────────────────────────────────────────────────

/// Complexity snapshot of a React component's body.
struct ComplexityMetrics {
    jsx_elements: usize,
    hook_calls: usize,
}

/// Walk `body` to count JSX elements and hook calls.
fn measure_complexity(body: &FunctionBody<'_>) -> ComplexityMetrics {
    let mut v = ComplexityVisitor {
        jsx_elements: 0,
        hook_calls: 0,
    };
    v.visit_function_body(body);
    ComplexityMetrics {
        jsx_elements: v.jsx_elements,
        hook_calls: v.hook_calls,
    }
}

/// Walk an arrow function body to count JSX elements and hook calls.
fn measure_complexity_arrow(arrow: &ArrowFunctionExpression<'_>) -> ComplexityMetrics {
    let mut v = ComplexityVisitor {
        jsx_elements: 0,
        hook_calls: 0,
    };
    v.visit_arrow_function_expression(arrow);
    ComplexityMetrics {
        jsx_elements: v.jsx_elements,
        hook_calls: v.hook_calls,
    }
}

/// Visits the AST and accumulates JSX element + hook call counts.
struct ComplexityVisitor {
    jsx_elements: usize,
    hook_calls: usize,
}

impl<'a> Visit<'a> for ComplexityVisitor {
    /// Count every JSX element opening tag (fragments are counted separately).
    fn visit_jsx_element(&mut self, elem: &oxc_ast::ast::JSXElement<'a>) {
        self.jsx_elements += 1;
        // Continue walking to count nested JSX too.
        walk::walk_jsx_element(self, elem);
    }

    fn visit_jsx_fragment(&mut self, frag: &oxc_ast::ast::JSXFragment<'a>) {
        self.jsx_elements += 1;
        walk::walk_jsx_fragment(self, frag);
    }

    /// Count calls to React hooks (`use[A-Z]...` naming convention).
    ///
    /// We only match the direct callee name pattern — we don't require
    /// that `React` is imported, which keeps the check fast and
    /// compatible with Preact, Solid, and other React-compatible runtimes.
    fn visit_call_expression(&mut self, call: &oxc_ast::ast::CallExpression<'a>) {
        if is_hook_call(&call.callee) {
            self.hook_calls += 1;
        }
        // Continue walking to detect hooks nested inside callbacks.
        walk::walk_call_expression(self, call);
    }
}

/// Returns `true` if `callee` looks like a React hook call (`use[A-Z]...`).
///
/// Matches:
/// - `useEffect(...)` — plain identifier
/// - `React.useEffect(...)` — member expression (any namespace)
fn is_hook_call(callee: &Expression<'_>) -> bool {
    match callee {
        Expression::Identifier(id) => looks_like_hook(id.name.as_str()),
        Expression::StaticMemberExpression(m) => looks_like_hook(m.property.name.as_str()),
        _ => false,
    }
}

/// Returns `true` if `name` matches the `use[A-Z]...` hook naming convention.
#[inline]
fn looks_like_hook(name: &str) -> bool {
    name.starts_with("use")
        && name.len() > 3
        && name
            .chars()
            .nth(3)
            .map(|c| c.is_uppercase())
            .unwrap_or(false)
}

// ─── HOC wrapping detection ───────────────────────────────────────────────────

/// Names of HOC wrappers whose first argument is the actual component function.
///
/// Supports both bare names and `React.name` forms:
/// - `memo(fn)` / `React.memo(fn)`
/// - `forwardRef(fn)` / `React.forwardRef(fn)`
const HOC_WRAPPERS: &[&str] = &["memo", "forwardRef"];

/// Attempt to unwrap one level of HOC wrapping from a `const X = hoc(fn)` init.
///
/// Returns `Some((inner_fn_expr, hoc_name))` when the pattern matches, so the
/// caller can measure `inner_fn_expr` instead of the outer call expression.
///
/// ```
/// const Foo = memo(() => { ... })
///                  ^^^^^^^^^^^^  ← returned inner expression
/// ```
fn unwrap_hoc<'a>(call: &'a CallExpression<'a>) -> Option<&'a Expression<'a>> {
    // The callee must be a known HOC wrapper.
    if !is_hoc_callee(&call.callee) {
        return None;
    }

    // The first argument must be a function (arrow or function expression).
    let first_arg = call.arguments.first()?;
    let inner = first_arg.as_expression()?;

    // Only unwrap if the argument actually is a function — if it's a plain
    // identifier like `memo(MyComponent)`, the component is defined elsewhere
    // and will be checked when we encounter its own declaration.
    if matches!(
        inner,
        Expression::ArrowFunctionExpression(_) | Expression::FunctionExpression(_)
    ) {
        Some(inner)
    } else {
        None
    }
}

/// Returns `true` if `callee` is one of the recognised HOC wrapper functions.
fn is_hoc_callee(callee: &Expression<'_>) -> bool {
    match callee {
        Expression::Identifier(id) => HOC_WRAPPERS.contains(&id.name.as_str()),
        Expression::StaticMemberExpression(m) => HOC_WRAPPERS.contains(&m.property.name.as_str()),
        _ => false,
    }
}

// ─── Top-level walker ─────────────────────────────────────────────────────────

/// Walk `program.body` and check every top-level React component declaration.
///
/// We iterate manually (not via the `Visit` trait) because we only care about
/// top-level declarations — components nested inside other functions are a
/// code-smell but are out of scope for this rule's MVP.
fn find_large_components(
    program: &Program<'_>,
    source_text: &str,
    file_path: &Path,
    max_lines: usize,
) -> Vec<Issue> {
    let mut issues = Vec::new();

    for stmt in &program.body {
        match stmt {
            // ── `function Dashboard() { ... }` ──────────────────────────────
            Statement::FunctionDeclaration(func) => {
                check_function(func, source_text, file_path, max_lines, &mut issues);
            }

            // ── `const Dashboard = () => ...` ────────────────────────────────
            // ── `const Dashboard = memo(() => ...)` ──────────────────────────
            Statement::VariableDeclaration(var_decl) => {
                for decl in &var_decl.declarations {
                    check_variable_declarator(decl, source_text, file_path, max_lines, &mut issues);
                }
            }

            // ── `export default function Dashboard() { ... }` ─────────────────
            Statement::ExportDefaultDeclaration(export) => {
                use oxc_ast::ast::ExportDefaultDeclarationKind;
                match &export.declaration {
                    ExportDefaultDeclarationKind::FunctionDeclaration(func) => {
                        check_function(func, source_text, file_path, max_lines, &mut issues);
                    }
                    // `export default () => { ... }` — anonymous default arrow export.
                    // Use the filename as the component name.
                    ExportDefaultDeclarationKind::ArrowFunctionExpression(arrow) => {
                        let name = extract_filename(file_path);
                        if arrow_contains_jsx(arrow) {
                            report_if_large_arrow(
                                arrow,
                                &name,
                                source_text,
                                file_path,
                                max_lines,
                                &mut issues,
                            );
                        }
                    }
                    _ => {}
                }
            }

            // ── `export function Dashboard() { ... }` ────────────────────────
            // ── `export const Dashboard = ...` ───────────────────────────────
            // ExportNamedDeclaration.declaration is `Option<Declaration<'a>>`,
            // where `Declaration` is a SEPARATE enum from `Statement`.
            Statement::ExportNamedDeclaration(export) => {
                if let Some(decl) = &export.declaration {
                    match decl {
                        Declaration::FunctionDeclaration(func) => {
                            check_function(func, source_text, file_path, max_lines, &mut issues);
                        }
                        Declaration::VariableDeclaration(var_decl) => {
                            for d in &var_decl.declarations {
                                check_variable_declarator(
                                    d,
                                    source_text,
                                    file_path,
                                    max_lines,
                                    &mut issues,
                                );
                            }
                        }
                        _ => {}
                    }
                }
            }

            _ => {}
        }
    }

    issues
}

// ─── Per-declaration checkers ─────────────────────────────────────────────────

/// Check a `function Foo() { ... }` declaration.
fn check_function(
    func: &Function<'_>,
    source_text: &str,
    file_path: &Path,
    max_lines: usize,
    issues: &mut Vec<Issue>,
) {
    let name = match &func.id {
        Some(id) => id.name.as_str(),
        None => return, // anonymous function expression — no reportable name
    };

    if !is_pascal_case(name) {
        return;
    }

    if let Some(body) = &func.body {
        if !body_contains_jsx(body) {
            return;
        }
        let metrics = measure_lines(source_text, func.span.start, func.span.end);
        if metrics.total > max_lines {
            let complexity = measure_complexity(body);
            report(
                func.span,
                name,
                &metrics,
                &complexity,
                source_text,
                file_path,
                max_lines,
                issues,
            );
        }
    }
}

/// Check a `const Foo = ...` variable declarator, including HOC-wrapped forms.
///
/// Handles:
/// - `const Foo = () => { ... }`
/// - `const Foo = function() { ... }`
/// - `const Foo = memo(() => { ... })`
/// - `const Foo = React.memo(function() { ... })`
/// - `const Foo = forwardRef((props, ref) => { ... })`
fn check_variable_declarator(
    decl: &VariableDeclarator<'_>,
    source_text: &str,
    file_path: &Path,
    max_lines: usize,
    issues: &mut Vec<Issue>,
) {
    let name = match &decl.id.kind {
        BindingPatternKind::BindingIdentifier(id) => id.name.as_str(),
        _ => return,
    };

    if !is_pascal_case(name) {
        return;
    }

    let Some(init) = &decl.init else { return };

    // Resolve the actual function expression — either direct or HOC-wrapped.
    //
    //   Direct:      const Foo = () => { ... }
    //   HOC-wrapped: const Foo = memo(() => { ... })
    //                                  ^^^^^^^^^^^^  ← this is `effective_fn`
    let effective_fn: &Expression<'_> = match init {
        // Possible HOC call: check if it wraps an inline component function.
        Expression::CallExpression(call) => match unwrap_hoc(call) {
            Some(inner) => inner,
            // Not a recognised HOC, or the argument is a reference (not inline).
            // Fall through: treat the call itself as a non-component expression.
            None => return,
        },
        // Direct function / arrow function.
        other => other,
    };

    match effective_fn {
        Expression::ArrowFunctionExpression(arrow) => {
            if arrow_contains_jsx(arrow) {
                report_if_large_arrow(arrow, name, source_text, file_path, max_lines, issues);
            }
        }
        Expression::FunctionExpression(func) => {
            if let Some(body) = &func.body {
                if body_contains_jsx(body) {
                    let metrics = measure_lines(source_text, func.span.start, func.span.end);
                    if metrics.total > max_lines {
                        let complexity = measure_complexity(body);
                        report(
                            func.span,
                            name,
                            &metrics,
                            &complexity,
                            source_text,
                            file_path,
                            max_lines,
                            issues,
                        );
                    }
                }
            }
        }
        _ => {}
    }
}

/// Measure and report a large arrow function component.
///
/// Extracted so both the `VariableDeclarator` path and the anonymous
/// `export default () => {}` path can share it.
fn report_if_large_arrow(
    arrow: &ArrowFunctionExpression<'_>,
    name: &str,
    source_text: &str,
    file_path: &Path,
    max_lines: usize,
    issues: &mut Vec<Issue>,
) {
    let metrics = measure_lines(source_text, arrow.span.start, arrow.span.end);
    if metrics.total > max_lines {
        let complexity = measure_complexity_arrow(arrow);
        report(
            arrow.span,
            name,
            &metrics,
            &complexity,
            source_text,
            file_path,
            max_lines,
            issues,
        );
    }
}

// ─── Issue emission ───────────────────────────────────────────────────────────

/// Build and push the `large_component` issue with full metrics context.
#[allow(clippy::too_many_arguments)]
fn report(
    span: Span,
    name: &str,
    lines: &LineMetrics,
    complexity: &ComplexityMetrics,
    source_text: &str,
    file_path: &Path,
    max_lines: usize,
    issues: &mut Vec<Issue>,
) {
    let (line, col) = offset_to_line_col(source_text, span.start);

    // Build a human-readable suggestion tailored to what the metrics show.
    let suggestion = build_split_suggestion(lines, complexity);

    issues.push(Issue {
        rule: "large_component".to_string(),
        message: format!(
            "Component '{name}' is {total} lines ({logical} logical, {blank} blank) — \
             limit is {max_lines}. \
             Render complexity: {jsx} JSX elements, {hooks} hooks. \
             {suggestion}",
            total = lines.total,
            logical = lines.logical(),
            blank = lines.blank,
            jsx = complexity.jsx_elements,
            hooks = complexity.hook_calls,
        ),
        file: file_path.to_path_buf(),
        line,
        column: col,
        severity: Severity::Medium,
        source: crate::rules::IssueSource::ReactPerfAnalyzer,
        category: crate::rules::IssueCategory::Performance,
    });
}

/// Build a splitting suggestion based on what's driving the component's size.
fn build_split_suggestion(lines: &LineMetrics, cx: &ComplexityMetrics) -> String {
    let logical = lines.logical();

    if cx.hook_calls >= 5 && cx.jsx_elements >= 10 {
        format!(
            "With {} hooks and {} JSX elements, this component has too many responsibilities. \
             Extract each major UI section into its own sub-component, and consider a \
             custom hook to consolidate related state.",
            cx.hook_calls, cx.jsx_elements
        )
    } else if cx.hook_calls >= 5 {
        format!(
            "{} hook calls suggest complex state logic. Extract into a custom hook \
             (e.g. `use{}State`) to separate concerns.",
            cx.hook_calls,
            // Suggest a hook name based on the first capital letter pattern.
            "Component"
        )
    } else if cx.jsx_elements >= 10 {
        format!(
            "{} JSX elements make this hard to memoize selectively. \
             Split the render into 2-3 focused sub-components, each wrapped \
             in `React.memo` for fine-grained update control.",
            cx.jsx_elements
        )
    } else if logical > 200 {
        "Consider splitting this component at its natural seams \
         (tabs, sections, or feature boundaries)."
            .to_string()
    } else {
        "Consider extracting sub-components or custom hooks to reduce size.".to_string()
    }
}

// ─── JSX presence check ───────────────────────────────────────────────────────

/// Returns `true` if `body` contains at least one JSX element or fragment.
///
/// Uses a short-circuit visitor: stops walking after the first JSX node is
/// found to avoid full traversal of large bodies.
fn body_contains_jsx(body: &FunctionBody<'_>) -> bool {
    let mut checker = JsxPresenceChecker { found: false };
    checker.visit_function_body(body);
    checker.found
}

/// Returns `true` if the arrow function contains any JSX.
fn arrow_contains_jsx(arrow: &ArrowFunctionExpression<'_>) -> bool {
    let mut checker = JsxPresenceChecker { found: false };
    checker.visit_arrow_function_expression(arrow);
    checker.found
}

/// Minimal visitor that short-circuits on the first JSX node.
///
/// `visit_expression` is overridden to prune further walking once JSX is
/// confirmed — this is O(1) best-case for components whose first statement
/// is a JSX return.
struct JsxPresenceChecker {
    found: bool,
}

impl<'a> Visit<'a> for JsxPresenceChecker {
    fn visit_jsx_element(&mut self, elem: &oxc_ast::ast::JSXElement<'a>) {
        self.found = true;
        let _ = elem; // Don't walk deeper — we have our answer.
    }

    fn visit_jsx_fragment(&mut self, frag: &oxc_ast::ast::JSXFragment<'a>) {
        self.found = true;
        let _ = frag;
    }

    /// Skip all expressions once JSX is found — avoids scanning the rest of
    /// a 300-line component when JSX is in the first 5 lines.
    fn visit_expression(&mut self, expr: &Expression<'a>) {
        if !self.found {
            walk::walk_expression(self, expr);
        }
    }
}

// ─── Utilities ────────────────────────────────────────────────────────────────

/// Returns `true` if `name` starts with an ASCII uppercase letter (PascalCase).
///
/// React's convention: HTML intrinsics are lowercase (`div`, `span`),
/// components are PascalCase (`Dashboard`, `UserCard`).
#[inline]
fn is_pascal_case(name: &str) -> bool {
    name.chars()
        .next()
        .map(|c| c.is_ascii_uppercase())
        .unwrap_or(false)
}

/// Derive a display name from the file path for anonymous default exports.
///
/// `src/pages/Dashboard.tsx` → `"Dashboard"`
/// Falls back to `"DefaultExport"` if the stem can't be determined.
fn extract_filename(path: &Path) -> String {
    path.file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("DefaultExport")
        .to_string()
}
