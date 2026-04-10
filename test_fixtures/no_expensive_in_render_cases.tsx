// test_fixtures/no_expensive_in_render_cases.tsx
// Exercises every detection path in the no_expensive_in_render rule.
// Run with: react-perf-analyzer ./test_fixtures/no_expensive_in_render_cases.tsx
//
// All ❌ expressions should warn; all ✅ expressions should be silent.

import React, { useMemo } from "react";

// ─────────────────────────────────────────────────────────────────────────────
// ❌ PATTERN 1: .filter() directly in a prop
// ─────────────────────────────────────────────────────────────────────────────

function FilterInRender({ users }) {
  return (
    <div>
      {/* ❌ no_expensive_in_render: .filter() recomputed every render */}
      <UserList users={users.filter((u) => u.active)} />
    </div>
  );
}

// ─────────────────────────────────────────────────────────────────────────────
// ❌ PATTERN 2: .sort() directly in a prop
// ─────────────────────────────────────────────────────────────────────────────

function SortInRender({ scores }) {
  return (
    <div>
      {/* ❌ no_expensive_in_render: .sort() recomputed and mutates every render */}
      <Leaderboard scores={scores.sort((a, b) => b.score - a.score)} />
    </div>
  );
}

// ─────────────────────────────────────────────────────────────────────────────
// ❌ PATTERN 3: .reduce() directly in a prop
// ─────────────────────────────────────────────────────────────────────────────

function ReduceInRender({ items }) {
  return (
    <div>
      {/* ❌ no_expensive_in_render: .reduce() recomputed every render */}
      <Summary total={items.reduce((acc, i) => acc + i.price, 0)} />
    </div>
  );
}

// ─────────────────────────────────────────────────────────────────────────────
// ❌ PATTERN 4: .find() directly in a prop
// ─────────────────────────────────────────────────────────────────────────────

function FindInRender({ docs, activeId }) {
  return (
    <div>
      {/* ❌ no_expensive_in_render: .find() recomputed every render */}
      <Editor doc={docs.find((d) => d.id === activeId)} />
    </div>
  );
}

// ─────────────────────────────────────────────────────────────────────────────
// ❌ PATTERN 5: .findIndex() directly in a prop
// ─────────────────────────────────────────────────────────────────────────────

function FindIndexInRender({ items, selectedId }) {
  return (
    <div>
      {/* ❌ no_expensive_in_render: .findIndex() recomputed every render */}
      <Tabs activeIndex={items.findIndex((item) => item.id === selectedId)} />
    </div>
  );
}

// ─────────────────────────────────────────────────────────────────────────────
// ❌ PATTERN 6: .flatMap() directly in a prop
// ─────────────────────────────────────────────────────────────────────────────

function FlatMapInRender({ groups }) {
  return (
    <div>
      {/* ❌ no_expensive_in_render: .flatMap() recomputed every render */}
      <TagCloud tags={groups.flatMap((g) => g.tags)} />
    </div>
  );
}

// ─────────────────────────────────────────────────────────────────────────────
// ❌ PATTERN 7: Expensive call in a ternary branch
// ─────────────────────────────────────────────────────────────────────────────

function TernaryExpensive({ items, loaded }) {
  return (
    <div>
      {/* ❌ no_expensive_in_render: .filter() inside ternary still runs every render */}
      <List items={loaded ? items.filter((i) => i.visible) : []} />
    </div>
  );
}

// ─────────────────────────────────────────────────────────────────────────────
// ❌ PATTERN 8: Expensive call in a logical expression
// ─────────────────────────────────────────────────────────────────────────────

function LogicalExpensive({ items, isReady }) {
  return (
    <div>
      {/* ❌ no_expensive_in_render: .sort() inside && still runs when isReady is true */}
      <SortedList items={isReady && items.sort()} />
    </div>
  );
}

// ─────────────────────────────────────────────────────────────────────────────
// ✅ PATTERN 9: useMemo-wrapped — no warning
// ─────────────────────────────────────────────────────────────────────────────

function FilterWithMemo({ users }) {
  // ✅ Memoized — only recomputes when `users` changes
  const activeUsers = useMemo(() => users.filter((u) => u.active), [users]);

  return (
    <div>
      <UserList users={activeUsers} />
    </div>
  );
}

// ─────────────────────────────────────────────────────────────────────────────
// ✅ PATTERN 10: useMemo directly in prop — no warning
// ─────────────────────────────────────────────────────────────────────────────

function InlineMemoFilter({ scores }) {
  return (
    <div>
      {/* ✅ useMemo directly inline — recognized as stable */}
      <Leaderboard scores={useMemo(() => scores.sort((a, b) => b - a), [scores])} />
    </div>
  );
}

// ─────────────────────────────────────────────────────────────────────────────
// ✅ PATTERN 11: .map() for rendering — NOT flagged (intentional exclusion)
// ─────────────────────────────────────────────────────────────────────────────

function MapForRendering({ items }) {
  return (
    <ul>
      {/* ✅ .map() for JSX rendering is expected and correct — no warning */}
      {items.map((item) => (
        <li key={item.id}>{item.name}</li>
      ))}
    </ul>
  );
}

// ─────────────────────────────────────────────────────────────────────────────
// ✅ PATTERN 12: Stable variable reference — no warning
// ─────────────────────────────────────────────────────────────────────────────

const SORTED_CONFIG = [1, 2, 3].sort();

function StaticSorted() {
  return (
    <div>
      {/* ✅ Computed once at module level — always same reference */}
      <ConfigList items={SORTED_CONFIG} />
    </div>
  );
}

declare const UserList: React.FC<{ users: any[] }>;
declare const Leaderboard: React.FC<{ scores: any[] }>;
declare const Summary: React.FC<{ total: number }>;
declare const Editor: React.FC<{ doc: any }>;
declare const Tabs: React.FC<{ activeIndex: number }>;
declare const TagCloud: React.FC<{ tags: any[] }>;
declare const List: React.FC<{ items: any[] }>;
declare const SortedList: React.FC<{ items: any }>;
declare const ConfigList: React.FC<{ items: any[] }>;
