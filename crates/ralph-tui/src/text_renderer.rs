//! Text-to-lines rendering for TUI display.
//!
//! This module provides functions to convert text (markdown/ANSI) into
//! styled ratatui `Line` objects. It handles both plain markdown and
//! text that already contains ANSI escape sequences.
//!
//! This is extracted from `ralph-adapters::stream_handler` to be shared
//! between the TuiStreamHandler and the RPC event source.

use std::borrow::Cow;

use ansi_to_tui::IntoText;
use ratatui::text::{Line, Span};
use termimad::MadSkin;

/// Converts text to styled ratatui Lines, handling both ANSI and markdown.
///
/// When text contains ANSI escape sequences (e.g., from CLI tools like Kiro),
/// uses `ansi_to_tui` to preserve colors and formatting. Otherwise, uses
/// `termimad` to parse markdown (matching non-TUI mode behavior), then
/// converts the ANSI output via `ansi_to_tui`.
///
/// Using `termimad` ensures parity between TUI and non-TUI modes, as both
/// use the same markdown processing engine with the same line-breaking rules.
pub fn text_to_lines(text: &str) -> Vec<Line<'static>> {
    if text.is_empty() {
        return Vec::new();
    }

    // Ratatui content must not contain control characters like carriage returns.
    let text = sanitize_tui_block_text(text);
    let text = text.as_ref();
    if text.is_empty() {
        return Vec::new();
    }

    // Convert text to ANSI-styled string
    // - If already contains ANSI: use as-is
    // - If plain/markdown: process through termimad (matches non-TUI behavior)
    let ansi_text = if contains_ansi(text) {
        text.to_string()
    } else {
        // Use termimad to process markdown
        let skin = MadSkin::default();
        skin.term_text(text).to_string()
    };

    // Parse ANSI codes to ratatui Text
    match ansi_text.as_str().into_text() {
        Ok(parsed_text) => {
            // Convert Text to owned Lines
            parsed_text
                .lines
                .into_iter()
                .map(|line| {
                    let owned_spans: Vec<Span<'static>> = line
                        .spans
                        .into_iter()
                        .map(|span| Span::styled(span.content.into_owned(), span.style))
                        .collect();
                    Line::from(owned_spans)
                })
                .collect()
        }
        Err(_) => {
            // Fallback: split on newlines and treat as plain text
            text.split('\n')
                .map(|line| Line::from(line.to_string()))
                .collect()
        }
    }
}

/// Detects if text contains ANSI escape sequences.
///
/// Checks for the common ANSI escape sequence prefix `\x1b[` (ESC + `[`)
/// which is used for colors, formatting, and cursor control.
#[inline]
pub fn contains_ansi(text: &str) -> bool {
    text.contains("\x1b[")
}

/// Normalizes terminal control characters that commonly break ratatui rendering.
///
/// In particular:
/// - `\r` (carriage return) is used by many CLIs (git, cargo, etc.) to render
///   progress updates on a single line. When embedded in ratatui content it can
///   move the cursor and corrupt layout.
/// - Some other C0 controls (bell, backspace, vertical tab, form feed) can also
///   cause display corruption or odd glyphs.
///
/// We keep `\n` and `\t` intact.
pub fn sanitize_tui_block_text(text: &str) -> Cow<'_, str> {
    let has_cr = text.contains('\r');
    let has_other_ctrl = text
        .chars()
        .any(|c| matches!(c, '\u{0007}' | '\u{0008}' | '\u{000b}' | '\u{000c}'));

    if !has_cr && !has_other_ctrl {
        return Cow::Borrowed(text);
    }

    let mut s = if has_cr {
        // Normalize CRLF and bare CR to LF.
        text.replace("\r\n", "\n").replace('\r', "\n")
    } else {
        text.to_string()
    };

    if has_other_ctrl {
        s.retain(|c| !matches!(c, '\u{0007}' | '\u{0008}' | '\u{000b}' | '\u{000c}'));
    }

    Cow::Owned(s)
}

/// Truncates a string to approximately `max_len` characters, appending `"..."`.
///
/// Works on Unicode code-point boundaries, not bytes, so multi-byte characters
/// are counted correctly.
pub fn truncate(s: &str, max_len: usize) -> String {
    if s.chars().count() <= max_len {
        s.to_string()
    } else {
        let byte_idx = s
            .char_indices()
            .nth(max_len)
            .map(|(idx, _)| idx)
            .unwrap_or(s.len());
        format!("{}...", &s[..byte_idx])
    }
}

/// Sanitizes text that must stay on a *single* TUI line (tool summaries, errors).
/// Removes embedded newlines and carriage returns entirely.
pub fn sanitize_tui_inline_text(text: &str) -> String {
    let mut s = text.replace("\r\n", " ").replace(['\r', '\n'], " ");

    // Drop other control characters that can corrupt the terminal.
    s.retain(|c| !matches!(c, '\u{0007}' | '\u{0008}' | '\u{000b}' | '\u{000c}'));

    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_text_to_lines_empty() {
        assert!(text_to_lines("").is_empty());
    }

    #[test]
    fn test_text_to_lines_plain() {
        let lines = text_to_lines("hello world");
        assert!(!lines.is_empty());
    }

    #[test]
    fn test_text_to_lines_multiline() {
        let lines = text_to_lines("line1\nline2\nline3");
        assert!(lines.len() >= 3);
    }

    #[test]
    fn test_text_to_lines_ansi() {
        // ANSI red text
        let ansi_text = "\x1b[31mred text\x1b[0m";
        let lines = text_to_lines(ansi_text);
        assert!(!lines.is_empty());
    }

    #[test]
    fn test_contains_ansi() {
        assert!(!contains_ansi("plain text"));
        assert!(contains_ansi("\x1b[31mred\x1b[0m"));
    }

    #[test]
    fn test_sanitize_block_text_no_changes() {
        let text = "normal text\nwith newlines";
        let result = sanitize_tui_block_text(text);
        assert_eq!(result, text);
        assert!(matches!(result, Cow::Borrowed(_)));
    }

    #[test]
    fn test_sanitize_block_text_cr() {
        let text = "line1\r\nline2\rline3";
        let result = sanitize_tui_block_text(text);
        assert_eq!(result, "line1\nline2\nline3");
    }

    #[test]
    fn test_sanitize_block_text_bell() {
        let text = "text\u{0007}with bell";
        let result = sanitize_tui_block_text(text);
        assert_eq!(result, "textwith bell");
    }

    #[test]
    fn test_sanitize_inline_text() {
        let text = "line1\nline2\rline3";
        let result = sanitize_tui_inline_text(text);
        assert_eq!(result, "line1 line2 line3");
    }
}
