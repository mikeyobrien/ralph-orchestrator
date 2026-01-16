//! Main application loop for the TUI.

use crate::input::{Command, InputRouter, RouteResult};
use crate::scroll::ScrollManager;
use crate::state::TuiState;
use crate::widgets::{footer, header, help, terminal::TerminalWidget};
use anyhow::Result;
use crossterm::{
    cursor::Show,
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyEventKind,
        KeyModifiers, MouseEventKind,
    },
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ralph_adapters::pty_handle::PtyHandle;
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
};
use scopeguard::defer;
use std::io;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};
use tokio::sync::{mpsc, watch};
use tokio::time::{Duration, interval};

/// Main TUI application.
pub struct App {
    state: Arc<Mutex<TuiState>>,
    terminal_widget: Arc<Mutex<TerminalWidget>>,
    input_router: InputRouter,
    scroll_manager: ScrollManager,
    input_tx: mpsc::UnboundedSender<Vec<u8>>,
    control_tx: mpsc::UnboundedSender<ralph_adapters::pty_handle::ControlCommand>,
    /// Atomic iteration counter for synchronizing the PTY output task.
    /// Incremented when iteration boundaries are detected, allowing the
    /// background task to skip orphaned bytes from previous iterations.
    iteration_counter: Arc<AtomicU32>,
    /// Receives notification when PTY process terminates.
    /// Used to break the event loop and exit cleanly on double Ctrl+C.
    terminated_rx: watch::Receiver<bool>,
}

impl App {
    /// Creates a new App with shared state and PTY handle.
    #[allow(dead_code)] // Public API - may be used by external callers
    pub fn new(state: Arc<Mutex<TuiState>>, pty_handle: PtyHandle) -> Self {
        Self::with_prefix(
            state,
            pty_handle,
            KeyCode::Char('a'),
            crossterm::event::KeyModifiers::CONTROL,
        )
    }

    /// Creates a new App with custom prefix key.
    pub fn with_prefix(
        state: Arc<Mutex<TuiState>>,
        pty_handle: PtyHandle,
        prefix_key: KeyCode,
        prefix_modifiers: crossterm::event::KeyModifiers,
    ) -> Self {
        let terminal_widget = Arc::new(Mutex::new(TerminalWidget::new()));
        let iteration_counter = Arc::new(AtomicU32::new(0));

        let PtyHandle {
            mut output_rx,
            input_tx,
            control_tx,
            terminated_rx,
        } = pty_handle;

        // Spawn task to read PTY output and feed to terminal widget.
        // Uses iteration_counter to detect iteration boundaries and skip
        // orphaned bytes from previous iterations, preventing rendering corruption.
        let widget_clone = Arc::clone(&terminal_widget);
        let iteration_for_task = Arc::clone(&iteration_counter);
        tokio::spawn(async move {
            let mut last_seen_iteration = 0;
            while let Some(bytes) = output_rx.recv().await {
                let current = iteration_for_task.load(Ordering::Acquire);
                if current != last_seen_iteration {
                    // Iteration boundary detected - skip orphaned bytes from old iteration
                    last_seen_iteration = current;
                    continue;
                }
                if let Ok(mut widget) = widget_clone.lock() {
                    widget.process(&bytes);
                }
            }
        });

        Self {
            state,
            terminal_widget,
            input_router: InputRouter::with_prefix(prefix_key, prefix_modifiers),
            scroll_manager: ScrollManager::new(),
            input_tx,
            control_tx,
            iteration_counter,
            terminated_rx,
        }
    }

