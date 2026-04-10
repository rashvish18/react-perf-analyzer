# SAST Expansion Plan — react-perf-analyzer → React Quality + Security Scanner

## Vision

**We are NOT building a general SAST engine. That wheel already exists.**

Instead, `react-perf-analyzer` becomes the **orchestration layer + React specialist**:

```
oxc_linter     → general JS/TS rules       (call it, don't rewrite it)
cargo-audit    → Rust CVEs                 (call it, don't rewrite it)
clippy         → Rust lints                (call it, don't rewrite it)
OUR RULES      → React-specific perf + security gaps nobody else covers
OUR REPORT     → Unified HTML report across ALL tools in one place
OUR CI         → One command, one report, one exit code
```

**Why this is the right call:**
- `oxc_linter` already has `no-eval`, `no-dangerously-set-inner-html`, secrets detection
- `clippy` already flags unsafe blocks and raw pointer misuse
- `cargo-audit` already scans CVEs in Cargo.lock
- Rewriting all of that = 2 years of wheel reinvention

**What nobody has built:**
- React/JSX-specific security rules (XSS via props, unsafe patterns in components)
- A unified report combining perf + security + CVEs in one beautiful HTML output
- Single binary that wraps all these tools with one command, zero config
- Pre-commit mode that runs in <10ms on changed files only

---

## What We Are NOT Changing

- The existing 15 React performance rules stay exactly as-is
- The existing `Rule` trait, `Issue` struct, `Analyzer`, `FileLoader`, `Parser` stay
- The existing HTML / JSON / text reporters stay (we extend them)
- crates.io publishing workflow stays

We are **extending + orchestrating**, not rewriting.

---

## The Architecture We're Building

```
react-perf-analyzer ./src

Internally runs in parallel:
┌─────────────────────────────────────────────────┐
│  Our Engine (oxc-based)                         │
│  ├── 15 React perf rules (existing)             │
│  └── React security rules (NEW — our gap)       │
├─────────────────────────────────────────────────┤
│  oxc_linter (subprocess or library call)        │
│  └── 400+ general JS/TS rules                   │
├─────────────────────────────────────────────────┤
│  cargo-audit (subprocess, if Cargo.lock found)  │
│  └── CVE scanning for Rust deps                 │
└─────────────────────────────────────────────────┘
         ↓ all results merged
  Unified HTML report + SARIF output
  One exit code based on --fail-on threshold
```

---

## Repository Layout After Expansion

```
src/
├── main.rs                  ← orchestrate all tools, --category flag
├── cli.rs                   ← --fail-on, --baseline, --only-changed, --category
├── analyzer.rs              ← unchanged (our oxc rules)
├── file_loader.rs           ← unchanged
├── parser.rs                ← unchanged
├── reporter.rs              ← SARIF output + unified report from all sources
├── orchestrator.rs          ← NEW: runs oxc_linter, cargo-audit, merges results
├── rules/
│   ├── mod.rs
│   ├── perf/                ← existing 15 rules moved here
│   │   ├── mod.rs
│   │   └── ... (all 15 existing rules, unchanged)
│   └── security/            ← NEW: React-specific security only
│       ├── mod.rs
│       └── react/
│           ├── no_xss_via_jsx_prop.rs         ← our unique gap
│           ├── no_dangerously_set_inner_html_unescaped.rs
│           ├── no_hardcoded_secret_in_jsx.rs  ← API keys in JSX props
│           ├── no_unsafe_href.rs              ← javascript: in href
│           └── no_postmessage_wildcard.rs     ← postMessage("*")
test_fixtures/
├── security/react/
.github/
└── workflows/
    └── ci.yml
```

---

## What We Own vs What We Delegate

| Rule Category | Who handles it | Our role |
|---|---|---|
| React perf anti-patterns | **US** (15 existing rules) | Own |
| React security (XSS via JSX, unsafe href) | **US** (new rules) | Own — nobody else has this |
| General JS/TS lint | oxc_linter | Delegate + surface in our report |
| Rust unsafe / lint | clippy | Delegate + surface in our report |
| Rust CVEs | cargo-audit | Delegate + surface in our report |
| Hardcoded secrets | oxc_linter (in progress) | Delegate once they ship |
| SQL injection | oxc_linter | Delegate |

