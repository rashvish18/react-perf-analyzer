// Test fixture: no_useless_memo rule
// ❌ patterns should fire, ✅ patterns should not

import { useMemo, useCallback } from 'react';

// ─── ❌ Bad: useMemo with empty deps ─────────────────────────────────────────

function BadMemoObject() {
  // ❌ Never recomputes — should be a module-level constant
  const config = useMemo(() => ({ theme: 'dark', lang: 'en' }), []);
  return <App config={config} />;
}

function BadMemoString() {
  // ❌ Static string wrapped in useMemo pointlessly
  const BASE_URL = useMemo(() => 'https://api.example.com', []);
  return <Fetcher url={BASE_URL} />;
}

function BadMemoArray() {
  // ❌ Static array — should be module-level constant
  const TABS = useMemo(() => ['Home', 'Profile', 'Settings'], []);
  return <Nav tabs={TABS} />;
}

// ─── ❌ Bad: useCallback with empty deps ──────────────────────────────────────

function BadCallbackNoop() {
  // ❌ Function never changes — should be module-level
  const noop = useCallback(() => {}, []);
  return <Button onClick={noop}>Click</Button>;
}

function BadCallbackLogger() {
  // ❌ Static function — should be module-level
  const logClick = useCallback(() => console.log('clicked'), []);
  return <Button onClick={logClick}>Log</Button>;
}

// ─── ✅ Good: useMemo WITH deps ───────────────────────────────────────────────

function GoodMemo({ items, filter }: { items: string[]; filter: string }) {
  // ✅ Has deps — actually memoizes based on changing values
  const filtered = useMemo(() => items.filter(i => i.includes(filter)), [items, filter]);
  return <List items={filtered} />;
}

// ─── ✅ Good: useCallback WITH deps ──────────────────────────────────────────

function GoodCallback({ onSave, data }: { onSave: (d: any) => void; data: any }) {
  // ✅ Has deps — handler closes over changing value
  const handleSave = useCallback(() => onSave(data), [onSave, data]);
  return <Button onClick={handleSave}>Save</Button>;
}

// ─── ✅ Good: module-level constants (the correct fix) ───────────────────────

const CONFIG = { theme: 'dark', lang: 'en' };
const BASE_URL = 'https://api.example.com';
const TABS = ['Home', 'Profile', 'Settings'];
const noop = () => {};

function GoodStatic() {
  return (
    <div>
      <App config={CONFIG} />
      <Fetcher url={BASE_URL} />
      <Nav tabs={TABS} />
      <Button onClick={noop}>Click</Button>
    </div>
  );
}

// Dummy components
declare function App(props: any): any;
declare function Fetcher(props: any): any;
declare function Nav(props: any): any;
declare function Button(props: any): any;
declare function List(props: any): any;
