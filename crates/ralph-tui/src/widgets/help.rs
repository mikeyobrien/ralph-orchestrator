//! Help overlay widget.

use ratatui::{
    Frame,
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};

/// Renders help overlay centered on screen.
pub fn render(f: &mut Frame, area: Rect) {
    let block = Block::default()
        .title(" Help ")
        .borders(Borders::ALL)
        .style(Style::default().bg(Color::Black).fg(Color::White));

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
        Line::from(Span::styled(
            "Guidance:",
            Style::default().fg(Color::Yellow),
        )),
        Line::from(vec![
            Span::styled("  :", Style::default().fg(Color::Cyan)),
            Span::raw("      Send guidance (next iteration)"),
        ]),
        Line::from(vec![
            Span::styled("  !", Style::default().fg(Color::Cyan)),
            Span::raw("      Send guidance (now, current iteration)"),
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

    let paragraph = Paragraph::new(help_text)
        .block(block)
        .alignment(Alignment::Left);

    let popup_area = centered_rect(50, 60, area);
    f.render_widget(Clear, popup_area);
    f.render_widget(paragraph, popup_area);
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
