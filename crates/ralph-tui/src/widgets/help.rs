//! Help overlay widget.

use ratatui::{
    Frame,
    layout::{Alignment, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph},
};

/// Renders help overlay centered on screen.
///
/// When `autoloop_source` is true the TUI is fed by the autoloop `--events`
/// stream, which has no back-channel to the subprocess, so the guidance keys
/// do nothing and are not listed.
pub fn render(f: &mut Frame, area: Rect, autoloop_source: bool) {
    let heading =
        |text: &'static str| Line::from(Span::styled(text, Style::default().fg(Color::Yellow)));
    let key = |keys: &'static str, text: &'static str| {
        Line::from(vec![
            Span::styled(format!("  {keys:<7}"), Style::default().fg(Color::Cyan)),
            Span::raw(text),
        ])
    };

    let mut sections = vec![
        vec![
            heading("Navigation:"),
            key("h/←", "Previous iteration"),
            key("l/→", "Next iteration"),
        ],
        vec![
            heading("Scrolling:"),
            key("j/↓", "Scroll down"),
            key("k/↑", "Scroll up"),
            key("g", "Scroll to top"),
            key("G", "Scroll to bottom"),
            key("m", "Toggle mouse mode (select/scroll)"),
        ],
        vec![
            heading("Search:"),
            key("/", "Start search"),
            key("n/N", "Next/prev match"),
        ],
        vec![
            heading("Export:"),
            key("e", "Export current iteration"),
            key("E", "Export all iterations"),
        ],
    ];
    if !autoloop_source {
        sections.push(vec![
            heading("Guidance:"),
            key(":", "Send guidance (next prompt)"),
            key("!", "Urgent steer (blocks handoff until seen)"),
        ]);
    }
    sections.push(vec![
        heading("Other:"),
        key("q", "Quit"),
        key("?", "Show this help"),
        key("Esc", "Dismiss/cancel"),
    ]);

    // Blank rows between sections only when every binding still fits; a
    // clipped overlay would hide the tail of the list.
    let rows: usize = sections.iter().map(Vec::len).sum();
    let spaced = rows + sections.len() - 1 + 2 <= area.height as usize;
    let mut help_text = Vec::new();
    for (index, section) in sections.into_iter().enumerate() {
        if spaced && index > 0 {
            help_text.push(Line::from(""));
        }
        help_text.extend(section);
    }

    let content_width = help_text.iter().map(Line::width).max().unwrap_or(0);
    let width = (content_width as u16 + 4).min(area.width);
    let height = (help_text.len() as u16 + 2).min(area.height);
    let popup_area = Rect {
        x: area.x + (area.width - width) / 2,
        y: area.y + (area.height - height) / 2,
        width,
        height,
    };
    let block = Block::default()
        .title(" Help · Esc to dismiss ")
        .borders(Borders::ALL)
        .style(Style::default().bg(Color::Black).fg(Color::White));
    let paragraph = Paragraph::new(help_text)
        .block(block)
        .alignment(Alignment::Left);
    f.render_widget(Clear, popup_area);
    f.render_widget(paragraph, popup_area);
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
        assert!(text.contains("Urgent steer"), "got: {text}");
    }

    #[test]
    fn guidance_keys_are_not_listed_under_autoloop_source() {
        let text = render_to_string(true);
        assert!(!text.contains("Guidance:"), "got: {text}");
        assert!(!text.contains("Urgent steer"), "got: {text}");
    }

    #[test]
    fn every_binding_fits_a_standard_terminal() {
        for (width, height) in [(80, 24), (60, 24)] {
            let backend = TestBackend::new(width, height);
            let mut terminal = Terminal::new(backend).unwrap();
            terminal.draw(|f| render(f, f.area(), true)).unwrap();
            let buffer = terminal.backend().buffer();
            let text: Vec<String> = (0..height)
                .map(|y| {
                    (0..width)
                        .map(|x| buffer.cell((x, y)).unwrap().symbol())
                        .collect()
                })
                .collect();
            let text = text.join("\n");
            for binding in [
                "Toggle mouse mode (select/scroll)",
                "Export all iterations",
                "Dismiss/cancel",
                "Esc to dismiss",
            ] {
                assert!(
                    text.contains(binding),
                    "{width}x{height} lost {binding:?}:\n{text}"
                );
            }
        }
    }

    #[test]
    fn help_overlay_has_no_wave_section() {
        // autoloop runs declarative waves inside one iteration and exposes no
        // per-branch stream, so the TUI has no wave view to document.
        for autoloop_source in [false, true] {
            let text = render_to_string(autoloop_source);
            assert!(text.contains("Other:"), "help should render, got: {text}");
            assert!(
                !text.contains("Wave"),
                "help must not list waves, got: {text}"
            );
            assert!(
                !text.contains("  w "),
                "help must not list a w keybinding, got: {text}"
            );
        }
    }
}
