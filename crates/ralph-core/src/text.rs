//! Text utilities for the Ralph Orchestrator.
//!
//! This module provides common text manipulation functions used throughout
//! the codebase, including UTF-8 safe string truncation.

/// Finds the largest byte index <= `index` that is a valid UTF-8 character boundary.
///
/// This is needed because Rust strings cannot be sliced at arbitrary byte positions -
/// only at valid character boundaries. Multi-byte characters (emojis, etc.) would cause
/// a panic if sliced in the middle.
///
/// This is a stable Rust implementation of `str::floor_char_boundary` (nightly-only).
///
/// # Examples
///
/// ```
/// use ralph_core::floor_char_boundary;
///
/// let s = "Hello 🦀 World";  // 🦀 is at bytes 6-9
/// assert_eq!(floor_char_boundary(s, 6), 6);   // At start of emoji - valid boundary
/// assert_eq!(floor_char_boundary(s, 7), 6);   // Inside emoji - returns start
/// assert_eq!(floor_char_boundary(s, 8), 6);   // Inside emoji - returns start
/// assert_eq!(floor_char_boundary(s, 10), 10); // After emoji - valid boundary
/// ```
#[must_use]
pub fn floor_char_boundary(s: &str, index: usize) -> usize {
    if index >= s.len() {
        return s.len();
    }
    // Walk backwards from index until we find a valid char boundary
    let mut boundary = index;
    while boundary > 0 && !s.is_char_boundary(boundary) {
        boundary -= 1;
    }
    boundary
}

/// Truncates a string to a maximum number of characters, adding "..." if truncated.
///
/// This function is UTF-8 safe: it uses character boundaries, not byte boundaries,
/// so it will never split a multi-byte character (emoji, non-ASCII, etc.).
///
/// # Arguments
///
/// * `s` - The string to truncate
/// * `max_chars` - Maximum number of characters (not bytes) before truncation
///
/// # Returns
///
/// - The original string if its character count is <= `max_chars`
/// - A truncated string with "..." appended if longer
///
/// # Examples
///
/// ```
/// use ralph_core::truncate_with_ellipsis;
///
/// // Short strings pass through unchanged
/// assert_eq!(truncate_with_ellipsis("hello", 10), "hello");
///
/// // Long strings are truncated with ellipsis
/// assert_eq!(truncate_with_ellipsis("hello world", 5), "hello...");
///
/// // UTF-8 safe: emojis are not split
/// assert_eq!(truncate_with_ellipsis("🎉🎊🎁🎄", 2), "🎉🎊...");
/// ```
pub fn truncate_with_ellipsis(s: &str, max_chars: usize) -> String {
    if s.chars().count() <= max_chars {
        s.to_string()
    } else {
        // Find the byte index of the max_chars-th character
        // This ensures we never slice in the middle of a multi-byte character
        let byte_idx = s
            .char_indices()
            .nth(max_chars)
            .map(|(idx, _)| idx)
            .unwrap_or(s.len());
        format!("{}...", &s[..byte_idx])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_floor_char_boundary_ascii() {
        let s = "hello";
        assert_eq!(floor_char_boundary(s, 0), 0);
        assert_eq!(floor_char_boundary(s, 3), 3);
        assert_eq!(floor_char_boundary(s, 5), 5);
        assert_eq!(floor_char_boundary(s, 10), 5); // Beyond string length
    }

    #[test]
    fn test_floor_char_boundary_emoji() {
        // 🦀 is 4 bytes (U+1F980)
        let s = "hi🦀ok"; // h=0, i=1, 🦀=2-5, o=6, k=7
        assert_eq!(floor_char_boundary(s, 2), 2); // Start of emoji
        assert_eq!(floor_char_boundary(s, 3), 2); // Inside emoji
        assert_eq!(floor_char_boundary(s, 4), 2); // Inside emoji
        assert_eq!(floor_char_boundary(s, 5), 2); // Inside emoji
        assert_eq!(floor_char_boundary(s, 6), 6); // After emoji
    }

    #[test]
    fn test_floor_char_boundary_checkmark() {
        // ✅ is 3 bytes (U+2705)
        let s = "a✅b"; // a=0, ✅=1-3, b=4
        assert_eq!(floor_char_boundary(s, 1), 1); // Start of checkmark
        assert_eq!(floor_char_boundary(s, 2), 1); // Inside checkmark
        assert_eq!(floor_char_boundary(s, 3), 1); // Inside checkmark
        assert_eq!(floor_char_boundary(s, 4), 4); // At 'b'
    }

    #[test]
    fn test_floor_char_boundary_empty() {
        assert_eq!(floor_char_boundary("", 0), 0);
        assert_eq!(floor_char_boundary("", 5), 0);
    }

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
            "this is a ..."
        );
        assert_eq!(truncate_with_ellipsis("abcdef", 3), "abc...");
    }

    #[test]
    fn test_utf8_boundaries_arrows() {
        // Arrow characters are 3 bytes each in UTF-8
        let arrows = "→→→→→→→→";
        assert_eq!(truncate_with_ellipsis(arrows, 5), "→→→→→...");
    }

    #[test]
    fn test_utf8_boundaries_mixed() {
        let mixed = "a→b→c→d";
        assert_eq!(truncate_with_ellipsis(mixed, 5), "a→b→c...");
    }

    #[test]
    fn test_utf8_boundaries_emoji() {
        // Emojis are 4 bytes each in UTF-8
        let emoji = "🎉🎊🎁🎄";
        assert_eq!(truncate_with_ellipsis(emoji, 3), "🎉🎊🎁...");
    }

    #[test]
    fn test_utf8_complex_emoji() {
        // Rust crab emoji
        let s = "hi 🦀 there";
        // "hi 🦀" = 4 characters (h, i, space, 🦀)
        assert_eq!(truncate_with_ellipsis(s, 4), "hi 🦀...");
    }

    #[test]
    fn test_zero_max_chars() {
        assert_eq!(truncate_with_ellipsis("hello", 0), "...");
    }

    #[test]
    fn test_single_char_truncation() {
        assert_eq!(truncate_with_ellipsis("hello", 1), "h...");
        assert_eq!(truncate_with_ellipsis("🎉hello", 1), "🎉...");
    }
}
