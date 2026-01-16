//! State management for the TUI.

use ralph_proto::{Event, HatId};
use std::collections::HashMap;
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
    /// Previous iteration number (for detecting changes).
    pub prev_iteration: u32,
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
    /// Maximum iterations from config.
    pub max_iterations: Option<u32>,
    /// Idle timeout countdown.
    pub idle_timeout_remaining: Option<Duration>,
    /// Map of event topics to hat display information (for custom hats).
    /// Key: event topic (e.g., "review.security")
    /// Value: (HatId, display name including emoji)
    hat_map: HashMap<String, (HatId, String)>,
}

impl TuiState {
    /// Creates empty state.
    pub fn new() -> Self {
        Self {
            pending_hat: None,
            iteration: 0,
            prev_iteration: 0,
            loop_started: None,
            iteration_started: None,
            last_event: None,
            last_event_at: None,
            show_help: false,
            loop_mode: LoopMode::Auto,
            in_scroll_mode: false,
            search_query: String::new(),
            search_forward: true,
            max_iterations: None,
            idle_timeout_remaining: None,
            hat_map: HashMap::new(),
        }
    }

    /// Creates state with a custom hat map for dynamic topic-to-hat resolution.
    pub fn with_hat_map(hat_map: HashMap<String, (HatId, String)>) -> Self {
        Self {
            pending_hat: None,
            iteration: 0,
            prev_iteration: 0,
            loop_started: None,
            iteration_started: None,
            last_event: None,
            last_event_at: None,
            show_help: false,
            loop_mode: LoopMode::Auto,
            in_scroll_mode: false,
            search_query: String::new(),
            search_forward: true,
            max_iterations: None,
            idle_timeout_remaining: None,
            hat_map,
        }
    }

    /// Updates state based on event topic.
    pub fn update(&mut self, event: &Event) {
        let now = Instant::now();
        let topic = event.topic.as_str();

        self.last_event = Some(topic.to_string());
        self.last_event_at = Some(now);

        // First, check if we have a custom hat mapping for this topic
        if let Some((hat_id, hat_display)) = self.hat_map.get(topic) {
            self.pending_hat = Some((hat_id.clone(), hat_display.clone()));
            // Handle iteration timing for custom hats
            if topic.starts_with("build.") {
                self.iteration_started = Some(now);
            }
            return;
        }

        // Fall back to hardcoded mappings for backward compatibility
        match topic {
            "task.start" => {
                // Save hat_map before resetting
                let saved_hat_map = std::mem::take(&mut self.hat_map);
                *self = Self::new();
                self.hat_map = saved_hat_map;
                self.loop_started = Some(now);
                self.pending_hat = Some((HatId::new("planner"), "📋Planner".to_string()));
                self.last_event = Some(topic.to_string());
                self.last_event_at = Some(now);
            }
            "task.resume" => {
                self.loop_started = Some(now);
                self.pending_hat = Some((HatId::new("planner"), "📋Planner".to_string()));
            }
            "build.task" => {
                self.pending_hat = Some((HatId::new("builder"), "🔨Builder".to_string()));
                self.iteration_started = Some(now);
            }
            "build.done" => {
                self.pending_hat = Some((HatId::new("planner"), "📋Planner".to_string()));
                self.prev_iteration = self.iteration;
                self.iteration += 1;
            }
            "build.blocked" => {
                self.pending_hat = Some((HatId::new("planner"), "📋Planner".to_string()));
            }
            "loop.terminate" => {
                self.pending_hat = None;
            }
            _ => {
                // Unknown topic - don't change pending_hat
            }
        }
    }

    /// Returns formatted hat display (emoji + name).
    pub fn get_pending_hat_display(&self) -> String {
        self.pending_hat
            .as_ref()
            .map_or_else(|| "—".to_string(), |(_, display)| display.clone())
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
            .is_some_and(|t| t.elapsed() < Duration::from_secs(2))
    }

    /// True if iteration changed since last check.
    pub fn iteration_changed(&self) -> bool {
        self.iteration != self.prev_iteration
    }
}

impl Default for TuiState {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn iteration_changed_detects_boundary() {
        let mut state = TuiState::new();
        assert!(!state.iteration_changed(), "no change at start");

        // Simulate build.done event (increments iteration)
        let event = Event::new("build.done", "");
        state.update(&event);

        assert_eq!(state.iteration, 1);
        assert_eq!(state.prev_iteration, 0);
        assert!(state.iteration_changed(), "should detect iteration change");
    }

    #[test]
    fn iteration_changed_resets_after_check() {
        let mut state = TuiState::new();
        let event = Event::new("build.done", "");
        state.update(&event);

        assert!(state.iteration_changed());

        // Simulate clearing the flag (app.rs does this by updating prev_iteration)
        state.prev_iteration = state.iteration;
        assert!(!state.iteration_changed(), "flag should reset");
    }

    #[test]
    fn multiple_iterations_tracked() {
        let mut state = TuiState::new();

        for i in 1..=3 {
            let event = Event::new("build.done", "");
            state.update(&event);
            assert_eq!(state.iteration, i);
            assert!(state.iteration_changed());
            state.prev_iteration = state.iteration; // simulate app clearing flag
        }
    }

    #[test]
    fn custom_hat_topics_update_pending_hat() {
        // Test that custom hat topics (not hardcoded) update pending_hat correctly
        use std::collections::HashMap;

        // Create a hat map for custom hats
        let mut hat_map = HashMap::new();
        hat_map.insert(
            "review.security".to_string(),
            (
                HatId::new("security_reviewer"),
                "🔒 Security Reviewer".to_string(),
            ),
        );
        hat_map.insert(
            "review.correctness".to_string(),
            (
                HatId::new("correctness_reviewer"),
                "🎯 Correctness Reviewer".to_string(),
            ),
        );

        let mut state = TuiState::with_hat_map(hat_map);

        // Publish review.security event
        let event = Event::new("review.security", "Review PR #123");
        state.update(&event);

        // Should update pending_hat to security reviewer
        assert_eq!(
            state.get_pending_hat_display(),
            "🔒 Security Reviewer",
            "Should display security reviewer hat for review.security topic"
        );

        // Publish review.correctness event
        let event = Event::new("review.correctness", "Check logic");
        state.update(&event);

        // Should update to correctness reviewer
        assert_eq!(
            state.get_pending_hat_display(),
            "🎯 Correctness Reviewer",
            "Should display correctness reviewer hat for review.correctness topic"
        );
    }

    #[test]
    fn unknown_topics_keep_pending_hat_unchanged() {
        // Test that unknown topics don't clear pending_hat
        let mut state = TuiState::new();

        // Set initial hat
        state.pending_hat = Some((HatId::new("planner"), "📋Planner".to_string()));

        // Publish unknown event
        let event = Event::new("unknown.topic", "Some payload");
        state.update(&event);

        // Should keep the planner hat
        assert_eq!(
            state.get_pending_hat_display(),
            "📋Planner",
            "Unknown topics should not clear pending_hat"
        );
    }
}