---

## Phases

---

## Phase 1 — Foundation Refactor (Week 1–2)

**Goal**: Restructure codebase to support perf + security + external tool results.
Zero behaviour change for existing users.

### Step 1.1 — Move existing rules into `src/rules/perf/`

```bash
mkdir src/rules/perf
mv src/rules/unstable_props.rs          src/rules/perf/
mv src/rules/no_inline_jsx_fn.rs        src/rules/perf/
mv src/rules/no_array_index_key.rs      src/rules/perf/
mv src/rules/large_component.rs         src/rules/perf/
mv src/rules/no_new_context_value.rs    src/rules/perf/
mv src/rules/no_expensive_in_render.rs  src/rules/perf/
mv src/rules/no_component_in_component.rs src/rules/perf/
mv src/rules/no_unstable_hook_deps.rs   src/rules/perf/
mv src/rules/no_new_in_jsx_prop.rs      src/rules/perf/
mv src/rules/no_use_state_lazy_init_missing.rs src/rules/perf/
mv src/rules/no_json_in_render.rs       src/rules/perf/
mv src/rules/no_object_entries_in_render.rs src/rules/perf/
mv src/rules/no_regex_in_render.rs      src/rules/perf/
mv src/rules/no_math_random_in_render.rs src/rules/perf/
mv src/rules/no_useless_memo.rs         src/rules/perf/
```

Create `src/rules/perf/mod.rs` — re-exports all 15 rules + `perf_rules()` fn.  
Update `src/rules/mod.rs`:

```rust
pub mod perf;
pub mod security;

pub fn all_rules(category: &Category) -> Vec<Box<dyn Rule>> {
    match category {
        Category::Perf     => perf::perf_rules(),
        Category::Security => security::security_rules(),
        Category::All      => {
            let mut r = perf::perf_rules();
            r.extend(security::security_rules());
            r
        }
    }
}
```

### Step 1.2 — Create empty `src/rules/security/` structure

```rust
// src/rules/security/mod.rs
pub mod react;

pub fn security_rules() -> Vec<Box<dyn crate::rules::Rule>> {
    react::react_security_rules()
}

// src/rules/security/react/mod.rs
pub fn react_security_rules() -> Vec<Box<dyn crate::rules::Rule>> {
    vec![]  // fills up in Phase 3
}
```

### Step 1.3 — Add `--category` flag to CLI

```rust
// cli.rs
#[derive(clap::ValueEnum, Clone, Default, PartialEq)]
pub enum Category {
    #[default]
    All,
    Perf,
    Security,
}

#[arg(long, value_enum, default_value = "all",
      help = "Rule category: all | perf | security")]
pub category: Category,
```

### Step 1.4 — Verify CI still passes

```bash
cargo fmt --all
cargo clippy -- -D warnings
cargo test
cargo build --release
./target/release/react-perf-analyzer ./test_fixtures --format text
# Output must be identical to before
```

**Deliverable**: Existing behaviour 100% unchanged. `--category perf` = old behaviour.

---

## Phase 2 — SARIF Output + Unified Issue Model (Week 2–3)

**Goal**: All results (ours + external tools) flow through one `Issue` struct and
emit SARIF for GitHub inline PR annotations.

### Step 2.1 — Extend `Issue` to track source

```rust
pub struct Issue {
    pub rule: String,
    pub message: String,
    pub file: PathBuf,
    pub line: u32,
    pub column: u32,
    pub severity: Severity,
    pub source: IssueSource,   // NEW
    pub category: IssueCategory, // NEW
}

pub enum IssueSource {
    ReactPerfAnalyzer,   // our own rules
    OxcLinter,           // from oxc_linter subprocess
    CargoAudit,          // from cargo-audit subprocess
    Clippy,              // from clippy subprocess
}

pub enum IssueCategory {
    Performance,
    Security,
    Dependency,
}
```

### Step 2.2 — Add SARIF output format

SARIF is the standard format GitHub/GitLab/Azure DevOps understand natively.