    /// Runs the TUI event loop.
    #[allow(clippy::too_many_lines)]
    pub async fn run(mut self) -> Result<()> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        // CRITICAL: Ensure terminal cleanup on ANY exit path (normal, abort, or panic).
        // When cleanup_tui() calls handle.abort(), the task is cancelled immediately
        // at its current await point, skipping all code after the loop. This defer!
        // guard runs on Drop, which is guaranteed even during task cancellation.
        defer! {
            let _ = disable_raw_mode();
            let _ = execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture, Show);
        }

        let mut tick = interval(Duration::from_millis(100));

        // Track previous terminal size to detect changes
        let mut last_terminal_size: Option<(u16, u16)> = None;

        loop {
            tokio::select! {
                _ = tick.tick() => {
                    // Check for iteration change and clear terminal
                    {
                        let mut state = self.state.lock().unwrap();
                        if state.iteration_changed() {
                            state.prev_iteration = state.iteration;
                            drop(state);

                            // Signal iteration boundary to output task BEFORE clearing.
                            // This ensures orphaned bytes from the old iteration are skipped.
                            self.iteration_counter.fetch_add(1, Ordering::Release);

                            let mut widget = self.terminal_widget.lock().unwrap();
                            widget.clear();
                            self.scroll_manager.reset();
                        }
                    }

                    // Compute layout to get terminal area dimensions
                    let frame_size = terminal.size()?;
                    let frame_area = ratatui::layout::Rect::new(0, 0, frame_size.width, frame_size.height);
                    let chunks = Layout::default()
                        .direction(Direction::Vertical)
                        .constraints([
                            Constraint::Length(3),
                            Constraint::Min(0),
                            Constraint::Length(3),
                        ])
                        .split(frame_area);

                    let terminal_area = chunks[1];
                    let new_size = (terminal_area.height, terminal_area.width);

                    // Resize terminal widget and notify PTY if size changed
                    if last_terminal_size != Some(new_size) {
                        last_terminal_size = Some(new_size);
                        {
                            let mut widget = self.terminal_widget.lock().unwrap();
                            widget.resize(new_size.0, new_size.1);
                        }
                        // Notify PTY of resize (cols, rows order per ControlCommand::Resize)
                        let _ = self.control_tx.send(ralph_adapters::pty_handle::ControlCommand::Resize(new_size.1, new_size.0));
                    }

                    let state = self.state.lock().unwrap();
                    let widget = self.terminal_widget.lock().unwrap();
                    terminal.draw(|f| {
                        f.render_widget(header::render(&state), chunks[0]);
                        f.render_widget(tui_term::widget::PseudoTerminal::new(widget.parser().screen()), chunks[1]);
                        f.render_widget(footer::render(&state, &self.scroll_manager), chunks[2]);

                        if state.show_help {
                            help::render(f, f.area());
                        }
                    })?;

                    // Poll for input events (keyboard and mouse)
                    if event::poll(Duration::from_millis(0))? {
                        match event::read()? {
                            Event::Mouse(mouse) => {
                                // Handle mouse scroll - works in any mode for better UX
                                match mouse.kind {
                                    MouseEventKind::ScrollUp => {
                                        // Enter scroll mode if not already
                                        if !self.state.lock().unwrap().in_scroll_mode {
                                            self.input_router.enter_scroll_mode();
                                            self.state.lock().unwrap().in_scroll_mode = true;
                                            let widget = self.terminal_widget.lock().unwrap();
                                            let total_lines = widget.total_lines();
                                            drop(widget);
                                            self.scroll_manager.update_dimensions(
                                                total_lines,
                                                terminal.size()?.height as usize - 6,
                                            );
                                        }
                                        self.scroll_manager.scroll_up(3);
                                    }
                                    MouseEventKind::ScrollDown => {
                                        if self.state.lock().unwrap().in_scroll_mode {
                                            self.scroll_manager.scroll_down(3);
                                            // Exit scroll mode if we've scrolled to bottom
                                            if self.scroll_manager.offset() == 0 {
                                                self.input_router.exit_scroll_mode();
                                                self.scroll_manager.reset();
                                                self.state.lock().unwrap().in_scroll_mode = false;
                                            }
                                        }
                                    }
                                    _ => {}
                                }
                            }
                            Event::Key(key) if key.kind == KeyEventKind::Press => {
                                // Dismiss help on any key
                                if self.state.lock().unwrap().show_help {
                                    self.state.lock().unwrap().show_help = false;
                                    continue;
                                }

                                match self.input_router.route_key(key) {
                                    RouteResult::Forward(key) => {
                                        // Only forward to PTY if not paused
                                        let is_paused = self.state.lock().unwrap().loop_mode == crate::state::LoopMode::Paused;
                                        if !is_paused {
                                            // Convert key to bytes and send to PTY
                                            if let Some(bytes) = key_event_to_bytes(key) {
                                                let _ = self.input_tx.send(bytes);
                                            }
                                        }
                                    }
                                    RouteResult::Command(cmd) => {
                                        match cmd {
                                            Command::Quit => break,
                                            Command::Help => {
                                                self.state.lock().unwrap().show_help = true;
                                            }
                                            Command::Pause => {
                                                let mut state = self.state.lock().unwrap();
                                                state.loop_mode = match state.loop_mode {
                                                    crate::state::LoopMode::Auto => crate::state::LoopMode::Paused,
                                                    crate::state::LoopMode::Paused => crate::state::LoopMode::Auto,
                                                };
                                            }
                                            Command::Skip => {
                                                let _ = self.control_tx.send(ralph_adapters::pty_handle::ControlCommand::Skip);
                                            }
                                            Command::Abort => {
                                                let _ = self.control_tx.send(ralph_adapters::pty_handle::ControlCommand::Abort);
                                            }
                                            Command::EnterScroll => {
                                                self.input_router.enter_scroll_mode();
                                                self.state.lock().unwrap().in_scroll_mode = true;
                                                // Update scroll dimensions
                                                let widget = self.terminal_widget.lock().unwrap();
                                                let total_lines = widget.total_lines();
                                                drop(widget);
                                                self.scroll_manager.update_dimensions(total_lines, terminal.size()?.height as usize - 6);
                                            }
                                            Command::Unknown => {}
                                        }
                                    }
                                    RouteResult::ScrollKey(key) => {
                                        // Handle n/N for search navigation
                                        match key.code {
                                            KeyCode::Char('n') => self.scroll_manager.next_match(),
                                            KeyCode::Char('N') => self.scroll_manager.prev_match(),
                                            _ => self.scroll_manager.handle_key(key),
                                        }
                                    }
                                    RouteResult::ExitScroll => {
                                        self.scroll_manager.reset();
                                        self.scroll_manager.clear_search();
                                        self.state.lock().unwrap().in_scroll_mode = false;
                                    }
                                    RouteResult::EnterSearch { forward } => {
                                        let mut state = self.state.lock().unwrap();
                                        state.search_query.clear();
                                        state.search_forward = forward;
                                    }
                                    RouteResult::SearchInput(key) => {
                                        if let KeyCode::Char(c) = key.code {
                                            self.state.lock().unwrap().search_query.push(c);
                                        } else if matches!(key.code, KeyCode::Backspace) {
                                            self.state.lock().unwrap().search_query.pop();
                                        }
                                    }
                                    RouteResult::ExecuteSearch => {
                                        let state = self.state.lock().unwrap();
                                        let query = state.search_query.clone();
                                        let direction = if state.search_forward {
                                            crate::scroll::SearchDirection::Forward
                                        } else {
                                            crate::scroll::SearchDirection::Backward
                                        };
                                        drop(state);

                                        // Get terminal contents
                                        let widget = self.terminal_widget.lock().unwrap();
                                        let screen = widget.parser().screen();
                                        let (_rows, cols) = screen.size();
                                        let lines: Vec<String> = screen.rows(0, cols).collect();
                                        drop(widget);

                                        self.scroll_manager.start_search(query, direction, &lines);
                                    }
                                    RouteResult::CancelSearch => {
                                        self.state.lock().unwrap().search_query.clear();
                                    }
                                    RouteResult::Consumed => {
                                        // Prefix consumed, wait for command
                                    }
                                }
                            }
                            // Ignore other events (FocusGained, FocusLost, Paste, Resize, key releases)
                            _ => {}
                        }
                    }
                }
                _ = tokio::signal::ctrl_c() => {
                    break;
                }
                _ = self.terminated_rx.changed() => {
                    // PTY process terminated (e.g., double Ctrl+C)
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

fn key_event_to_bytes(key: KeyEvent) -> Option<Vec<u8>> {
    match key.code {
        KeyCode::Char(c) => {
            if key.modifiers.contains(KeyModifiers::CONTROL) {
                let upper = c.to_ascii_uppercase() as u8;
                return Some(vec![upper & 0x1f]);
            }
            Some(vec![c as u8])
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ctrl_c_maps_to_etx() {
        let key = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        let bytes = key_event_to_bytes(key).expect("bytes");
        assert_eq!(bytes, vec![3]);
    }

    #[test]
    fn plain_char_maps_to_byte() {
        let key = KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE);
        let bytes = key_event_to_bytes(key).expect("bytes");
        assert_eq!(bytes, vec![b'x']);
    }
}
