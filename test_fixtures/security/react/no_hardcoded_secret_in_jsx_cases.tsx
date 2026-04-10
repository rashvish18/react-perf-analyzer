// no_hardcoded_secret_in_jsx test fixtures
// BAD cases — should be flagged

// High-entropy API key in JSX prop
export const Bad1 = () => (
  <ApiProvider apiKey="sk-1a2b3c4d5e6f7g8h9i0j1k2l3m4n5o6p" />
);

// Stripe key in prop
export const Bad2 = () => (
  <StripeProvider token="pk_live_51AbCdEfGhIjKlMnOpQrStUvWx" />
);

// Secret in a variable name that matches pattern
const API_SECRET = "xK9mP2qR5tL8nW1vY4hJ7cF0";

// Generic key constant
const AUTH_TOKEN = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.abc.def";

// GOOD cases — should NOT be flagged

// Environment variable (not a string literal)
export const Good1 = () => (
  <ApiProvider apiKey={process.env.NEXT_PUBLIC_API_KEY} />
);

// Placeholder value
export const Good2 = () => (
  <TestProvider apiKey="your-api-key-here" />
);

// Non-secret prop name
export const Good3 = () => <Component label="hello world" />;

// Short string (too short to be a real secret)
const SHORT_KEY = "abc123";
