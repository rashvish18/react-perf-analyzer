// test_fixtures/good_component.tsx
// This file uses React best practices — react-perf-lint should produce 0 warnings.

import React, { useCallback, useMemo, useState } from "react";

// ─── Stable constants (extracted outside component) ──────────────────────────
// Defined at module level so they are created once, not on every render.

const BUTTON_STYLE = { padding: "8px 16px", borderRadius: 4 };
const NAV_ITEMS = ["Overview", "Analytics", "Settings"] as const;
const TABLE_COLUMNS = ["Name", "Age", "Email"];

// ─── Clean button — no inline function props ──────────────────────────────────
interface ButtonProps {
  label: string;
  onClick: () => void;
}

function GoodButton({ label, onClick }: ButtonProps) {
  // onClick is a stable reference passed from parent — no warning.
  return (
    <button style={BUTTON_STYLE} onClick={onClick}>
      {label}
    </button>
  );
}

// ─── Clean list — no inline object/array props ────────────────────────────────
interface ListProps {
  items: string[];
}

function GoodList({ items }: ListProps) {
  return (
    <ul>
      {items.map((item) => (
        <li key={item}>{item}</li>
      ))}
    </ul>
  );
}

// ─── Clean table ──────────────────────────────────────────────────────────────
interface TableProps {
  data: Array<Record<string, unknown>>;
}

function GoodTable({ data }: TableProps) {
  // TABLE_COLUMNS is a module-level constant — stable reference.
  return (
    <table>
      <thead>
        <tr>
          {TABLE_COLUMNS.map((col) => (
            <th key={col}>{col}</th>
          ))}
        </tr>
      </thead>
      <tbody>
        {data.map((row, idx) => (
          <tr key={idx}>
            {TABLE_COLUMNS.map((col) => (
              <td key={col}>{String(row[col] ?? "")}</td>
            ))}
          </tr>
        ))}
      </tbody>
    </table>
  );
}

// ─── Clean form — handlers extracted with useCallback ─────────────────────────
function GoodForm() {
  const [value, setValue] = useState("");

  // useCallback stabilizes the reference — not a new function on every render.
  const handleChange = useCallback(
    (e: React.ChangeEvent<HTMLInputElement>) => {
      setValue(e.target.value);
    },
    [] // no deps — setValue is stable
  );

  const handleSubmit = useCallback(
    (e: React.FormEvent) => {
      e.preventDefault();
      console.log("Submitted:", value);
    },
    [value]
  );

  return (
    <form onSubmit={handleSubmit}>
      <input value={value} onChange={handleChange} />
      <button type="submit">Submit</button>
    </form>
  );
}

// ─── Clean dashboard — props all stable ──────────────────────────────────────
interface DashboardProps {
  userId: string;
}

function GoodDashboard({ userId }: DashboardProps) {
  const [activeTab, setActiveTab] = useState<(typeof NAV_ITEMS)[number]>("Overview");

  // Stable handlers via useCallback.
  const handleOverview   = useCallback(() => setActiveTab("Overview"),   []);
  const handleAnalytics  = useCallback(() => setActiveTab("Analytics"),  []);
  const handleSettings   = useCallback(() => setActiveTab("Settings"),   []);

  // useMemo for derived data that would otherwise be recomputed every render.
  const mockData = useMemo(
    () => [{ Name: "Alice", Age: "30", Email: "alice@example.com" }],
    [] // stable — no deps change
  );

  return (
    <div>
      <nav>
        <GoodButton label="Overview"  onClick={handleOverview}  />
        <GoodButton label="Analytics" onClick={handleAnalytics} />
        <GoodButton label="Settings"  onClick={handleSettings}  />
      </nav>

      {activeTab === "Overview" && (
        <section>
          <h2>Overview</h2>
          <GoodList items={NAV_ITEMS as unknown as string[]} />
        </section>
      )}

      {activeTab === "Analytics" && (
        <section>
          <h2>Analytics</h2>
          <GoodTable data={mockData} />
        </section>
      )}

      {activeTab === "Settings" && (
        <section>
          <h2>Settings</h2>
          <GoodForm />
        </section>
      )}

      <footer>
        <p>Logged in as: {userId}</p>
      </footer>
    </div>
  );
}

export default GoodDashboard;