```rust
// reporter.rs — add report_sarif()
pub fn report_sarif(issues: &[Issue], version: &str) -> String {
    // Emit SARIF 2.1.0 JSON
    // Each issue → one SARIF result with ruleId, level, message, physicalLocation
}
```

Add `Format::Sarif` to the existing `Format` enum in `cli.rs`.

### Step 2.3 — Wire SARIF in main.rs

```rust
Format::Sarif => {
    let sarif = reporter::report_sarif(&issues, env!("CARGO_PKG_VERSION"));
    let out = args.output.clone().unwrap_or_else(|| "results.sarif".into());
    fs::write(&out, sarif)?;
    eprintln!("✅ SARIF written to: {}", out.display());
}
```

### Step 2.4 — Update HTML report to show source badges

In the HTML rule cards and issue rows, show source badge:
- `[react-perf-analyzer]` — orange
- `[oxc-linter]` — blue
- `[cargo-audit]` — red
- `[clippy]` — yellow

**Deliverable**: `--format sarif` works. GitHub shows inline annotations on PRs.

---

## Phase 3 — React-Specific Security Rules (Week 3–7)

**Goal**: The rules nobody else has — React/JSX-specific security patterns.
These are our differentiation. oxc_linter does not cover these.

### Step 3.1 — `no_unsafe_href`

**What it catches** (XSS via javascript: protocol):
```tsx
<a href={userInput}>click</a>           // href from user input
<a href={`javascript:${handler}`}>      // explicit javascript: injection
<Link to={props.url}>                   // Next.js/React Router with user URL
```

**Why oxc_linter doesn't cover this**: It has a generic `no-script-url` rule but
doesn't understand React component props deeply or `<Link>` from router libraries.

**Implementation**:
- `visit_jsx_opening_element`: check `href`, `to`, `src` attributes
- Value is a non-literal (identifier / template literal / member expression) → flag
- Severity: **High**

**File**: `src/rules/security/react/no_unsafe_href.rs`

### Step 3.2 — `no_xss_via_jsx_prop`

**What it catches** (user data directly in dangerous props):
```tsx
<div title={req.query.msg} />          // reflected XSS via title
<img alt={userInput} />                // alt injection
<input placeholder={userData} />       // content injection
<Component label={props.userContent} /> // arbitrary component props
```

**Why this matters**: These don't cause XSS in most browsers today but are
a common pattern that leads to injection in SSR (Next.js / Remix) environments.

**Implementation**:
- `visit_jsx_attribute`: check if value expression is a member access on
  `req.query`, `req.body`, `req.params`, `searchParams`
- Only flag when the prop name is in a known dangerous list
- Severity: **Medium**

**File**: `src/rules/security/react/no_xss_via_jsx_prop.rs`

### Step 3.3 — `no_hardcoded_secret_in_jsx`

**What it catches** (secrets exposed in JSX / client-side bundles):
```tsx
<ApiProvider key="sk-1234abcdef..." />   // API key in JSX prop
<Script src={`https://maps.google.com/api?key=${API_KEY}`} />
const STRIPE_KEY = "pk_live_xxxxx";      // Stripe publishable key in component
```

**Why this is unique**: oxc_linter's secret detection targets variable declarations.
This rule specifically targets JSX prop values and component-level constants that
get bundled into client-side JS — visible to anyone who views source.

**Implementation**:
- `visit_jsx_attribute`: string literal value with entropy > 3.5
- `visit_variable_declarator` inside React component function bodies
  where name matches secret pattern and the component is exported
- Severity: **High**

**File**: `src/rules/security/react/no_hardcoded_secret_in_jsx.rs`

### Step 3.4 — `no_dangerously_set_inner_html_unescaped`

**What it catches** (more precise than oxc_linter's generic rule):
```tsx
// oxc catches this — we DON'T duplicate:
<div dangerouslySetInnerHTML={{ __html: userInput }} />

