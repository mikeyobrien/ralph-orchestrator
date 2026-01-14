//! Main application loop for the TUI.

use crate::input::{Command, InputRouter, RouteResult};
use crate::scroll::ScrollManager;
use crate::state::TuiState;
use crate::widgets::{footer, header, help, terminal::TerminalWidget};
use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ralph_adapters::pty_handle::PtyHandle;
use ratatui::{
    Terminal,
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
};
use std::io;
use std::sync::{Arc, Mutex};
use tokio::sync::mpsc;
use tokio::time::{Duration, interval};

/// Main TUI application.
pub struct App {
    state: Arc<Mutex<TuiState>>,
    terminal_widget: Arc<Mutex<TerminalWidget>>,
    input_router: InputRouter,
    scroll_manager: ScrollManager,
    input_tx: mpsc::UnboundedSender<Vec<u8>>,
    control_tx: mpsc::UnboundedSender<ralph_adapters::pty_handle::ControlCommand>,
}

impl App {
    /// Creates a new App with shared state and PTY handle.
    pub fn new(state: Arc<Mutex<TuiState>>, pty_handle: PtyHandle) -> Self {
        let terminal_widget = Arc::new(Mutex::new(TerminalWidget::new()));

        let PtyHandle {
            mut output_rx,
            input_tx,
            control_tx,
        } = pty_handle;

        // Spawn task to read PTY output and feed to terminal widget
        let widget_clone = Arc::clone(&terminal_widget);
        tokio::spawn(async move {
            while let Some(bytes) = output_rx.recv().await {
                if let Ok(mut widget) = widget_clone.lock() {
                    widget.process(&bytes);
                }
            }
        });

        Self {
            state,
            terminal_widget,
            input_router: InputRouter::new(),
            scroll_manager: ScrollManager::new(),
            input_tx,
            control_tx,
        }
    }

    /// Runs the TUI event loop.
    pub async fn run(mut self) -> Result<()> {
        enable_raw_mode()?;
        let mut stdout = io::stdout();
        execute!(stdout, EnterAlternateScreen)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        let mut tick = interval(Duration::from_millis(100));

        loop {
            tokio::select! {
                _ = tick.tick() => {
                    let state = self.state.lock().unwrap();
                    let widget = self.terminal_widget.lock().unwrap();
                    terminal.draw(|f| {
                        let chunks = Layout::default()
                            .direction(Direction::Vertical)
                            .constraints([
                                Constraint::Length(3),
                                Constraint::Min(0),
                                Constraint::Length(3),
                            ])
                            .split(f.area());

                        f.render_widget(header::render(&state), chunks[0]);
                        f.render_widget(tui_term::widget::PseudoTerminal::new(widget.parser().screen()), chunks[1]);
                        f.render_widget(footer::render(&state, &self.scroll_manager), chunks[2]);

                        if state.show_help {
                            help::render(f, f.area());
                        }
                    })?;

                    // Poll for keyboard events
                    if event::poll(Duration::from_millis(0))? {
                        if let Event::Key(key) = event::read()? {
                            if key.kind == KeyEventKind::Press {
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
                                            if let KeyCode::Char(c) = key.code {
                                                let _ = self.input_tx.send(vec![c as u8]);
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
                        }
                    }
                }
                _ = tokio::signal::ctrl_c() => {
                    break;
                }
            }
        }

        disable_raw_mode()?;
        execute!(terminal.backend_mut(), LeaveAlternateScreen)?;

        Ok(())
    }
}
