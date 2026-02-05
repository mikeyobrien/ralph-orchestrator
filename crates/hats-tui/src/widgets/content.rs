//! Content pane widget for rendering iteration output.
//!
//! This widget replaces the VT100 terminal widget with a simpler line-based
//! renderer that displays formatted Lines from an IterationBuffer.

use crate::state::IterationBuffer;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Widget,
};
use unicode_width::UnicodeWidthChar;

/// Widget that renders the content of an iteration buffer.
///
/// The widget displays the visible lines from the buffer (respecting scroll offset)
/// and optionally highlights search matches.
pub struct ContentPane<'a> {
    /// Reference to the iteration buffer to render
    buffer: &'a IterationBuffer,
    /// Optional search query for highlighting matches
    search_query: Option<&'a str>,
}

impl<'a> ContentPane<'a> {
    /// Creates a new ContentPane for the given iteration buffer.
    pub fn new(buffer: &'a IterationBuffer) -> Self {
        Self {
            buffer,
            search_query: None,
        }
    }

    /// Sets the search query for highlighting matches.
    pub fn with_search(mut self, query: &'a str) -> Self {
        if !query.is_empty() {
            self.search_query = Some(query);
        }
        self
    }
}

impl Widget for ContentPane<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.width == 0 || area.height == 0 {
            return;
        }

        // Get visible lines from the buffer (now returns owned Vec due to interior mutability)
        let visible = self.buffer.visible_lines(area.height as usize);

        let mut y = area.y;
        for line in &visible {
            if y >= area.y + area.height {
                break;
            }

            // Apply search highlighting if we have a query
            let rendered_line = if let Some(query) = self.search_query {
                highlight_search_matches(line, query)
            } else {
                line.clone()
            };

            // Render the line into the buffer with soft wrapping
            let mut x = area.x;
            let right_edge = area.x + area.width;
            for span in &rendered_line.spans {
                let content = span.content.as_ref();
                for ch in content.chars() {
                    let char_width = ch.width().unwrap_or(0) as u16;

                    // Skip zero-width characters (combining marks, etc.)
                    if char_width == 0 {
                        continue;
                    }

                    // Soft wrap: if the character won't fit on this row, move to next row
                    if x + char_width > right_edge {
                        // Clear any remaining cells on this row
                        while x < right_edge {
                            buf[(x, y)].set_char(' ').set_style(Style::default());
                            x += 1;
                        }
                        y += 1;
                        x = area.x;
                        // Stop if we've filled the viewport
                        if y >= area.y + area.height {
                            return;
                        }
                    }

                    buf[(x, y)].set_char(ch).set_style(span.style);
                    // Reset trailing cells for wide characters
                    let next_x = x + char_width;
                    x += 1;
                    while x < next_x {
                        buf[(x, y)].reset();
                        x += 1;
                    }
                }
            }

            // Clear remaining cells on this row after the line content
            while x < area.x + area.width {
                buf[(x, y)].set_char(' ').set_style(Style::default());
                x += 1;
            }

            // Move to the next row for the next logical line
            y += 1;
        }

        // Clear remaining rows below the content to prevent artifacts
        // when switching to an iteration with fewer lines
        while y < area.y + area.height {
            for x in area.x..area.x + area.width {
                buf[(x, y)].set_char(' ').set_style(Style::default());
            }
            y += 1;
        }
    }
}

