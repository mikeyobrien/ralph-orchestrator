use crate::state::TuiState;
use ratatui::{
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

// ============================================================================
// Width Breakpoints for Priority-Based Progressive Disclosure
// ============================================================================
// At narrower widths, lower-priority components are hidden or compressed.
//
// Priority levels (lower number = more important, always shown):
// - Priority 1: Iteration counter [iter N/M] - always shown (TUI pagination)
// - Priority 2: Mode indicator [LIVE]/[REVIEW] (▶/◀ compressed) - always shown
// - Priority 3: Hat display, Scroll indicator - compressed at 50
// - Priority 4: Iteration elapsed time MM:SS - hidden at 50
// - Priority 5: Idle countdown - hidden at 40
// - Priority 6: Help hint - hidden at 65
// ============================================================================

/// Width breakpoint constants
const WIDTH_FULL: u16 = 80; // Show everything including help hint
#[allow(dead_code)] // Kept for documentation of breakpoint tiers
const WIDTH_HIDE_HELP: u16 = 65; // Below this: help hint hidden
const WIDTH_COMPRESS: u16 = 50; // Compress mode/hat, hide time
const WIDTH_MINIMAL: u16 = 40; // Hide idle countdown

/// Renders the header widget with priority-based progressive disclosure.
///
/// At narrower terminal widths, lower-priority components are hidden or compressed
/// to ensure critical information (iteration, mode) remains visible.
pub fn render(state: &TuiState, width: u16) -> Paragraph<'static> {
    let mut spans = vec![];

    // Priority 1: Iteration counter - ALWAYS shown
    // Uses TUI pagination state (current_view/total_iterations) not Ralph loop iteration
    let current = state
        .current_iteration()
        .map(|buffer| buffer.number)
        .unwrap_or_else(|| (state.current_view + 1) as u32);
    let total_iterations = state.total_iterations() as u32;
    let total_display = state.max_iterations.unwrap_or(total_iterations);
    let iter_display = format!("[iter {}/{}]", current, total_display);
    spans.push(Span::raw(iter_display));

    // Priority 4: Elapsed time (iteration) - hidden at WIDTH_COMPRESS and below
    if let Some(elapsed) = state.get_iteration_elapsed()
        && width > WIDTH_COMPRESS
    {
        let total_secs = elapsed.as_secs();
        let mins = total_secs / 60;
        let secs = total_secs % 60;
        spans.push(Span::raw(format!(" {mins:02}:{secs:02}")));
    }

    // Priority 3: Hat display - compressed at WIDTH_COMPRESS and below
    spans.push(Span::raw(" | "));
    let hat_display = state
        .current_iteration_hat_display()
        .map(|display| display.to_string())
        .unwrap_or_else(|| state.get_pending_hat_display());
    let hat_with_backend = if let Some(backend) = state.current_iteration_backend()
        && width > WIDTH_COMPRESS
    {
        format!("{hat_display} @{backend}")
    } else {
        hat_display.clone()
    };
    if width > WIDTH_COMPRESS {
        // Full hat display: "🔨 Builder"
        spans.push(Span::raw(hat_with_backend));
    } else {
        // Compressed: emoji only (first character cluster)
        let emoji = hat_display.chars().next().unwrap_or('?');
        spans.push(Span::raw(emoji.to_string()));
    }

    // Priority 5: Idle countdown - hidden at WIDTH_MINIMAL and below
    if let Some(idle) = state.idle_timeout_remaining
        && width > WIDTH_MINIMAL
    {
        spans.push(Span::raw(format!(" | idle: {}s", idle.as_secs())));
    }

    // Priority 2: Mode indicator - ALWAYS shown (compressed at WIDTH_COMPRESS and below)
    // Shows [LIVE] when following latest iteration, [REVIEW] when viewing history
    spans.push(Span::raw(" | "));
    let mode = if state.following_latest {
        if width > WIDTH_COMPRESS {
            Span::styled("[LIVE]", Style::default().fg(Color::Green))
        } else {
            Span::styled("▶", Style::default().fg(Color::Green))
        }
    } else if width > WIDTH_COMPRESS {
        Span::styled("[REVIEW]", Style::default().fg(Color::Yellow))
    } else {
        Span::styled("◀", Style::default().fg(Color::Yellow))
    };
    spans.push(mode);

    // Priority 3: Scroll indicator - compressed at WIDTH_COMPRESS and below
    if state.in_scroll_mode {
        if width > WIDTH_COMPRESS {
            spans.push(Span::styled(" [SCROLL]", Style::default().fg(Color::Cyan)));
        } else {
            spans.push(Span::styled(" [S]", Style::default().fg(Color::Cyan)));
        }
    }

    // Priority 6: Help hint - shown only at WIDTH_FULL (80+)
    if width >= WIDTH_FULL {
        spans.push(Span::styled(
            " | ? help",
            Style::default().fg(Color::DarkGray),
        ));
    }

    let line = Line::from(spans);
    let block = Block::default().borders(Borders::BOTTOM);
    Paragraph::new(line).block(block)
}

