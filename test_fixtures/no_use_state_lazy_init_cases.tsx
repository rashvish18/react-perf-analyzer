// Test fixture: no_use_state_lazy_init_missing rule
// ❌ patterns should fire, ✅ patterns should not

import { useState } from 'react';

// ─── ❌ Bad: JSON.parse directly in useState ──────────────────────────────────

function DataLoader({ raw }: { raw: string }) {
  // ❌ JSON.parse runs on every render, only used on mount
  const [data, setData] = useState(JSON.parse(raw));
  return <pre>{JSON.stringify(data)}</pre>;
}

// ─── ❌ Bad: JSON.stringify in useState ───────────────────────────────────────

function StateDebug({ initial }: { initial: object }) {
  // ❌ JSON.stringify evaluated every render
  const [debug, setDebug] = useState(JSON.stringify(initial));
  return <code>{debug}</code>;
}

// ─── ❌ Bad: expensive function call with arguments ───────────────────────────

function ItemList({ rawItems }: { rawItems: string }) {
  // ❌ parseItems runs on every render
  const [items, setItems] = useState(parseItems(rawItems));
  return <ul>{items.map((i: string) => <li key={i}>{i}</li>)}</ul>;
}

// ─── ❌ Bad: function call from localStorage ──────────────────────────────────

function ThemeToggle() {
  // ❌ localStorage.getItem runs on every render
  const [theme, setTheme] = useState(localStorage.getItem('theme'));
  return <button onClick={() => setTheme('dark')}>{theme}</button>;
}

// ─── ❌ Bad: buildConfig with arguments ───────────────────────────────────────

function ConfigPanel({ userId }: { userId: string }) {
  // ❌ buildConfig(userId) called every render
  const [config, setConfig] = useState(buildConfig(userId));
  return <div>{config.name}</div>;
}

// ─── ✅ Good: lazy initializer form ──────────────────────────────────────────

function GoodDataLoader({ raw }: { raw: string }) {
  // ✅ JSON.parse only called on mount
  const [data, setData] = useState(() => JSON.parse(raw));
  return <pre>{JSON.stringify(data)}</pre>;
}

function GoodItemList({ rawItems }: { rawItems: string }) {
  // ✅ parseItems only called on mount
  const [items, setItems] = useState(() => parseItems(rawItems));
  return <ul>{items.map((i: string) => <li key={i}>{i}</li>)}</ul>;
}

// ─── ✅ Good: primitive initial values ────────────────────────────────────────

function GoodPrimitives() {
  const [count, setCount] = useState(0);
  const [name, setName] = useState('');
  const [active, setActive] = useState(false);
  return <div>{count} {name} {String(active)}</div>;
}

// ─── ✅ Good: variable reference (not a call) ─────────────────────────────────

function GoodVarInit({ defaultValue }: { defaultValue: number }) {
  // ✅ Variable ref, not a call expression
  const [value, setValue] = useState(defaultValue);
  return <input value={value} onChange={e => setValue(Number(e.target.value))} />;
}

// Helpers
declare function parseItems(s: string): string[];
declare function buildConfig(id: string): { name: string };
