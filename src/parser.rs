/// parser.rs — OXC parser wrapper.
///
/// OXC uses an arena allocator pattern: all AST nodes are allocated into
/// an `Allocator` arena. The `Program<'a>` borrows from that allocator,
/// so the allocator MUST outlive the `Program`. This module exposes a thin
/// wrapper that handles source type detection and error collection.
///
/// # Lifetime Design
///
/// The caller creates an `Allocator` and source string, then calls `parse_file`.
/// Both must live long enough to use the returned `Program<'a>`. In `main.rs`
/// we create them inside the Rayon closure so each thread owns its own allocator.
///
/// # Source Type
///
/// OXC infers the correct mode from the file extension:
///   .ts  / .tsx  → TypeScript enabled
///   .jsx / .tsx  → JSX enabled
///   .js  / .jsx  → default JS
use std::path::Path;

use oxc_allocator::Allocator;
use oxc_ast::ast::Program;
use oxc_parser::Parser;
use oxc_span::SourceType;

/// Wraps OXC parse errors as a simple string for reporting.
#[derive(Debug)]
pub struct ParseError {
    pub file: String,
    pub messages: Vec<String>,
}

/// Parse `source_text` as a JS/TS/JSX file and return the AST `Program`.
///
/// # Arguments
/// * `allocator`   — Arena allocator that owns all AST node memory.
///   Must outlive the returned `Program`.
/// * `path`        — File path, used only for `SourceType` inference and errors.
/// * `source_text` — The raw source code string.
///
/// # Returns
/// * `Ok(Program<'a>)` on success (parse errors are tolerated — OXC recovers).
/// * `Err(ParseError)` if the source type cannot be inferred (rare).
///
/// # Note on error recovery
/// OXC is an error-recovering parser. Even files with syntax errors will
/// produce a partial `Program`. We surface fatal errors only; warnings are
/// silently ignored in the MVP.
pub fn parse_file<'a>(
    allocator: &'a Allocator,
    path: &Path,
    source_text: &'a str,
) -> Result<Program<'a>, ParseError> {
    // Infer source type (JS vs TS, plain vs JSX) from the file extension.
    // Falls back to plain JS if the extension is unknown.
    let source_type = SourceType::from_path(path).unwrap_or_default();

    // Execute the parse using OXC defaults — they already handle JSX and
    // TypeScript correctly based on the SourceType we set above.
    //
    // OXC returns a `ParserReturn` containing:
    //   .program  — the AST (borrows from `allocator` and `source_text`)
    //   .errors   — non-fatal parse diagnostics (recoverable syntax errors)
    //   .panicked — true only if OXC hit a completely unrecoverable state
    let result = Parser::new(allocator, source_text, source_type).parse();

    if result.panicked {
        // OXC panicked internally — likely completely unparseable input.
        let messages = result
            .errors
            .iter()
            .map(|e| e.to_string())
            .collect::<Vec<_>>();

        return Err(ParseError {
            file: path.display().to_string(),
            messages,
        });
    }

    // Non-fatal errors (e.g. recoverable syntax issues) are intentionally
    // ignored here. The partial AST is still useful for lint rules.
    Ok(result.program)
}
