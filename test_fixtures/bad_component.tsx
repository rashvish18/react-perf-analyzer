// test_fixtures/bad_component.tsx
// This file intentionally contains React performance anti-patterns.
// Running react-perf-lint on this file should produce 3+ warnings.

import React, { useState } from "react";

interface Props {
  userId: string;
}

// ─── Rule: no_inline_jsx_fn ───────────────────────────────────────────────────
// WARN: onClick uses an inline arrow function
function BadButton({ onClick }: { onClick: () => void }) {
  return <button onClick={onClick}>Click me</button>;
}

// ─── Rule: unstable_props ─────────────────────────────────────────────────────
// WARN: style={{ ... }} creates a new object on every render
// WARN: items={["a", "b"]} creates a new array on every render
function BadList() {
  return (
    <div style={{ padding: 16, margin: 8 }}>
      <BadButton onClick={() => console.log("clicked")} />
      <ul>
        {["apple", "banana", "cherry"].map((item) => (
          <li key={item}>{item}</li>
        ))}
      </ul>
    </div>
  );
}

// ─── Rule: unstable_props (nested component) ──────────────────────────────────
function DataTable({ data }: { data: any[] }) {
  return (
    <table>
      {/* WARN: columns prop is an inline array literal */}
      <thead columns={["Name", "Age", "Email"]} />
      {/* WARN: config prop is an inline object literal */}
      <tbody config={{ striped: true, bordered: false }} />
    </table>
  );
}

// ─── Rule: no_inline_jsx_fn (multiple handlers) ───────────────────────────────
function FormComponent() {
  const [value, setValue] = useState("");

  return (
    <form onSubmit={(e) => { e.preventDefault(); console.log(value); }}>
      {/* WARN: onChange uses inline arrow */}
      <input
        value={value}
        onChange={(e) => setValue(e.target.value)}
      />
      {/* WARN: inline function expression (not arrow) */}
      <select onChange={function(e) { setValue(e.target.value); }}>
        <option value="a">Option A</option>
        <option value="b">Option B</option>
      </select>
      <button type="submit">Submit</button>
    </form>
  );
}

// ─── Rule: large_component ────────────────────────────────────────────────────
// This component exceeds 300 lines when run with --max-component-lines 50
// Use: react-perf-lint ./test_fixtures --max-component-lines 50
function Dashboard({ userId }: Props) {
  const [tab, setTab] = useState("overview");

  return (
    <div style={{ display: "flex", flexDirection: "column" }}>
      <header>
        <h1>Dashboard</h1>
        <nav>
          {/* WARN: inline arrow in onClick */}
          <button onClick={() => setTab("overview")}>Overview</button>
          <button onClick={() => setTab("analytics")}>Analytics</button>
          <button onClick={() => setTab("settings")}>Settings</button>
        </nav>
      </header>
      <main>
        {tab === "overview" && (
          <section>
            {/* WARN: object literal in style prop */}
            <div style={{ padding: 24, background: "#f0f0f0" }}>
              <h2>Overview</h2>
              <BadList />
              <DataTable data={[]} />
            </div>
          </section>
        )}
        {tab === "analytics" && (
          <section>
            <h2>Analytics</h2>
            {/* WARN: array literal in columns prop */}
            <DataTable
              data={[{ id: 1, name: "Alice" }]}
            />
          </section>
        )}
        {tab === "settings" && (
          <section>
            <h2>Settings</h2>
            <FormComponent />
          </section>
        )}
      </main>
      <footer style={{ marginTop: 32 }}>
        <p>User ID: {userId}</p>
      </footer>
    </div>
  );
}

export default Dashboard;
