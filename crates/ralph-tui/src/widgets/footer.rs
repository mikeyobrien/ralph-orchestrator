use crate::scroll::ScrollManager;
use crate::state::TuiState;
use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Widget},
};

/// Footer widget that adapts to terminal width.
pub struct Footer<'a> {
    state: &'a TuiState,
    scroll_manager: &'a ScrollManager,
}

impl<'a> Footer<'a> {
    pub fn new(state: &'a TuiState, scroll_manager: &'a ScrollManager) -> Self {
        Self {
            state,
            scroll_manager,
        }
    }
}

impl Widget for Footer<'_> {
    fn render(self, area: Rect, buf: &mut ratatui::buffer::Buffer) {
        let block = Block::default().borders(Borders::ALL);
        let inner = block.inner(area);
        block.render(area, buf);

        // If in search mode, render search display
        if !self.state.search_query.is_empty() || self.state.in_scroll_mode {
            if let Some(search_state) = self.scroll_manager.search_state() {
                let match_info = if search_state.matches.is_empty() {
                    "no matches".to_string()
                } else {
                    format!(
                        "{}/{}",
                        search_state.current_match + 1,
                        search_state.matches.len()
                    )
                };

                let line = Line::from(vec![
                    Span::raw(" "),
                    Span::styled(
                        format!("Search: {} ", search_state.query),
                        Style::default().fg(Color::Yellow),
                    ),
                    Span::styled(match_info, Style::default().fg(Color::Cyan)),
                ]);

                Paragraph::new(line).render(inner, buf);
                return;
            }

            // Show search input prompt
            if !self.state.search_query.is_empty() {
                let prompt = if self.state.search_forward { "/" } else { "?" };
                let line = Line::from(vec![
                    Span::raw(" "),
                    Span::styled(
                        format!("{}{}", prompt, self.state.search_query),
                        Style::default().fg(Color::Yellow),
                    ),
                ]);

                Paragraph::new(line).render(inner, buf);
                return;
            }
        }

        // Default footer with flexible layout
        let last_event = self
            .state
            .last_event
            .as_ref()
            .map_or_else(|| "Last: —".to_string(), |e| format!("Last: {e}"));

        let indicator_text = if self.state.pending_hat.is_none() {
            "■ done"
        } else if self.state.is_active() {
            "◉ active"
        } else {
            "◯ idle"
        };

        let indicator_style = if self.state.pending_hat.is_none() {
            Style::default().fg(Color::Blue)
        } else if self.state.is_active() {
            Style::default().fg(Color::Green)
        } else {
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::DIM)
        };

        // Use horizontal layout: left content | flexible spacer | right indicator
        let chunks = Layout::horizontal([
            Constraint::Length((last_event.len() + 2) as u16), // " Last: event"
            Constraint::Fill(1),                               // Flexible spacer
            Constraint::Length((indicator_text.len() + 2) as u16), // "indicator "
        ])
        .split(inner);

        // Render left side (last event)
        let left = Line::from(vec![Span::raw(" "), Span::raw(last_event)]);
        Paragraph::new(left).render(chunks[0], buf);

        // Render right side (indicator)
        let right = Line::from(vec![
            Span::styled(indicator_text, indicator_style),
            Span::raw(" "),
        ]);
        Paragraph::new(right).render(chunks[2], buf);
    }
}

/// Convenience function matching original API.
pub fn render<'a>(state: &'a TuiState, scroll_manager: &'a ScrollManager) -> Footer<'a> {
    Footer::new(state, scroll_manager)
}
