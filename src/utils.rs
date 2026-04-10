//! utils.rs — Shared utility functions for span → position conversion.
//!
//! OXC represents source locations as byte offsets (u32) in the `Span` type.
//! The reporter needs (line, column) numbers, so we convert here.
//! These are 1-indexed to match editor conventions.

/// Convert a byte offset within `source` to a 1-indexed (line, column) tuple.
///
/// # Arguments
/// * `source` - The full source text of the file being analyzed.
/// * `offset` - Byte offset from OXC's `Span::start` or `Span::end`.
///
/// # Returns
/// `(line, column)` where both are 1-indexed.
///
/// # Example
/// ```
/// let src = "let x = 1;\nlet y = 2;";
/// assert_eq!(offset_to_line_col(src, 11), (2, 1));
/// ```
pub fn offset_to_line_col(source: &str, offset: u32) -> (u32, u32) {
    // Clamp offset to valid range to avoid panics on malformed spans.
    let offset = (offset as usize).min(source.len());

    // Work only with the slice of source *before* the offset.
    // This lets us count newlines efficiently.
    let before = &source[..offset];

    // Line = number of newline characters seen + 1 (1-indexed).
    let line = before.chars().filter(|&c| c == '\n').count() as u32 + 1;

    // Column = characters after the last newline (or from start if none).
    // rfind returns a byte index, so we subtract to get chars-since-newline.
    let col = match before.rfind('\n') {
        Some(newline_pos) => {
            // Characters between the last newline and the offset.
            // We use byte subtraction here — valid because we're working with
            // ASCII positions in nearly all practical JS source files.
            (offset - newline_pos - 1) as u32 + 1
        }
        None => {
            // No newline found — offset is on the first line.
            offset as u32 + 1
        }
    };

    (line, col)
}

/// Count the number of source lines spanned by a byte range `[start, end)`.
///
/// Used by the `large_component` rule to measure component size.
///
/// # Arguments
/// * `source` - The full source text.
/// * `start`  - Byte offset of the range start (OXC `Span::start`).
/// * `end`    - Byte offset of the range end   (OXC `Span::end`).
#[allow(dead_code)] // Utility available for future rules; superseded by measure_lines in large_component.
pub fn count_lines_in_range(source: &str, start: u32, end: u32) -> usize {
    let start = (start as usize).min(source.len());
    let end = (end as usize).min(source.len());

    if start >= end {
        return 1;
    }

    // Count newlines in the span + 1 to get line count.
    source[start..end].chars().filter(|&c| c == '\n').count() + 1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_offset_to_line_col_first_line() {
        let src = "let x = 1;";
        assert_eq!(offset_to_line_col(src, 0), (1, 1));
        assert_eq!(offset_to_line_col(src, 4), (1, 5));
    }

    #[test]
    fn test_offset_to_line_col_second_line() {
        let src = "let x = 1;\nlet y = 2;";
        assert_eq!(offset_to_line_col(src, 11), (2, 1));
        assert_eq!(offset_to_line_col(src, 15), (2, 5));
    }

    #[test]
    fn test_count_lines_single_line() {
        let src = "const x = 1;";
        assert_eq!(count_lines_in_range(src, 0, src.len() as u32), 1);
    }

    #[test]
    fn test_count_lines_multi_line() {
        let src = "line1\nline2\nline3";
        assert_eq!(count_lines_in_range(src, 0, src.len() as u32), 3);
    }
}
