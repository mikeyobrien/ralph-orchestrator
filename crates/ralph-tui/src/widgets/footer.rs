use crate::state::TuiState;
use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Widget},
};

/// Footer widget that adapts to terminal width.
pub struct Footer<'a> {
    state: &'a TuiState,
}

impl<'a> Footer<'a> {
    pub fn new(state: &'a TuiState) -> Self {
        Self { state }
    }
}

impl Widget for Footer<'_> {
    fn render(self, area: Rect, buf: &mut ratatui::buffer::Buffer) {
        // Render block with top border as separator
        let block = Block::default().borders(Borders::TOP);
        let inner_area = block.inner(area);
        block.render(area, buf);

        // Guidance input mode takes priority
        if let Some(mode) = self.state.guidance_mode {
            let label = match mode {
                crate::state::GuidanceMode::Next => "guidance (next)",
                crate::state::GuidanceMode::Now => "guidance (now!)",
            };
            let line = Line::from(vec![
                Span::raw(" "),
                Span::styled(format!("{}: ", label), Style::default().fg(Color::Yellow)),
                Span::raw(&self.state.guidance_input),
                Span::styled("\u{2588}", Style::default().fg(Color::Yellow)), // block cursor
            ]);
            Paragraph::new(line).render(inner_area, buf);
            return;
        }

        // Guidance flash (brief after attempting send)
        if let Some((mode, result)) = self.state.active_guidance_flash() {
            let (msg, color) = match (mode, result) {
                (crate::state::GuidanceMode::Next, crate::state::GuidanceResult::Queued) => {
                    ("\u{2713} guidance queued (next)", Color::Green)
                }
                (crate::state::GuidanceMode::Now, crate::state::GuidanceResult::Sent) => {
                    ("\u{2713} guidance sent (now!)", Color::Green)
                }
                (_, crate::state::GuidanceResult::Failed) => {
                    ("\u{2717} failed to send guidance", Color::Red)
                }
                // Shouldn't happen, but degrade gracefully
                _ => ("\u{2717} failed to send guidance", Color::Red),
            };

            let line = Line::from(vec![
                Span::raw(" "),
                Span::styled(msg, Style::default().fg(color)),
            ]);
            Paragraph::new(line).render(inner_area, buf);
            return;
        }

        // If search state has an active query, render search display
        if let Some(query) = &self.state.search_state.query {
            let match_info = if self.state.search_state.matches.is_empty() {
                "no matches".to_string()
            } else {
                format!(
                    "{}/{}",
                    self.state.search_state.current_match + 1,
                    self.state.search_state.matches.len()
                )
            };

            let line = Line::from(vec![
                Span::raw(" "),
                Span::styled(
                    format!("Search: {} ", query),
                    Style::default().fg(Color::Yellow),
                ),
                Span::styled(match_info, Style::default().fg(Color::Cyan)),
            ]);

            Paragraph::new(line).render(inner_area, buf);
            return;
        }

        // Show search input prompt (legacy fallback for when search_query is used)
        if !self.state.search_query.is_empty() {
            let prompt = if self.state.search_forward { "/" } else { "?" };
            let line = Line::from(vec![
                Span::raw(" "),
                Span::styled(
                    format!("{}{}", prompt, self.state.search_query),
                    Style::default().fg(Color::Yellow),
                ),
            ]);

            Paragraph::new(line).render(inner_area, buf);
            return;
        }

        // Default footer with flexible layout
        // Build left content: optional alert + elapsed time
        let mut left_spans = vec![Span::raw(" ")];

        // Show new iteration alert when viewing history and a new iteration arrived
        if let Some(iter_num) = self.state.new_iteration_alert
            && !self.state.following_latest
        {
            left_spans.push(Span::styled(
                format!("▶ New: iter {} ", iter_num),
                Style::default().fg(Color::Green),
            ));
            left_spans.push(Span::raw("│ "));
        }

        // Show total elapsed time (default to 00:00 if loop hasn't started)
        let elapsed_display = if let Some(elapsed) = self.state.get_loop_elapsed() {
            let total_secs = elapsed.as_secs();
            let mins = total_secs / 60;
            let secs = total_secs % 60;
            format!("Total Time Elapsed: {mins:02}:{secs:02}")
        } else {
            "Total Time Elapsed: 00:00".to_string()
        };
        left_spans.push(Span::raw(elapsed_display));

        let indicator_text = if self.state.loop_completed {
            "■ DONE"
        } else {
            "◉ ACTIVE"
        };

        let indicator_style = if self.state.loop_completed {
            Style::default().fg(Color::Blue)
        } else {
            Style::default().fg(Color::Green)
        };

        // Calculate left content width for layout
        let left_content_width: usize = left_spans.iter().map(|s| s.width()).sum();

        // Use horizontal layout: left content | flexible spacer | right indicator
        let chunks = Layout::horizontal([
            Constraint::Length(left_content_width as u16), // Alert + " Last: event"
            Constraint::Fill(1),                           // Flexible spacer
            Constraint::Length((indicator_text.len() + 2) as u16), // "indicator "
        ])
        .split(inner_area);

        // Render left side (alert + last event)
        let left = Line::from(left_spans);
        Paragraph::new(left).render(chunks[0], buf);

        // Render right side (indicator)
        let right = Line::from(vec![
            Span::styled(indicator_text, indicator_style),
            Span::raw(" "),
        ]);
        Paragraph::new(right).render(chunks[2], buf);
    }
}

