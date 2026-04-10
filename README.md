# react-perf-analyzer

[![Crates.io](https://img.shields.io/crates/v/react-perf-analyzer.svg)](https://crates.io/crates/react-perf-analyzer)
[![Downloads](https://img.shields.io/crates/d/react-perf-analyzer.svg)](https://crates.io/crates/react-perf-analyzer)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![CI](https://github.com/rashvish18/react-perf-analyzer/actions/workflows/ci.yml/badge.svg)](https://github.com/rashvish18/react-perf-analyzer/actions/workflows/ci.yml)

A high-performance Rust CLI that detects React performance anti-patterns in JS/TS/JSX files using deep AST analysis.

> ⚡ Powered by [OXC](https://oxc-project.github.io/) — the fastest JS/TS parser in the ecosystem.

Built with [OXC](https://oxc-project.github.io/) (Rust-native JS/TS parser), [Rayon](https://github.com/rayon-rs/rayon) for parallel file processing, and [clap](https://github.com/clap-rs/clap) for the CLI.

---

## Rules

| Rule | What it detects |
|---|---|
| `no_inline_jsx_fn` | Inline arrow/function expressions in JSX props — creates a new function reference on every render |
| `unstable_props` | Object/array literals in JSX props — creates a new reference on every render |
| `large_component` | React components exceeding a configurable logical-line threshold |
| `no_new_context_value` | Object/array/function passed as `Context.Provider value` — causes all consumers to re-render |
| `no_array_index_key` | Array index used as JSX `key` prop — breaks reconciliation on list mutations |
| `no_expensive_in_render` | `.sort()` / `.filter()` / `.reduce()` / `.find()` / `.flatMap()` called directly in JSX props without `useMemo` |

---

## Installation

### From crates.io (recommended)

```bash
cargo install react-perf-analyzer
```

### Build from source

```bash
git clone https://github.com/rashvish18/react-perf-analyzer
cd react-perf-analyzer
cargo build --release
```

The binary is at `./target/release/react-perf-analyzer`.

---

## Usage

```
react-perf-analyzer [OPTIONS] <PATH>
```

### Arguments

| Argument | Description |
|---|---|
| `<PATH>` | File or directory to scan (recursively) |

### Options

| Flag | Default | Description |
|---|---|---|
| `--format <FORMAT>` | `text` | Output format: `text`, `json`, or `html` |
| `--output <FILE>` | stdout | Write output to a file instead of stdout |
| `--max-component-lines <N>` | `300` | Line threshold for the `large_component` rule |
| `--include-tests` | off | Include `*.test.*`, `*.spec.*`, `*.stories.*` files |

---

## Examples

### Scan a single file

```bash
react-perf-analyzer src/components/UserCard.tsx
```

### Scan a full project

```bash
react-perf-analyzer ./src
```

### Scan an entire monorepo

```bash
react-perf-analyzer /path/to/monorepo
```

`node_modules`, `dist`, `build`, and hidden directories are automatically skipped.

### Tune the component size threshold

```bash
react-perf-analyzer ./src --max-component-lines 150
```

### JSON output (for CI or tooling)

```bash
react-perf-analyzer ./src --format json > results.json
```

### HTML report (shareable, self-contained)

```bash
# Generates react-perf-report.html in the current directory
react-perf-analyzer ./src --format html

# Custom output path
react-perf-analyzer ./src --format html --output /tmp/my-report.html
```

The HTML report includes:
- **Summary cards** — total issues, files scanned, files with issues
- **Per-rule breakdown** — colour-coded issue counts for each rule
- **Top 10 files bar chart** — quickly spot the worst offenders
- **Collapsible issue table** — all issues grouped by file with inline rule badges

No CDN dependencies — the output is a single self-contained `.html` file.

---

## Sample output

### Text format (default)

```
src/components/UserCard.tsx:14:17  warning  unstable_props          Object literal in 'style' prop creates a new reference on every render.
src/components/UserCard.tsx:21:24  warning  no_inline_jsx_fn        Inline arrow function in 'onClick' prop. Wrap with useCallback.
src/pages/Dashboard.tsx:1:1        warning  large_component         Component 'Dashboard' is 340 lines (310 logical) — limit is 300.
src/contexts/Theme.tsx:12:30       warning  no_new_context_value    Context Provider 'value' receives a new object literal on every render.
src/lists/UserList.tsx:8:18        warning  no_array_index_key      Array index used as 'key' prop.
src/tables/DataGrid.tsx:45:30      warning  no_expensive_in_render  `.filter()` called directly in render — wrap with useMemo.

✖ 6 issues found

Scanned 42 file(s), found 6 issue(s).
```

### HTML report

```bash
react-perf-analyzer ./src --format html --output report.html
# ✅ HTML report written to: report.html
```

Open `report.html` in any browser — share it with your team, attach it to a Jira ticket, or archive it as a performance snapshot.

---

## Rule details

### `no_inline_jsx_fn`

Detects inline functions passed as JSX props that create a new reference on every render, breaking `React.memo` and `shouldComponentUpdate` optimizations.

**Detects:**
- `onClick={() => doSomething()}`
- `onChange={function(e) { ... }}`
- Functions inside ternaries: `onClick={flag ? () => a() : () => b()}`

**Ignores:** `useCallback`-wrapped and `useMemo`-wrapped functions

**Fix:**
```jsx
// ❌ Before
<Button onClick={() => handleDelete(id)} />

// ✅ After
const handleDelete = useCallback(() => deleteItem(id), [id]);
<Button onClick={handleDelete} />
```

---

### `unstable_props`

Detects object and array literals passed directly as JSX prop values. In JavaScript, `{a:1} === {a:1}` is `false` — a new reference on every render defeats `React.memo`.

**Detects:** `style={{ color: "red" }}`, `columns={["id", "name"]}`, literals in ternaries / logical expressions

**Ignores:** `useMemo`-wrapped values, stable variable references

**Fix:**
```jsx
// ❌ Before
<DataTable columns={["id", "name"]} style={{ fontSize: 14 }} />

// ✅ After
const COLUMNS = ["id", "name"];
const style = useMemo(() => ({ fontSize }), [fontSize]);
<DataTable columns={COLUMNS} style={style} />
```

---

### `large_component`

Flags React components whose logical line count exceeds the configured threshold (default 300). Reports total lines, logical lines, JSX element count, and hook count.

**Fix:** Extract sub-components and custom hooks by concern.

---

### `no_new_context_value`

Detects object/array literals or inline functions passed as the `value` prop to a React Context Provider. Every render creates a new reference → all consumers re-render even when the data hasn't changed.

**Detects:**
```jsx
// ❌ New object every render → ALL consumers re-render
<ThemeContext.Provider value={{ theme, toggle }} />

// ❌ New array every render
<UserContext.Provider value={[user, setUser]} />
```

**Fix:**
```jsx
// ✅ Stable reference via useMemo
const value = useMemo(() => ({ theme, toggle }), [theme]);
<ThemeContext.Provider value={value} />
```

---

### `no_array_index_key`

Detects `.map()` callbacks that use the array index as the JSX `key` prop. When items are inserted, removed, or reordered, React matches elements by key — an index key causes incorrect component reuse and broken state (inputs, focus, animations).

**Detects:**
```jsx
// ❌ index as key
items.map((item, index) => <li key={index}>{item.name}</li>)
items.map((item, i) => <Row key={i} />)
items.map((item, idx) => <Card key={`card-${idx}`} />)
```

**Fix:**
```jsx
// ✅ Stable ID from data
items.map((item) => <li key={item.id}>{item.name}</li>)
```

---

### `no_expensive_in_render`

Detects expensive array operations (`.sort()`, `.filter()`, `.reduce()`, `.find()`, `.findIndex()`, `.flatMap()`) called directly in JSX attribute values without `useMemo`. These recompute on every render, including renders triggered by unrelated state changes.

**Detects:**
```jsx
// ❌ filter() runs on every render
<UserList users={allUsers.filter(u => u.active)} />

// ❌ sort() runs and mutates on every render
<Leaderboard scores={scores.sort((a, b) => b - a)} />

// ❌ Even inside a ternary
<List items={loaded ? items.filter(isVisible) : []} />
```

**Fix:**
```jsx
// ✅ Memoized — only recomputes when allUsers changes
const activeUsers = useMemo(() => allUsers.filter(u => u.active), [allUsers]);
<UserList users={activeUsers} />

// ✅ Or inline useMemo in the prop
<Leaderboard scores={useMemo(() => [...scores].sort((a, b) => b - a), [scores])} />
```

---

## Exit codes

| Code | Meaning |
|---|---|
| `0` | No issues found |
| `1` | One or more issues found |
| `2` | Fatal error (path not found, write error) |

CI usage:

```bash
react-perf-analyzer ./src || echo "Performance issues detected — failing build"
```

---

## Architecture

```
src/
├── main.rs          # Entry point — Rayon parallel file pipeline
├── cli.rs           # clap CLI definitions (path, format, output, thresholds)
├── file_loader.rs   # Recursive file discovery (walkdir)
├── parser.rs        # OXC JS/TS/JSX parser wrapper
├── analyzer.rs      # Runs all rules against a parsed AST
├── reporter.rs      # Text, JSON, and HTML output formatters
├── utils.rs         # Byte offset → line/column helper
└── rules/
    ├── mod.rs                    # Rule trait, Issue/Severity types, registry
    ├── no_inline_jsx_fn.rs       # Rule 1
    ├── unstable_props.rs         # Rule 2
    ├── large_component.rs        # Rule 3
    ├── no_new_context_value.rs   # Rule 4
    ├── no_array_index_key.rs     # Rule 5
    └── no_expensive_in_render.rs # Rule 6
```

**Key dependencies:**

| Crate | Purpose |
|---|---|
| `oxc_parser` / `oxc_ast` / `oxc_ast_visit` | Rust-native JS/TS/JSX parser and AST |
| `rayon` | Data-parallel file processing |
| `walkdir` | Recursive directory traversal |
| `clap` | CLI argument parsing |
| `serde` / `serde_json` | JSON output serialization |

---

## Test fixtures

```bash
# Rule 1 — inline functions
react-perf-analyzer ./test_fixtures/inline_fn_cases.tsx

# Rule 2 — unstable props
react-perf-analyzer ./test_fixtures/unstable_props_cases.tsx

# Rule 3 — large components
react-perf-analyzer ./test_fixtures/large_component_cases.tsx --max-component-lines 20

# Rule 4 — context value
react-perf-analyzer ./test_fixtures/no_new_context_value_cases.tsx

# Rule 5 — array index key
react-perf-analyzer ./test_fixtures/no_array_index_key_cases.tsx

# Rule 6 — expensive in render
react-perf-analyzer ./test_fixtures/no_expensive_in_render_cases.tsx

# Zero issues — stable, well-written component
react-perf-analyzer ./test_fixtures/good_component.tsx
```
