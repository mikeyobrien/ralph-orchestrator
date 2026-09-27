use super::fit::{Item, fit, spans_width};
use crate::state::{TuiState, UpdateStatus};
use ralph_core::truncate_with_ellipsis;
use ratatui::{
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
};

// ============================================================================
// Priority-Based Item Selection
// ============================================================================
// Each item offers full, compact, and minimal forms and the row is fitted to
// the real render width (see [`super::fit`]). Items upgrade in priority order,
// so at narrow widths lower-priority items shrink or disappear instead of the
// row being clipped. The four live questions come first: where the loop is,
// what needs attention, whether the view is live, and what is running.
//
// Priority (lower = more important):
// 1. Iteration counter / status - required
// 2. Attention warning - required when present
// 3. Mode [LIVE]/[REVIEW] (▶/◀) - required
// 4. Hat and harness: `🔨Builder @pi` / `🔨Builder` / `🔨`
// 5. Scroll indicator [SCROLL]/[S]
// 6. Iteration elapsed time
// 7. Idle countdown
// 8. Git branch (full or 12-char ellipsis form)
// 9. Help hint
// The update badge takes whatever width is left on the right.
// ============================================================================

/// Width breakpoints still used by the update badge's forms.
const WIDTH_FULL: u16 = 80;
const WIDTH_HIDE_HELP: u16 = 65;
const WIDTH_MINIMAL: u16 = 40;

/// Branch names longer than this get an ellipsis form.
const BRANCH_COMPACT_CHARS: usize = 12;

fn sep(text: impl Into<String>, style: Style) -> Vec<Span<'static>> {
    vec![Span::raw(" | "), Span::styled(text.into(), style)]
}

/// Renders the header widget, choosing each item's form by `width`.
pub fn render(state: &TuiState, width: u16) -> Paragraph<'static> {
    let mut items = Vec::new();

    // 1. Iteration counter or status indicator.
    let status = if state.subprocess_error.is_some() {
        Span::styled("[ERROR]".to_string(), Style::default().fg(Color::Red))
    } else if state.iterations.is_empty() && state.last_event.is_none() {
        // No events received yet: the event source is not connected.
        Span::styled(
            "[connecting]".to_string(),
            Style::default().fg(Color::DarkGray),
        )
    } else {
        // Uses TUI pagination state (current_view/total_iterations), not the engine iteration.
        let current = state
            .current_iteration()
            .map(|buffer| buffer.number)
            .unwrap_or_else(|| (state.current_view + 1) as u32);
        let total_display = state
            .max_iterations
            .unwrap_or(state.total_iterations() as u32);
        Span::raw(format!("[iter {current}/{total_display}]"))
    };
    items.push(Item::required(1, vec![vec![status]]));

    // 2. Attention: progress the view lost. Replaces decoration, never clipped.
    if let Some(attention) = &state.attention {
        let style = Style::default().fg(Color::Yellow);
        items.push(Item::required(
            2,
            vec![
                vec![Span::raw(" "), Span::styled(attention.full.clone(), style)],
                vec![
                    Span::raw(" "),
                    Span::styled(attention.compact.clone(), style),
                ],
                vec![Span::raw(" "), Span::styled("\u{26A0}", style)],
            ],
        ));
    }

    // 6. Elapsed time.
    if let Some(elapsed) = state.get_iteration_elapsed() {
        items.push(Item::optional(
            6,
            vec![vec![Span::raw(format!(
                " {}",
                ralph_core::utils::format_elapsed(elapsed)
            ))]],
        ));
    }

    // 4. Hat and the harness that ran it.
    let iteration_finished = state.current_iteration().and_then(|b| b.elapsed).is_some();
    let hat_display = if iteration_finished && state.pending_hat.is_some() {
        // Iteration done and next hat known: show it instead of the stale frozen hat.
        state.get_pending_hat_display()
    } else {
        state
            .current_iteration_hat_display()
            .map(|d| d.to_string())
            .unwrap_or_else(|| state.get_pending_hat_display())
    };
    let mut hat_forms = Vec::new();
    if let Some(backend) = state.current_iteration_backend() {
        hat_forms.push(sep(format!("{hat_display} @{backend}"), Style::default()));
    }
    hat_forms.push(sep(hat_display.clone(), Style::default()));
    hat_forms.push(sep(
        hat_display.chars().next().unwrap_or('?').to_string(),
        Style::default(),
    ));
    items.push(Item::optional(4, hat_forms));

    // 7. Idle countdown.
    if let Some(idle) = state.idle_timeout_remaining {
        items.push(Item::optional(
            7,
            vec![sep(format!("idle: {}s", idle.as_secs()), Style::default())],
        ));
    }

    // 3. Mode: [LIVE] when following latest, [REVIEW] when viewing history.
    let (full_mode, compact_mode, mode_color) = if state.following_latest {
        ("[LIVE]", "▶", Color::Green)
    } else {
        ("[REVIEW]", "◀", Color::Yellow)
    };
    items.push(Item::required(
        3,
        vec![
            sep(full_mode, Style::default().fg(mode_color)),
            sep(compact_mode, Style::default().fg(mode_color)),
        ],
    ));

    // 5. Scroll indicator.
    if state.in_scroll_mode {
        let style = Style::default().fg(Color::Cyan);
        items.push(Item::optional(
            5,
            vec![
                vec![Span::styled(" [SCROLL]", style)],
                vec![Span::styled(" [S]", style)],
            ],
        ));
    }

    // 8. Git branch.
    if let Some(branch) = state.current_branch() {
        let style = Style::default().fg(Color::Cyan);
        let mut forms = vec![sep(format!("git:{branch}"), style)];
        if branch.chars().count() > BRANCH_COMPACT_CHARS {
            forms.push(sep(
                format!(
                    "git:{}",
                    truncate_with_ellipsis(branch, BRANCH_COMPACT_CHARS)
                ),
                style,
            ));
        }
        items.push(Item::optional(8, forms));
    }

    // 9. Help hint.
    items.push(Item::optional(
        9,
        vec![sep("? help", Style::default().fg(Color::DarkGray))],
    ));

    let (mut spans, left_width) = fit(items, width as usize);
    if let Some(right_spans) = update_badge_spans(state, width, left_width) {
        let gap = width as usize - left_width - spans_width(&right_spans);
        spans.push(Span::raw(" ".repeat(gap)));
        spans.extend(right_spans);
    }

    let line = Line::from(spans);
    let block = Block::default().borders(Borders::BOTTOM);
    Paragraph::new(line).block(block)
}

