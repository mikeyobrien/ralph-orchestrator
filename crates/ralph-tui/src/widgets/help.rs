//! Help overlay widget.

use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};

/// Renders help overlay centered on screen.
///
/// When `autoloop_source` is true the TUI is fed by the autoloop `--events`
/// stream, which has no back-channel to the subprocess — guidance/steer keys
/// no-op, so their help rows are greyed out and annotated (FIX gap#6).
pub fn render(f: &mut Frame, area: Rect, autoloop_source: bool) {
    let block = Block::default()
        .title(" Help ")
        .borders(Borders::ALL)
        .style(Style::default().bg(Color::Black).fg(Color::White));

    // Guidance keys (`:` / `!`) have no effect under the autoloop source; grey
    // them out instead of advertising live bindings that do nothing.
    let (guidance_heading_style, guidance_key_style, guidance_suffix) = if autoloop_source {
        (
            Style::default().fg(Color::DarkGray),
            Style::default().fg(Color::DarkGray),
            " (unavailable in this mode)",
        )
    } else {
        (
            Style::default().fg(Color::Yellow),
            Style::default().fg(Color::Cyan),
            "",
        )
    };

    let help_text = vec![
        Line::from(Span::styled(
            "Navigation:",
            Style::default().fg(Color::Yellow),
        )),
        Line::from(vec![
            Span::styled("  h/←", Style::default().fg(Color::Cyan)),
            Span::raw("    Previous iteration"),
        ]),
        Line::from(vec![
            Span::styled("  l/→", Style::default().fg(Color::Cyan)),
            Span::raw("    Next iteration"),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "Scrolling:",
            Style::default().fg(Color::Yellow),
        )),
        Line::from(vec![
            Span::styled("  j/↓", Style::default().fg(Color::Cyan)),
            Span::raw("    Scroll down"),
        ]),
        Line::from(vec![
            Span::styled("  k/↑", Style::default().fg(Color::Cyan)),
            Span::raw("    Scroll up"),
        ]),
        Line::from(vec![
            Span::styled("  g", Style::default().fg(Color::Cyan)),
            Span::raw("      Scroll to top"),
        ]),
        Line::from(vec![
            Span::styled("  G", Style::default().fg(Color::Cyan)),
            Span::raw("      Scroll to bottom"),
        ]),
        Line::from(vec![
            Span::styled("  m", Style::default().fg(Color::Cyan)),
            Span::raw("      Toggle mouse mode (select/scroll)"),
        ]),
        Line::from(""),
        Line::from(Span::styled("Search:", Style::default().fg(Color::Yellow))),
        Line::from(vec![
            Span::styled("  /", Style::default().fg(Color::Cyan)),
            Span::raw("      Start search"),
        ]),
        Line::from(vec![
            Span::styled("  n/N", Style::default().fg(Color::Cyan)),
            Span::raw("    Next/prev match"),
        ]),
        Line::from(""),
        Line::from(Span::styled("Export:", Style::default().fg(Color::Yellow))),
        Line::from(vec![
            Span::styled("  e", Style::default().fg(Color::Cyan)),
            Span::raw("      Export current iteration"),
        ]),
        Line::from(vec![
            Span::styled("  E", Style::default().fg(Color::Cyan)),
            Span::raw("      Export all iterations"),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            format!("Guidance:{guidance_suffix}"),
            guidance_heading_style,
        )),
        Line::from(vec![
            Span::styled("  :", guidance_key_style),
            Span::styled("      Send guidance (next prompt)", guidance_heading_style),
        ]),
        Line::from(vec![
            Span::styled("  !", guidance_key_style),
            Span::styled(
                "      Urgent steer (blocks handoff until seen)",
                guidance_heading_style,
            ),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "Wave Workers:",
            Style::default().fg(Color::Yellow),
        )),
        Line::from(vec![
            Span::styled("  w", Style::default().fg(Color::Cyan)),
            Span::raw("      Enter wave worker view"),
        ]),
        Line::from(vec![
            Span::styled("  h/l", Style::default().fg(Color::Cyan)),
            Span::raw("    Cycle through workers"),
        ]),
        Line::from(vec![
            Span::styled("  Esc", Style::default().fg(Color::Cyan)),
            Span::raw("    Exit wave view"),
        ]),
        Line::from(""),
        Line::from(Span::styled("Other:", Style::default().fg(Color::Yellow))),
        Line::from(vec![
            Span::styled("  q", Style::default().fg(Color::Cyan)),
            Span::raw("      Quit"),
        ]),
        Line::from(vec![
            Span::styled("  ?", Style::default().fg(Color::Cyan)),
            Span::raw("      Show this help"),
        ]),
        Line::from(vec![
            Span::styled("  Esc", Style::default().fg(Color::Cyan)),
            Span::raw("    Dismiss/cancel"),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "Press Esc to dismiss",
            Style::default().fg(Color::DarkGray),
        )),
    ];

    // Size the popup to its content (plus borders) so every binding stays
    // visible; a fixed percentage height silently clips the tail of the
    // help text on short terminals.
    let content_height = help_text.len() as u16 + 2;
    let paragraph = Paragraph::new(help_text)
        .block(block)
        .alignment(Alignment::Left);

    let popup_area = centered_rect_fixed_height(50, content_height, area);
    f.render_widget(Clear, popup_area);
    f.render_widget(paragraph, popup_area);
}

/// Center a popup horizontally at `percent_x` width with an exact height,
/// clamped to the available area.
fn centered_rect_fixed_height(percent_x: u16, height: u16, r: Rect) -> Rect {
    let height = height.min(r.height);
    let top = (r.height - height) / 2;
    let vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(top),
            Constraint::Length(height),
            Constraint::Min(0),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(vertical[1])[1]
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    fn render_to_string(autoloop_source: bool) -> String {
        let backend = TestBackend::new(100, 40);
        let mut terminal = Terminal::new(backend).unwrap();
        terminal
            .draw(|f| render(f, f.area(), autoloop_source))
            .unwrap();
        terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol())
            .collect::<String>()
    }

    #[test]
    fn guidance_section_plain_in_normal_mode() {
        let text = render_to_string(false);
        assert!(text.contains("Guidance:"), "should show guidance section");
        assert!(
            !text.contains("unavailable in this mode"),
            "normal mode should not mark guidance unavailable, got: {text}"
        );
    }

    #[test]
    fn guidance_section_marked_unavailable_under_autoloop_source() {
        let text = render_to_string(true);
        assert!(
            text.contains("unavailable in this mode"),
            "autoloop source should mark guidance unavailable, got: {text}"
        );
    }
}