/// Highlights search matches in a line with a distinct style.
fn highlight_search_matches(line: &Line<'static>, query: &str) -> Line<'static> {
    if query.is_empty() {
        return line.clone();
    }

    let query_lower = query.to_lowercase();
    let highlight_style = Style::default()
        .fg(Color::Black)
        .bg(Color::Yellow)
        .add_modifier(Modifier::REVERSED);

    let mut new_spans = Vec::new();

    for span in &line.spans {
        let content = span.content.as_ref();
        let content_lower = content.to_lowercase();
        let mut last_end = 0;

        // Find all matches in this span's content
        for (match_start, _) in content_lower.match_indices(&query_lower) {
            let match_end = match_start + query.len();

            // Add the part before the match with original style
            if match_start > last_end {
                new_spans.push(Span::styled(
                    content[last_end..match_start].to_string(),
                    span.style,
                ));
            }

            // Add the matched part with highlight style
            new_spans.push(Span::styled(
                content[match_start..match_end].to_string(),
                highlight_style,
            ));

            last_end = match_end;
        }

        // Add any remaining content after the last match
        if last_end < content.len() {
            new_spans.push(Span::styled(content[last_end..].to_string(), span.style));
        } else if last_end == 0 {
            // No matches found, keep original span
            new_spans.push(span.clone());
        }
    }

    Line::from(new_spans)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    /// Helper to render ContentPane and return buffer content as strings
    fn render_content_pane(
        buffer: &IterationBuffer,
        search: Option<&str>,
        width: u16,
        height: u16,
    ) -> Vec<String> {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                let mut widget = ContentPane::new(buffer);
                if let Some(q) = search {
                    widget = widget.with_search(q);
                }
                f.render_widget(widget, f.area());
            })
            .unwrap();

        let buf = terminal.backend().buffer();
        // Extract lines from the buffer
        (0..height)
            .map(|y| {
                (0..width)
                    .map(|x| buf[(x, y)].symbol().to_string())
                    .collect::<String>()
            })
            .collect()
    }

    /// Helper to check if a cell has the highlight style
    fn has_highlight_style(
        buffer: &IterationBuffer,
        search: &str,
        width: u16,
        height: u16,
        x: u16,
        y: u16,
    ) -> bool {
        let backend = TestBackend::new(width, height);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                let widget = ContentPane::new(buffer).with_search(search);
                f.render_widget(widget, f.area());
            })
            .unwrap();

        let buf = terminal.backend().buffer();
        let cell = &buf[(x, y)];
        // Check for highlight: typically reverse or yellow background
        cell.modifier.contains(Modifier::REVERSED)
            || cell.bg == Color::Yellow
            || cell.fg == Color::Black
    }

    // =========================================================================
    // Acceptance Criteria 1: Renders Lines
    // =========================================================================

    #[test]
    fn renders_lines_when_viewport_fits_all() {
        // Given a buffer with 3 lines
        let mut buffer = IterationBuffer::new(1);
        buffer.append_line(Line::from("first line"));
        buffer.append_line(Line::from("second line"));
        buffer.append_line(Line::from("third line"));

        // When ContentPane renders with viewport height >= 3
        let lines = render_content_pane(&buffer, None, 40, 5);

        // Then all 3 lines are visible in the output
        assert!(
            lines[0].contains("first line"),
            "first line should be visible, got: {:?}",
            lines
        );
        assert!(
            lines[1].contains("second line"),
            "second line should be visible, got: {:?}",
            lines
        );
        assert!(
            lines[2].contains("third line"),
            "third line should be visible, got: {:?}",
            lines
        );
    }

    #[test]
    fn renders_lines_preserves_styling() {
        // Given a buffer with styled lines
        let mut buffer = IterationBuffer::new(1);
        buffer.append_line(Line::from(vec![
            Span::styled("error: ", Style::default().fg(Color::Red)),
            Span::raw("something went wrong"),
        ]));

        // When ContentPane renders
        let backend = TestBackend::new(40, 3);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                let widget = ContentPane::new(&buffer);
                f.render_widget(widget, f.area());
            })
            .unwrap();

        // Then the styled spans are rendered (check color of first cell)
        let buf = terminal.backend().buffer();
        // The 'e' in 'error' should be red
        assert_eq!(
            buf[(0, 0)].fg,
            Color::Red,
            "styled span should preserve color"
        );
    }

    // =========================================================================
    // Acceptance Criteria 2: Respects Scroll Offset
    // =========================================================================

    #[test]
    fn respects_scroll_offset() {
        // Given a buffer with 10 lines and scroll_offset 5
        let mut buffer = IterationBuffer::new(1);
        for i in 0..10 {
            buffer.append_line(Line::from(format!("line {}", i)));
        }
        buffer.scroll_offset = 5;

        // When ContentPane renders with viewport height 5
        let lines = render_content_pane(&buffer, None, 40, 5);

        // Then lines 5-9 are shown (not 0-4)
        assert!(
            lines[0].contains("line 5"),
            "should show line 5 first, got: {:?}",
            lines
        );
        assert!(
            lines[4].contains("line 9"),
            "should show line 9 last, got: {:?}",
            lines
        );
        assert!(
            !lines.iter().any(|l| l.contains("line 0")),
            "line 0 should not be visible"
        );
    }

    #[test]
    fn scroll_offset_at_end_shows_last_lines() {
        let mut buffer = IterationBuffer::new(1);
        for i in 0..10 {
            buffer.append_line(Line::from(format!("line {}", i)));
        }
        buffer.scroll_bottom(3); // viewport 3, should show lines 7-9

        let lines = render_content_pane(&buffer, None, 40, 3);

        assert!(
            lines[0].contains("line 7"),
            "first visible should be line 7, got: {:?}",
            lines
        );
        assert!(
            lines[2].contains("line 9"),
            "last visible should be line 9, got: {:?}",
            lines
        );
    }

    // =========================================================================
    // Acceptance Criteria 3: Search Highlight
    // =========================================================================

    #[test]
    fn search_highlights_matches() {
        // Given a buffer with lines containing "foo"
        let mut buffer = IterationBuffer::new(1);
        buffer.append_line(Line::from("this contains foo in the middle"));
        buffer.append_line(Line::from("no match here"));
        buffer.append_line(Line::from("foo at start"));

        // When ContentPane renders with search query "foo"
        // Then "foo" spans are highlighted (different style)
        // Check that the 'f' in 'foo' at position 14 (line 0) has highlight style
        assert!(
            has_highlight_style(&buffer, "foo", 40, 3, 14, 0),
            "search match 'foo' should be highlighted"
        );
    }

    #[test]
    fn search_highlights_multiple_matches_per_line() {
        let mut buffer = IterationBuffer::new(1);
        buffer.append_line(Line::from("foo and another foo here"));

        // Both occurrences should be highlighted
        assert!(
            has_highlight_style(&buffer, "foo", 40, 1, 0, 0),
            "first 'foo' should be highlighted"
        );
        assert!(
            has_highlight_style(&buffer, "foo", 40, 1, 16, 0),
            "second 'foo' should be highlighted"
        );
    }

    #[test]
    fn search_case_insensitive() {
        let mut buffer = IterationBuffer::new(1);
        buffer.append_line(Line::from("Contains FOO uppercase"));

        // Search for lowercase should match uppercase
        assert!(
            has_highlight_style(&buffer, "foo", 40, 1, 9, 0),
            "case-insensitive search should highlight FOO"
        );
    }

    #[test]
    fn empty_search_query_no_highlight() {
        let mut buffer = IterationBuffer::new(1);
        buffer.append_line(Line::from("some text"));

        // Empty search shouldn't highlight anything
        let backend = TestBackend::new(40, 1);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                let widget = ContentPane::new(&buffer).with_search("");
                f.render_widget(widget, f.area());
            })
            .unwrap();

        let buf = terminal.backend().buffer();
        // No cells should have highlight modifier
        for x in 0..40 {
            assert!(
                !buf[(x, 0)].modifier.contains(Modifier::REVERSED),
                "empty search should not highlight"
            );
        }
    }

    // =========================================================================
    // Acceptance Criteria 4: Empty Buffer Handling
    // =========================================================================

    #[test]
    fn empty_buffer_renders_without_panic() {
        // Given an empty IterationBuffer
        let buffer = IterationBuffer::new(1);

        // When ContentPane renders
        // Then no panic occurs and empty area is shown
        let lines = render_content_pane(&buffer, None, 40, 5);

        // All lines should be empty (spaces)
        for line in &lines {
            assert!(
                line.trim().is_empty(),
                "empty buffer should render blank lines, got: {:?}",
                line
            );
        }
    }

    #[test]
    fn empty_buffer_with_search_renders_without_panic() {
        let buffer = IterationBuffer::new(1);

        // Should not panic even with search query on empty buffer
        let lines = render_content_pane(&buffer, Some("search"), 40, 5);

        for line in &lines {
            assert!(line.trim().is_empty());
        }
    }

    // =========================================================================
    // Acceptance Criteria 5: Widget Integration
    // =========================================================================

    #[test]
    fn widget_fills_provided_rect() {
        let mut buffer = IterationBuffer::new(1);
        buffer.append_line(Line::from("test"));

        let backend = TestBackend::new(80, 24);
        let mut terminal = Terminal::new(backend).unwrap();

        // Render into a specific sub-area
        let area = Rect::new(5, 5, 30, 10);
        terminal
            .draw(|f| {
                let widget = ContentPane::new(&buffer);
                f.render_widget(widget, area);
            })
            .unwrap();

        // Content should be at position (5, 5), not (0, 0)
        let buf = terminal.backend().buffer();
        assert_eq!(buf[(5, 5)].symbol(), "t", "content should start at area.x");
    }

    #[test]
    fn widget_wraps_lines_at_area_width() {
        let mut buffer = IterationBuffer::new(1);
        buffer.append_line(Line::from(
            "this is a very long line that exceeds the width",
        ));

        // Render with narrow width and enough height for wrapping
        let lines = render_content_pane(&buffer, None, 20, 3);

        // First row should have first 20 chars
        assert!(
            lines[0].starts_with("this is a very long "),
            "first row should have first 20 chars, got: {:?}",
            lines[0]
        );
        // Second row should have continuation
        assert!(
            lines[1].starts_with("line that exceeds th"),
            "second row should have continuation, got: {:?}",
            lines[1]
        );
        // Third row should have the rest
        assert!(
            lines[2].starts_with("e width"),
            "third row should have remaining text, got: {:?}",
            lines[2]
        );
    }

    // =========================================================================
    // Acceptance Criteria 5b: Wide Character Handling
    // =========================================================================

    #[test]
    fn wide_chars_do_not_shift_subsequent_text() {
        // Given a line with emoji (2-column wide) followed by ASCII text
        let mut buffer = IterationBuffer::new(1);
        buffer.append_line(Line::from("🔨 Builder"));

        // When ContentPane renders
        let lines = render_content_pane(&buffer, None, 40, 1);

        // Then "Builder" should appear at the correct position (after emoji + space)
        // 🔨 takes 2 columns, space takes 1, so "Builder" starts at column 3
        assert!(
            lines[0].contains("Builder"),
            "text after emoji should be intact, got: {:?}",
            lines[0]
        );
    }

    #[test]
    fn wide_char_at_edge_wraps_to_next_line() {
        // Given a line where a wide character would straddle the right edge
        // With width 5: "abcd🔨" - 'abcd' fills 4 cols, emoji needs 2 but only 1 left
        let mut buffer = IterationBuffer::new(1);
        buffer.append_line(Line::from("abcd🔨x"));

        // When rendered at width 5, emoji should wrap to next line
        let lines = render_content_pane(&buffer, None, 5, 3);

        // First row: "abcd " (4 chars + 1 cleared cell since emoji doesn't fit)
        assert_eq!(
            lines[0].trim_end(),
            "abcd",
            "first row should have 'abcd', got: {:?}",
            lines[0]
        );
        // Second row: "🔨x" (emoji + 'x')
        assert!(
            lines[1].contains('x'),
            "second row should have the emoji and 'x', got: {:?}",
            lines[1]
        );
    }

    #[test]
    fn multiple_wide_chars_render_correctly() {
        // Given a line with multiple emoji
        let mut buffer = IterationBuffer::new(1);
        buffer.append_line(Line::from("★ Insight ─────"));

        // When rendered wide enough
        let lines = render_content_pane(&buffer, None, 40, 1);

        // Then all characters should be present without garbling
        let rendered = lines[0].trim_end();
        assert!(
            rendered.contains("Insight"),
            "content should be intact, got: {:?}",
            rendered
        );
    }

    // =========================================================================
    // Acceptance Criteria 6: Buffer Clearing (Artifact Prevention)
    // =========================================================================

    #[test]
    fn clears_remaining_rows_when_content_shorter_than_viewport() {
        // Given a pre-filled ratatui buffer (simulating previous frame's content)
        let area = Rect::new(0, 0, 40, 10);
        let mut buf = Buffer::empty(area);

        // Pre-fill the buffer with "X" characters to simulate previous iteration content
        for y in 0..10 {
            for x in 0..40 {
                buf[(x, y)].set_char('X');
            }
        }

        // And an IterationBuffer with only 3 lines
        let mut iter_buffer = IterationBuffer::new(1);
        iter_buffer.append_line(Line::from("line one"));
        iter_buffer.append_line(Line::from("line two"));
        iter_buffer.append_line(Line::from("line three"));

        // When ContentPane renders (only 3 lines of content)
        let widget = ContentPane::new(&iter_buffer);
        widget.render(area, &mut buf);

        // Then rows 0-2 should have the content
        assert!(
            buf[(0, 0)].symbol() == "l",
            "row 0 should have content, got: {}",
            buf[(0, 0)].symbol()
        );

        // And rows 3-9 should be cleared (no 'X' artifacts)
        for y in 3..10 {
            for x in 0..40 {
                let symbol = buf[(x, y)].symbol();
                assert!(
                    symbol != "X",
                    "row {} col {} should be cleared, but found artifact 'X'",
                    y,
                    x
                );
            }
        }
    }
}