fn update_badge_spans(
    state: &TuiState,
    width: u16,
    left_width: usize,
) -> Option<Vec<Span<'static>>> {
    let UpdateStatus::Available { latest } = &state.update_status else {
        return None;
    };

    let style = Style::default().fg(Color::Yellow);
    let candidates = [
        (width >= WIDTH_FULL, format!("[update {latest}]")),
        (width >= WIDTH_HIDE_HELP, format!("[↑ {latest}]")),
        (width > WIDTH_MINIMAL, "↑".to_string()),
    ];

    candidates
        .into_iter()
        .filter(|(allowed, _)| *allowed)
        .map(|(_, label)| vec![Span::styled(label, style)])
        .find(|candidate| left_width + 1 + spans_width(candidate) <= width as usize)
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

        let expected_time = ralph_core::utils::format_elapsed(elapsed);

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

    /// The header's first row, exactly as rendered.
    fn header_row(state: &TuiState, width: u16) -> String {
        render_to_string_with_width(state, width)
            .chars()
            .take(width as usize)
            .collect()
    }

    #[test]
    fn header_at_50_chars_keeps_high_priority_items_whole() {
        let row = header_row(&create_full_state(), 50);
        for item in ["[iter 3/10]", "[LIVE]", "Builder", "04:32", "[SCROLL]"] {
            assert!(row.contains(item), "{item} missing at 50: {row:?}");
        }
        // Idle (priority 7) is the first item that no longer fits.
        assert!(!row.contains("idle"), "{row:?}");
    }

    #[test]
    fn header_at_40_chars_drops_time_before_compressing_mode() {
        let row = header_row(&create_full_state(), 40);
        for item in ["[iter 3/10]", "[LIVE]", "Builder", "[S]"] {
            assert!(row.contains(item), "{item} missing at 40: {row:?}");
        }
        assert!(!row.contains("04:32"), "{row:?}");
        assert!(!row.contains("idle"), "{row:?}");
    }

    #[test]
    fn header_at_30_chars_keeps_iteration_and_mode() {
        let row = header_row(&create_full_state(), 30);
        assert!(row.contains("[iter 3/10]"), "{row:?}");
        assert!(row.contains("[LIVE]"), "{row:?}");
        assert!(
            !row.contains("Builder"),
            "hat should compress first: {row:?}"
        );
    }

    #[test]
    fn header_warning_displaces_lower_priority_items_at_52_to_56_columns() {
        let mut state = create_full_state();
        state.set_current_branch(Some("main".to_string()));
        state.attention = Some(crate::state::Attention {
            full: "\u{26A0} 3 engine events skipped".to_string(),
            compact: "\u{26A0} 3 skipped".to_string(),
        });

        for width in 52..=56 {
            let row = header_row(&state, width);
            // The warning renders whole, never clipped, next to the
            // always-shown iteration and mode.
            assert!(
                row.contains("⚠ 3 engine events skipped"),
                "warning clipped at {width}: {row:?}"
            );
            assert!(row.contains("[iter 3/10]"), "{width}: {row:?}");
            assert!(row.contains("[LIVE]"), "{width}: {row:?}");
            // Lower-priority content gave up its room instead.
            for displaced in ["Builder", "04:32", "idle", "git:main", "? help"] {
                assert!(
                    !row.contains(displaced),
                    "{displaced} kept at {width} while a warning needed room: {row:?}"
                );
            }
        }

        // Without the warning, the same widths show the displaced items.
        state.attention = None;
        let row = header_row(&state, 54);
        assert!(row.contains("Builder") && row.contains("04:32"), "{row:?}");
    }

    #[test]
    fn header_warning_falls_back_to_compact_form_before_clipping() {
        let mut state = create_full_state();
        state.attention = Some(crate::state::Attention {
            full: "\u{26A0} 3 engine events skipped".to_string(),
            compact: "\u{26A0} 3 skipped".to_string(),
        });
        let row = header_row(&state, 40);
        assert!(row.contains("⚠ 3 skipped"), "{row:?}");
        assert!(!row.contains("engine events"), "{row:?}");
        assert!(
            row.contains("[iter 3/10]") && row.contains("[LIVE]"),
            "{row:?}"
        );
    }

    #[test]
    fn header_has_no_wave_mode_in_any_state() {
        let mut state = create_full_state();
        for following in [true, false] {
            state.following_latest = following;
            for width in [30, 52, 80, 120] {
                let row = header_row(&state, width);
                assert!(!row.contains("WAVE") && !row.contains("worker"), "{row:?}");
            }
        }
    }

    #[test]
    fn header_shows_branch_on_wide_terminal() {
        let mut state = create_full_state();
        state.set_current_branch(Some("feature/show-branch".to_string()));

        let text = render_to_string_with_width(&state, 80);
        assert!(
            text.contains("git:feature"),
            "branch prefix should be visible on wide terminals, got: {}",
            text
        );
        assert!(
            text.contains("..."),
            "long branch names should use an ellipsis, got: {}",
            text
        );
    }

    #[test]
    fn header_hides_branch_on_compact_terminal() {
        let mut state = create_full_state();
        state.set_current_branch(Some("main".to_string()));

        let text = render_to_string_with_width(&state, 50);
        assert!(
            !text.contains("git:main"),
            "branch should be hidden on compact terminals, got: {}",
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
            iteration.elapsed = Some(Duration::from_mins(5));
        }

        let text = render_to_string(&state);
        assert!(
            text.contains("05:00"),
            "should preserve elapsed time display, got: {}",
            text
        );
    }

    #[test]
    fn header_handles_empty_iterations_no_events() {
        // Given no iterations and no events yet (subprocess hasn't connected)
        let state = TuiState::new();

        let text = render_to_string(&state);
        // Before any events arrive, shows connecting state
        assert!(
            text.contains("[connecting]"),
            "should show [connecting] when no events received, got: {}",
            text
        );
    }

    #[test]
    fn header_handles_empty_iterations_with_events() {
        // Given no iterations but events have been processed (event bus mode)
        let mut state = TuiState::new();
        state.last_event = Some("task.start".to_string());

        let text = render_to_string(&state);
        // With events but no iteration buffers, falls back to iter counter
        assert!(
            text.contains("[iter 1/0]"),
            "should show [iter 1/0] when events exist but no iterations, got: {}",
            text
        );
    }

    #[test]
    fn header_shows_error_when_subprocess_died() {
        // Given subprocess died before sending events
        let mut state = TuiState::new();
        state.subprocess_error = Some("Subprocess exited before starting".to_string());

        let text = render_to_string(&state);
        assert!(
            text.contains("[ERROR]"),
            "should show [ERROR] when subprocess died, got: {}",
            text
        );
    }

    #[test]
    fn header_shows_full_update_badge_on_wide_terminal() {
        let mut state = TuiState::new();
        state.start_new_iteration();
        state.update_status = UpdateStatus::Available {
            latest: "2.8.0".to_string(),
        };

        let text = render_to_string_with_width(&state, 80);
        assert!(
            text.contains("[update 2.8.0]"),
            "should show full update badge at wide widths, got: {}",
            text
        );
    }

    #[test]
    fn header_compresses_update_badge_on_medium_terminal() {
        let mut state = TuiState::new();
        state.start_new_iteration();
        state.update_status = UpdateStatus::Available {
            latest: "2.8.0".to_string(),
        };

        let text = render_to_string_with_width(&state, 65);
        assert!(
            text.contains("[↑ 2.8.0]"),
            "should show compact update badge at medium widths, got: {}",
            text
        );
        assert!(
            !text.contains("[update 2.8.0]"),
            "should not show full badge at medium widths, got: {}",
            text
        );
    }

    #[test]
    fn header_shows_update_icon_on_narrow_terminal() {
        let mut state = TuiState::new();
        state.start_new_iteration();
        state.update_status = UpdateStatus::Available {
            latest: "2.8.0".to_string(),
        };

        let text = render_to_string_with_width(&state, 45);
        assert!(
            text.contains('↑'),
            "should show icon-only update indicator on narrow widths, got: {}",
            text
        );
        assert!(
            !text.contains("2.8.0"),
            "should hide version text on narrow widths, got: {}",
            text
        );
    }

    #[test]
    fn header_hides_update_badge_when_too_narrow() {
        let mut state = TuiState::new();
        state.start_new_iteration();
        state.update_status = UpdateStatus::Available {
            latest: "2.8.0".to_string(),
        };

        let text = render_to_string_with_width(&state, 18);
        assert!(
            !text.contains('↑') && !text.contains("update"),
            "should hide update indicator when terminal is too narrow, got: {}",
            text
        );
    }

    /// Regression: when the current iteration is finished (elapsed set) and
    /// pending_hat has been updated, the header should show the NEW pending hat,
    /// not the stale frozen hat from the completed iteration.
    ///
    /// Before the fix, the header always preferred the frozen iteration hat_display
    /// over pending_hat, so during the gap between iterations the stale hat was shown.
    #[test]
    fn header_prefers_pending_hat_when_iteration_finished() {
        let mut state = TuiState::new();

        // Iteration 1: hat was "Planner" — now finished
        state.start_new_iteration_with_metadata(
            Some("📋 Planner".to_string()),
            Some("claude".to_string()),
        );
        // Mark iteration as finished (elapsed is set)
        if let Some(iteration) = state.iterations.first_mut() {
            iteration.elapsed = Some(Duration::from_mins(1));
        }
        // current_view = 0 (still viewing the finished iteration, following latest)
        state.current_view = 0;
        state.following_latest = true;

        // pending_hat updated to the NEXT hat (Builder) — this happens between
        // iterations when the event loop selects the next hat
        state.pending_hat = Some((HatId::new("builder"), "🔨 Builder".to_string()));

        let text = render_to_string(&state);

        // Should show the NEW pending hat, not the old frozen one
        assert!(
            text.contains("Builder"),
            "should show pending hat 'Builder' when iteration is finished, got: {}",
            text
        );
        assert!(
            !text.contains("Planner"),
            "should NOT show stale frozen hat 'Planner' when iteration is finished, got: {}",
            text
        );
    }

    #[test]
    fn task_start_preserves_current_branch() {
        let mut state = TuiState::new();
        state.set_current_branch(Some("main".to_string()));

        state.update(&Event::new("task.start", ""));

        assert_eq!(state.current_branch(), Some("main"));
    }
}