// We catch THIS — the sanitized-looking but actually unsafe pattern:
<div dangerouslySetInnerHTML={{ __html: marked(userInput) }} />  // marked() is unsafe
<div dangerouslySetInnerHTML={{ __html: content.replace(/<script>/g,'') }} />  // bypass
```

**Why this is unique**: We know which "sanitizer" functions are actually unsafe.
oxc_linter just flags all dangerouslySetInnerHTML — we're smarter about it.

**Unsafe sanitizer list**: `marked`, `marked.parse`, `showdown`, `sanitizeHtml` (if misconfigured)  
**Safe sanitizer allowlist**: `DOMPurify.sanitize`, `xss`, `sanitize-html` (with safe config)

**File**: `src/rules/security/react/no_dangerously_set_inner_html_unescaped.rs`

### Step 3.5 — `no_postmessage_wildcard`

**What it catches** (cross-origin message security):
```tsx
// In useEffect / event handlers:
window.postMessage(data, "*");           // sends to any origin — dangerous
iframe.contentWindow.postMessage(x, "*");
```

**Why this matters**: Common in micro-frontend architectures (like Walmart's).
Sending messages to `"*"` leaks data to any embedded iframe or parent page.

**Implementation**:
- `visit_call_expression`: callee ends with `.postMessage`
- Second argument is `"*"` string literal → flag
- Severity: **High**

**File**: `src/rules/security/react/no_postmessage_wildcard.rs`

### Step 3.6 — Register all React security rules

```rust
// src/rules/security/react/mod.rs
pub fn react_security_rules() -> Vec<Box<dyn Rule>> {
    vec![
        Box::new(no_unsafe_href::NoUnsafeHref),
        Box::new(no_xss_via_jsx_prop::NoXssViaJsxProp),
        Box::new(no_hardcoded_secret_in_jsx::NoHardcodedSecretInJsx),
        Box::new(no_dangerously_set_inner_html_unescaped::NoDangerouslySetInnerHtmlUnescaped),
        Box::new(no_postmessage_wildcard::NoPostmessageWildcard),
    ]
}
```

### Step 3.7 — Write fixture files

```
test_fixtures/security/react/
  no_unsafe_href_cases.tsx          (bad + good cases)
  no_xss_via_jsx_prop_cases.tsx
  no_hardcoded_secret_in_jsx_cases.tsx
  no_dangerously_set_inner_html_cases.tsx
  no_postmessage_wildcard_cases.tsx
```

Each fixture: 3-5 BAD cases + 2-3 GOOD cases (false positive tests).

---

## Phase 4 — External Tool Orchestration (Week 7–10)

**Goal**: Call oxc_linter, cargo-audit, clippy as subprocesses and merge their
results into our unified report. This gives us 400+ rules for free.

### Step 4.1 — Create `src/orchestrator.rs`

```rust
pub struct OrchestratorResult {
    pub issues: Vec<Issue>,
    pub tools_run: Vec<String>,
    pub tools_failed: Vec<String>,
}

pub fn run_all(path: &Path, category: &Category) -> OrchestratorResult {
    let mut all_issues = vec![];
    let mut tools_run = vec![];
    let mut tools_failed = vec![];

    // Always run our own rules
    // oxc_linter if available in PATH
    // cargo-audit if Cargo.lock exists at path
    // clippy if src/*.rs files found

    OrchestratorResult { issues: all_issues, tools_run, tools_failed }
}
```

### Step 4.2 — oxc_linter integration

```rust
fn run_oxc_linter(path: &Path) -> Vec<Issue> {
    // Check: which oxlint (is it in PATH?)
    let Ok(output) = Command::new("oxlint")
        .args(["--format", "json", path.to_str().unwrap()])
        .output()
    else {
        return vec![];  // not installed — silently skip
    };

    // Parse oxlint JSON output → Vec<Issue> with source: IssueSource::OxcLinter
    parse_oxlint_json(&output.stdout)
}
```

**Key design**: If `oxlint` is not in PATH → silently skip, don't fail.
Show in summary: `⚠ oxlint not found — install with: npm i -g oxlint`

### Step 4.3 — cargo-audit integration

```rust
fn run_cargo_audit(path: &Path) -> Vec<Issue> {
    // Only run if Cargo.lock exists
    if !path.join("Cargo.lock").exists() {
        return vec![];
    }

    let Ok(output) = Command::new("cargo")
        .args(["audit", "--json"])
        .current_dir(path)
        .output()
    else {
        return vec![];  // cargo-audit not installed — skip
    };

    // Parse RUSTSEC advisory JSON → Vec<Issue> with source: IssueSource::CargoAudit
    parse_cargo_audit_json(&output.stdout)
}
```

### Step 4.4 — Update HTML report to show tool breakdown

In the summary stats row, add:
```
[142 issues]  [17 files scanned]  [3 tools ran]
```

In the "Issues by Rule" section, group by source:
```
⚛️  React Performance  (15 rules)    → our rules
🔒  React Security     (5 rules)     → our rules
🔍  General JS/TS     (oxc_linter)   → delegated
📦  Dependencies      (cargo-audit)  → delegated
```

### Step 4.5 — Handle tool unavailability gracefully

```
react-perf-analyzer ./src