#[cfg(test)]
mod tests {
    use super::*;
    use ralph_proto::{Event, HatId};
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;
    use std::time::{Duration, Instant};

    fn render_to_string(state: &TuiState) -> String {
        render_to_string_with_width(state, 80)
    }

    fn render_to_string_with_width(state: &TuiState, width: u16) -> String {
        // Height of 2: 1 for content + 1 for bottom border
        let backend = TestBackend::new(width, 2);
        let mut terminal = Terminal::new(backend).unwrap();

        terminal
            .draw(|f| {
                let widget = render(state, width);
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
    fn header_shows_iteration_position() {
        // Now uses TUI pagination state (current_view/total_iterations)
        let mut state = TuiState::new();
        state.start_new_iteration();
        state.start_new_iteration();
        state.start_new_iteration();
        state.current_view = 2; // Viewing iteration 3

        let text = render_to_string(&state);
        assert!(
            text.contains("[iter 3/3]"),
            "should show [iter 3/3], got: {}",
            text
        );
    }

    #[test]
    fn header_shows_iteration_at_first() {
        // Viewing first of multiple iterations
        let mut state = TuiState::new();
        state.start_new_iteration();
        state.start_new_iteration();
        state.start_new_iteration();
        state.current_view = 0; // Viewing first iteration

        let text = render_to_string(&state);
        assert!(
            text.contains("[iter 1/3]"),
            "should show [iter 1/3], got: {}",
            text
        );
    }

    #[test]
    fn header_uses_max_iterations_when_available() {
        let mut state = TuiState::new();
        state.max_iterations = Some(50);
        state.start_new_iteration();

        let text = render_to_string(&state);
        assert!(
            text.contains("[iter 1/50]"),
            "should show [iter 1/50], got: {}",
            text
        );
    }

    #[test]
    fn header_shows_elapsed_time() {
        let mut state = TuiState::new();
        let event = Event::new("task.start", "");
        state.update(&event);

        // Simulate 4 minutes 32 seconds elapsed for current iteration
        state.iteration_started = Some(
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
    fn header_uses_iteration_metadata_for_review() {
        let mut state = TuiState::new();
        state.start_new_iteration_with_metadata(
            Some("🔨 Builder".to_string()),
            Some("claude".to_string()),
        );
        if let Some(iteration) = state.iterations.get_mut(0) {
            iteration.elapsed = Some(Duration::from_secs(125));
        }
        state.start_new_iteration_with_metadata(
            Some("🧪 Reviewer".to_string()),
            Some("kiro".to_string()),
        );
        state.current_view = 0; // Review first iteration

        let text = render_to_string(&state);
        assert!(text.contains("Builder"), "should show hat, got: {}", text);
        assert!(
            text.contains("@claude"),
            "should show backend, got: {}",
            text
        );
        assert!(text.contains("02:05"), "should show 02:05, got: {}", text);
    }

    #[test]
    fn header_uses_per_iteration_hat_from_events_when_reviewing() {
        use std::collections::HashMap;

        let mut hat_map = HashMap::new();
        hat_map.insert(
            "review.security".to_string(),
            (HatId::new("security_reviewer"), "🛡Security".to_string()),
        );
        hat_map.insert(
            "review.correctness".to_string(),
            (
                HatId::new("correctness_reviewer"),
                "🧪Correctness".to_string(),
            ),
        );

        let mut state = TuiState::with_hat_map(hat_map);

        state.update(&Event::new("review.security", "Check auth"));
        state.start_new_iteration();

        state.update(&Event::new("review.correctness", "Check logic"));
        state.start_new_iteration();

        state.current_view = 0;
        state.following_latest = false;

        let text = render_to_string(&state);
        assert!(
            text.contains("Security"),
            "should show iteration 1 hat, got: {}",
            text
        );
        assert!(
            !text.contains("Correctness"),
            "should not show current hat while reviewing, got: {}",
            text
        );
    }

    #[test]
    fn header_review_uses_frozen_elapsed_and_backend_from_events() {
        use std::collections::HashMap;

        let mut hat_map = HashMap::new();
        hat_map.insert(
            "build.done".to_string(),
            (HatId::new("planner"), "📋Planner".to_string()),
        );

        let mut state = TuiState::with_hat_map(hat_map);

        state.start_new_iteration_with_metadata(
            Some("🔨 Builder".to_string()),
            Some("claude".to_string()),
        );
        if let Some(iteration) = state.iterations.first_mut() {
            iteration.started_at = Some(
                Instant::now()
                    .checked_sub(Duration::from_secs(125))
                    .expect("instant should support backdating"),
            );
        }

        state.update(&Event::new("build.done", "Done"));
        let elapsed = state
            .iterations
            .first()
            .and_then(|iteration| iteration.elapsed)
            .expect("iteration elapsed should be frozen on build.done");

        state.start_new_iteration_with_metadata(
            Some("🧪 Reviewer".to_string()),
            Some("kiro".to_string()),
        );
        state.current_view = 0;
        state.following_latest = false;

        let total_secs = elapsed.as_secs();
        let mins = total_secs / 60;
        let secs = total_secs % 60;
        let expected_time = format!("{mins:02}:{secs:02}");

        let text = render_to_string(&state);
        assert!(
            text.contains("@claude"),
            "should show iteration backend, got: {}",
            text
        );
        assert!(
            text.contains(&expected_time),
            "should show frozen elapsed time, got: {}",
            text
        );
        assert!(
            !text.contains("@kiro"),
            "should not show current backend while reviewing, got: {}",
            text
        );
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

        // Set up TUI pagination state (10 iterations, viewing iteration 3)
        for _ in 0..10 {
            state.start_new_iteration();
        }
        state.current_view = 2; // Viewing iteration 3 of 10
        state.following_latest = true;

        if let Some(iteration) = state.iterations.get_mut(2) {
            iteration.elapsed = Some(Duration::from_secs(272));
            iteration.hat_display = Some("🔨Builder".to_string());
        }
        state.pending_hat = Some((HatId::new("builder"), "🔨Builder".to_string()));
        state.idle_timeout_remaining = Some(Duration::from_secs(25));
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
        assert!(text.contains("[LIVE]"), "missing mode, got: {}", text);
        assert!(
            text.contains("[SCROLL]"),
            "missing scroll indicator, got: {}",
            text
        );
        assert!(
            text.contains("? help"),
            "missing help hint at width 80, got: {}",
            text
        );
    }

    // =========================================================================
    // Priority-Based Progressive Disclosure Tests
    // =========================================================================

    fn create_full_state() -> TuiState {
        let mut state = TuiState::new();
        let event = Event::new("task.start", "");
        state.update(&event);

        // Set up TUI pagination state (10 iterations, viewing iteration 3)
        for _ in 0..10 {
            state.start_new_iteration();
        }
        state.current_view = 2; // Viewing iteration 3 of 10
        state.following_latest = true; // In LIVE mode

        if let Some(iteration) = state.iterations.get_mut(2) {
            iteration.elapsed = Some(Duration::from_secs(272));
            iteration.hat_display = Some("🔨Builder".to_string());
        }
        state.pending_hat = Some((HatId::new("builder"), "🔨Builder".to_string()));
        state.idle_timeout_remaining = Some(Duration::from_secs(25));
        state.in_scroll_mode = true;
        state
    }

    #[test]
    fn header_at_80_chars_shows_help_hint() {
        // At 80+ chars, help hint should be visible
        let state = create_full_state();
        let text = render_to_string_with_width(&state, 80);

        // Should contain help hint
        assert!(
            text.contains("? help"),
            "help hint should be visible at 80 chars, got: {}",
            text
        );

        // Should still show all other components
        assert!(
            text.contains("[iter 3/10]"),
            "iteration should be visible, got: {}",
            text
        );
        assert!(
            text.contains("[LIVE]"),
            "mode should be visible, got: {}",
            text
        );
    }

    #[test]
    fn header_at_65_chars_hides_help() {
        // At 65 chars, help hint should be hidden but everything else visible
        let state = create_full_state();
        let text = render_to_string_with_width(&state, 65);

        // Should NOT contain help hint
        assert!(
            !text.contains("? help"),
            "help hint should be hidden at 65 chars, got: {}",
            text
        );

        // Should still show core components
        assert!(
            text.contains("[iter 3/10]"),
            "iteration should be visible, got: {}",
            text
        );
        assert!(
            text.contains("[LIVE]"),
            "mode should be visible (not compressed), got: {}",
            text
        );
    }

    #[test]
    fn header_at_50_chars_compresses_mode() {
        // At 50 chars, mode should be compressed to icon only
        let state = create_full_state();
        let text = render_to_string_with_width(&state, 50);

        // Mode should be compressed: "[LIVE]" -> "▶"
        // Should have the icon but not "[LIVE]"
        assert!(
            text.contains('▶'),
            "mode icon should be visible, got: {}",
            text
        );
        assert!(
            !text.contains("[LIVE]"),
            "mode text '[LIVE]' should be hidden at 50 chars, got: {}",
            text
        );

        // Time should be hidden at 50 chars
        assert!(
            !text.contains("04:32"),
            "elapsed time should be hidden at 50 chars, got: {}",
            text
        );

        // Iteration should always be visible
        assert!(
            text.contains("[iter 3/10]"),
            "iteration should be visible, got: {}",
            text
        );
    }

    #[test]
    fn header_at_40_chars_minimal() {
        // At 40 chars, only critical components should be visible
        let state = create_full_state();
        let text = render_to_string_with_width(&state, 40);

        // Iteration (priority 1) always visible
        assert!(
            text.contains("[iter"),
            "iteration should be visible at 40 chars, got: {}",
            text
        );

        // Mode icon (priority 2) always visible
        assert!(
            text.contains('▶'),
            "mode icon should be visible at 40 chars, got: {}",
            text
        );

        // Idle should be hidden (priority 5)
        assert!(
            !text.contains("idle"),
            "idle should be hidden at 40 chars, got: {}",
            text
        );
    }

    #[test]
    fn header_at_30_chars_extreme() {
        // At 30 chars (extreme narrow), show only absolute minimum
        let state = create_full_state();
        let text = render_to_string_with_width(&state, 30);

        // Should at least show iteration
        assert!(
            text.contains("[iter"),
            "iteration should be visible even at 30 chars, got: {}",
            text
        );

        // Mode icon should be visible (critical)
        assert!(
            text.contains('▶'),
            "mode icon should be visible even at 30 chars, got: {}",
            text
        );
    }

    // =========================================================================
    // TUI Iteration Pagination Tests (Task 05)
    // =========================================================================

    #[test]
    fn header_shows_iteration_position_from_tui_state() {
        // Given current_view = 2 (0-indexed, displays as 3) and total_iterations = 5
        let mut state = TuiState::new();
        // Create 5 iterations
        for _ in 0..5 {
            state.start_new_iteration();
        }
        state.current_view = 2; // Viewing iteration 3

        let text = render_to_string(&state);
        assert!(
            text.contains("[iter 3/5]"),
            "should show [iter 3/5] for current_view=2, total=5, got: {}",
            text
        );
    }

    #[test]
    fn header_shows_single_iteration() {
        // Given 1 iteration
        let mut state = TuiState::new();
        state.start_new_iteration();

        let text = render_to_string(&state);
        assert!(
            text.contains("[iter 1/1]"),
            "should show [iter 1/1] for single iteration, got: {}",
            text
        );
    }

    #[test]
    fn header_shows_live_mode_when_following_latest() {
        // Given following_latest = true
        let mut state = TuiState::new();
        state.start_new_iteration();
        state.following_latest = true;

        let text = render_to_string(&state);
        assert!(
            text.contains("[LIVE]"),
            "should show [LIVE] when following_latest=true, got: {}",
            text
        );
    }

    #[test]
    fn header_shows_review_mode_when_not_following_latest() {
        // Given following_latest = false
        let mut state = TuiState::new();
        state.start_new_iteration();
        state.start_new_iteration();
        state.current_view = 0;
        state.following_latest = false;

        let text = render_to_string(&state);
        assert!(
            text.contains("[REVIEW]"),
            "should show [REVIEW] when following_latest=false, got: {}",
            text
        );
    }

    #[test]
    fn header_preserves_hat_display_with_new_format() {
        // Given hat = "Builder" with emoji "🔨"
        let mut state = TuiState::new();
        state.start_new_iteration();
        state.pending_hat = Some((HatId::new("builder"), "🔨Builder".to_string()));

        let text = render_to_string(&state);
        assert!(
            text.contains("Builder"),
            "should preserve hat display, got: {}",
            text
        );
    }

    #[test]
    fn header_preserves_elapsed_time_with_new_format() {
        // Given 5 minutes elapsed for current iteration
        let mut state = TuiState::new();
        state.start_new_iteration();
        let event = Event::new("task.start", "");
        state.update(&event);
        if let Some(iteration) = state.iterations.get_mut(0) {
            iteration.elapsed = Some(Duration::from_secs(300));
        }

        let text = render_to_string(&state);
        assert!(
            text.contains("05:00"),
            "should preserve elapsed time display, got: {}",
            text
        );
    }

    #[test]
    fn header_handles_empty_iterations() {
        // Given no iterations yet
        let state = TuiState::new();

        let text = render_to_string(&state);
        // current_view starts at 0, so display is (0+1)=1, total is 0
        // Shows [iter 1/0] which indicates "viewing position 1 of 0 total"
        assert!(
            text.contains("[iter 1/0]"),
            "should show [iter 1/0] for empty state (position 1, 0 total), got: {}",
            text
        );
    }
}
