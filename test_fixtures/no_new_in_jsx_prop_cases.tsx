// Test fixture: no_new_in_jsx_prop rule
// ❌ patterns should fire, ✅ patterns should not

import { useMemo } from 'react';

// ─── ❌ Bad: new Date() in prop ───────────────────────────────────────────────

function DateChart({ data }: { data: number[] }) {
  // ❌ New Date on every render
  return <Chart startDate={new Date()} data={data} />;
}

// ─── ❌ Bad: new Map() in prop ────────────────────────────────────────────────

function DataTable({ entries }: { entries: [string, number][] }) {
  // ❌ New Map on every render
  return <Table config={new Map(entries)} />;
}

// ─── ❌ Bad: new custom class in prop ─────────────────────────────────────────

function StyledGrid() {
  // ❌ New StyleSheet instance on every render
  return <Grid theme={new StyleSheet({ color: 'red', padding: 16 })} />;
}

// ─── ❌ Bad: new in ternary ───────────────────────────────────────────────────

function ConditionalNew({ isActive }: { isActive: boolean }) {
  // ❌ Either branch creates a new instance
  return <Widget config={isActive ? new ActiveConfig() : new DefaultConfig()} />;
}

// ─── ❌ Bad: new Set() in prop ────────────────────────────────────────────────

function SetProp({ ids }: { ids: string[] }) {
  return <Filter allowedIds={new Set(ids)} />;
}

// ─── ✅ Good: memoized new expression ────────────────────────────────────────

function GoodDateChart({ data }: { data: number[] }) {
  const startDate = useMemo(() => new Date(), []);
  return <Chart startDate={startDate} data={data} />;
}

// ─── ✅ Good: module-level constant ──────────────────────────────────────────

const STATIC_CONFIG = new Map([['key', 'value']]);

function GoodTable() {
  return <Table config={STATIC_CONFIG} />;
}

// ─── ✅ Good: string/number props are fine ────────────────────────────────────

function GoodPrimitives() {
  return <Button label="Click me" count={42} flag={true} />;
}

// Dummy components
declare function Chart(props: any): any;
declare function Table(props: any): any;
declare function Grid(props: any): any;
declare function Widget(props: any): any;
declare function Filter(props: any): any;
declare function Button(props: any): any;
declare function StyleSheet(config: any): any;
declare function ActiveConfig(): any;
declare function DefaultConfig(): any;