✅ Our rules:    12 issues
⚠  oxlint:      not found  (install: npm i -g oxlint)
⚠  cargo-audit: not found  (install: cargo install cargo-audit)

Run with --install-deps to install missing tools automatically
```

Optional `--install-deps` flag that runs the install commands.

---

## Phase 5 — Severity Levels & Pipeline Gates (Week 10–11)

### Step 5.1 — Extend Severity enum

```rust
pub enum Severity {
    Info,
    Low,
    Medium,   // existing perf rules map here
    High,
    Critical,
}
```

Severity mapping:

| Rule/Source | Severity |
|---|---|
| React perf rules (existing 15) | Medium |
| no_unsafe_href | High |
| no_xss_via_jsx_prop | Medium |
| no_hardcoded_secret_in_jsx | High |
| no_dangerously_set_inner_html_unescaped | Critical |
| no_postmessage_wildcard | High |
| cargo-audit CVEs | Critical (CVSS ≥ 9) / High (CVSS ≥ 7) / Medium |
| oxc_linter results | map from their severity |

### Step 5.2 — Add `--fail-on` CLI flag

```rust
#[arg(long, value_enum, default_value = "none")]
pub fail_on: FailOn,

pub enum FailOn { None, Low, Medium, High, Critical }
```

```rust
// main.rs
let exit_code = if issues.iter().any(|i| i.severity >= args.fail_on.threshold()) {
    1
} else {
    0
};
std::process::exit(exit_code);
```

### Step 5.3 — Add `--only-changed` flag

```rust
fn changed_files_from_git() -> Vec<PathBuf> {
    Command::new("git")
        .args(["diff", "--name-only", "--cached"])  // staged files
        .output()
        .map(|o| parse_file_list(&o.stdout))
        .unwrap_or_default()
}
```

`--only-changed` → 8ms pre-commit scan on typical PRs (changed files only).

---

## Phase 6 — Baseline Mode (Week 11–12)

**Goal**: Teams adopt without being blocked by existing issues in legacy code.

### Step 6.1 — Generate baseline

```bash
react-perf-analyzer . --format json --output .sast-baseline.json
git add .sast-baseline.json
git commit -m "chore: add sast baseline"
```

### Step 6.2 — PR check against baseline

```bash
react-perf-analyzer . --baseline .sast-baseline.json --fail-on high
# Only fails if NEW issues (not in baseline) exceed threshold
```

### Step 6.3 — Baseline diff logic

```rust
fn new_issues(current: &[Issue], baseline: &[Issue]) -> Vec<Issue> {
    current.iter().filter(|issue| {
        !baseline.iter().any(|b|
            b.file == issue.file && b.rule == issue.rule && b.line == issue.line
        )
    }).cloned().collect()
}
```

### Step 6.4 — HTML report: new vs existing badge

- 🆕 **New** — introduced in this branch, will block PR
- 📋 **Existing** — in baseline, tracked but not blocking

---

## Phase 7 — CI/CD Integration Package (Week 12–14)

### Step 7.1 — GitHub Actions action.yml

```yaml
# action.yml (repo root)
name: 'react-perf-analyzer'
description: 'React performance + security scanner. Zero config.'
inputs:
  path:        { default: '.' }
  fail-on:     { default: 'high' }
  category:    { default: 'all' }
  baseline:    { default: '' }

