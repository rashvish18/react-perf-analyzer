//! rules/no_array_index_key.rs — Detect array index used as React `key` prop.
//!
//! # What this detects
//!
//! The second parameter of a `.map()` callback (the array index) used directly
//! as the `key` prop on a JSX element:
//!
//! ```jsx
//! // ❌ Classic pattern — index as key
//! items.map((item, index) => <li key={index}>{item.name}</li>)
//!
//! // ❌ Short variable name
//! items.map((item, i) => <Row key={i} data={item} />)
//!
//! // ❌ Nested JSX — index used on inner element
//! items.map((item, idx) => (
//!   <div>
//!     <Card key={idx} title={item.title} />
//!   </div>
//! ))
//!
//! // ❌ Template literal wrapping the index
//! items.map((item, i) => <li key={`item-${i}`}>{item}</li>)
//!
//! // ✅ Stable ID from the data — no warning
//! items.map((item) => <li key={item.id}>{item.name}</li>)
//!
//! // ✅ Composite key using item data — no warning
//! items.map((item) => <li key={`${item.type}-${item.id}`}>{item.name}</li>)
//! ```
//!
//! # Why it's a problem
//!
//! React uses the `key` prop to identify which elements in a list have changed
//! between renders. When you use the array index:
//! - Inserting at the beginning shifts every element's index
//! - React sees "all elements changed" and re-renders the entire list
//! - Component state (inputs, focus, animations) is incorrectly transferred
//!   between items that happen to share the same index
//!
//! Always use a stable, unique identifier from the data itself (`item.id`,
//! `item.uuid`, `item.slug`).
//!
//! # AST traversal strategy
//!
//! 1. `visit_call_expression` — detect `.map(callback)` calls.
//! 2. Extract the second parameter name from the callback (the index variable).
//! 3. Push that name onto `index_params` stack and walk the call expression.
//! 4. `visit_jsx_opening_element` — while inside a map callback, check if
//!    the `key` attribute references any name in `index_params`.
//! 5. Pop the name after the call expression walk completes.
//!
//! The stack approach handles nested `.map()` calls correctly since each level
//! pushes/pops independently.

use std::path::Path;

use oxc_ast::ast::{
    BindingPatternKind, CallExpression, Expression, JSXAttributeItem, JSXAttributeName,
    JSXAttributeValue, JSXOpeningElement,
};
use oxc_ast_visit::{walk, Visit};
use oxc_span::Span;

use crate::{
    rules::{Issue, RuleContext, Severity},
    utils::offset_to_line_col,
};

// ─── Rule struct ──────────────────────────────────────────────────────────────

/// Zero-size rule struct — all per-file state lives in the visitor.
pub struct NoArrayIndexKey;

impl super::Rule for NoArrayIndexKey {
    fn name(&self) -> &str {
        "no_array_index_key"
    }

    fn run(&self, ctx: &RuleContext<'_>) -> Vec<Issue> {
        let mut visitor = ArrayIndexKeyVisitor {
            issues: Vec::new(),
            source_text: ctx.source_text,
            file_path: ctx.file_path,
            index_params: Vec::new(),
        };
        visitor.visit_program(ctx.program);
        visitor.issues
    }
}

// ─── Visitor ──────────────────────────────────────────────────────────────────

/// Walks the AST and accumulates `no_array_index_key` issues.
struct ArrayIndexKeyVisitor<'a> {
    issues: Vec<Issue>,
    source_text: &'a str,
    file_path: &'a Path,
    /// Stack of index parameter names currently in scope from `.map()` callbacks.
    ///
    /// Each entry corresponds to one active `.map()` callback level, allowing
    /// correct handling of nested `.map()` calls.
    index_params: Vec<String>,
}

