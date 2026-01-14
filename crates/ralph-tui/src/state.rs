//! State management for the TUI.

use ralph_proto::{Event, HatId};
use std::time::{Duration, Instant};

/// Loop execution mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LoopMode {
    Auto,
    Paused,
}

/// Observable state derived from loop events.
pub struct TuiState {
    /// Which hat will process next event (ID + display name).
    pub pending_hat: Option<(HatId, String)>,
    /// Current iteration number (0-indexed, display as +1).
    pub iteration: u32,
    /// When loop began.
    pub loop_started: Option<Instant>,
    /// When current iteration began.
    pub iteration_started: Option<Instant>,
    /// Most recent event topic.
    pub last_event: Option<String>,
    /// Timestamp of last event.
    pub last_event_at: Option<Instant>,
    /// Whether to show help overlay.
    pub show_help: bool,
    /// Loop execution mode.
    pub loop_mode: LoopMode,
    /// Whether in scroll mode.
    pub in_scroll_mode: bool,
    /// Current search query (if in search input mode).
    pub search_query: String,
    /// Search direction (true = forward, false = backward).
    pub search_forward: bool,
}

impl TuiState {
    /// Creates empty state.
    pub fn new() -> Self {
        Self {
            pending_hat: None,
            iteration: 0,
            loop_started: None,
            iteration_started: None,
            last_event: None,
            last_event_at: None,
            show_help: false,
            loop_mode: LoopMode::Auto,
            in_scroll_mode: false,
            search_query: String::new(),
            search_forward: true,
        }
    }

    /// Updates state based on event topic.
    pub fn update(&mut self, event: &Event) {
        let now = Instant::now();
        let topic = event.topic.as_str();

        self.last_event = Some(topic.to_string());
        self.last_event_at = Some(now);

        match topic {
            "task.start" => {
                *self = Self::new();
                self.loop_started = Some(now);
                self.pending_hat = Some((HatId::new("planner"), "📋 Planner".to_string()));
                self.last_event = Some(topic.to_string());
                self.last_event_at = Some(now);
            }
            "task.resume" => {
                self.loop_started = Some(now);
                self.pending_hat = Some((HatId::new("planner"), "📋 Planner".to_string()));
            }
            "build.task" => {
                self.pending_hat = Some((HatId::new("builder"), "🔨 Builder".to_string()));
                self.iteration_started = Some(now);
            }
            "build.done" => {
                self.pending_hat = Some((HatId::new("planner"), "📋 Planner".to_string()));
                self.iteration += 1;
            }
            "build.blocked" => {
                self.pending_hat = Some((HatId::new("planner"), "📋 Planner".to_string()));
            }
            "loop.terminate" => {
                self.pending_hat = None;
            }
            _ => {}
        }
    }

    /// Returns formatted hat display (emoji + name).
    pub fn get_pending_hat_display(&self) -> String {
        self.pending_hat
            .as_ref()
            .map(|(_, display)| display.clone())
            .unwrap_or_else(|| "—".to_string())
    }

    /// Time since loop started.
    pub fn get_loop_elapsed(&self) -> Option<Duration> {
        self.loop_started.map(|start| start.elapsed())
    }

    /// Time since iteration started.
    pub fn get_iteration_elapsed(&self) -> Option<Duration> {
        self.iteration_started.map(|start| start.elapsed())
    }

    /// True if event received in last 2 seconds.
    pub fn is_active(&self) -> bool {
        self.last_event_at
            .map(|t| t.elapsed() < Duration::from_secs(2))
            .unwrap_or(false)
    }
}

impl Default for TuiState {
    fn default() -> Self {
        Self::new()
    }
}
