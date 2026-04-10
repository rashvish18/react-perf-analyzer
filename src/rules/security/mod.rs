/// rules/security/mod.rs — React-specific security rule registry.
///
/// We own only React/JSX-specific security patterns here.
/// General JS/TS security (eval, SQLi, secrets) is delegated to oxc_linter.
/// Rust CVEs are delegated to cargo-audit.
pub mod react;

use crate::rules::Rule;

pub fn security_rules() -> Vec<Box<dyn Rule>> {
    react::react_security_rules()
}
