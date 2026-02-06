//! Main application loop for the TUI.
//!
//! This module provides a read-only observation dashboard that displays
//! formatted output from the Ralph orchestrator, with iteration navigation,
//! scroll, and search functionality.

use crate::input::{Action, map_key};
use crate::state::TuiState;
use crate::widgets::{content::ContentPane, footer, header, help};
use anyhow::Result;
use crossterm::{
    cursor::Show,
    event::{
        DisableMouseCapture, EnableMouseCapture, Event, EventStream, KeyCode, KeyEventKind,
        KeyModifiers, MouseEventKind,
    },
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use futures::StreamExt;
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
};
use scopeguard::defer;
use std::io;
use std::sync::{Arc, Mutex};
use tokio::sync::watch;
use tokio::time::{Duration, interval};
use tracing::info;

/// Dispatches an action to the TuiState.
///
/// Returns `true` if the action signals to quit the application.
pub fn dispatch_action(action: Action, state: &mut TuiState, viewport_height: usize) -> bool {
    match action {
        Action::Quit => return true,
        Action::ScrollDown => {
            if let Some(buffer) = state.current_iteration_mut() {
                buffer.scroll_down(viewport_height);
            }
        }
        Action::ScrollUp => {
            if let Some(buffer) = state.current_iteration_mut() {
                buffer.scroll_up();
            }
        }
        Action::ScrollTop => {
            if let Some(buffer) = state.current_iteration_mut() {
                buffer.scroll_top();
            }
        }
        Action::ScrollBottom => {
            if let Some(buffer) = state.current_iteration_mut() {
                buffer.scroll_bottom(viewport_height);
            }
        }
        Action::NextIteration => {
            state.navigate_next();
        }
        Action::PrevIteration => {
            state.navigate_prev();
        }
        Action::ShowHelp => {
            state.show_help = true;
        }
        Action::DismissHelp => {
            state.show_help = false;
            state.clear_search();
        }
        Action::StartSearch => {
            state.search_state.search_mode = true;
        }
        Action::SearchNext => {
            state.next_match();
        }
        Action::SearchPrev => {
            state.prev_match();
        }
        Action::None => {}
    }
    false
}

/// Main TUI application for read-only observation.
pub struct App {
    state: Arc<Mutex<TuiState>>,
    /// Receives notification when the underlying process terminates.
    /// This is the ONLY exit path for the TUI event loop (besides Action::Quit).
    terminated_rx: watch::Receiver<bool>,
    /// Channel to signal main loop on Ctrl+C.
    /// In raw terminal mode, SIGINT is not generated, so TUI must signal
    /// the main orchestration loop through this channel.
    interrupt_tx: Option<watch::Sender<bool>>,
}

impl App {
    /// Creates a new App with shared state, termination signal, and optional interrupt channel.
    pub fn new(
        state: Arc<Mutex<TuiState>>,
        terminated_rx: watch::Receiver<bool>,
        interrupt_tx: Option<watch::Sender<bool>>,
    ) -> Self {
        Self {
            state,
            terminated_rx,
            interrupt_tx,
        }
    }

    /// Runs the TUI event loop.
    pub async fn run(mut self) -> Result<()> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;
        terminal.clear()?;