runs:
  using: composite
  steps:
    - name: Install react-perf-analyzer
      run: cargo install react-perf-analyzer
      shell: bash

    - name: Scan
      run: |
        react-perf-analyzer ${{ inputs.path }} \
          --format sarif --output results.sarif \
          --fail-on ${{ inputs.fail-on }} \
          --category ${{ inputs.category }} \
          ${{ inputs.baseline != '' && format('--baseline {0}', inputs.baseline) || '' }}
      shell: bash

    - name: Upload SARIF
      uses: github/codeql-action/upload-sarif@v3
      with:
        sarif_file: results.sarif
      if: always()
```

### Step 7.2 — Pre-commit hooks

```yaml
# .pre-commit-hooks.yaml
- id: react-perf-analyzer
  name: react-perf-analyzer (security)
  entry: react-perf-analyzer
  args: [--only-changed, --fail-on, high, --category, security]
  language: rust
  pass_filenames: false

- id: react-perf-analyzer-perf
  name: react-perf-analyzer (perf)
  entry: react-perf-analyzer
  args: [--only-changed, --fail-on, medium, --category, perf]
  language: rust
  pass_filenames: false
```

### Step 7.3 — GitLab CI template

```yaml
# gitlab-ci-template.yml
react-perf-sast:
  stage: test
  script:
    - cargo install react-perf-analyzer
    - react-perf-analyzer . --format gitlab-sast --output gl-sast-report.json
  artifacts:
    reports:
      sast: gl-sast-report.json
```

Add `Format::GitlabSast` to reporter (GitLab has its own JSON schema).

---

## Phase 8 — Custom Rule DSL (Week 14–18)

**Goal**: Teams write project-specific rules in TOML — no Rust required.
This is the ecosystem lock-in, same as Semgrep's rule language.

### Step 8.1 — Rule DSL design

```toml
# .sast-rules/no-walmart-internal-url.toml
[rule]
id       = "no_internal_url_in_source"
severity = "high"
message  = "Internal Walmart URL found in source — remove before deploy"
category = "security"

[[rule.pattern]]
type    = "string_literal"
matches = "walmart\\.internal|corp\\.walmart\\.com"

[[rule.pattern]]
type   = "jsx_prop"
name   = "href"
matches = "walmart\\.internal"
```

```toml
# .sast-rules/no-console-log.toml
[rule]
id       = "no_console_log"
severity = "low"
message  = "console.log() should not be committed — use structured logging"

[[rule.pattern]]
type   = "call_expression"
callee = "console.log"
```

### Step 8.2 — DSL engine

```rust
// src/rules/custom_rule.rs
pub struct CustomRule {
    pub id: String,
    pub severity: Severity,
    pub message: String,
    pub category: IssueCategory,
    pub patterns: Vec<RulePattern>,
}

