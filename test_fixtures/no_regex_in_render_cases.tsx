// Test fixture: no_regex_in_render rule
// ❌ patterns should fire, ✅ patterns should not

// ─── ❌ Bad: regex literal in JSX prop ───────────────────────────────────────

function PhoneInput({ value }: { value: string }) {
  // ❌ New RegExp object on every render
  return <Input pattern={/^\d{3}-\d{4}$/} value={value} />;
}

function EmailFilter({ items }: { items: string[] }) {
  // ❌ New RegExp on every render
  return <Filter test={/^[\w.]+@[\w.]+\.[a-z]{2,}$/i} items={items} />;
}

// ─── ❌ Bad: regex in ternary ─────────────────────────────────────────────────

function ConditionalRegex({ strict }: { strict: boolean }) {
  // ❌ New regex in either branch
  return <Validator rule={strict ? /^\d{10}$/ : /^\d{7,10}$/} />;
}

// ─── ❌ Bad: regex in logical expression ──────────────────────────────────────

function LogicalRegex({ enabled }: { enabled: boolean }) {
  return <Input pattern={enabled && /^[A-Z]+$/} />;
}

// ─── ❌ Bad: regex with flags ─────────────────────────────────────────────────

function CaseInsensitive() {
  return <Search pattern={/hello world/gi} />;
}

// ─── ✅ Good: module-level regex constant ─────────────────────────────────────

const PHONE_RE = /^\d{3}-\d{4}$/;
const EMAIL_RE = /^[\w.]+@[\w.]+\.[a-z]{2,}$/i;

function GoodPhoneInput({ value }: { value: string }) {
  return <Input pattern={PHONE_RE} value={value} />;
}

function GoodEmailFilter({ items }: { items: string[] }) {
  return <Filter test={EMAIL_RE} items={items} />;
}

// ─── ✅ Good: string pattern prop is fine ─────────────────────────────────────

function GoodStringPattern() {
  // ✅ String, not a RegExp object
  return <Input pattern="^\d{3}-\d{4}$" />;
}

// Dummy components
declare function Input(props: any): any;
declare function Filter(props: any): any;
declare function Validator(props: any): any;
declare function Search(props: any): any;
