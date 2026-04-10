// Test fixture: no_math_random_in_render rule
// ❌ patterns should fire, ✅ patterns should not

import { useMemo, useState } from 'react';

// ─── ❌ Bad: Math.random() in prop ───────────────────────────────────────────

function RandomAvatar({ name }: { name: string }) {
  // ❌ Different seed on every render
  return <Avatar seed={Math.random()} name={name} />;
}

// ─── ❌ Bad: Date.now() in prop ───────────────────────────────────────────────

function TimestampBadge() {
  // ❌ Different timestamp on every render
  return <Badge value={Date.now()} />;
}

// ─── ❌ Bad: Math.random in ternary ──────────────────────────────────────────

function ConditionalRandom({ show }: { show: boolean }) {
  // ❌ Math.random in a branch
  return <Widget id={show ? Math.random() : 0} />;
}

// ─── ❌ Bad: Date.now in logical expression ───────────────────────────────────

function LogicalTimestamp({ track }: { track: boolean }) {
  return <Tracker ts={track && Date.now()} />;
}

// ─── ✅ Good: Math.random with useMemo ───────────────────────────────────────

function GoodAvatar({ name }: { name: string }) {
  // ✅ Generated once on mount
  const seed = useMemo(() => Math.random(), []);
  return <Avatar seed={seed} name={name} />;
}

// ─── ✅ Good: Date.now with useState ─────────────────────────────────────────

function GoodTimestamp() {
  // ✅ Generated once on mount
  const [ts] = useState(() => Date.now());
  return <Badge value={ts} />;
}

// ─── ✅ Good: static number prop is fine ──────────────────────────────────────

function GoodStatic() {
  return <Widget id={42} value={3.14} />;
}

// Dummy components
declare function Avatar(props: any): any;
declare function Badge(props: any): any;
declare function Widget(props: any): any;
declare function Tracker(props: any): any;
