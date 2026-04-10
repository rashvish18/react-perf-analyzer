// test_fixtures/large_component_cases.tsx
// Exercises every detection path in the large_component rule.
// Run with: react-perf-lint ./test_fixtures/large_component_cases.tsx --max-component-lines 20
//
// With --max-component-lines 20, all ❌ components should warn.
// ✅ components and non-components should produce no warning.

import React, { memo, forwardRef, useState, useEffect, useCallback, useMemo, useRef } from "react";

// ── Helper to pad any function to N lines ────────────────────────────────────
// (These comments count as lines in the total but NOT in logical lines.)

// ─────────────────────────────────────────────────────────────────────────────
// ❌ PATTERN 1: Function declaration — exceeds 20 lines
// ─────────────────────────────────────────────────────────────────────────────
function FunctionDeclarationComponent({ userId }: { userId: string }) {
  const [data, setData] = useState(null);          // hook 1
  const [loading, setLoading] = useState(true);    // hook 2
  const [error, setError] = useState<string | null>(null); // hook 3

  useEffect(() => {                                 // hook 4
    setLoading(true);
    fetch(`/api/users/${userId}`)
      .then(r => r.json())
      .then(d => { setData(d); setLoading(false); })
      .catch(e => { setError(e.message); setLoading(false); });
  }, [userId]);

  if (loading) return <div className="spinner">Loading...</div>;
  if (error)   return <div className="error">{error}</div>;

  return (
    <section className="user-profile">
      <h1>User: {userId}</h1>
      <pre>{JSON.stringify(data, null, 2)}</pre>
    </section>
  );
}

// ─────────────────────────────────────────────────────────────────────────────
// ❌ PATTERN 2: Arrow function variable — exceeds 20 lines
// ─────────────────────────────────────────────────────────────────────────────
const ArrowFunctionComponent = ({ title }: { title: string }) => {
  const [count, setCount] = useState(0);           // hook 1
  const [visible, setVisible] = useState(true);    // hook 2
  const increment = useCallback(() => setCount(c => c + 1), []); // hook 3
  const decrement = useCallback(() => setCount(c => c - 1), []); // hook 4

  useEffect(() => {                                 // hook 5
    document.title = `${title} (${count})`;
  }, [title, count]);

  if (!visible) return null;

  return (
    <div className="counter">
      <h2>{title}</h2>
      <button onClick={decrement}>−</button>
      <span>{count}</span>
      <button onClick={increment}>+</button>
      <button onClick={() => setVisible(false)}>Hide</button>
    </div>
  );
};

// ─────────────────────────────────────────────────────────────────────────────
// ❌ PATTERN 3: React.memo() wrapped — exceeds 20 lines
// The component is the INNER function passed to memo().
// ─────────────────────────────────────────────────────────────────────────────
const MemoWrappedComponent = memo(({ items }: { items: string[] }) => {
  const [selected, setSelected] = useState<string | null>(null); // hook 1
  const [filter, setFilter] = useState("");         // hook 2

  const filtered = useMemo(                        // hook 3
    () => items.filter(i => i.toLowerCase().includes(filter.toLowerCase())),
    [items, filter]
  );

  useEffect(() => {                                // hook 4
    console.log("selection changed:", selected);
  }, [selected]);

  return (
    <div>
      <input
        placeholder="Filter..."
        value={filter}
        onChange={e => setFilter(e.target.value)}
      />
      <ul>
        {filtered.map(item => (
          <li
            key={item}
            className={item === selected ? "selected" : ""}
            onClick={() => setSelected(item)}
          >
            {item}
          </li>
        ))}
      </ul>
      {selected && <p>Selected: {selected}</p>}
    </div>
  );
});

// ─────────────────────────────────────────────────────────────────────────────
// ❌ PATTERN 4: forwardRef() wrapped — exceeds 20 lines
// ─────────────────────────────────────────────────────────────────────────────
const ForwardRefComponent = forwardRef<HTMLInputElement, { label: string; placeholder: string }>(
  (props, ref) => {
    const [value, setValue] = useState("");           // hook 1
    const [focused, setFocused] = useState(false);    // hook 2
    const [touched, setTouched] = useState(false);    // hook 3
    const innerRef = useRef<HTMLInputElement>(null);  // hook 4

    const handleFocus = useCallback(() => {          // hook 5
      setFocused(true);
      setTouched(true);
    }, []);

    const handleBlur = useCallback(() => setFocused(false), []); // hook 6

    useEffect(() => {
      if (ref && typeof ref === "object") {
        (ref as React.MutableRefObject<HTMLInputElement | null>).current = innerRef.current;
      }
    }, [ref]);

    return (
      <div className={`field ${focused ? "focused" : ""} ${touched ? "touched" : ""}`}>
        <label>{props.label}</label>
        <input
          ref={innerRef}
          value={value}
          placeholder={props.placeholder}
          onFocus={handleFocus}
          onBlur={handleBlur}
          onChange={e => setValue(e.target.value)}
        />
      </div>
    );
  }
);

// ─────────────────────────────────────────────────────────────────────────────
// ❌ PATTERN 5: export function declaration — exceeds 20 lines
// ─────────────────────────────────────────────────────────────────────────────
export function ExportedFunctionComponent({ id }: { id: number }) {
  const [tab, setTab] = useState("overview");       // hook 1
  const [data, setData] = useState<any>(null);      // hook 2

  useEffect(() => {                                  // hook 3
    fetch(`/api/items/${id}`).then(r => r.json()).then(setData);
  }, [id]);

  return (
    <div>
      <nav>
        <button onClick={() => setTab("overview")}>Overview</button>
        <button onClick={() => setTab("details")}>Details</button>
      </nav>
      {tab === "overview" && (
        <section>
          <h2>Overview</h2>
          <pre>{JSON.stringify(data?.overview, null, 2)}</pre>
        </section>
      )}
      {tab === "details" && (
        <section>
          <h2>Details</h2>
          <pre>{JSON.stringify(data?.details, null, 2)}</pre>
        </section>
      )}
    </div>
  );
}

// ─────────────────────────────────────────────────────────────────────────────
// ✅ SHORT components — should produce NO warning with --max-component-lines 20
// ─────────────────────────────────────────────────────────────────────────────

// ✅ Short function declaration
function ShortButton({ label, onClick }: { label: string; onClick: () => void }) {
  return <button onClick={onClick}>{label}</button>;
}

// ✅ Short arrow
const ShortBadge = ({ text }: { text: string }) => <span className="badge">{text}</span>;

// ✅ Short memo
const ShortMemo = memo(({ value }: { value: number }) => <div>{value}</div>);

// ─────────────────────────────────────────────────────────────────────────────
// ✅ NON-COMPONENTS — must never warn (lowercase name, no JSX, plain functions)
// ─────────────────────────────────────────────────────────────────────────────

// ✅ Plain utility function — not PascalCase
function formatCurrency(value: number, currency = "USD"): string {
  return new Intl.NumberFormat("en-US", { style: "currency", currency }).format(value);
}

// ✅ Helper that returns a string, not JSX
function buildClassName(...parts: string[]): string {
  return parts.filter(Boolean).join(" ");
}

// ✅ memo wrapping a reference (not inline) — not detected here, component defined elsewhere
const ReferencedMemo = memo(ShortButton);

export default FunctionDeclarationComponent;
