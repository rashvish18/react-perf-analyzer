// no_unsafe_href test fixtures
// BAD cases — should be flagged

// javascript: URL in href
export const Bad1 = () => <a href="javascript:alert(1)">click</a>;

// Dynamic href from props
export const Bad2 = ({ url }: { url: string }) => <a href={url}>link</a>;

// React Router Link with dynamic to
export const Bad3 = ({ returnUrl }: { returnUrl: string }) => (
  <Link to={returnUrl}>back</Link>
);

// Template literal with javascript: prefix
export const Bad4 = ({ fn: handler }: { fn: string }) => (
  <a href={`javascript:${handler}`}>run</a>
);

// Member expression in href
export const Bad5 = ({ router }: any) => (
  <a href={router.query.redirect}>redirect</a>
);

// GOOD cases — should NOT be flagged

// Static string literal
export const Good1 = () => <a href="/dashboard">dashboard</a>;

// Absolute URL
export const Good2 = () => <a href="https://example.com">external</a>;

// Non-href prop with dynamic value
export const Good3 = ({ label }: { label: string }) => (
  <a href="/home" aria-label={label}>home</a>
);
