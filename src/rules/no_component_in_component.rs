//! rules/no_component_in_component.rs — Detect React components defined inside other components.
//!
//! # What this detects
//!
//! A PascalCase function or arrow component defined inside another component's
//! render body.
//!
//! ```jsx
//! // ❌ Card is recreated as a NEW component type on every Dashboard render.
//! function Dashboard() {
//!   const Card = ({ title }) => <div>{title}</div>;
//!   return <Card title="Hello" />;
//! }
//!
//! // ✅ Move Card outside Dashboard
//! const Card = ({ title }) => <div>{title}</div>;
//! function Dashboard() {
//!   return <Card title="Hello" />;
//! }
//! ```
//!
//! # Why it's a problem
//!
//! React identifies component types by reference equality. When a component
//! function is defined inside another component's render body, a **new function
//! object** is created on every render. React sees a different type → it fully
//! unmounts the old subtree and mounts a fresh one.
//!
//! # AST traversal strategy
//!
//! We scan statement lists manually. Starting from the program body, we detect
//! PascalCase function declarations and const arrow/fn assignments. When found
//! at depth > 0 (i.e., inside another component), we emit. We then recurse
//! into the found function's body at depth+1.

use std::path::Path;

use oxc_allocator::Vec as OxcVec;
use oxc_ast::ast::{BindingPatternKind, Declaration, Expression, FunctionBody, Statement};
use oxc_ast_visit::Visit;
use oxc_span::Span;

use crate::{
    rules::{Issue, RuleContext, Severity},
    utils::offset_to_line_col,
};

// ─── Rule struct ──────────────────────────────────────────────────────────────

pub struct NoComponentInComponent;

impl super::Rule for NoComponentInComponent {
    fn name(&self) -> &str {
        "no_component_in_component"
    }

    fn run(&self, ctx: &RuleContext<'_>) -> Vec<Issue> {
        let mut visitor = ComponentNestingVisitor {
            issues: Vec::new(),
            source_text: ctx.source_text,
            file_path: ctx.file_path,
        };
        // Start scanning from depth=0 (module level).
        visitor.scan_statements(&ctx.program.body, 0);
        visitor.issues
    }
}

// ─── Visitor ──────────────────────────────────────────────────────────────────

struct ComponentNestingVisitor<'a> {
    issues: Vec<Issue>,
    source_text: &'a str,
    file_path: &'a Path,
}

impl<'a> Visit<'a> for ComponentNestingVisitor<'_> {}

impl ComponentNestingVisitor<'_> {
    /// Recursively scan a list of statements.
    ///
    /// `depth` is the number of component-like function scopes we are currently
    /// nested inside. 0 = module level, 1 = inside one component, etc.
    fn scan_statements<'a>(&mut self, stmts: &OxcVec<'a, Statement<'a>>, depth: usize) {
        for stmt in stmts {
            match stmt {
                // function MyComponent() { ... }
                Statement::FunctionDeclaration(func) => {
                    let name = func
                        .id
                        .as_ref()
                        .map(|id| id.name.as_str().to_string())
                        .unwrap_or_default();

                    if is_pascal_case(&name) {
                        if depth > 0 {
                            self.emit(&name, func.span);
                        }
                        if let Some(body) = &func.body {
                            self.scan_function_body(body, depth + 1);
                        }
                    } else if let Some(body) = &func.body {
                        // Non-component function — still scan body at same depth.
                        self.scan_function_body(body, depth);
                    }
                }

                // const MyComponent = () => { ... } or = function() { ... }
                Statement::VariableDeclaration(var_decl) => {
                    for declarator in &var_decl.declarations {
                        let var_name = match &declarator.id.kind {
                            BindingPatternKind::BindingIdentifier(id) => {
                                id.name.as_str().to_string()
                            }
                            _ => String::new(),
                        };

                        if is_pascal_case(&var_name) {
                            if let Some(init) = &declarator.init {
                                match init {
                                    Expression::ArrowFunctionExpression(arrow) => {
                                        if depth > 0 {
                                            self.emit(&var_name, arrow.span);
                                        }
                                        self.scan_function_body(&arrow.body, depth + 1);
                                        continue;
                                    }
                                    Expression::FunctionExpression(func) => {
                                        if depth > 0 {
                                            self.emit(&var_name, func.span);
                                        }
                                        if let Some(body) = &func.body {
                                            self.scan_function_body(body, depth + 1);
                                        }
                                        continue;
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                }

                // For return statements, if/else, blocks, etc. — keep scanning at same depth.
                Statement::ReturnStatement(_)
                | Statement::IfStatement(_)
                | Statement::BlockStatement(_)
                | Statement::ExpressionStatement(_) => {
                    // We don't recurse into control flow for component detection.
                    // Most component definitions are direct statements, not nested in if/for.
                }

                _ => {}
            }
        }
    }

    fn scan_function_body<'a>(&mut self, body: &FunctionBody<'a>, depth: usize) {
        self.scan_statements(&body.statements, depth);
    }

    fn emit(&mut self, name: &str, span: Span) {
        let (line, col) = offset_to_line_col(self.source_text, span.start);
        self.issues.push(Issue {
            rule: "no_component_in_component".to_string(),
            message: format!(
                "Component `{name}` is defined inside another component. \
                 React creates a new component type on every render, causing the subtree \
                 to unmount and remount. Move `{name}` outside the parent component."
            ),
            file: self.file_path.to_path_buf(),
            line,
            column: col,
            severity: Severity::Warning,
        });
    }
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn is_pascal_case(name: &str) -> bool {
    name.starts_with(|c: char| c.is_ascii_uppercase())
}

#[allow(dead_code)]
fn is_component_declaration(decl: &Declaration<'_>) -> bool {
    if let Declaration::FunctionDeclaration(func) = decl {
        if let Some(id) = &func.id {
            return is_pascal_case(id.name.as_str());
        }
    }
    false
}
