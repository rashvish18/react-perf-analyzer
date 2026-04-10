// test_fixtures/unstable_props_cases.tsx
// Exercises every detection path in the unstable_props rule.
// Run with: react-perf-lint ./test_fixtures/unstable_props_cases.tsx
//
// All ❌ expressions should warn; all ✅ expressions should be silent.

import React, { useMemo, useState } from "react";

// ─────────────────────────────────────────────────────────────────────────────
// ❌ PATTERN 1: Direct object literal in a prop
// ─────────────────────────────────────────────────────────────────────────────

function DirectObjectLiterals() {
  return (
    <div>
      {/* ❌ unstable_props: object literal in 'style' prop */}
      <div style={{ color: "red", fontSize: 14 }} />

      {/* ❌ unstable_props: object literal in 'sx' prop (MUI) */}
      <Box sx={{ p: 2, mt: 1 }} />

      {/* ❌ unstable_props: object literal in generic 'config' prop */}
      <DataGrid config={{ dense: true, striped: false }} />

      {/* ❌ unstable_props: object literal in 'options' prop */}
      <Chart options={{ type: "bar", legend: true }} />
    </div>
  );
}

// ─────────────────────────────────────────────────────────────────────────────
// ❌ PATTERN 2: Direct array literal in a prop
// ─────────────────────────────────────────────────────────────────────────────

function DirectArrayLiterals() {
  return (
    <div>
      {/* ❌ unstable_props: array literal in 'columns' prop */}
      <DataTable columns={["id", "name", "email"]} />

      {/* ❌ unstable_props: array literal in 'items' prop */}
      <List items={[1, 2, 3]} />

      {/* ❌ unstable_props: array literal in 'tags' prop */}
      <TagCloud tags={["react", "typescript", "rust"]} />

      {/* ❌ unstable_props: empty array — still a new reference each render */}
      <Select options={[]} />
    </div>
  );
}

// ─────────────────────────────────────────────────────────────────────────────
// ❌ PATTERN 3: Object/array literal inside a ternary expression
// ─────────────────────────────────────────────────────────────────────────────

function TernaryLiterals({ isActive }: { isActive: boolean }) {
  const stableStyle = { color: "blue" };  // stable reference

  return (
    <div>
      {/* ❌ unstable_props: object literal in ternary consequent */}
      <div style={isActive ? { color: "green" } : stableStyle} />

      {/* ❌ unstable_props: object literal in ternary alternate */}
      <div style={isActive ? stableStyle : { color: "grey" }} />

      {/* ❌ unstable_props: object literal in BOTH branches (2 warnings) */}
      <div style={isActive ? { color: "green" } : { color: "grey" }} />

      {/* ❌ unstable_props: array literal in ternary consequent */}
      <List items={isActive ? ["a", "b"] : []} />
    </div>
  );
}

// ─────────────────────────────────────────────────────────────────────────────
// ❌ PATTERN 4: Object/array literal inside a logical expression
// ─────────────────────────────────────────────────────────────────────────────

function LogicalLiterals({ enabled, debug }: { enabled: boolean; debug: boolean }) {
  return (
    <div>
      {/* ❌ unstable_props: object literal in && right-hand side */}
      <Grid config={enabled && { dense: true }} />

      {/* ❌ unstable_props: object literal in || right-hand side */}
      <Card theme={null || { primary: "blue" }} />

      {/* ❌ unstable_props: object literal in ?? right-hand side */}
      <Modal options={undefined ?? { closeOnEsc: true }} />

      {/* ❌ unstable_props: array literal in && right-hand side */}
      <TagCloud tags={debug && ["debug", "verbose"]} />
    </div>
  );
}

// ─────────────────────────────────────────────────────────────────────────────
// ❌ PATTERN 5: Parenthesized object/array literal
// ─────────────────────────────────────────────────────────────────────────────

function ParenthesizedLiterals() {
  return (
    <div>
      {/* ❌ unstable_props: parenthesized object literal in 'style' */}
      <div style={({ color: "red" })} />

      {/* ❌ unstable_props: parenthesized array literal in 'items' */}
      <List items={(["a", "b", "c"])} />
    </div>
  );
}

// ─────────────────────────────────────────────────────────────────────────────
// ✅ PATTERN 6: Stable references — should produce NO warnings
// ─────────────────────────────────────────────────────────────────────────────

// ✅ Module-level constants are stable across all renders
const COLUMNS = ["id", "name", "email"];
const CARD_STYLE = { color: "red", fontSize: 14 };
const DEFAULT_OPTIONS: Record<string, unknown> = { type: "bar" };

function StableReferences({ isActive }: { isActive: boolean }) {
  // ✅ Component-level stable refs (created once per instance)
  const activeStyle = { color: "green" };  // NOTE: would warn if detected inline but this is a variable ref
  const items = COLUMNS;

  return (
    <div>
      {/* ✅ Module-level constant — always the same reference */}
      <DataTable columns={COLUMNS} />

      {/* ✅ Module-level constant for style */}
      <div style={CARD_STYLE} />

      {/* ✅ Ternary with stable refs on BOTH branches — no warning */}
      <div style={isActive ? CARD_STYLE : DEFAULT_OPTIONS} />

      {/* ✅ Logical with stable ref on right-hand side */}
      <Chart options={isActive && DEFAULT_OPTIONS} />

      {/* ✅ Plain identifier reference */}
      <List items={items} />
    </div>
  );
}

// ─────────────────────────────────────────────────────────────────────────────
// ✅ PATTERN 7: useMemo-wrapped values — developer has intentionally memoized
// ─────────────────────────────────────────────────────────────────────────────

function MemoizedProps({ color, density }: { color: string; density: number }) {
  const memoStyle = useMemo(() => ({ color, fontSize: density * 2 }), [color, density]);
  const memoItems = useMemo(() => ["a", "b", color], [color]);
  const reactMemo = React.useMemo(() => ({ type: "bar" }), []);

  return (
    <div>
      {/* ✅ useMemo reference — developer has memoized */}
      <div style={memoStyle} />

      {/* ✅ useMemo reference for array */}
      <List items={memoItems} />

      {/* ✅ React.useMemo namespace form */}
      <Chart options={reactMemo} />

      {/* ✅ Inline useMemo — top-level useMemo call suppresses warning */}
      <div style={useMemo(() => ({ color }), [color])} />

      {/* ✅ React.useMemo inline */}
      <Chart options={React.useMemo(() => ({ type: "pie" }), [])} />
    </div>
  );
}

// ─────────────────────────────────────────────────────────────────────────────
// ✅ Non-expression attributes — should never warn
// ─────────────────────────────────────────────────────────────────────────────

function NonExpressionAttributes() {
  return (
    <div>
      {/* ✅ Boolean flag — not an expression container */}
      <input disabled />

      {/* ✅ String literal value — not an object/array */}
      <div className="container" />

      {/* ✅ Number — not an object/array */}
      <Grid cols={3} />

      {/* ✅ Identifier (string variable) — not an object/array */}
      <div id="main-content" />
    </div>
  );
}

export default DirectObjectLiterals;
