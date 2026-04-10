/// rules/security/react/mod.rs — React-specific security rules.
///
/// These are rules that no other tool covers:
/// - JSX-specific XSS vectors
/// - Secrets exposed in client-side component bundles
/// - React Router / Next.js unsafe href patterns
/// - postMessage wildcard origin leaks
pub mod no_dangerously_set_inner_html_unescaped;
pub mod no_hardcoded_secret_in_jsx;
pub mod no_postmessage_wildcard;
pub mod no_unsafe_href;
pub mod no_xss_via_jsx_prop;

use crate::rules::Rule;

pub fn react_security_rules() -> Vec<Box<dyn Rule>> {
    vec![
        Box::new(no_unsafe_href::NoUnsafeHref),
        Box::new(no_xss_via_jsx_prop::NoXssViaJsxProp),
        Box::new(no_hardcoded_secret_in_jsx::NoHardcodedSecretInJsx),
        Box::new(no_dangerously_set_inner_html_unescaped::NoDangerouslySetInnerHtmlUnescaped),
        Box::new(no_postmessage_wildcard::NoPostmessageWildcard),
    ]
}
