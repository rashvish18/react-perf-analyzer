// Test fixture: no_unstable_hook_deps rule
// ❌ patterns should fire, ✅ patterns should not

import { useEffect, useMemo, useCallback, useLayoutEffect } from 'react';

// ─── ❌ Bad: object literal in deps ──────────────────────────────────────────

function ObjectInDeps({ userId }: { userId: string }) {
  // ❌ New object on every render → useEffect runs every render
  useEffect(() => {
    fetchUser({ id: userId });
  }, [{ id: userId }]);
  return null;
}

// ─── ❌ Bad: array literal in deps ────────────────────────────────────────────

function ArrayInDeps({ a, b }: { a: number; b: number }) {
  // ❌ New array on every render
  const result = useMemo(() => compute(a, b), [[a, b]]);
  return <div>{result}</div>;
}

// ─── ❌ Bad: arrow function in deps ──────────────────────────────────────────

function FnInDeps({ onLoad }: { onLoad: () => void }) {
  // ❌ Arrow function creates new reference every render
  const handler = useCallback(() => doThing(), [() => helper()]);
  return <button onClick={handler}>Click</button>;
}

// ─── ❌ Bad: object in useLayoutEffect deps ───────────────────────────────────

function LayoutDeps({ config }: { config: object }) {
  useLayoutEffect(() => {
    applyConfig(config);
  }, [{ theme: 'dark' }]);
  return null;
}

// ─── ❌ Bad: function expression in deps ──────────────────────────────────────

function FuncExprInDeps() {
  useEffect(() => {
    doSomething();
  }, [function() { return 42; }]);
  return null;
}

// ─── ✅ Good: stable primitive deps ──────────────────────────────────────────

function GoodDeps({ userId, role }: { userId: string; role: string }) {
  useEffect(() => {
    fetchUser(userId);
  }, [userId, role]);
  return null;
}

// ─── ✅ Good: empty deps array is fine (not unstable, just static) ────────────

function GoodEmptyDeps() {
  useEffect(() => {
    initOnce();
  }, []);
  return null;
}

// ─── ✅ Good: variable refs in deps ──────────────────────────────────────────

function GoodVarDeps({ filters, page }: { filters: string[]; page: number }) {
  const data = useMemo(() => computeData(filters, page), [filters, page]);
  return <div>{data}</div>;
}

// Helpers
declare function fetchUser(arg: any): void;
declare function compute(a: number, b: number): number;
declare function doThing(): void;
declare function helper(): void;
declare function applyConfig(c: object): void;
declare function doSomething(): void;
declare function initOnce(): void;
declare function computeData(f: string[], p: number): any;
