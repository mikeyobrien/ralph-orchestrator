use crate::state::{LoopMode, TuiState};
use ratatui::{
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

pub fn render(state: &TuiState) -> Paragraph<'static> {
    let mut spans = vec![];

    // [iter N/M] or [iter N]
    let iter_display = if let Some(max) = state.max_iterations {
        format!("[iter {}/{}]", state.iteration + 1, max)
    } else {
        format!("[iter {}]", state.iteration + 1)
    };
    spans.push(Span::raw(iter_display));

    // MM:SS elapsed time
    if let Some(elapsed) = state.get_loop_elapsed() {
        let total_secs = elapsed.as_secs();
        let mins = total_secs / 60;
        let secs = total_secs % 60;
        spans.push(Span::raw(format!(" {mins:02}:{secs:02}")));
    }

    // | 🎯 Hat
    spans.push(Span::raw(" | "));
    spans.push(Span::raw(state.get_pending_hat_display()));

    // idle: Ns (only if Some)
    if let Some(idle) = state.idle_timeout_remaining {
        spans.push(Span::raw(format!(" | idle: {}s", idle.as_secs())));
    }

    // | ▶ auto / ⏸ paused
    spans.push(Span::raw(" | "));
    let mode = match state.loop_mode {
        LoopMode::Auto => Span::styled("▶ auto", Style::default().fg(Color::Green)),
        LoopMode::Paused => Span::styled("⏸ paused", Style::default().fg(Color::Yellow)),
    };
    spans.push(mode);

    // [SCROLL] indicator
    if state.in_scroll_mode {
        spans.push(Span::styled(" [SCROLL]", Style::default().fg(Color::Cyan)));
    }

    let line = Line::from(spans);
    Paragraph::new(line).block(Block::default().borders(Borders::ALL))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ralph_proto::{Event, HatId};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use std::time::Duration;

    fn render_to_string(state: &TuiState) -> String {
        let backend = TestBackend::new(80, 3);
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

    #[test]
    fn header_shows_iteration_without_max() {
        let mut state = TuiState::new();
        state.iteration = 2;
        state.max_iterations = None;

        let text = render_to_string(&state);
        assert!(
            text.contains("[iter 3]"),
            "should show [iter 3], got: {}",
            text
        );
    }

    #[test]
    fn header_shows_iteration_with_max() {
        let mut state = TuiState::new();
        state.iteration = 2;
        state.max_iterations = Some(10);

        let text = render_to_string(&state);
        assert!(
            text.contains("[iter 3/10]"),
            "should show [iter 3/10], got: {}",
            text
        );
    }

    #[test]
    fn header_shows_elapsed_time() {
        let mut state = TuiState::new();
        let event = Event::new("task.start", "");
        state.update(&event);

        // Simulate 4 minutes 32 seconds elapsed
        state.loop_started = Some(
            std::time::Instant::now()
                .checked_sub(Duration::from_secs(272))
                .unwrap(),
        );

        let text = render_to_string(&state);
        assert!(text.contains("04:32"), "should show 04:32, got: {}", text);
    }

    #[test]
    fn header_shows_hat() {
        let mut state = TuiState::new();
        state.pending_hat = Some((HatId::new("builder"), "🔨Builder".to_string()));

        let text = render_to_string(&state);
        assert!(text.contains("Builder"), "should show hat, got: {}", text);
    }

    #[test]
    fn header_shows_idle_countdown_when_present() {
        let mut state = TuiState::new();
        state.idle_timeout_remaining = Some(Duration::from_secs(25));

        let text = render_to_string(&state);
        assert!(
            text.contains("idle: 25s"),
            "should show idle countdown, got: {}",
            text
        );
    }

    #[test]
    fn header_hides_idle_countdown_when_none() {
        let mut state = TuiState::new();
        state.idle_timeout_remaining = None;

        let text = render_to_string(&state);
        assert!(
            !text.contains("idle:"),
            "should not show idle when None, got: {}",
            text
        );
    }

    #[test]
    fn header_shows_auto_mode() {
        let mut state = TuiState::new();
        state.loop_mode = LoopMode::Auto;

        let text = render_to_string(&state);
        assert!(
            text.contains("▶ auto"),
            "should show auto mode, got: {}",
            text
        );
    }

    #[test]
    fn header_shows_paused_mode() {
        let mut state = TuiState::new();
        state.loop_mode = LoopMode::Paused;

        let text = render_to_string(&state);
        assert!(
            text.contains("⏸ paused"),
            "should show paused mode, got: {}",
            text
        );
    }

    #[test]
    fn header_shows_scroll_indicator() {
        let mut state = TuiState::new();
        state.in_scroll_mode = true;

        let text = render_to_string(&state);
        assert!(
            text.contains("[SCROLL]"),
            "should show scroll indicator, got: {}",
            text
        );
    }

    #[test]
    fn header_full_format() {
        let mut state = TuiState::new();
        let event = Event::new("task.start", "");
        state.update(&event);

        state.iteration = 2;
        state.max_iterations = Some(10);
        state.loop_started = Some(
            std::time::Instant::now()
                .checked_sub(Duration::from_secs(272))
                .unwrap(),
        );
        state.pending_hat = Some((HatId::new("builder"), "🔨Builder".to_string()));
        state.idle_timeout_remaining = Some(Duration::from_secs(25));
        state.loop_mode = LoopMode::Auto;
        state.in_scroll_mode = true;

        let text = render_to_string(&state);

        // Verify all components present
        assert!(
            text.contains("[iter 3/10]"),
            "missing iteration, got: {}",
            text
        );
        assert!(
            text.contains("04:32"),
            "missing elapsed time, got: {}",
            text
        );
        assert!(text.contains("Builder"), "missing hat, got: {}", text);
        assert!(
            text.contains("idle: 25s"),
            "missing idle countdown, got: {}",
            text
        );
        assert!(text.contains("▶ auto"), "missing mode, got: {}", text);
        assert!(
            text.contains("[SCROLL]"),
            "missing scroll indicator, got: {}",
            text
        );
    }
}