impl<'a> Visit<'a> for ArrayIndexKeyVisitor<'_> {
    /// Override call expressions to detect `.map()` callbacks and track their
    /// index parameters on a stack while walking child nodes.
    fn visit_call_expression(&mut self, expr: &CallExpression<'a>) {
        if let Some(index_name) = extract_map_index_param(expr) {
            // Push the index param name so inner JSX key-checks can see it.
            self.index_params.push(index_name);
            walk::walk_call_expression(self, expr);
            self.index_params.pop();
        } else {
            walk::walk_call_expression(self, expr);
        }
    }

    /// While inside a `.map()` callback, check if any JSX `key` prop references
    /// a tracked index parameter.
    fn visit_jsx_opening_element(&mut self, elem: &JSXOpeningElement<'a>) {
        if !self.index_params.is_empty() {
            for attr_item in &elem.attributes {
                if let JSXAttributeItem::Attribute(attr) = attr_item {
                    if !is_key_prop(&attr.name) {
                        continue;
                    }

                    let Some(JSXAttributeValue::ExpressionContainer(container)) = &attr.value
                    else {
                        continue;
                    };

                    let Some(expr) = container.expression.as_expression() else {
                        continue;
                    };

                    if let Some(span) = self.find_index_ref(expr) {
                        self.emit(span);
                    }
                }
            }
        }

        // Always walk children — key may appear on a deeply nested element.
        walk::walk_jsx_opening_element(self, elem);
    }
}

impl ArrayIndexKeyVisitor<'_> {
    /// Recursively search `expr` for a direct reference to any tracked index
    /// parameter. Returns the `Span` of the first match found.
    ///
    /// Handles common patterns:
    /// - `key={index}` — plain identifier
    /// - `key={index.toString()}` — method call on the index
    /// - `` key={`prefix-${index}`} `` — template literal
    /// - `key={(index)}` — parenthesized
    fn find_index_ref(&self, expr: &Expression<'_>) -> Option<Span> {
        match expr {
            // Plain identifier: `key={index}`, `key={i}`, `key={idx}`
            Expression::Identifier(id) => {
                if self.index_params.iter().any(|p| p == id.name.as_str()) {
                    Some(id.span)
                } else {
                    None
                }
            }

            // Method call on index: `key={index.toString()}`
            Expression::CallExpression(call) => {
                if let Expression::StaticMemberExpression(member) = &call.callee {
                    return self.find_index_ref(&member.object);
                }
                None
            }

            // Template literal: `key={\`item-${index}\`}`
            Expression::TemplateLiteral(tpl) => {
                for inner_expr in tpl.expressions.iter() {
                    if let Some(span) = self.find_index_ref(inner_expr) {
                        return Some(span);
                    }
                }
                None
            }

            // Parenthesized: `key={(index)}`
            Expression::ParenthesizedExpression(paren) => self.find_index_ref(&paren.expression),

            _ => None,
        }
    }

    fn emit(&mut self, span: Span) {
        let (line, col) = offset_to_line_col(self.source_text, span.start);
        self.issues.push(Issue {
            rule: "no_array_index_key".to_string(),
            message: "Array index used as 'key' prop causes incorrect reconciliation when items \
                      are added, removed, or reordered. Use a stable unique ID from the data instead \
                      (e.g. `key={item.id}`)."
                .to_string(),
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

/// If `expr` is an `.map(callback)` call expression whose callback has a second
/// parameter, return that parameter's name. Otherwise return `None`.
///
/// Handles both arrow functions and regular function expressions:
/// - `items.map((item, index) => ...)`
/// - `items.map(function(item, i) { ... })`
fn extract_map_index_param(expr: &CallExpression<'_>) -> Option<String> {
    // Must be a member expression callee ending in `.map`.
    let is_map = matches!(
        &expr.callee,
        Expression::StaticMemberExpression(m) if m.property.name.as_str() == "map"
    );

    if !is_map {
        return None;
    }

    // Get the callback (first argument).
    let callback_arg = expr.arguments.first()?;
    let callback_expr = callback_arg.as_expression()?;

    // Extract formal parameters from arrow function or function expression.
    let params = match callback_expr {
        Expression::ArrowFunctionExpression(arrow) => &arrow.params,
        Expression::FunctionExpression(func) => &func.params,
        _ => return None,
    };

    // The index is the second parameter (position 1).
    let second = params.items.get(1)?;

    if let BindingPatternKind::BindingIdentifier(id) = &second.pattern.kind {
        Some(id.name.to_string())
    } else {
        None
    }
}

/// Returns `true` if the JSX attribute name is literally `key`.
fn is_key_prop(name: &JSXAttributeName<'_>) -> bool {
    matches!(name, JSXAttributeName::Identifier(id) if id.name.as_str() == "key")
}
