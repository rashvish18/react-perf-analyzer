// Test fixture: no_object_entries_in_render rule
// ❌ patterns should fire, ✅ patterns should not

import { useMemo } from 'react';

// ─── ❌ Bad: Object.entries in prop ───────────────────────────────────────────

const countryMap = { US: 'United States', CA: 'Canada', GB: 'Great Britain' };

function CountrySelect() {
  // ❌ Object.entries returns a new array on every render
  return <Select options={Object.entries(countryMap)} />;
}

// ─── ❌ Bad: Object.keys in prop ─────────────────────────────────────────────

function ConfigList({ config }: { config: Record<string, unknown> }) {
  // ❌ New array reference every render
  return <List items={Object.keys(config)} />;
}

// ─── ❌ Bad: Object.values in prop ────────────────────────────────────────────

function ValueTable({ dataMap }: { dataMap: Record<string, number> }) {
  // ❌ New array on every render → child always re-renders
  return <Table rows={Object.values(dataMap)} />;
}

// ─── ❌ Bad: Object.entries in ternary branch ─────────────────────────────────

function ConditionalEntries({ isAdmin, adminMap, userMap }: {
  isAdmin: boolean;
  adminMap: Record<string, string>;
  userMap: Record<string, string>;
}) {
  return <Nav items={isAdmin ? Object.entries(adminMap) : Object.entries(userMap)} />;
}

// ─── ❌ Bad: Object.keys in logical expression ────────────────────────────────

function LogicalKeys({ config }: { config: Record<string, unknown> | null }) {
  return <Filter keys={config && Object.keys(config)} />;
}

// ─── ✅ Good: memoized Object.entries ────────────────────────────────────────

function GoodSelect({ map }: { map: Record<string, string> }) {
  const options = useMemo(() => Object.entries(map), [map]);
  return <Select options={options} />;
}

function GoodList({ config }: { config: Record<string, unknown> }) {
  const keys = useMemo(() => Object.keys(config), [config]);
  return <List items={keys} />;
}

// ─── ✅ Good: module-level precomputed ───────────────────────────────────────

const COUNTRY_OPTIONS = Object.entries(countryMap);

function GoodStaticSelect() {
  return <Select options={COUNTRY_OPTIONS} />;
}

// Dummy components
declare function Select(props: any): any;
declare function List(props: any): any;
declare function Table(props: any): any;
declare function Nav(props: any): any;
declare function Filter(props: any): any;