        // CRITICAL: Ensure terminal cleanup on ANY exit path (normal, abort, or panic).
        // When cleanup_tui() calls handle.abort(), the task is cancelled immediately
        // at its current await point, skipping all code after the loop. This defer!
        // guard runs on Drop, which is guaranteed even during task cancellation.
        defer! {
            let _ = disable_raw_mode();
            let _ = execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture, Show);
        }

        // Event-driven architecture: input polling is the primary driver
        // Render is throttled to ~60fps via interval tick
        let mut events = EventStream::new();
        let mut render_tick = interval(Duration::from_millis(16));

        // Track viewport height for scroll calculations
        let mut viewport_height: usize = 24; // Default, updated on render

        loop {
            // Use biased select to prioritize input over render ticks
            tokio::select! {
                biased;

                // Priority 1: Handle input events immediately for responsiveness
                maybe_event = events.next() => {
                    match maybe_event {
                        Some(Ok(event)) => {
                            match event {
                                // Handle Ctrl+C: signal main loop and exit.
                                // In raw mode, SIGINT is not generated, so we must signal the
                                // main orchestration loop through interrupt_tx channel.
                                Event::Key(key) if key.kind == KeyEventKind::Press
                                    && key.code == KeyCode::Char('c')
                                    && key.modifiers.contains(KeyModifiers::CONTROL) =>
                                {
                                    info!("Ctrl+C detected, signaling main loop");
                                    if let Some(ref tx) = self.interrupt_tx {
                                        let _ = tx.send(true);
                                    }
                                    break;
                                }
                                Event::Mouse(mouse) => {
                                    match mouse.kind {
                                        MouseEventKind::ScrollUp => {
                                            let mut state = self.state.lock().unwrap();
                                            if let Some(buffer) = state.current_iteration_mut() {
                                                for _ in 0..3 {
                                                    buffer.scroll_up();
                                                }
                                            }
                                        }
                                        MouseEventKind::ScrollDown => {
                                            let mut state = self.state.lock().unwrap();
                                            if let Some(buffer) = state.current_iteration_mut() {
                                                for _ in 0..3 {
                                                    buffer.scroll_down(viewport_height);
                                                }
                                            }
                                        }
                                        _ => {}
                                    }
                                }
                                Event::Key(key) if key.kind == KeyEventKind::Press => {
                                    // Dismiss help on any key when help is showing
                                    {
                                        let mut state = self.state.lock().unwrap();
                                        if state.show_help {
                                            state.show_help = false;
                                            continue;
                                        }
                                    }

                                    // Map key to action and dispatch
                                    let action = map_key(key);
                                    let mut state = self.state.lock().unwrap();
                                    if dispatch_action(action, &mut state, viewport_height) {
                                        break;
                                    }
                                }
                                // Ignore other events (FocusGained, FocusLost, Paste, Resize, key releases)
                                _ => {}
                            }
                        }
                        Some(Err(e)) => {
                            // Log error but continue - transient errors shouldn't crash TUI
                            tracing::warn!("Event stream error: {}", e);
                        }
                        None => {
                            // Stream ended unexpectedly
                            break;
                        }
                    }
                }

                // Priority 2: Render at throttled rate (~60fps)
                _ = render_tick.tick() => {
                    let frame_size = terminal.size()?;
                    let frame_area = ratatui::layout::Rect::new(0, 0, frame_size.width, frame_size.height);
                    let chunks = Layout::default()
                        .direction(Direction::Vertical)
                        .constraints([
                            Constraint::Length(2),  // Header: content + bottom border
                            Constraint::Min(0),     // Content: flexible
                            Constraint::Length(2),  // Footer: top border + content
                        ])
                        .split(frame_area);

                    let content_area = chunks[1];
                    viewport_height = content_area.height as usize;

                    let mut state = self.state.lock().unwrap();

                    // Autoscroll: if user hasn't scrolled away, keep them at the bottom
                    // as new content arrives. This mimics standard terminal behavior.
                    if let Some(buffer) = state.current_iteration_mut()
                        && buffer.following_bottom
                    {
                        let max_scroll = buffer.line_count().saturating_sub(viewport_height);
                        buffer.scroll_offset = max_scroll;
                    }

                    let state = state; // Rebind as immutable for rendering
                    terminal.draw(|f| {
                        // Render header
                        f.render_widget(header::render(&state, chunks[0].width), chunks[0]);

                        // Render content using ContentPane
                        if let Some(buffer) = state.current_iteration() {
                            let mut content_widget = ContentPane::new(buffer);
                            if let Some(query) = &state.search_state.query {
                                content_widget = content_widget.with_search(query);
                            }
                            f.render_widget(content_widget, content_area);
                        }

                        // Render footer
                        f.render_widget(footer::render(&state), chunks[2]);

                        // Render help overlay if active
                        if state.show_help {
                            help::render(f, f.area());
                        }
                    })?;
                }

                // Priority 3: Handle termination signal
                _ = self.terminated_rx.changed() => {
                    if *self.terminated_rx.borrow() {
                        break;
                    }
                }
            }
        }

        // NOTE: Explicit cleanup removed - now handled by defer! guard above.
        // The guard ensures cleanup happens even on task abort or panic.
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::{Action, map_key};
    use crate::state::TuiState;
    use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
    use ratatui::text::Line;

    // =========================================================================
    // AC1: Events Reach State — TuiStreamHandler → IterationBuffer
    // =========================================================================

    #[test]
    fn dispatch_action_scroll_down_calls_scroll_down_on_current_buffer() {
        // Given TuiState with an iteration buffer containing content
        let mut state = TuiState::new();
        state.start_new_iteration();
        let buffer = state.current_iteration_mut().unwrap();
        for i in 0..20 {
            buffer.append_line(Line::from(format!("line {}", i)));
        }
        let initial_offset = state.current_iteration().unwrap().scroll_offset;
        assert_eq!(initial_offset, 0);

        // When dispatch_action with ScrollDown and viewport_height 10
        dispatch_action(Action::ScrollDown, &mut state, 10);

        // Then scroll_offset is incremented
        assert_eq!(
            state.current_iteration().unwrap().scroll_offset,
            1,
            "scroll_down should increment scroll_offset"
        );
    }

    // =========================================================================
    // AC2: Keyboard Triggers Actions — 'j' → scroll_down()
    // =========================================================================

    #[test]
    fn j_key_triggers_scroll_down_action() {
        // Given key press 'j'
        let key = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE);

        // When map_key is called
        let action = map_key(key);

        // Then Action::ScrollDown is returned
        assert_eq!(action, Action::ScrollDown);
    }

    #[test]
    fn dispatch_action_scroll_up_calls_scroll_up_on_current_buffer() {
        let mut state = TuiState::new();
        state.start_new_iteration();
        let buffer = state.current_iteration_mut().unwrap();
        for i in 0..20 {
            buffer.append_line(Line::from(format!("line {}", i)));
        }
        // Set initial scroll offset to 5
        state.current_iteration_mut().unwrap().scroll_offset = 5;

        dispatch_action(Action::ScrollUp, &mut state, 10);

        assert_eq!(
            state.current_iteration().unwrap().scroll_offset,
            4,
            "scroll_up should decrement scroll_offset"
        );
    }

    #[test]
    fn dispatch_action_scroll_top_jumps_to_top() {
        let mut state = TuiState::new();
        state.start_new_iteration();
        let buffer = state.current_iteration_mut().unwrap();
        for _ in 0..20 {
            buffer.append_line(Line::from("line"));
        }
        state.current_iteration_mut().unwrap().scroll_offset = 10;

        dispatch_action(Action::ScrollTop, &mut state, 10);

        assert_eq!(state.current_iteration().unwrap().scroll_offset, 0);
    }

    #[test]
    fn dispatch_action_scroll_bottom_jumps_to_bottom() {
        let mut state = TuiState::new();
        state.start_new_iteration();
        let buffer = state.current_iteration_mut().unwrap();
        for _ in 0..20 {
            buffer.append_line(Line::from("line"));
        }

        dispatch_action(Action::ScrollBottom, &mut state, 10);

        // max_scroll = 20 - 10 = 10
        assert_eq!(state.current_iteration().unwrap().scroll_offset, 10);
    }

    #[test]
    fn dispatch_action_next_iteration_navigates_forward() {
        let mut state = TuiState::new();
        state.start_new_iteration();
        state.start_new_iteration();
        state.start_new_iteration();
        state.current_view = 0;
        state.following_latest = false;

        dispatch_action(Action::NextIteration, &mut state, 10);

        assert_eq!(state.current_view, 1);
    }

    #[test]
    fn dispatch_action_prev_iteration_navigates_backward() {
        let mut state = TuiState::new();
        state.start_new_iteration();
        state.start_new_iteration();
        state.start_new_iteration();
        state.current_view = 2;

        dispatch_action(Action::PrevIteration, &mut state, 10);

        assert_eq!(state.current_view, 1);
    }

    #[test]
    fn dispatch_action_show_help_sets_show_help() {
        let mut state = TuiState::new();
        assert!(!state.show_help);

        dispatch_action(Action::ShowHelp, &mut state, 10);

        assert!(state.show_help);
    }

    #[test]
    fn dispatch_action_dismiss_help_clears_show_help() {
        let mut state = TuiState::new();
        state.show_help = true;

        dispatch_action(Action::DismissHelp, &mut state, 10);

        assert!(!state.show_help);
    }

    #[test]
    fn dispatch_action_search_next_calls_next_match() {
        let mut state = TuiState::new();
        state.start_new_iteration();
        let buffer = state.current_iteration_mut().unwrap();
        buffer.append_line(Line::from("find me"));
        buffer.append_line(Line::from("find me again"));
        state.search("find");
        assert_eq!(state.search_state.current_match, 0);

        dispatch_action(Action::SearchNext, &mut state, 10);

        assert_eq!(state.search_state.current_match, 1);
    }

    #[test]
    fn dispatch_action_search_prev_calls_prev_match() {
        let mut state = TuiState::new();
        state.start_new_iteration();
        let buffer = state.current_iteration_mut().unwrap();
        buffer.append_line(Line::from("find me"));
        buffer.append_line(Line::from("find me again"));
        state.search("find");
        state.search_state.current_match = 1;

        dispatch_action(Action::SearchPrev, &mut state, 10);

        assert_eq!(state.search_state.current_match, 0);
    }

    // =========================================================================
    // AC5: Quit Returns True to Exit Loop
    // =========================================================================

    #[test]
    fn dispatch_action_quit_returns_true() {
        let mut state = TuiState::new();
        let should_quit = dispatch_action(Action::Quit, &mut state, 10);
        assert!(should_quit, "Quit action should return true to signal exit");
    }

    #[test]
    fn dispatch_action_non_quit_returns_false() {
        let mut state = TuiState::new();
        state.start_new_iteration();
        let buffer = state.current_iteration_mut().unwrap();
        buffer.append_line(Line::from("line"));

        let should_quit = dispatch_action(Action::ScrollDown, &mut state, 10);
        assert!(!should_quit, "Non-quit actions should return false");
    }

    // =========================================================================
    // AC6: No PTY Code — Structural Test
    // =========================================================================

    #[test]
    fn no_pty_handle_in_app() {
        let source = include_str!("app.rs");
        let test_module_start = source.find("#[cfg(test)]").unwrap_or(source.len());
        let production_code = &source[..test_module_start];

        // Check for PTY-related imports/code
        assert!(
            !production_code.contains("PtyHandle"),
            "app.rs should not contain PtyHandle after refactor"
        );
        assert!(
            !production_code.contains("tui_term"),
            "app.rs should not contain tui_term references after refactor"
        );
        assert!(
            !production_code.contains("TerminalWidget"),
            "app.rs should not contain TerminalWidget after refactor"
        );
    }

    /// Regression test: TUI must NOT have tokio::signal::ctrl_c() handler.
    ///
    /// Raw mode prevents SIGINT, so tokio's signal handler never fires.
    /// TUI must detect Ctrl+C directly via crossterm events.
    #[test]
    fn no_tokio_signal_handler_in_app() {
        let source = include_str!("app.rs");
        let pattern = ["tokio", "::", "signal", "::", "ctrl_c", "()"].concat();
        let test_module_start = source.find("#[cfg(test)]").unwrap_or(source.len());
        let production_code = &source[..test_module_start];
        let occurrences: Vec<_> = production_code.match_indices(&pattern).collect();
        assert!(
            occurrences.is_empty(),
            "Found {} occurrence(s) of tokio::signal::ctrl_c() in production code. \
             This doesn't work in raw mode - use crossterm events instead.",
            occurrences.len()
        );
    }

    /// Verify Ctrl+C handling exists in production code.
    ///
    /// Since raw mode prevents SIGINT, we must handle Ctrl+C via crossterm events.
    /// TUI is observation-only, so Ctrl+C breaks out of the event loop.
    #[test]
    fn ctrl_c_handling_exists_in_app() {
        let source = include_str!("app.rs");
        let test_module_start = source.find("#[cfg(test)]").unwrap_or(source.len());
        let production_code = &source[..test_module_start];

        assert!(
            production_code.contains("KeyCode::Char('c')")
                && production_code.contains("KeyModifiers::CONTROL"),
            "Production code must detect Ctrl+C via crossterm events"
        );
    }
}
