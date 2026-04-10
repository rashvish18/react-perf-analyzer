// test_fixtures/no_new_context_value_cases.tsx
// Exercises every detection path in the no_new_context_value rule.
// Run with: react-perf-analyzer ./test_fixtures/no_new_context_value_cases.tsx
//
// All ❌ expressions should warn; all ✅ expressions should be silent.

import React, { createContext, useMemo, useCallback, useState } from "react";

const ThemeContext = createContext(null);
const AuthContext = createContext(null);
const UserContext = createContext(null);
const CallbackContext = createContext(null);

// ─────────────────────────────────────────────────────────────────────────────
// ❌ PATTERN 1: Object literal as context value
// ─────────────────────────────────────────────────────────────────────────────

function ObjectLiteralProvider() {
  const [theme, setTheme] = useState("light");
  return (
    // ❌ no_new_context_value: object literal — new object every render
    <ThemeContext.Provider value={{ theme, setTheme }}>
      <App />
    </ThemeContext.Provider>
  );
}

// ─────────────────────────────────────────────────────────────────────────────
// ❌ PATTERN 2: Array literal as context value
// ─────────────────────────────────────────────────────────────────────────────

function ArrayLiteralProvider() {
  const [user, setUser] = useState(null);
  return (
    // ❌ no_new_context_value: array literal — new array every render
    <UserContext.Provider value={[user, setUser]}>
      <App />
    </UserContext.Provider>
  );
}

// ─────────────────────────────────────────────────────────────────────────────
// ❌ PATTERN 3: Inline arrow function as context value
// ─────────────────────────────────────────────────────────────────────────────

function ArrowFnProvider() {
  return (
    // ❌ no_new_context_value: arrow function — new function every render
    <CallbackContext.Provider value={() => console.log("called")}>
      <App />
    </CallbackContext.Provider>
  );
}

// ─────────────────────────────────────────────────────────────────────────────
// ❌ PATTERN 4: Function expression as context value
// ─────────────────────────────────────────────────────────────────────────────

function FnExprProvider() {
  return (
    // ❌ no_new_context_value: function expression — new function every render
    <CallbackContext.Provider value={function() { return 42; }}>
      <App />
    </CallbackContext.Provider>
  );
}

// ─────────────────────────────────────────────────────────────────────────────
// ❌ PATTERN 5: Object literal in a ternary branch
// ─────────────────────────────────────────────────────────────────────────────

function TernaryProvider({ isAdmin }) {
  return (
    // ❌ no_new_context_value: object literal in the ternary consequent
    <AuthContext.Provider value={isAdmin ? { role: "admin", level: 5 } : defaultAuth}>
      <App />
    </AuthContext.Provider>
  );
}

// ─────────────────────────────────────────────────────────────────────────────
// ❌ PATTERN 6: Object literal in a logical expression
// ─────────────────────────────────────────────────────────────────────────────

function LogicalProvider({ override }) {
  return (
    // ❌ no_new_context_value: object literal in logical expression
    <ThemeContext.Provider value={override || { theme: "default" }}>
      <App />
    </ThemeContext.Provider>
  );
}

// ─────────────────────────────────────────────────────────────────────────────
// ✅ PATTERN 7: useMemo-wrapped object — no warning
// ─────────────────────────────────────────────────────────────────────────────

function MemoizedProvider() {
  const [theme, setTheme] = useState("light");

  // ✅ Stable reference via useMemo
  const contextValue = useMemo(() => ({ theme, setTheme }), [theme]);

  return (
    <ThemeContext.Provider value={contextValue}>
      <App />
    </ThemeContext.Provider>
  );
}

// ─────────────────────────────────────────────────────────────────────────────
// ✅ PATTERN 8: useMemo directly in prop — no warning
// ─────────────────────────────────────────────────────────────────────────────

function InlineMemoProvider() {
  const [user, setUser] = useState(null);
  return (
    // ✅ useMemo directly inline — recognized as stable
    <UserContext.Provider value={useMemo(() => ({ user, setUser }), [user])}>
      <App />
    </UserContext.Provider>
  );
}

// ─────────────────────────────────────────────────────────────────────────────
// ✅ PATTERN 9: Stable variable reference — no warning
// ─────────────────────────────────────────────────────────────────────────────

const STATIC_THEME = { theme: "light" };

function StaticProvider() {
  return (
    // ✅ Module-level constant — always the same reference
    <ThemeContext.Provider value={STATIC_THEME}>
      <App />
    </ThemeContext.Provider>
  );
}

// ─────────────────────────────────────────────────────────────────────────────
// ✅ PATTERN 10: Plain identifier (stable ref) — no warning
// ─────────────────────────────────────────────────────────────────────────────

function IdentifierProvider({ value }) {
  return (
    // ✅ Identifier passed directly — may or may not be stable, but we don't warn
    <ThemeContext.Provider value={value}>
      <App />
    </ThemeContext.Provider>
  );
}

declare const App: React.FC;
declare const defaultAuth: { role: string; level: number };
