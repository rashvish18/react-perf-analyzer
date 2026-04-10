// test_fixtures/no_array_index_key_cases.tsx
// Exercises every detection path in the no_array_index_key rule.
// Run with: react-perf-analyzer ./test_fixtures/no_array_index_key_cases.tsx
//
// All ❌ expressions should warn; all ✅ expressions should be silent.

import React from "react";

// ─────────────────────────────────────────────────────────────────────────────
// ❌ PATTERN 1: Classic index as key
// ─────────────────────────────────────────────────────────────────────────────

function ClassicIndexKey({ items }) {
  return (
    <ul>
      {/* ❌ no_array_index_key: index used directly as key */}
      {items.map((item, index) => (
        <li key={index}>{item.name}</li>
      ))}
    </ul>
  );
}

// ─────────────────────────────────────────────────────────────────────────────
// ❌ PATTERN 2: Short variable name `i`
// ─────────────────────────────────────────────────────────────────────────────

function ShortNameIndexKey({ items }) {
  return (
    <ul>
      {/* ❌ no_array_index_key: `i` is the map index param */}
      {items.map((item, i) => (
        <li key={i}>{item.label}</li>
      ))}
    </ul>
  );
}

// ─────────────────────────────────────────────────────────────────────────────
// ❌ PATTERN 3: `idx` variable name
// ─────────────────────────────────────────────────────────────────────────────

function IdxIndexKey({ rows }) {
  return (
    <table>
      <tbody>
        {/* ❌ no_array_index_key: `idx` is the map index param */}
        {rows.map((row, idx) => (
          <tr key={idx}>
            <td>{row.value}</td>
          </tr>
        ))}
      </tbody>
    </table>
  );
}

// ─────────────────────────────────────────────────────────────────────────────
// ❌ PATTERN 4: Index in template literal
// ─────────────────────────────────────────────────────────────────────────────

function TemplateIndexKey({ items }) {
  return (
    <ul>
      {/* ❌ no_array_index_key: index embedded in template literal */}
      {items.map((item, index) => (
        <li key={`item-${index}`}>{item.name}</li>
      ))}
    </ul>
  );
}

// ─────────────────────────────────────────────────────────────────────────────
// ❌ PATTERN 5: Function expression callback (not arrow)
// ─────────────────────────────────────────────────────────────────────────────

function FunctionExprIndexKey({ items }) {
  return (
    <ul>
      {/* ❌ no_array_index_key: index from function expression callback */}
      {items.map(function(item, index) {
        return <li key={index}>{item.name}</li>;
      })}
    </ul>
  );
}

// ─────────────────────────────────────────────────────────────────────────────
// ❌ PATTERN 6: Index on a nested JSX element
// ─────────────────────────────────────────────────────────────────────────────

function NestedIndexKey({ cards }) {
  return (
    <div>
      {cards.map((card, index) => (
        <div className="wrapper">
          {/* ❌ no_array_index_key: index on nested element */}
          <article key={index}>
            <h2>{card.title}</h2>
          </article>
        </div>
      ))}
    </div>
  );
}

// ─────────────────────────────────────────────────────────────────────────────
// ✅ PATTERN 7: Stable ID from data — no warning
// ─────────────────────────────────────────────────────────────────────────────

function StableIdKey({ items }) {
  return (
    <ul>
      {/* ✅ item.id is stable — correct usage */}
      {items.map((item) => (
        <li key={item.id}>{item.name}</li>
      ))}
    </ul>
  );
}

// ─────────────────────────────────────────────────────────────────────────────
// ✅ PATTERN 8: Composite key from data properties — no warning
// ─────────────────────────────────────────────────────────────────────────────

function CompositeKey({ items }) {
  return (
    <ul>
      {/* ✅ Composite key using item data — no warning */}
      {items.map((item) => (
        <li key={`${item.type}-${item.id}`}>{item.name}</li>
      ))}
    </ul>
  );
}

// ─────────────────────────────────────────────────────────────────────────────
// ✅ PATTERN 9: UUID field — no warning
// ─────────────────────────────────────────────────────────────────────────────

function UuidKey({ users }) {
  return (
    <ul>
      {/* ✅ Stable UUID from the data */}
      {users.map((user) => (
        <li key={user.uuid}>{user.name}</li>
      ))}
    </ul>
  );
}

// ─────────────────────────────────────────────────────────────────────────────
// ✅ PATTERN 10: No second param — no warning
// ─────────────────────────────────────────────────────────────────────────────

function NoIndexParam({ items }) {
  return (
    <ul>
      {/* ✅ Map callback has no index param — nothing to track */}
      {items.map((item) => (
        <li key={item.id}>{item.name}</li>
      ))}
    </ul>
  );
}
