//! Text utilities for the Ralph Orchestrator.
//!
//! This module provides common text manipulation functions used throughout
//! the codebase, including UTF-8 safe string truncation and TUI text
//! sanitization.

use std::borrow::Cow;

/// Truncates a string to a maximum number of characters, including "..." if truncated.
///
/// This function is UTF-8 safe: it uses character boundaries, not byte boundaries,
/// so it will never split a multi-byte character (emoji, non-ASCII, etc.).
///
/// If truncated, the resulting string length will be exactly `max_chars`.
/// If `max_chars` is less than 3, no ellipsis is added and it just takes `max_chars`.
///
/// # Arguments
///
/// * `s` - The string to truncate
/// * `max_chars` - Maximum number of characters (not bytes) for the final string
///
/// # Returns
///
/// - The original string if its character count is <= `max_chars`
/// - A truncated string of length `max_chars` with "..." suffix if longer
///
/// # Examples
///
/// ```
/// use ralph_core::truncate_with_ellipsis;
///
/// // Short strings pass through unchanged
/// assert_eq!(truncate_with_ellipsis("hello", 10), "hello");
///
/// // Long strings are truncated so the total length is max_chars
/// assert_eq!(truncate_with_ellipsis("hello world", 8), "hello...");
///
/// // UTF-8 safe: emojis are not split
/// assert_eq!(truncate_with_ellipsis("🎉🎊🎁🎄", 3), "...");
/// ```
pub fn truncate_with_ellipsis(s: &str, max_chars: usize) -> String {
    let char_count = s.chars().count();
    if char_count <= max_chars {
        s.to_string()
    } else if max_chars < 3 {
        // Not enough room for ellipsis, just take characters
        s.chars().take(max_chars).collect()
    } else {
        // Take max_chars - 3 characters and add ellipsis
        let keep = max_chars - 3;
        let byte_idx = s
            .char_indices()
            .nth(keep)
            .map(|(idx, _)| idx)
            .unwrap_or(s.len());
        format!("{}...", &s[..byte_idx])
    }
}

/// Sanitizes text for multi-line TUI display (block content like agent output).
///
/// - Normalizes `\r\n` and bare `\r` to `\n`.
/// - Strips C0 control characters (bell, backspace, vertical tab, form feed)
///   that can corrupt terminal layout.
/// - Preserves `\n` and `\t`.
pub fn sanitize_tui_block_text(text: &str) -> Cow<'_, str> {
    let has_cr = text.contains('\r');
    let has_other_ctrl = text
        .chars()
        .any(|c| matches!(c, '\u{0007}' | '\u{0008}' | '\u{000b}' | '\u{000c}'));

    if !has_cr && !has_other_ctrl {
        return Cow::Borrowed(text);
    }

    let mut s = if has_cr {
        text.replace("\r\n", "\n").replace('\r', "\n")
    } else {
        text.to_string()
    };

    if has_other_ctrl {
        s.retain(|c| !matches!(c, '\u{0007}' | '\u{0008}' | '\u{000b}' | '\u{000c}'));
    }

    Cow::Owned(s)
}

/// Sanitizes text for single-line TUI display (tool summaries, errors).
///
/// Replaces all newlines and carriage returns with spaces, then strips
/// C0 control characters that can corrupt the terminal.
pub fn sanitize_tui_inline_text(text: &str) -> String {
    let mut s = text.replace("\r\n", " ").replace(['\r', '\n'], " ");
    s.retain(|c| !matches!(c, '\u{0007}' | '\u{0008}' | '\u{000b}' | '\u{000c}'));
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_short_string_unchanged() {
        assert_eq!(truncate_with_ellipsis("short", 10), "short");
        assert_eq!(truncate_with_ellipsis("", 5), "");
        assert_eq!(truncate_with_ellipsis("exact", 5), "exact");
    }

    #[test]
    fn test_long_string_truncated() {
        assert_eq!(
            truncate_with_ellipsis("this is a long string", 10),
            "this is..."
        );
        assert_eq!(truncate_with_ellipsis("abcdef", 3), "...");
    }

    #[test]
    fn test_utf8_boundaries_arrows() {
        // Arrow characters are 3 bytes each in UTF-8
        let arrows = "→→→→→→→→";
        assert_eq!(truncate_with_ellipsis(arrows, 5), "→→...");
    }

    #[test]
    fn test_utf8_boundaries_mixed() {
        let mixed = "a→b→c→d";
        assert_eq!(truncate_with_ellipsis(mixed, 5), "a→...");
    }

    #[test]
    fn test_utf8_boundaries_emoji() {
        // Emojis are 4 bytes each in UTF-8
        let emoji = "🎉🎊🎁🎄";
        assert_eq!(truncate_with_ellipsis(emoji, 3), "...");
    }

    #[test]
    fn test_utf8_complex_emoji() {
        // Rust crab emoji
        let s = "hi 🦀 there";
        // "hi 🦀" = 4 characters (h, i, space, 🦀)
        assert_eq!(truncate_with_ellipsis(s, 4), "h...");
    }

    #[test]
    fn test_zero_max_chars() {
        assert_eq!(truncate_with_ellipsis("hello", 0), "");
    }

    #[test]
    fn test_single_char_truncation() {
        assert_eq!(truncate_with_ellipsis("hello", 1), "h");
        assert_eq!(truncate_with_ellipsis("🎉hello", 1), "🎉");
    }

    #[test]
    fn sanitize_block_normalizes_crlf() {
        let result = sanitize_tui_block_text("line1\r\nline2\rline3");
        assert_eq!(result.as_ref(), "line1\nline2\nline3");
    }

    #[test]
    fn sanitize_block_strips_control_chars() {
        let result = sanitize_tui_block_text("hello\u{0007}world\u{000b}!");
        assert_eq!(result.as_ref(), "helloworld!");
    }

    #[test]
    fn sanitize_block_borrows_clean_text() {
        let result = sanitize_tui_block_text("clean text\n\ttabs ok");
        assert!(matches!(result, Cow::Borrowed(_)));
    }

    #[test]
    fn sanitize_inline_replaces_newlines_with_spaces() {
        let s = "line1\r\nline2\nline3\rline4";
        assert_eq!(sanitize_tui_inline_text(s), "line1 line2 line3 line4");
    }

    #[test]
    fn sanitize_inline_strips_control_chars() {
        let s = "hello\u{0007}\u{0008}world";
        assert_eq!(sanitize_tui_inline_text(s), "helloworld");
    }
}
