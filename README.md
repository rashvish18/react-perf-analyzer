# react-perf-analyzer

[![Crates.io](https://img.shields.io/crates/v/react-perf-analyzer.svg)](https://crates.io/crates/react-perf-analyzer)
[![Downloads](https://img.shields.io/crates/d/react-perf-analyzer.svg)](https://crates.io/crates/react-perf-analyzer)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![CI](https://github.com/rashvish18/react-perf-analyzer/actions/workflows/ci.yml/badge.svg)](https://github.com/rashvish18/react-perf-analyzer/actions/workflows/ci.yml)

**React performance + security scanner. Single binary. Zero config. SARIF output.**

> ⚡ Powered by [OXC](https://oxc-project.github.io/) — the fastest JS/TS parser in the ecosystem.

`react-perf-analyzer` is the orchestration layer for React quality:

```
oxc_linter   → 400+ general JS/TS rules       (called automatically)
cargo-audit  → CVE scanning for Rust deps      (called if Cargo.lock found)
OUR RULES    → React-specific perf + security  (what no other tool covers)
OUR REPORT   → Unified HTML + SARIF across all sources in one command
```

---

## Installation

```bash
cargo install react-perf-analyzer
```

Or download a pre-built binary from [Releases](https://github.com/rashvish18/react-perf-analyzer/releases).

---

## Quick start

```bash
# Scan a project (text output)
react-perf-analyzer ./src

# HTML report (auto-opens in browser)
react-perf-analyzer ./src --format html

# SARIF output for GitHub/GitLab inline PR annotations
react-perf-analyzer ./src --format sarif --output results.sarif

# CI gate — fail only on high/critical issues
react-perf-analyzer ./src --fail-on high

# Pre-commit mode — only scan changed files (<10 ms)
react-perf-analyzer ./src --only-changed --fail-on high

# Suppress known issues with a baseline
react-perf-analyzer ./src --baseline .sast-baseline.json --fail-on high

# Custom TOML rules (no Rust required)
react-perf-analyzer ./src --rules react-perf-rules.toml
```

---

## Rules

### Performance (15 rules)

| Rule | What it detects |
|---|---|
| `no_inline_jsx_fn` | Inline arrow/function expressions in JSX props |
| `unstable_props` | Object/array literals in JSX props (new ref every render) |
| `large_component` | Components exceeding configurable line threshold |
| `no_new_context_value` | Object/array/function in Context.Provider value |
| `no_array_index_key` | Array index used as JSX `key` prop |
| `no_expensive_in_render` | `.sort()/.filter()/.reduce()` in JSX props without `useMemo` |
| `no_component_in_component` | Component definitions nested inside another component |
| `no_unstable_hook_deps` | Unstable objects/arrays in `useEffect`/`useCallback` deps array |
| `no_new_in_jsx_prop` | `new` expressions in JSX props |
| `no_use_state_lazy_init_missing` | `useState(expensiveCall())` without lazy initializer |
| `no_json_in_render` | `JSON.parse()` / `JSON.stringify()` inside render |
| `no_object_entries_in_render` | `Object.entries()` / `Object.keys()` without `useMemo` |
| `no_regex_in_render` | RegExp literals created in render |
| `no_math_random_in_render` | `Math.random()` called on every render |
| `no_useless_memo` | `useMemo` around a primitive value |

### Security (5 rules)

| Rule | Severity | What it detects |
|---|---|---|
| `no_unsafe_href` | Critical/Medium | `javascript:` URLs and dynamic `href`/`src`/`action` props |
| `no_xss_via_jsx_prop` | High | Unescaped `req.query`/`req.body`/`req.params` in JSX props |
| `no_hardcoded_secret_in_jsx` | High | High-entropy secrets in JSX props and variable declarations |
| `no_dangerously_set_inner_html_unescaped` | High | `dangerouslySetInnerHTML` without a safe sanitizer |
| `no_postmessage_wildcard` | Medium | `postMessage(data, "*")` without origin restriction |

---

## Options

| Flag | Default | Description |
|---|---|---|
| `--format` | `text` | Output: `text` \| `json` \| `html` \| `sarif` |
| `--output <FILE>` | stdout | Write output to file |
| `--category` | `all` | Rule category: `all` \| `perf` \| `security` |
| `--fail-on` | `none` | Severity gate: `none` \| `low` \| `medium` \| `high` \| `critical` |
| `--only-changed` | off | Only analyze git-changed files (pre-commit mode) |
| `--baseline <FILE>` | — | Suppress known issues; fail only on new regressions |
| `--rules <FILE>` | auto | TOML file with custom lint rules (no Rust required) |
| `--max-component-lines` | `300` | Line threshold for `large_component` rule |
| `--include-tests` | off | Include `*.test.*`, `*.spec.*`, `*.stories.*` files |

---

## CI Integration

### GitHub Actions

```yaml
- name: React Perf + Security Scan
  uses: rashvish18/react-perf-analyzer@v0.5
  with:
    path: './src'
    fail-on: 'high'
    upload-sarif: 'true'   # Shows inline PR annotations
```

Or use the bundled workflow directly:

```yaml
# .github/workflows/scan.yml
jobs:
  scan:
    uses: rashvish18/react-perf-analyzer/.github/workflows/react-perf-analyzer.yml@main
```

### pre-commit hook

```bash
pip install pre-commit
# Copy .pre-commit-config.yaml from this repo into your project
pre-commit install
```

The hook runs `--only-changed` so only modified files are scanned on each commit.

### GitLab CI

```yaml
include:
  - project: 'rashvish18/react-perf-analyzer'
    file: '.github/workflows/gitlab-ci-template.yml'
```

---

## Baseline Mode

Suppress known issues so CI only fails on new regressions:

```bash
# 1. Generate baseline (commit this file)
react-perf-analyzer ./src --format json --output .sast-baseline.json

# 2. Use in CI
react-perf-analyzer ./src --baseline .sast-baseline.json --fail-on high
```

---

## Custom Rules (TOML DSL)

Define team-specific rules without writing Rust. Create `react-perf-rules.toml`:

```toml
[[rule]]
id        = "no-console-log"
message   = "Remove console.log() before merging"
severity  = "medium"
category  = "perf"
pattern   = "console\\.log\\s*\\("
file_glob = "src/**/*.{ts,tsx}"
ignore_if = "//\\s*nolint"

[[rule]]
id       = "no-inner-html"
message  = "Direct innerHTML causes XSS — use DOMPurify"
severity = "high"
category = "security"
pattern  = "\\.innerHTML\\s*="
```

The file is auto-discovered in your project root. See `react-perf-rules.toml.example` for more examples.

---

## HTML Report

The self-contained HTML report includes:

- **6 stat tiles** — Total Issues, Files Scanned, Files with Issues, React Rules, oxlint, cargo-audit
- **Per-rule cards** — click to filter the issue table
- **Top 10 files bar chart** — click bars to jump to file section
- **Collapsible issue table** — severity + source badge on every row
- **Search** — filter by filename in real time

```bash
react-perf-analyzer ./src --format html
# ✅ HTML report written to: react-perf-report.html  (auto-opens on macOS)
```

---

## Exit codes

| Code | Meaning |
|---|---|
| `0` | No issues (or all below `--fail-on` threshold) |
| `1` | Issues found at or above `--fail-on` threshold |
| `2` | Fatal error (path not found, write error) |

---

## Architecture

```
src/
├── main.rs             # Entry point — parallel pipeline + orchestration
├── cli.rs              # clap CLI flags
├── file_loader.rs      # Recursive file discovery (walkdir)
├── parser.rs           # OXC JS/TS/JSX parser wrapper
├── analyzer.rs         # Runs built-in rules against parsed AST
├── orchestrator.rs     # Runs oxlint + cargo-audit as subprocesses
├── baseline.rs         # Baseline load/filter (suppress known issues)
├── changed_files.rs    # Git-modified file detection (--only-changed)
├── custom_rules.rs     # TOML rule DSL engine (regex line scanner)
├── reporter.rs         # Text / JSON / HTML / SARIF output
├── utils.rs            # Byte offset → line/column
└── rules/
    ├── mod.rs          # Rule trait, Issue, Severity, IssueSource
    ├── perf/           # 15 React performance rules
    └── security/
        └── react/      # 5 React security rules
```

---

## Contributing

```bash
git clone https://github.com/rashvish18/react-perf-analyzer
cd react-perf-analyzer
cargo build
cargo test
cargo clippy -- -D warnings
```

PRs welcome! See `SAST_PLAN.md` for the roadmap.
