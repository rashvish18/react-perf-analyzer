// no_xss_via_jsx_prop test fixtures
// BAD cases — should be flagged (req.* in JSX props)

export const Bad1 = ({ req }: any) => (
  <div title={req.query.msg} />
);

export const Bad2 = ({ req }: any) => (
  <input placeholder={req.body.name} />
);

export const Bad3 = ({ req }: any) => (
  <meta content={req.params.id} />
);

export const Bad4 = ({ request }: any) => (
  <Component label={request.query.search} />
);

// GOOD cases — should NOT be flagged

// Static string
export const Good1 = () => <div title="static" />;

// Sanitised value
export const Good2 = ({ req }: any) => (
  <div title={sanitize(req.query.msg)} />
);

// Non-request dynamic prop
export const Good3 = ({ label }: { label: string }) => (
  <div title={label} />
);
