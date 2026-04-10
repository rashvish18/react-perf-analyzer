// no_dangerously_set_inner_html_unescaped test fixtures
// BAD cases — should be flagged

// marked() is not a real sanitiser — produces raw HTML
export const Bad1 = ({ content }: { content: string }) => (
  <div dangerouslySetInnerHTML={{ __html: marked(content) }} />
);

// showdown is unsafe
export const Bad2 = ({ md }: { md: string }) => (
  <div dangerouslySetInnerHTML={{ __html: showdown.makeHtml(md) }} />
);

// Regex replace is bypassable
export const Bad3 = ({ html }: { html: string }) => (
  <div dangerouslySetInnerHTML={{ __html: html.replace(/<script>/g, '') }} />
);

// Raw variable — no sanitisation at all
export const Bad4 = ({ html }: { html: string }) => (
  <div dangerouslySetInnerHTML={{ __html: html }} />
);

// GOOD cases — should NOT be flagged

// DOMPurify is the gold standard
export const Good1 = ({ html }: { html: string }) => (
  <div dangerouslySetInnerHTML={{ __html: DOMPurify.sanitize(html) }} />
);

// Static string literal is safe
export const Good2 = () => (
  <div dangerouslySetInnerHTML={{ __html: "<b>static</b>" }} />
);

// xss package
export const Good3 = ({ html }: { html: string }) => (
  <div dangerouslySetInnerHTML={{ __html: xss(html) }} />
);
