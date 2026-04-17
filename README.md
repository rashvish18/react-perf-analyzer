# react-perf-analyzer

[![Crates.io](https://img.shields.io/crates/v/react-perf-analyzer.svg)](https://crates.io/crates/react-perf-analyzer)
[![Downloads](https://img.shields.io/crates/d/react-perf-analyzer.svg)](https://crates.io/crates/react-perf-analyzer)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![CI](https://github.com/rashvish18/react-perf-analyzer/actions/workflows/ci.yml/badge.svg)](https://github.com/rashvish18/react-perf-analyzer/actions/workflows/ci.yml)

**React performance + security scanner. Single binary. Zero config. SARIF output.**

> ⚡ Powered by [OXC](https://oxc-project.github.io/) — the fastest JS/TS parser in the ecosystem.

`react-perf-analyzer` is the orchestration layer for React quality:

```
oxc_linter   → 400+ general JS/TS rules       (opt-in via --external)
cargo-audit  → CVE scanning for Rust deps      (opt-in via --external)
OUR RULES    → React-specific perf + security  (always runs — zero config)
OUR REPORT   → Unified HTML + SARIF + AI-ready prompts in one command
```

> By default only the built-in React rules run. Pass `--external` to also invoke
> oxlint and cargo-audit. This keeps scans fast and avoids unexpected failures in
> environments where those tools are not installed.

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

# Also run oxlint + cargo-audit (external tools, off by default)
react-perf-analyzer ./src --external

# Pre-commit mode — only scan changed files (<10 ms)
react-perf-analyzer ./src --only-changed --fail-on high

# Suppress known issues with a baseline
react-perf-analyzer ./src --baseline .sast-baseline.json --fail-on high

# Custom TOML rules (no Rust required)
react-perf-analyzer ./src --rules react-perf-rules.toml

# AI prompt — generate fix prompts for Claude / Copilot / Cursor
react-perf-analyzer ./src --format ai-prompt --output ./ai-fix-prompts/
# → Paste ai-fix-prompts/index.md into your AI assistant to start fixing
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

## AI Prompt Mode (`--format ai-prompt`)

Generate AI-ready fix prompts for every file with issues. Instead of auto-fixing (which is fragile for complex changes), the tool produces rich, self-contained markdown prompts that you paste into **Claude, GitHub Copilot Chat, or Cursor** to guide the AI through fixing each file.

### Single file
```bash
react-perf-analyzer ./src/components/MyComponent.tsx --format ai-prompt
# Writes: ai-fix-prompts.md — paste into your AI assistant
```

### Directory mode (one prompt per file + orchestrator)

> **`--output` is required for directory mode** — you must specify a folder path ending with `/`.
> The tool will create the folder if it doesn't exist.
> Without `--output`, the result is written to a single `ai-fix-prompts.md` file.

```bash
# ✅ Correct — trailing slash tells the tool to use directory mode
react-perf-analyzer ./src --format ai-prompt --output ./ai-fix-prompts/

# What gets created:
#   ./ai-fix-prompts/index.md             ← paste THIS into your AI assistant
#   ./ai-fix-prompts/src_lib_Foo.tsx.md   ← one file per component with issues
#   ./ai-fix-prompts/src_lib_Bar.tsx.md
#   ...

# ❌ Without --output → single file mode (all issues in one file)
react-perf-analyzer ./src --format ai-prompt
# Writes: ai-fix-prompts.md  (may be very large for big codebases)
```

**Only `index.md` needs to be pasted into your AI.** The individual `.md` files are read automatically by the AI as it works through each fix — their full paths are embedded in `index.md` so no manual file hunting is needed.

### What a per-file prompt looks like

Each `.md` file in the output folder is a self-contained fix prompt for one source file. It contains:
- Every issue with line number, rule name, severity, and a plain-English explanation
- The full annotated source code (with `// ← ⚠ Issue N` markers on affected lines)
- An instruction block telling the AI exactly how to fix all issues

Using the built-in `test_fixtures/bad_component.tsx` (15 issues) as an example:

```bash
react-perf-analyzer test_fixtures/bad_component.tsx --format ai-prompt \
  --output ./bad-component-prompts/
```

The generated `bad_component.tsx.md` looks like this:

---

```markdown
# Fix React Issues: `test_fixtures/bad_component.tsx`

> **15 issues found.** Fix all of them without changing component logic or render output.

## Issues

### Issue 1 — Line 22, Col 17 | `unstable_props` | medium
**Why it hurts**: Object/array literals create a new reference each render.
`React.memo` comparisons always fail, causing wasted re-renders.
**Problem**: Object literal in 'style' prop creates a new reference on every render.
Extract to a module-level constant or wrap with useMemo.

### Issue 2 — Line 23, Col 27 | `no_inline_jsx_fn` | medium
**Why it hurts**: A new function object is created on every render.
**Problem**: Inline arrow function in 'onClick' prop — extract to a named handler
or wrap with useCallback.

... (15 issues total) ...

## Full Source Code

> Lines marked with `// ← ⚠ Issue N` show exactly where each issue is.

    1 | import React, { useState } from 'react';
   22 |   <div style={{ color: 'red' }}>          // ← ⚠ Issue 1
   23 |     <Button onClick={() => doThing()} />   // ← ⚠ Issue 2
   ...

## Instructions for AI

You are an expert React developer. Fix ALL 15 issues listed above.
Do not change component logic, prop names, or render output.
Return the complete fixed file.
```

---

Each prompt is sized to fit comfortably within an AI context window (~500–36K tokens depending on file size).

### Orchestrator workflow

The generated `index.md` is an **agentic orchestrator prompt** — paste the entire file into your AI assistant and it will:

1. **Ask 4 intake questions** (scope, severity, PR, skip list) — reply with numbers like `1, 1, 2, 1`
2. **Fix files one at a time** in priority order (security → high-impact → by module)
3. **Auto-validate** after each file: runs `react-perf-analyzer`, TypeScript check, and ESLint — re-fixes any errors automatically
4. **Update checkboxes** in `index.md` as it goes (`[ ]` → `[x] ✅ fixed 2026-04-15`)
5. **Ask before committing** — choose: commit only / commit + PR / review diff first / skip
6. **Track progress** — the same `index.md` acts as a live status board; re-paste it anytime to resume

```bash
# Example: scan a large monorepo
react-perf-analyzer ./libs/item --format ai-prompt --output ./ai-fix-prompts/

# Then paste ai-fix-prompts/index.md into Claude or Copilot Chat
# AI guides you through all 2,618 issues across 674 files — one file at a time
```

The file paths for both the prompt directory and source root are embedded in `index.md`, so the AI always knows exactly where to read and edit files.

---

## Options

| Flag | Default | Description |
|---|---|---|
| `--format` | `text` | Output: `text` \| `json` \| `html` \| `sarif` \| `ai-prompt` |
| `--output <PATH>` | stdout | Write output to file; append `/` for directory mode (ai-prompt only) |
| `--category` | `all` | Rule category: `all` \| `perf` \| `security` |
| `--fail-on` | `none` | Severity gate: `none` \| `low` \| `medium` \| `high` \| `critical` |
| `--external` | off | Also run oxlint (JS/TS rules) + cargo-audit (Rust CVEs) |
| `--only-changed` | off | Only analyze git-changed files (pre-commit mode) |
| `--baseline <FILE>` | — | Suppress known issues; fail only on new regressions |
| `--rules <FILE>` | auto | TOML file with custom lint rules (no Rust required) |
| `--max-component-lines` | `300` | Line threshold for `large_component` rule |
| `--include-tests` | off | Include `*.test.*`, `*.spec.*`, `*.stories.*` files |

---

## CI Integration

### Jenkins / Looper / Buildkite (shell-based CI)

For any CI system that runs shell steps directly, add two steps — download the
binary and run the scan. The tool exits `1` when issues meet `--fail-on`,
which automatically fails the PR check.

```yaml
# Generic shell step — adapt syntax to your CI system
- name: download-react-perf-analyzer
  sh: |
    curl -sf -L -o /usr/local/bin/react-perf-analyzer \
      https://github.com/rashvish18/react-perf-analyzer/releases/latest/download/react-perf-analyzer-linux-amd64
    chmod +x /usr/local/bin/react-perf-analyzer

- name: react-perf-scan
  sh: |
    # Exit code 1 = issues found at/above --fail-on level → blocks PR merge
    react-perf-analyzer ./src --fail-on high --format sarif --output results.sarif
```

A ready-to-use template is available at `.looper.yml.example` in this repo.

### GitHub Actions

```yaml
- name: React Perf + Security Scan
  uses: rashvish18/react-perf-analyzer@v0.5.4
  with:
    path: './src'
    fail-on: 'high'
    upload-sarif: 'true'   # Shows inline PR annotations
```

Or copy the bundled workflow template into your repo:

```bash
curl -o .github/workflows/react-perf-analyzer.yml \
  https://raw.githubusercontent.com/rashvish18/react-perf-analyzer/main/templates/react-perf-analyzer.yml
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
    file: 'templates/gitlab-ci-template.yml'
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
  *(oxlint and cargo-audit tiles show `N/A` when `--external` was not passed — distinguishes "not run" from "zero issues found")*
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
├── reporter.rs         # Text / JSON / HTML / SARIF / AI-prompt output
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
