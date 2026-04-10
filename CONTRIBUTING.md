# Contributing to react-perf-analyzer

Thank you for your interest in contributing! This document explains how to get started, add new rules, and submit changes.

---

## Prerequisites

- [Rust](https://rustup.rs/) stable (1.75+)
- `cargo` in your `PATH`

```bash
rustup update stable
rustup component add clippy rustfmt
```

---

## Building

```bash
# Debug build (fast compile, slower binary)
cargo build

# Release build (LTO enabled, production speed)
cargo build --release
```

The binary lands at `target/release/react-perf-analyzer`.

---

## Running tests

```bash
# Unit tests only
cargo test

# With output (useful for debugging)
cargo test -- --nocapture
```

Test fixtures live in `test_fixtures/`. Each file is a self-contained TSX file designed to trigger (or explicitly not trigger) specific rules.

---

## Linting & formatting

CI enforces both. Run locally before pushing:

```bash
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
```

---

## Adding a new rule

1. **Create** `src/rules/my_rule.rs`
2. **Implement** the `Rule` trait (see `src/rules/mod.rs`):

```rust
use crate::rules::{Issue, Rule, Severity};
use oxc_ast::ast::Program;

pub struct MyRule;

impl Rule for MyRule {
    fn name(&self) -> &'static str { "my-rule" }

    fn run(&self, program: &Program, source: &str, path: &str) -> Vec<Issue> {
        // Walk the AST and collect Issues
        vec![]
    }
}
```

3. **Register** it in `src/rules/mod.rs` inside `all_rules()`:

```rust
pub fn all_rules() -> Vec<Box<dyn Rule>> {
    vec![
        Box::new(no_inline_jsx_fn::NoInlineJsxFn),
        Box::new(unstable_props::UnstableProps),
        Box::new(large_component::LargeComponent::default()),
        Box::new(my_rule::MyRule),  // ← add here
    ]
}
```

4. **Add a fixture file** under `test_fixtures/` covering both passing and failing cases.
5. **Write unit tests** directly in your rule file under `#[cfg(test)]`.

---

## Submitting a PR

1. Fork the repository and create a branch: `git checkout -b feat/my-rule`
2. Make your changes
3. Ensure `cargo test`, `cargo fmt`, and `cargo clippy` all pass cleanly
4. Open a PR against `main` with a clear description of what the rule detects and why it matters for React performance

---

## Project layout

```
src/
├── main.rs             Entry point, Rayon parallel pipeline, exit codes
├── cli.rs              clap argument definitions
├── file_loader.rs      Recursive file discovery with smart filtering
├── parser.rs           OXC parser wrapper
├── analyzer.rs         Rule dispatcher
├── reporter.rs         Text and JSON output formatters
├── utils.rs            Byte offset → (line, col) conversion
└── rules/
    ├── mod.rs          Rule trait, Issue/Severity types, rule registry
    ├── no_inline_jsx_fn.rs
    ├── unstable_props.rs
    └── large_component.rs
```

---

## Code style

- Keep functions small and focused; prefer early returns
- All public items must have doc comments
- Avoid `unwrap()` in non-test code — use `?`, `unwrap_or_default()`, or explicit error handling
- No `unsafe` blocks without a clear justification and a `// SAFETY:` comment
