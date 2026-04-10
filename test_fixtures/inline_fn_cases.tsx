// test_fixtures/inline_fn_cases.tsx
// Exercises every detection path in no_inline_jsx_fn rule.
// Expected: warnings on all ❌ lines, silence on all ✅ lines.

import React, { useCallback, useMemo } from "react";

function InlineFnTestCases() {
  const [active, setActive] = React.useState(false);

  // ── Stable refs (should produce NO warnings) ──────────────────────────────

  // ✅ Plain identifier reference — not inline
  const handleClick = () => setActive(true);
  const handleChange = (e: React.ChangeEvent<HTMLInputElement>) => console.log(e);

  // ✅ Wrapped with useCallback — explicitly memoized
  const memoizedClick = useCallback(() => setActive(true), []);
  const memoizedChange = useCallback((e: any) => console.log(e), []);

  // ✅ Wrapped with React.useCallback — namespace form
  const nsClick = React.useCallback(() => setActive(false), []);

  // ✅ useMemo — also skip (user is intentionally memoizing)
  const memoizedHandler = useMemo(() => () => setActive(true), []);

  return (
    <div>

      {/* ── Direct inline arrow functions (should WARN) ──────────────── */}

      {/* ❌ no_inline_jsx_fn: inline arrow in onClick */}
      <button onClick={() => setActive(true)}>Direct arrow</button>

      {/* ❌ no_inline_jsx_fn: inline arrow with parameter in onChange */}
      <input onChange={(e) => console.log(e.target.value)} />

      {/* ❌ no_inline_jsx_fn: async inline arrow */}
      <button onClick={async () => { await fetch("/api"); }}>Async arrow</button>

      {/* ── Direct inline function expressions (should WARN) ─────────── */}

      {/* ❌ no_inline_jsx_fn: function expression in onChange */}
      <select onChange={function(e) { console.log(e.target.value); }} />

      {/* ❌ no_inline_jsx_fn: named function expression (still inline!) */}
      <button onClick={function handleIt() { setActive(true); }}>Named fn expr</button>

      {/* ── Conditional (ternary) expressions (should WARN) ──────────── */}

      {/* ❌ no_inline_jsx_fn: inline fn in ternary consequent */}
      <button onClick={active ? () => setActive(false) : handleClick}>
        Ternary consequent
      </button>

      {/* ❌ no_inline_jsx_fn: inline fn in ternary alternate */}
      <button onClick={active ? handleClick : () => setActive(true)}>
        Ternary alternate
      </button>

      {/* ❌ no_inline_jsx_fn: inline fn in BOTH branches */}
      <button onClick={active ? () => setActive(false) : () => setActive(true)}>
        Both branches (2 warnings)
      </button>

      {/* ── Logical expressions (should WARN) ────────────────────────── */}

      {/* ❌ no_inline_jsx_fn: inline fn in && right-hand side */}
      <button onClick={active && (() => setActive(false))}>
        Logical AND inline
      </button>

      {/* ❌ no_inline_jsx_fn: inline fn in || right-hand side fallback */}
      <button onClick={handleClick || (() => setActive(true))}>
        Logical OR fallback
      </button>

      {/* ── Parenthesized expressions (should WARN) ──────────────────── */}

      {/* ❌ no_inline_jsx_fn: inline fn wrapped in parens */}
      <button onClick={(() => setActive(true))}>Parenthesized arrow</button>

      {/* ── Stable refs — no warnings ─────────────────────────────────── */}

      {/* ✅ Plain stable reference */}
      <button onClick={handleClick}>Stable ref</button>

      {/* ✅ Stable ref from useCallback */}
      <button onClick={memoizedClick}>Memoized click</button>

      {/* ✅ Ternary with stable refs on both branches — no warning */}
      <button onClick={active ? handleClick : memoizedClick}>
        Ternary stable refs
      </button>

      {/* ✅ React.useCallback namespace form */}
      <button onClick={nsClick}>NS useCallback</button>

      {/* ✅ useMemo-produced handler */}
      <button onClick={memoizedHandler}>UseMemo handler</button>

      {/* ✅ onChange with stable ref */}
      <input onChange={handleChange} />
      <input onChange={memoizedChange} />

    </div>
  );
}

export default InlineFnTestCases;
