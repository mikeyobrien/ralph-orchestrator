use crate::scroll::ScrollManager;
use crate::state::TuiState;
use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

pub fn render(state: &TuiState, scroll_manager: &ScrollManager) -> Paragraph<'static> {
    // If in search input mode, show search prompt
    if !state.search_query.is_empty() || state.in_scroll_mode {
        if let Some(search_state) = scroll_manager.search_state() {
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

            return Paragraph::new(line).block(Block::default().borders(Borders::ALL));
        }

        // Show search input prompt
        if !state.search_query.is_empty() {
            let prompt = if state.search_forward { "/" } else { "?" };
            let line = Line::from(vec![
                Span::raw(" "),
                Span::styled(
                    format!("{}{}", prompt, state.search_query),
                    Style::default().fg(Color::Yellow),
                ),
            ]);

            return Paragraph::new(line).block(Block::default().borders(Borders::ALL));
        }
    }

    // Default footer
    let last_event = state
        .last_event
        .as_ref()
        .map(|e| format!("Last: {}", e))
        .unwrap_or_else(|| "Last: —".to_string());

    let indicator = if state.pending_hat.is_none() {
        Span::styled("■ done", Style::default().fg(Color::Blue))
    } else if state.is_active() {
        Span::styled("◉ active", Style::default().fg(Color::Green))
    } else {
        Span::styled(
            "◯ idle",
            Style::default()
                .fg(Color::DarkGray)
                .add_modifier(Modifier::DIM),
        )
    };

    let line = Line::from(vec![
        Span::raw(" "),
        Span::raw(last_event),
        Span::raw("                              "),
        indicator,
        Span::raw(" "),
    ]);

    Paragraph::new(line).block(Block::default().borders(Borders::ALL))
}
