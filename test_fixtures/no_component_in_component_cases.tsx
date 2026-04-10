// Test fixture: no_component_in_component rule
// ❌ patterns should fire, ✅ patterns should not

import React, { useState } from 'react';

// ─── ❌ Bad: function declaration inside component ────────────────────────────

function Parent({ data }: { data: string[] }) {
  // ❌ InnerList defined inside Parent's render body
  function InnerList({ items }: { items: string[] }) {
    return <ul>{items.map(i => <li key={i}>{i}</li>)}</ul>;
  }
  return <InnerList items={data} />;
}

// ─── ❌ Bad: const arrow component inside component ───────────────────────────

function Dashboard() {
  // ❌ Card is recreated on every Dashboard render
  const Card = ({ title }: { title: string }) => <div className="card">{title}</div>;
  return (
    <div>
      <Card title="Revenue" />
      <Card title="Users" />
    </div>
  );
}

// ─── ❌ Bad: const function expression inside component ───────────────────────

function ProfilePage({ userId }: { userId: string }) {
  // ❌ Avatar defined inside ProfilePage
  const Avatar = function({ size }: { size: number }) {
    return <img src={`/avatars/${userId}`} width={size} height={size} />;
  };
  return <Avatar size={64} />;
}

// ─── ❌ Bad: deeply nested component ─────────────────────────────────────────

function App() {
  function Layout({ children }: { children: React.ReactNode }) {
    // ❌ Header inside Layout inside App
    const Header = () => <header>My App</header>;
    return (
      <div>
        <Header />
        {children}
      </div>
    );
  }
  return <Layout><main>Content</main></Layout>;
}

// ─── ✅ Good: components defined at module level ──────────────────────────────

const GoodCard = ({ title }: { title: string }) => (
  <div className="card">{title}</div>
);

function GoodInnerList({ items }: { items: string[] }) {
  return <ul>{items.map(i => <li key={i}>{i}</li>)}</ul>;
}

function GoodParent({ data }: { data: string[] }) {
  return <GoodInnerList items={data} />;
}

function GoodDashboard() {
  return (
    <div>
      <GoodCard title="Revenue" />
      <GoodCard title="Users" />
    </div>
  );
}

// ─── ✅ Good: regular (non-component) helper functions inside component ────────

function ComponentWithHelpers({ items }: { items: number[] }) {
  // ✅ lowercase helper — not a component
  const formatItem = (n: number) => n.toFixed(2);
  const isEven = (n: number) => n % 2 === 0;
  return <ul>{items.filter(isEven).map(n => <li key={n}>{formatItem(n)}</li>)}</ul>;
}