/// Convenience function for rendering the footer.
pub fn render(state: &TuiState) -> Footer<'_> {
    Footer::new(state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    fn render_to_string(state: &TuiState) -> String {
        render_to_string_with_width(state, 80)
    }

    fn render_to_string_with_width(state: &TuiState, width: u16) -> String {
        // Height of 2: 1 for top border + 1 for content
        let backend = TestBackend::new(width, 2);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                let widget = render(state);
                f.render_widget(widget, f.area());
            })
            .unwrap();

        let buffer = terminal.backend().buffer();
        buffer
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect::<String>()
    }

    // =========================================================================
    // Acceptance Criteria Tests (Task 06)
    // =========================================================================

    #[test]
    fn footer_shows_new_iteration_alert() {
        // Given new_iteration_alert = Some(5) and following_latest = false
        let mut state = TuiState::new();
        state.new_iteration_alert = Some(5);
        state.following_latest = false;

        // When footer renders
        let text = render_to_string(&state);

        // Then output contains "▶ New: iter 5"
        assert!(
            text.contains("▶ New: iter 5"),
            "should show new iteration alert, got: {}",
            text
        );
    }

    #[test]
    fn footer_no_alert_when_following() {
        // Given following_latest = true (even if new_iteration_alert has a value)
        let mut state = TuiState::new();
        state.new_iteration_alert = Some(5);
        state.following_latest = true;

        // When footer renders
        let text = render_to_string(&state);

        // Then no alert is shown
        assert!(
            !text.contains("▶ New:"),
            "should NOT show alert when following_latest=true, got: {}",
            text
        );
    }

    #[test]
    fn footer_shows_elapsed_time() {
        // Given loop_started is set (simulating 2 minutes 30 seconds elapsed)
        let mut state = TuiState::new();
        state.loop_started = Some(
            std::time::Instant::now()
                .checked_sub(std::time::Duration::from_secs(150))
                .unwrap(),
        );

        // When footer renders
        let text = render_to_string(&state);

        // Then output contains "Total Time Elapsed: MM:SS" format
        assert!(
            text.contains("Total Time Elapsed: 02:30"),
            "should show 'Total Time Elapsed: 02:30', got: {}",
            text
        );
    }

    #[test]
    fn footer_shows_active_indicator() {
        // Given pending_hat is set (task in progress)
        let mut state = TuiState::new();
        state.pending_hat = Some((ralph_proto::HatId::new("builder"), "🔨Builder".to_string()));

        // When footer renders
        let text = render_to_string(&state);

        // Then output contains ◉ ACTIVE
        assert!(
            text.contains('◉') && text.contains("ACTIVE"),
            "should show ACTIVE indicator, got: {}",
            text
        );
    }

    #[test]
    fn footer_shows_search_query() {
        // Given search_state has an active query
        let mut state = TuiState::new();
        state.search_state.query = Some("test".to_string());
        state.search_state.matches = vec![(0, 0), (1, 0)]; // 2 matches

        // When footer renders
        let text = render_to_string(&state);

        // Then output contains "Search: test 1/2"
        assert!(
            text.contains("Search: test"),
            "should show search query, got: {}",
            text
        );
        assert!(
            text.contains("1/2"),
            "should show match position, got: {}",
            text
        );
    }

    #[test]
    fn footer_shows_no_matches_when_empty() {
        // Given search with no matches
        let mut state = TuiState::new();
        state.search_state.query = Some("notfound".to_string());
        state.search_state.matches = vec![];

        // When footer renders
        let text = render_to_string(&state);

        // Then output contains "no matches"
        assert!(
            text.contains("no matches"),
            "should show no matches indicator, got: {}",
            text
        );
    }

    #[test]
    fn footer_shows_done_indicator_when_complete() {
        // Given loop_completed = true (task complete after loop.terminate)
        let mut state = TuiState::new();
        state.loop_completed = true;

        // When footer renders
        let text = render_to_string(&state);

        // Then output contains ■ DONE
        assert!(
            text.contains('■') && text.contains("DONE"),
            "should show DONE indicator, got: {}",
            text
        );
    }

    #[test]
    fn footer_shows_active_at_startup() {
        // Given fresh state (loop not yet completed)
        let state = TuiState::new();

        // When footer renders
        let text = render_to_string(&state);

        // Then output contains ◉ ACTIVE (not DONE)
        assert!(
            text.contains('◉') && text.contains("ACTIVE"),
            "should show ACTIVE indicator at startup, got: {}",
            text
        );
    }
}