pub enum RulePattern {
    CallExpression { callee: String },
    StringLiteral  { regex: Regex },
    ImportFrom     { source: String },
    JsxProp        { name: String, value_matches: Option<Regex> },
}
```

### Step 8.3 — Auto-discover custom rules

Search order (highest to lowest priority):
1. `.sast-rules/*.toml` in current directory
2. `.sast-rules/*.toml` in git root
3. `~/.config/react-perf-analyzer/rules/*.toml`

Load and register transparently alongside built-in rules.

### Step 8.4 — New Cargo dependency

```toml
toml  = "0.8"
regex = "1"
```

---

## Phase 9 — Polish & Launch v0.5.0 (Week 18–22)

### Step 9.1 — Update README

- New headline: "React performance + security scanner. Wraps oxlint, cargo-audit, clippy."
- Quick start: `cargo install react-perf-analyzer && react-perf-analyzer ./src`
- Full rule catalog table (perf + security, with severity column)
- CI setup: GitHub Actions in 5 lines
- Badge: `![react-perf-analyzer](https://img.shields.io/badge/security-react--perf--analyzer-green)`

### Step 9.2 — Benchmark post

Run on Walmart monorepo. Compare:
- `react-perf-analyzer` vs Semgrep on same rules
- Show: speed, memory, false positive rate
- Headline: "We scanned 500k LOC of React in 3.2 seconds"

### Step 9.3 — Bump to v0.5.0

```toml
version     = "0.5.0"
description = "React performance + security scanner. Single binary. Zero config. CI-native."
keywords    = ["react", "security", "sast", "performance", "linter"]
```

### Step 9.4 — Community

- `RULES.md` — guide for contributing custom TOML rules
- GitHub Issues labeled `rule-request`
- `rules/community/` folder for user-contributed rule PRs
- dev.to / HN Show HN post

---

## Dependency Changes Summary

```toml
# Phase 8 — custom rule DSL
regex = "1"
toml  = "0.8"
```

**That's it.** No syn. No proc-macro2. No new parser.
We delegate Rust scanning to clippy/cargo-audit instead of reimplementing it.
The oxc stack already handles everything JS/TS/JSX.

---

## What We Explicitly Are NOT Building

| Temptation | Why we skip it |
|---|---|
| General JS/TS security rules (eval, SQLi) | oxc_linter already has these — delegate |
| Rust AST security scanning | clippy already does this — delegate |
| Taint tracking engine | 4 months of work, oxc_linter is already building this |
| Multi-language support (Go, Python, Java) | Out of scope — not React |
| Our own secret scanner | oxc_linter + trufflehog already do this |

---

## Testing Strategy

Each of our React security rules needs:

```
1. Bad cases fixture  (should flag — 3-5 examples)
   test_fixtures/security/react/no_unsafe_href_cases.tsx

2. Good cases fixture (should NOT flag — false positive test)
   test_fixtures/security/react/no_unsafe_href_clean.tsx

3. Edge cases         (tricky patterns — should/shouldn't flag)
   test_fixtures/security/react/no_unsafe_href_edge.tsx
```

External tool tests: mock the subprocess output JSON, verify we parse it correctly.

---

## Timeline Summary

| Phase | What | Weeks | Effort |
|---|---|---|---|
| 1 | Foundation refactor (perf/ + security/ folders, --category) | 1–2 | Low |
| 2 | SARIF output + unified Issue model | 2–3 | Low |
| 3 | 5 React-specific security rules (our unique gap) | 3–7 | Medium |
| 4 | oxc_linter + cargo-audit orchestration | 7–10 | Medium |
| 5 | Severity levels + --fail-on + --only-changed | 10–11 | Low |
| 6 | Baseline mode | 11–12 | Low |
| 7 | GitHub Actions + pre-commit + GitLab CI | 12–14 | Low |
| 8 | Custom rule DSL (TOML) | 14–18 | Medium |
| 9 | Polish + v0.5.0 launch | 18–22 | Low |

**Total: ~5 months solo, ~2.5 months with 2 engineers**  
*(Faster than the original plan because we're not rebuilding oxc_linter)*

---

## Why This Beats the "Build Everything" Approach

| | Build everything from scratch | This plan (orchestrate) |
|---|---|---|
| Time to first useful release | 6+ months | **6 weeks** (Phase 1-3 done) |
| Rule coverage at launch | ~6 rules | **400+ rules** (oxc_linter delegated) |
| Maintenance burden | You maintain every rule | Only maintain React-specific rules |
| Risk of wheel reinvention | High | None |
| Unique value | Low (others already have it) | **High (React specialization)** |

---

## What Makes This Different From Semgrep / oxc_linter

| | Semgrep | oxc_linter | react-perf-analyzer |
|---|---|---|---|
| React perf rules | ❌ None | ❌ None | ✅ 15 rules |
| React security rules | ⚠ Generic only | ❌ None | ✅ JSX-specific |
| Unified perf+security | ❌ | ❌ | ✅ |
| Wraps other tools | ❌ | ❌ | ✅ oxlint + audit + clippy |
| Single binary | ❌ (needs Python) | ✅ | ✅ |
| Beautiful HTML report | ❌ | ❌ | ✅ |
| Zero config | ❌ | ⚠ partial | ✅ |
| Code stays local | ⚠ optional | ✅ | ✅ |

---

*The pivot: we are the React specialist + the orchestration glue.
Not a general SAST engine. That wheel exists. We fill the gap.*
