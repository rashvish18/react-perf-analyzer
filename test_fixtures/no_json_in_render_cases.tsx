// Test fixture: no_json_in_render rule
// ❌ patterns should fire, ✅ patterns should not

import { useMemo } from 'react';

// ─── ❌ Bad: JSON.parse in JSX prop ───────────────────────────────────────────

function ConfigGrid({ raw }: { raw: string }) {
  // ❌ JSON.parse on every render
  return <DataGrid config={JSON.parse(raw)} />;
}

// ─── ❌ Bad: JSON.stringify in JSX prop ───────────────────────────────────────

function DebugPanel({ state }: { state: object }) {
  // ❌ JSON.stringify on every render
  return <code>{JSON.stringify(state)}</code>;
}

// ─── ❌ Bad: JSON.parse in ternary branch ─────────────────────────────────────

function ConditionalJson({ raw, fallback }: { raw: string; fallback: object }) {
  // ❌ JSON.parse in ternary → fires when raw is truthy
  return <Grid data={raw ? JSON.parse(raw) : fallback} />;
}

// ─── ❌ Bad: JSON.stringify in logical branch ─────────────────────────────────

function LogicalJson({ debug, state }: { debug: boolean; state: object }) {
  return <Debug value={debug && JSON.stringify(state)} />;
}

// ─── ✅ Good: JSON inside useMemo ────────────────────────────────────────────

function GoodConfigGrid({ raw }: { raw: string }) {
  const config = useMemo(() => JSON.parse(raw), [raw]);
  return <DataGrid config={config} />;
}

function GoodDebug({ state }: { state: object }) {
  const serialized = useMemo(() => JSON.stringify(state), [state]);
  return <code>{serialized}</code>;
}

// ─── ✅ Good: string literal props are fine ───────────────────────────────────

function GoodStringProp() {
  return <Component data='{"key": "value"}' />;
}

// Dummy components
declare function DataGrid(props: any): any;
declare function Grid(props: any): any;
declare function Debug(props: any): any;
declare function Component(props: any): any;
