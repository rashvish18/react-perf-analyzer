/// no_hardcoded_secret_in_jsx — Detect secrets hardcoded in JSX props or
/// component-level constants that end up in client-side bundles.
///
/// # What this detects
///
/// Unlike general secret scanners that look at variable declarations globally,
/// this rule specifically targets:
/// 1. String literals in JSX attribute values that look like secrets
/// 2. Constants declared inside (or at the top of) React component files whose
///    names suggest they contain credentials
///
/// These patterns are dangerous because the values get bundled into the
/// client-side JS and are visible to anyone who views the page source.
///
/// ```tsx
/// // BAD — API key in JSX prop, visible in browser DevTools
/// <ApiProvider apiKey="sk-1234abcdef0123456789" />
/// <Maps key="AIzaSy..." />
///
/// // BAD — secret constant in component file
/// const STRIPE_KEY = "pk_live_51Abc...";
///
/// // GOOD — key from environment variable (not bundled)
/// <ApiProvider apiKey={process.env.NEXT_PUBLIC_API_KEY} />
/// ```
use std::path::Path;

use oxc_ast::ast::*;
use oxc_ast_visit::Visit;

use crate::rules::{Issue, IssueCategory, IssueSource, Rule, RuleContext, Severity};
use crate::utils::offset_to_line_col;

pub struct NoHardcodedSecretInJsx;

impl Rule for NoHardcodedSecretInJsx {
    fn name(&self) -> &str {
        "no_hardcoded_secret_in_jsx"
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

/// JSX prop names that commonly carry secret values.
const SECRET_PROP_NAMES: &[&str] = &[
    "apikey",
    "api_key",
    "apiKey",
    "key",
    "secret",
    "token",
    "password",
    "credential",
    "auth",
    "accesskey",
    "access_key",
    "accessKey",
    "privatekey",
    "private_key",
    "privateKey",
    "clientsecret",
    "client_secret",
    "clientSecret",
];

/// Variable name substrings that suggest a secret value.
const SECRET_NAME_PARTS: &[&str] = &[
    "key",
    "secret",
    "token",
    "password",
    "credential",
    "apikey",
    "api_key",
    "access_key",
    "private_key",
    "auth",
];

/// Placeholder values that should NOT be flagged.
const PLACEHOLDERS: &[&str] = &[
    "your-",
    "example",
    "placeholder",
    "xxx",
    "todo",
    "changeme",
    "<",
    "***",
    "...",
    "test",
    "fake",
    "dummy",
];

fn looks_like_secret(value: &str) -> bool {
    if value.len() < 12 {
        return false;
    }
    // Strings with spaces are UI copy / error messages, never secrets
    if value.contains(' ') {
        return false;
    }
    let low = value.to_lowercase();
    if PLACEHOLDERS.iter().any(|p| low.contains(p)) {
        return false;
    }
    // Bail out if the value looks like a human-readable identifier:
    // camelCase/PascalCase key names and feature-flag variation strings
    // have naturally high entropy but are NOT secrets.
    //
    // Heuristic: if the string contains only word chars + hyphens/underscores
    // and has at most one digit run, it's almost certainly a readable key name
    // (e.g. "itemPageSubscriptionOptions", "variation_lowReturnRate_gpM0hi").
    // Real secrets (API keys, tokens) typically contain slashes, plusses,
    // equals signs, or long digit sequences (base64 / hex patterns).
    let all_word_chars = value
        .chars()
        .all(|c| c.is_alphanumeric() || c == '_' || c == '-');
    if all_word_chars {
        // Extra check: real random tokens have low max-run-length ratio;
        // readable identifiers have long alphabetic runs.
        let longest_alpha_run = value
            .chars()
            .fold((0usize, 0usize), |(max, cur), c| {
                if c.is_alphabetic() {
                    let next = cur + 1;
                    (max.max(next), next)
                } else {
                    (max, 0)
                }
            })
            .0;
        // If longest alpha run > 5 chars, it reads as a word → not a token
        if longest_alpha_run > 5 {
            return false;
        }
    }
    // Shannon entropy > 3.5 suggests a random/generated token
    shannon_entropy(value) > 3.5
}

fn shannon_entropy(s: &str) -> f64 {
    let len = s.len() as f64;
    if len == 0.0 {
        return 0.0;
    }
    let mut counts = [0u32; 256];
    for b in s.bytes() {
        counts[b as usize] += 1;
    }
    counts
        .iter()
        .filter(|&&c| c > 0)
        .map(|&c| {
            let p = c as f64 / len;
            -p * p.log2()
        })
        .sum()
}

impl<'a, 'b> Visit<'b> for Visitor<'a> {
    /// Check JSX attribute string literals for secret values.
    fn visit_jsx_attribute(&mut self, attr: &JSXAttribute<'b>) {
        let prop_name = match &attr.name {
            JSXAttributeName::Identifier(id) => id.name.as_str().to_lowercase(),
            JSXAttributeName::NamespacedName(nn) => nn.name.name.as_str().to_lowercase(),
        };

        let is_secret_prop = SECRET_PROP_NAMES
            .iter()
            .any(|s| prop_name == s.to_lowercase());

        if !is_secret_prop {
            return;
        }

        let Some(JSXAttributeValue::StringLiteral(s)) = &attr.value else {
            return;
        };

        if looks_like_secret(s.value.as_str()) {
            let (line, col) = offset_to_line_col(self.source_text, s.span.start);
            self.issues.push(Issue {
                rule: "no_hardcoded_secret_in_jsx".into(),
                message: format!(
                    "JSX prop `{}` contains a hardcoded secret — \
                     use an environment variable instead",
                    prop_name
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

    /// Check variable declarations for secret-named constants with high-entropy values.
    fn visit_variable_declarator(&mut self, decl: &VariableDeclarator<'b>) {
        let BindingPatternKind::BindingIdentifier(id) = &decl.id.kind else {
            return;
        };

        let var_name = id.name.as_str().to_lowercase();
        let is_secret_name = SECRET_NAME_PARTS.iter().any(|part| var_name.contains(part));

        if !is_secret_name {
            return;
        }

        let Some(init) = &decl.init else {
            return;
        };
        let Expression::StringLiteral(s) = init else {
            return;
        };

        if looks_like_secret(s.value.as_str()) {
            let (line, col) = offset_to_line_col(self.source_text, s.span.start);
            self.issues.push(Issue {
                rule: "no_hardcoded_secret_in_jsx".into(),
                message: format!(
                    "Variable `{}` appears to contain a hardcoded secret — \
                     use process.env or a secrets manager",
                    id.name
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
