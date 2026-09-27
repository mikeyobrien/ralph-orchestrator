use super::fit::{Item, fit, spans_width};
use crate::state::{ExportOutcome, TuiState};
use ratatui::{
    layout::Rect,
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

        // A pending human ask (autoloop HITL) is the most important thing to
        // surface — the run is blocked on it. Display only; answers go through RObot.
        if let Some(question) = &self.state.pending_ask {
            // The label is the warning and always renders whole; only the
            // question text gives up width, with an explicit ellipsis.
            let label = "\u{26A0} HUMAN ASK: ";
            let room =
                (inner_area.width as usize).saturating_sub(1 + spans_width(&[Span::raw(label)]));
            let line = Line::from(vec![
                Span::raw(" "),
                Span::styled(label, Style::default().fg(Color::Yellow)),
                Span::raw(fit_text(question, room)),
            ]);
            Paragraph::new(line).render(inner_area, buf);
            return;
        }

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

        // Export flash (brief after writing iteration buffers)
        if let Some(flash) = self.state.active_export_flash() {
            let (msg, color) = match &flash.outcome {
                ExportOutcome::Success { path } => (
                    format!(
                        "\u{2713} exported {}: {}",
                        flash.scope.label(),
                        self.state.display_export_path(path)
                    ),
                    Color::Green,
                ),
                ExportOutcome::Failed { message } => (
                    format!("\u{2717} export {} failed: {message}", flash.scope.label()),
                    Color::Red,
                ),
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
            let match_info = if query.is_empty() {
                // Still typing the query; no count to show yet.
                String::new()
            } else if self.state.search_state.matches.is_empty() {
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

        // Default footer: priority-selected items on the left, the run
        // indicator always whole on the right.
        let (indicator_text, indicator_style) = if self.state.loop_completed {
            ("■ DONE", Style::default().fg(Color::Blue))
        } else {
            ("◉ ACTIVE", Style::default().fg(Color::Green))
        };
        let indicator = vec![
            Span::styled(indicator_text, indicator_style),
            Span::raw(" "),
        ];
        let indicator_width = spans_width(&indicator);

        let mut items = Vec::new();
        // A new iteration arrived while reviewing history.
        if let Some(iter_num) = self.state.new_iteration_alert
            && !self.state.following_latest
        {
            let style = Style::default().fg(Color::Green);
            items.push(Item::required(
                1,
                vec![
                    vec![
                        Span::styled(format!("▶ New: iter {iter_num} "), style),
                        Span::raw("│ "),
                    ],
                    vec![
                        Span::styled(format!("▶ {iter_num} "), style),
                        Span::raw("│ "),
                    ],
                ],
            ));
        }
        // Total elapsed time (00:00 before the loop starts).
        let elapsed =
            ralph_core::utils::format_elapsed(self.state.get_loop_elapsed().unwrap_or_default());
        items.push(Item::required(
            2,
            vec![
                vec![Span::raw(format!("Total Time Elapsed: {elapsed}"))],
                vec![Span::raw(format!("Elapsed {elapsed}"))],
                vec![Span::raw(elapsed)],
            ],
        ));
        // Final run cost once the loop has completed (autoloop reports it on
        // its terminal event).
        if self.state.loop_completed
            && let Some(cost) = self.state.final_cost_usd
            && cost > 0.0
        {
            let style = Style::default().fg(Color::DarkGray);
            items.push(Item::optional(
                3,
                vec![
                    vec![
                        Span::raw(" │ "),
                        Span::styled(format!("Cost: ${cost:.2}"), style),
                    ],
                    vec![Span::raw(" │ "), Span::styled(format!("${cost:.2}"), style)],
                ],
            ));
        }
        items.push(Item::optional(
            5,
            vec![vec![
                Span::raw(" │ "),
                Span::styled("e export E all", Style::default().fg(Color::DarkGray)),
            ]],
        ));
        if self.state.mouse_capture_enabled {
            let style = Style::default().fg(Color::DarkGray);
            items.push(Item::optional(
                4,
                vec![
                    vec![Span::raw(" │ "), Span::styled("Mouse: scroll (m)", style)],
                    vec![Span::raw(" │ "), Span::styled("m", style)],
                ],
            ));
        }

        let left_room = (inner_area.width as usize).saturating_sub(1 + indicator_width + 1);
        let (left_spans, left_width) = fit(items, left_room);
        let mut spans = vec![Span::raw(" ")];
        spans.extend(left_spans);
        let gap = (inner_area.width as usize).saturating_sub(1 + left_width + indicator_width);
        spans.push(Span::raw(" ".repeat(gap)));
        spans.extend(indicator);
        Paragraph::new(Line::from(spans)).render(inner_area, buf);
    }
}

/// `text` whole when it fits `room` columns, else cut with a trailing `…`.
fn fit_text(text: &str, room: usize) -> String {
    use unicode_width::UnicodeWidthChar;
    if spans_width(&[Span::raw(text)]) <= room {
        return text.to_string();
    }
    let mut out = String::new();
    let mut used = 0;
    for ch in text.chars() {
        let w = ch.width().unwrap_or(0);
        if used + w + 1 > room {
            break;
        }
        out.push(ch);
        used += w;
    }
    out.push('…');
    out
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

    #[test]
    fn footer_shows_mouse_mode() {
        let mut state = TuiState::new();
        let select_text = render_to_string(&state);
        assert!(
            !select_text.contains("Mouse:"),
            "should keep default footer uncluttered when mouse capture is off, got: {}",
            select_text
        );

        state.mouse_capture_enabled = true;
        let scroll_text = render_to_string(&state);
        assert!(
            scroll_text.contains("Mouse: scroll (m)"),
            "should show scroll mode when mouse capture enabled, got: {}",
            scroll_text
        );
    }

    #[test]
    fn footer_shows_export_key_hint() {
        let state = TuiState::new();
        let text = render_to_string(&state);

        assert!(
            text.contains("e export E all"),
            "should show export key hint, got: {}",
            text
        );
    }

    #[test]
    fn footer_shows_export_success_flash() {
        let mut state = TuiState::new();
        state.set_export_workspace_root("/tmp/workspace");
        state.export_flash = Some(crate::state::ExportFlash {
            scope: crate::export::ExportScope::Current,
            outcome: ExportOutcome::Success {
                path: "/tmp/workspace/.ralph/tui-exports/ralph-tui-current.txt".into(),
            },
            when: std::time::Instant::now(),
        });

        let text = render_to_string(&state);

        assert!(
            text.contains("exported current iteration"),
            "should show export success, got: {}",
            text
        );
        assert!(
            text.contains(".ralph/tui-exports/ralph-tui-current.txt"),
            "should show relative export path, got: {}",
            text
        );
    }

    #[test]
    fn footer_shows_pending_ask() {
        // Given a pending human ask (autoloop HITL)
        let mut state = TuiState::new();
        state.pending_ask = Some("Delete the table?".to_string());

        // When footer renders
        let text = render_to_string(&state);

        // Then it surfaces the question prominently
        assert!(
            text.contains("HUMAN ASK") && text.contains("Delete the table?"),
            "should surface pending ask, got: {}",
            text
        );
    }

    #[test]
    fn footer_shows_final_cost_when_complete() {
        // Given a completed run with a known cost
        let mut state = TuiState::new();
        state.loop_completed = true;
        state.final_cost_usd = Some(0.42);

        // When footer renders
        let text = render_to_string(&state);

        // Then the cost is surfaced
        assert!(
            text.contains("Cost: $0.42"),
            "should surface final cost, got: {}",
            text
        );
    }

    #[test]
    fn footer_hides_cost_while_running() {
        // Given a running loop (not complete) even if cost happens to be set
        let mut state = TuiState::new();
        state.loop_completed = false;
        state.final_cost_usd = Some(0.42);

        let text = render_to_string(&state);

        assert!(
            !text.contains("Cost:"),
            "should NOT show cost while running, got: {}",
            text
        );
    }

    #[test]
    fn footer_shows_export_failure_flash() {
        let mut state = TuiState::new();
        state.export_flash = Some(crate::state::ExportFlash {
            scope: crate::export::ExportScope::All,
            outcome: ExportOutcome::Failed {
                message: "permission denied".to_string(),
            },
            when: std::time::Instant::now(),
        });

        let text = render_to_string(&state);

        assert!(
            text.contains("export all iterations failed: permission denied"),
            "should show export failure, got: {}",
            text
        );
    }

    fn footer_row(state: &TuiState, width: u16) -> String {
        // Row 0 is the top border; the content is the second row.
        render_to_string_with_width(state, width)
            .chars()
            .skip(width as usize)
            .collect()
    }

    #[test]
    fn human_ask_label_stays_whole_at_52_to_56_columns() {
        let mut state = TuiState::new();
        state.pending_ask =
            Some("Should the migration drop the legacy table or keep a view?".to_string());
        for width in 52..=56 {
            let row = footer_row(&state, width);
            assert!(row.contains("⚠ HUMAN ASK: Should"), "{width}: {row:?}");
            assert!(
                row.trim_end().ends_with('…'),
                "the question shortens with an ellipsis, never a silent cut: {row:?}"
            );
        }
    }

    #[test]
    fn run_indicator_is_never_displaced_at_52_to_56_columns() {
        let mut state = TuiState::new();
        state.loop_completed = true;
        state.final_cost_usd = Some(12.34);
        state.mouse_capture_enabled = true;
        state.new_iteration_alert = Some(7);
        state.following_latest = false;
        for width in 52..=56 {
            let row = footer_row(&state, width);
            assert!(row.trim_end().ends_with("■ DONE"), "{width}: {row:?}");
            assert!(row.contains("▶"), "the new-iteration alert stays: {row:?}");
            assert!(row.contains("Elapsed") || row.contains("00:00"), "{row:?}");
            // Lower-priority hints give way before anything is clipped.
            assert!(!row.contains("e export"), "{width}: {row:?}");
        }
    }
}
