//! State management for the TUI.

use ralph_proto::{Event, HatId};
use std::collections::HashMap;
use std::time::{Duration, Instant};

// ============================================================================
// TaskSummary - Summary of a single task for TUI display
// ============================================================================

/// Summary of a task for TUI display.
/// Contains only the fields needed for rendering.
#[derive(Debug, Clone, Default)]
pub struct TaskSummary {
    /// Task identifier (e.g., "task-1737372000-a1b2").
    pub id: String,
    /// Task title/description.
    pub title: String,
    /// Task status (e.g., "open", "closed", "blocked").
    pub status: String,
}

impl TaskSummary {
    /// Creates a new task summary.
    pub fn new(id: impl Into<String>, title: impl Into<String>, status: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            title: title.into(),
            status: status.into(),
        }
    }
}

// ============================================================================
// TaskCounts - Aggregate task statistics for TUI display
// ============================================================================

/// Aggregate task statistics for TUI display.
#[derive(Debug, Clone, Default)]
pub struct TaskCounts {
    /// Total number of tasks.
    pub total: usize,
    /// Number of open tasks.
    pub open: usize,
    /// Number of closed tasks.
    pub closed: usize,
    /// Number of ready (unblocked) tasks.
    pub ready: usize,
}

impl TaskCounts {
    /// Creates new task counts.
    pub fn new(total: usize, open: usize, closed: usize, ready: usize) -> Self {
        Self {
            total,
            open,
            closed,
            ready,
        }
    }
}

// ============================================================================
// SearchState - Search functionality for TUI content
// ============================================================================

/// Search state for finding and navigating matches in TUI content.
/// Tracks the current query, match positions, and navigation index.
#[derive(Debug, Default)]
pub struct SearchState {
    /// Current search query (None when no active search).
    pub query: Option<String>,
    /// Match positions as (line_index, char_offset) pairs.
    pub matches: Vec<(usize, usize)>,
    /// Index into matches vector for current match.
    pub current_match: usize,
    /// Whether search input mode is active (user is typing query).
    pub search_mode: bool,
}

impl SearchState {
    /// Creates a new empty search state.
    pub fn new() -> Self {
        Self::default()
    }

    /// Clears all search state.
    pub fn clear(&mut self) {
        self.query = None;
        self.matches.clear();
        self.current_match = 0;
        self.search_mode = false;
    }
}

/// Observable state derived from loop events.
pub struct TuiState {
    /// Which hat will process next event (ID + display name).
    pub pending_hat: Option<(HatId, String)>,
    /// Backend expected for the next iteration (used when metadata is missing).
    pub pending_backend: Option<String>,
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

    // ========================================================================
    // Iteration Management (new fields for TUI refactor)
    // ========================================================================
    /// Content buffers for each iteration.
    pub iterations: Vec<IterationBuffer>,
    /// Index of the iteration currently being viewed (0-indexed).
    pub current_view: usize,
    /// Whether to automatically follow the latest iteration.
    pub following_latest: bool,
    /// Alert about a new iteration (shown when viewing history and new iteration arrives).
    /// Contains the iteration number to alert about. Cleared when navigating to latest.
    pub new_iteration_alert: Option<usize>,

    // ========================================================================
    // Search State
    // ========================================================================
    /// Search state for finding and navigating matches in iteration content.
    pub search_state: SearchState,

    // ========================================================================
    // Completion State
    // ========================================================================
    /// Whether the loop has completed (received loop.terminate event).
    pub loop_completed: bool,
    /// Frozen elapsed time when loop completed (timer stops at this value).
    pub final_iteration_elapsed: Option<Duration>,
    /// Frozen total elapsed time when loop completed (footer timer stops).
    pub final_loop_elapsed: Option<Duration>,

    // ========================================================================
    // Task Tracking State
    // ========================================================================
    /// Aggregate task counts for display in TUI widgets.
    pub task_counts: TaskCounts,
    /// Currently active task (if any) for display in TUI widgets.
    pub active_task: Option<TaskSummary>,
}

impl TuiState {
    /// Creates empty state. Timer starts immediately at creation.
    pub fn new() -> Self {
        Self {
            pending_hat: None,
            pending_backend: None,
            iteration: 0,
            prev_iteration: 0,
            loop_started: Some(Instant::now()),
            iteration_started: None,
            last_event: None,
            last_event_at: None,
            show_help: false,
            in_scroll_mode: false,
            search_query: String::new(),
            search_forward: true,
            max_iterations: None,
            idle_timeout_remaining: None,
            hat_map: HashMap::new(),
            // Iteration management
            iterations: Vec::new(),
            current_view: 0,
            following_latest: true,
            new_iteration_alert: None,
            // Search state
            search_state: SearchState::new(),
            // Completion state
            loop_completed: false,
            final_iteration_elapsed: None,
            final_loop_elapsed: None,
            // Task tracking state
            task_counts: TaskCounts::default(),
            active_task: None,
        }
    }

    /// Creates state with a custom hat map for dynamic topic-to-hat resolution.
    /// Timer starts immediately at creation.
    pub fn with_hat_map(hat_map: HashMap<String, (HatId, String)>) -> Self {
        Self {
            pending_hat: None,
            pending_backend: None,
            iteration: 0,
            prev_iteration: 0,
            loop_started: Some(Instant::now()),
            iteration_started: None,
            last_event: None,
            last_event_at: None,
            show_help: false,
            in_scroll_mode: false,
            search_query: String::new(),
            search_forward: true,
            max_iterations: None,
            idle_timeout_remaining: None,
            hat_map,
            // Iteration management
            iterations: Vec::new(),
            current_view: 0,
            following_latest: true,
            new_iteration_alert: None,
            // Search state
            search_state: SearchState::new(),
            // Completion state
            loop_completed: false,
            final_iteration_elapsed: None,
            final_loop_elapsed: None,
            // Task tracking state
            task_counts: TaskCounts::default(),
            active_task: None,
        }
    }

    /// Updates state based on event topic.
    pub fn update(&mut self, event: &Event) {
        let now = Instant::now();
        let topic = event.topic.as_str();

        self.last_event = Some(topic.to_string());
        self.last_event_at = Some(now);

        let custom_hat = self.hat_map.get(topic).cloned();
        if let Some((hat_id, hat_display)) = custom_hat.clone() {
            self.pending_hat = Some((hat_id, hat_display));
            // Handle iteration timing for custom hats
            if topic.starts_with("build.") {
                self.iteration_started = Some(now);
            }
        }

        // Fall back to hardcoded mappings for backward compatibility
        match topic {
            "task.start" => {
                // Save state we want to preserve across reset
                let saved_hat_map = std::mem::take(&mut self.hat_map);
                let saved_loop_started = self.loop_started; // Preserve timer from TUI init
                let saved_max_iterations = self.max_iterations;
                // Preserve iteration buffers so TUI history survives across task restarts
                let saved_iterations = std::mem::take(&mut self.iterations);
                let saved_current_view = self.current_view;
                let saved_following_latest = self.following_latest;
                let saved_new_iteration_alert = self.new_iteration_alert.take();
                let saved_pending_backend = self.pending_backend.clone();
                *self = Self::new();
                self.hat_map = saved_hat_map;
                self.loop_started = saved_loop_started; // Keep original timer
                self.max_iterations = saved_max_iterations;
                self.iterations = saved_iterations;
                self.current_view = saved_current_view;
                self.following_latest = saved_following_latest;
                self.new_iteration_alert = saved_new_iteration_alert;
                self.pending_backend = saved_pending_backend;
                if let Some((hat_id, hat_display)) = custom_hat.clone() {
                    self.pending_hat = Some((hat_id, hat_display));
                } else {
                    self.pending_hat = Some((HatId::new("planner"), "📋Planner".to_string()));
                }
                self.last_event = Some(topic.to_string());
                self.last_event_at = Some(now);
            }
            "task.resume" => {
                // Don't reset timer on resume - keep counting from TUI init
                if custom_hat.is_none() {
                    self.pending_hat = Some((HatId::new("planner"), "📋Planner".to_string()));
                }
            }
            "build.task" => {
                if custom_hat.is_none() {
                    self.pending_hat = Some((HatId::new("builder"), "🔨Builder".to_string()));
                }
                // Resume the loop timer if a new iteration starts after a freeze.
                self.final_loop_elapsed = None;
                self.iteration_started = Some(now);
            }
            "build.done" => {
                if custom_hat.is_none() {
                    self.pending_hat = Some((HatId::new("planner"), "📋Planner".to_string()));
                }
                self.prev_iteration = self.iteration;
                self.iteration += 1;
                self.finish_latest_iteration();
                self.freeze_loop_elapsed();
            }
            "build.blocked" => {
                if custom_hat.is_none() {
                    self.pending_hat = Some((HatId::new("planner"), "📋Planner".to_string()));
                }
                self.finish_latest_iteration();
                self.freeze_loop_elapsed();
            }
            "loop.terminate" => {
                self.pending_hat = None;
                self.loop_completed = true;
                // Freeze the iteration timer at its current value
                self.final_iteration_elapsed = self.iteration_started.map(|start| start.elapsed());
                // Freeze the total loop timer for the footer display
                self.freeze_loop_elapsed();
                self.finish_latest_iteration();
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
        if let Some(final_elapsed) = self.final_loop_elapsed {
            return Some(final_elapsed);
        }
        self.loop_started.map(|start| start.elapsed())
    }

    /// Time since iteration started, or frozen value if loop completed.
    pub fn get_iteration_elapsed(&self) -> Option<Duration> {
        if let Some(buffer) = self.current_iteration() {
            if let Some(elapsed) = buffer.elapsed {
                return Some(elapsed);
            }
            if let Some(started_at) = buffer.started_at {
                return Some(started_at.elapsed());
            }
        }
        if let Some(final_elapsed) = self.final_iteration_elapsed {
            return Some(final_elapsed);
        }
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

    // ========================================================================
    // Task Tracking Methods
    // ========================================================================

    /// Returns a reference to the current task counts.
    pub fn get_task_counts(&self) -> &TaskCounts {
        &self.task_counts
    }

    /// Returns a reference to the active task, if any.
    pub fn get_active_task(&self) -> Option<&TaskSummary> {
        self.active_task.as_ref()
    }

    /// Updates the task counts.
    pub fn set_task_counts(&mut self, counts: TaskCounts) {
        self.task_counts = counts;
    }

    /// Sets the active task.
    pub fn set_active_task(&mut self, task: Option<TaskSummary>) {
        self.active_task = task;
    }

    /// Returns true if there are any open tasks.
    pub fn has_open_tasks(&self) -> bool {
        self.task_counts.open > 0
    }

    /// Returns a formatted string for task progress display (e.g., "3/5 tasks").
    pub fn get_task_progress_display(&self) -> String {
        if self.task_counts.total == 0 {
            "No tasks".to_string()
        } else {
            format!(
                "{}/{} tasks",
                self.task_counts.closed, self.task_counts.total
            )
        }
    }

    // ========================================================================
    // Iteration Management Methods
    // ========================================================================

    /// Starts a new iteration, creating a new IterationBuffer.
    /// If following_latest is true, current_view is updated to the new iteration.
    /// If not following, sets the new_iteration_alert to notify the user.
    pub fn start_new_iteration(&mut self) {
        self.start_new_iteration_with_metadata(None, None);
    }

    /// Starts a new iteration with optional metadata for hat and backend display.
    pub fn start_new_iteration_with_metadata(
        &mut self,
        hat_display: Option<String>,
        backend: Option<String>,
    ) {
        let hat_display = hat_display.or_else(|| {
            self.pending_hat
                .as_ref()
                .map(|(_, display)| display.clone())
        });
        let backend = backend.or_else(|| self.pending_backend.clone());
        let number = (self.iterations.len() + 1) as u32;
        let mut buffer = IterationBuffer::new(number);
        buffer.hat_display = hat_display;
        buffer.backend = backend;
        buffer.started_at = Some(Instant::now());
        if buffer.backend.is_some() {
            self.pending_backend = buffer.backend.clone();
        }
        self.iterations.push(buffer);

        // Auto-follow if enabled
        if self.following_latest {
            self.current_view = self.iterations.len().saturating_sub(1);
        } else {
            // Alert user about new iteration when reviewing history
            self.new_iteration_alert = Some(number as usize);
        }
    }

    /// Finalizes the latest iteration's elapsed time if it isn't already set.
    pub fn finish_latest_iteration(&mut self) {
        let Some(buffer) = self.iterations.last_mut() else {
            return;
        };
        if buffer.elapsed.is_some() {
            return;
        }
        if let Some(started_at) = buffer.started_at {
            buffer.elapsed = Some(started_at.elapsed());
        }
    }

    /// Freeze total loop elapsed time for the footer if it is still ticking.
    fn freeze_loop_elapsed(&mut self) {
        if self.final_loop_elapsed.is_some() {
            return;
        }
        self.final_loop_elapsed = self.loop_started.map(|start| start.elapsed());
    }

    /// Returns the hat display for the currently viewed iteration, if available.
    pub fn current_iteration_hat_display(&self) -> Option<&str> {
        self.current_iteration()
            .and_then(|buffer| buffer.hat_display.as_deref())
    }

    /// Returns the backend display for the currently viewed iteration, if available.
    pub fn current_iteration_backend(&self) -> Option<&str> {
        self.current_iteration()
            .and_then(|buffer| buffer.backend.as_deref())
    }

    /// Returns a reference to the currently viewed iteration buffer.
    pub fn current_iteration(&self) -> Option<&IterationBuffer> {
        self.iterations.get(self.current_view)
    }

    /// Returns a mutable reference to the currently viewed iteration buffer.
    pub fn current_iteration_mut(&mut self) -> Option<&mut IterationBuffer> {
        self.iterations.get_mut(self.current_view)
    }

    /// Returns a shared handle to the current iteration's lines buffer.
    ///
    /// This allows stream handlers to write directly to the buffer,
    /// enabling real-time streaming to the TUI during execution.
    pub fn current_iteration_lines_handle(
        &self,
    ) -> Option<std::sync::Arc<std::sync::Mutex<Vec<Line<'static>>>>> {
        self.iterations
            .get(self.current_view)
            .map(|buffer| buffer.lines_handle())
    }

    /// Returns a shared handle to the latest iteration's lines buffer.
    ///
    /// This should be used when writing output from the currently executing
    /// iteration, regardless of which iteration the user is viewing.
    /// This prevents output from being written to the wrong iteration when
    /// the user is reviewing an older iteration.
    pub fn latest_iteration_lines_handle(
        &self,
    ) -> Option<std::sync::Arc<std::sync::Mutex<Vec<Line<'static>>>>> {
        self.iterations.last().map(|buffer| buffer.lines_handle())
    }

    /// Navigates to the next iteration (if not at the last one).
    /// If reaching the last iteration, re-enables following_latest and clears alerts.
    pub fn navigate_next(&mut self) {
        if self.iterations.is_empty() {
            return;
        }
        let max_index = self.iterations.len().saturating_sub(1);
        if self.current_view < max_index {
            self.current_view += 1;
            // Re-enable following when reaching the latest
            if self.current_view == max_index {
                self.following_latest = true;
                self.new_iteration_alert = None;
            }
        }
    }

    /// Navigates to the previous iteration (if not at the first one).
    /// Disables following_latest when navigating backwards.
    pub fn navigate_prev(&mut self) {
        if self.current_view > 0 {
            self.current_view -= 1;
            self.following_latest = false;
        }
    }

    /// Returns the total number of iterations.
    pub fn total_iterations(&self) -> usize {
        self.iterations.len()
    }

    // ========================================================================
    // Search Methods
    // ========================================================================

    /// Searches for the given query in the current iteration's content.
    /// Populates matches with (line_index, char_offset) pairs.
    /// Search is case-insensitive.
    pub fn search(&mut self, query: &str) {
        self.search_state.query = Some(query.to_string());
        self.search_state.matches.clear();
        self.search_state.current_match = 0;

        // Check if we have an iteration to search
        if self.iterations.get(self.current_view).is_none() {
            return;
        }

        let query_lower = query.to_lowercase();

        // Collect matches first (avoid borrow conflicts)
        let matches: Vec<(usize, usize)> = self
            .iterations
            .get(self.current_view)
            .and_then(|buffer| {
                let lines = buffer.lines.lock().ok()?;
                let mut found = Vec::new();
                for (line_idx, line) in lines.iter().enumerate() {
                    // Get the text content of the line
                    let line_text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
                    let line_lower = line_text.to_lowercase();

                    // Find all occurrences in this line
                    let mut search_start = 0;
                    while let Some(pos) = line_lower[search_start..].find(&query_lower) {
                        let char_offset = search_start + pos;
                        found.push((line_idx, char_offset));
                        search_start = char_offset + query_lower.len();
                    }
                }
                Some(found)
            })
            .unwrap_or_default();

        self.search_state.matches = matches;

        // Jump to first match if any exist
        if !self.search_state.matches.is_empty() {
            self.jump_to_current_match();
        }
    }

    /// Navigates to the next match, cycling back to the first if at the end.
    pub fn next_match(&mut self) {
        if self.search_state.matches.is_empty() {
            return;
        }

        self.search_state.current_match =
            (self.search_state.current_match + 1) % self.search_state.matches.len();
        self.jump_to_current_match();
    }

    /// Navigates to the previous match, cycling to the last if at the beginning.
    pub fn prev_match(&mut self) {
        if self.search_state.matches.is_empty() {
            return;
        }

        if self.search_state.current_match == 0 {
            self.search_state.current_match = self.search_state.matches.len() - 1;
        } else {
            self.search_state.current_match -= 1;
        }
        self.jump_to_current_match();
    }

    /// Clears the search state.
    pub fn clear_search(&mut self) {
        self.search_state.clear();
    }

    /// Jumps to the current match by adjusting scroll_offset to show the match line.
    fn jump_to_current_match(&mut self) {
        if self.search_state.matches.is_empty() {
            return;
        }

        let (line_idx, _) = self.search_state.matches[self.search_state.current_match];

        // Adjust scroll to show the match line
        // Use a default viewport height for calculation (will be overridden by actual render)
        let viewport_height = 20;
        if let Some(buffer) = self.current_iteration_mut() {
            // If the match line is above the current view, scroll up to it
            if line_idx < buffer.scroll_offset {
                buffer.scroll_offset = line_idx;
            }
            // If the match line is below the current view, scroll down to show it
            else if line_idx >= buffer.scroll_offset + viewport_height {
                buffer.scroll_offset = line_idx.saturating_sub(viewport_height / 2);
            }
        }
    }
}

impl Default for TuiState {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// IterationBuffer - Content storage for a single iteration
// ============================================================================

use ratatui::text::Line;
use std::sync::{Arc, Mutex};

/// Stores formatted output content for a single Ralph iteration.
/// Each iteration has its own buffer with independent scroll state.
///
/// The `lines` field is wrapped in `Arc<Mutex<>>` to allow sharing
/// with stream handlers during execution, enabling real-time streaming
/// to the TUI instead of batch transfer after execution completes.
pub struct IterationBuffer {
    /// Iteration number (1-indexed for display)
    pub number: u32,
    /// Formatted lines of output (shared for streaming)
    pub lines: Arc<Mutex<Vec<Line<'static>>>>,
    /// Scroll position within this buffer
    pub scroll_offset: usize,
    /// Whether to auto-scroll to bottom as new content arrives.
    /// Starts true, becomes false when user scrolls up, restored when user
    /// scrolls to bottom (G key) or manually scrolls down to reach bottom.
    pub following_bottom: bool,
    /// Hat display name (emoji + name) for this iteration.
    pub hat_display: Option<String>,
    /// Backend used for this iteration (e.g., "claude", "kiro").
    pub backend: Option<String>,
    /// When this iteration started (for elapsed time calculation).
    pub started_at: Option<Instant>,
    /// Frozen elapsed duration for this iteration (set when completed).
    pub elapsed: Option<Duration>,
}

impl IterationBuffer {
    /// Creates a new buffer for the given iteration number.
    pub fn new(number: u32) -> Self {
        Self {
            number,
            lines: Arc::new(Mutex::new(Vec::new())),
            scroll_offset: 0,
            following_bottom: true, // Start following bottom for auto-scroll
            hat_display: None,
            backend: None,
            started_at: None,
            elapsed: None,
        }
    }

    /// Returns a shared handle to the lines buffer for streaming.
    ///
    /// This allows stream handlers to write directly to the buffer,
    /// enabling real-time streaming to the TUI.
    pub fn lines_handle(&self) -> Arc<Mutex<Vec<Line<'static>>>> {
        Arc::clone(&self.lines)
    }

    /// Appends a line to the buffer.
    pub fn append_line(&mut self, line: Line<'static>) {
        if let Ok(mut lines) = self.lines.lock() {
            lines.push(line);
        }
    }

    /// Returns the total number of lines in the buffer.
    pub fn line_count(&self) -> usize {
        self.lines.lock().map(|l| l.len()).unwrap_or(0)
    }

    /// Returns a clone of the visible lines based on scroll offset and viewport height.
    ///
    /// Note: Returns owned Vec instead of slice due to interior mutability.
    pub fn visible_lines(&self, viewport_height: usize) -> Vec<Line<'static>> {
        let Ok(lines) = self.lines.lock() else {
            return Vec::new();
        };
        if lines.is_empty() {
            return Vec::new();
        }
        let start = self.scroll_offset.min(lines.len());
        let end = (start + viewport_height).min(lines.len());
        lines[start..end].to_vec()
    }

    /// Scrolls up by one line.
    /// Disables auto-scroll since user is moving away from bottom.
    pub fn scroll_up(&mut self) {
        self.scroll_offset = self.scroll_offset.saturating_sub(1);
        self.following_bottom = false;
    }

    /// Scrolls down by one line, respecting the viewport bounds.
    /// Re-enables auto-scroll if user reaches the bottom.
    pub fn scroll_down(&mut self, viewport_height: usize) {
        let max_scroll = self.max_scroll_offset(viewport_height);
        if self.scroll_offset < max_scroll {
            self.scroll_offset += 1;
        }
        // Re-enable following if user scrolled to or past the bottom
        if self.scroll_offset >= max_scroll {
            self.following_bottom = true;
        }
    }

    /// Scrolls to the top of the buffer.
    /// Disables auto-scroll since user is moving away from bottom.
    pub fn scroll_top(&mut self) {
        self.scroll_offset = 0;
        self.following_bottom = false;
    }

    /// Scrolls to the bottom of the buffer.
    /// Re-enables auto-scroll since user explicitly went to bottom.
    pub fn scroll_bottom(&mut self, viewport_height: usize) {
        self.scroll_offset = self.max_scroll_offset(viewport_height);
        self.following_bottom = true;
    }

    /// Calculates the maximum scroll offset for the given viewport height.
    fn max_scroll_offset(&self, viewport_height: usize) -> usize {
        self.lines
            .lock()
            .map(|l| l.len().saturating_sub(viewport_height))
            .unwrap_or(0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // IterationBuffer Tests
    // ========================================================================

    mod iteration_buffer {
        use super::*;
        use ratatui::text::Line;

        #[test]
        fn new_creates_buffer_with_correct_initial_state() {
            let buffer = IterationBuffer::new(1);
            assert_eq!(buffer.number, 1);
            assert_eq!(buffer.line_count(), 0);
            assert_eq!(buffer.scroll_offset, 0);
        }

        #[test]
        fn append_line_adds_lines_in_order() {
            let mut buffer = IterationBuffer::new(1);
            buffer.append_line(Line::from("first"));
            buffer.append_line(Line::from("second"));
            buffer.append_line(Line::from("third"));

            assert_eq!(buffer.line_count(), 3);
            // Verify order by checking raw content
            let lines = buffer.lines.lock().unwrap();
            assert_eq!(lines[0].spans[0].content, "first");
            assert_eq!(lines[1].spans[0].content, "second");
            assert_eq!(lines[2].spans[0].content, "third");
        }

        #[test]
        fn line_count_returns_correct_count() {
            let mut buffer = IterationBuffer::new(1);
            assert_eq!(buffer.line_count(), 0);

            for i in 0..10 {
                buffer.append_line(Line::from(format!("line {}", i)));
            }
            assert_eq!(buffer.line_count(), 10);
        }

        #[test]
        fn visible_lines_returns_correct_slice_without_scroll() {
            let mut buffer = IterationBuffer::new(1);
            for i in 0..10 {
                buffer.append_line(Line::from(format!("line {}", i)));
            }

            let visible = buffer.visible_lines(5);
            assert_eq!(visible.len(), 5);
            // Should be lines 0-4
            assert_eq!(visible[0].spans[0].content, "line 0");
            assert_eq!(visible[4].spans[0].content, "line 4");
        }

        #[test]
        fn visible_lines_returns_correct_slice_with_scroll() {
            let mut buffer = IterationBuffer::new(1);
            for i in 0..10 {
                buffer.append_line(Line::from(format!("line {}", i)));
            }
            buffer.scroll_offset = 3;

            let visible = buffer.visible_lines(5);
            assert_eq!(visible.len(), 5);
            // Should be lines 3-7
            assert_eq!(visible[0].spans[0].content, "line 3");
            assert_eq!(visible[4].spans[0].content, "line 7");
        }

        #[test]
        fn visible_lines_handles_viewport_larger_than_content() {
            let mut buffer = IterationBuffer::new(1);
            for i in 0..3 {
                buffer.append_line(Line::from(format!("line {}", i)));
            }

            let visible = buffer.visible_lines(10);
            assert_eq!(visible.len(), 3); // Only 3 lines exist
        }

        #[test]
        fn visible_lines_handles_empty_buffer() {
            let buffer = IterationBuffer::new(1);
            let visible = buffer.visible_lines(5);
            assert!(visible.is_empty());
        }

        #[test]
        fn scroll_down_increases_offset() {
            let mut buffer = IterationBuffer::new(1);
            for i in 0..10 {
                buffer.append_line(Line::from(format!("line {}", i)));
            }

            assert_eq!(buffer.scroll_offset, 0);
            buffer.scroll_down(5); // viewport height 5
            assert_eq!(buffer.scroll_offset, 1);
            buffer.scroll_down(5);
            assert_eq!(buffer.scroll_offset, 2);
        }

        #[test]
        fn scroll_up_decreases_offset() {
            let mut buffer = IterationBuffer::new(1);
            for _ in 0..10 {
                buffer.append_line(Line::from("line"));
            }
            buffer.scroll_offset = 5;

            buffer.scroll_up();
            assert_eq!(buffer.scroll_offset, 4);
            buffer.scroll_up();
            assert_eq!(buffer.scroll_offset, 3);
        }

        #[test]
        fn scroll_up_does_not_underflow() {
            let mut buffer = IterationBuffer::new(1);
            buffer.append_line(Line::from("line"));
            buffer.scroll_offset = 0;

            buffer.scroll_up();
            assert_eq!(buffer.scroll_offset, 0); // Should stay at 0
        }

        #[test]
        fn scroll_down_does_not_overflow() {
            let mut buffer = IterationBuffer::new(1);
            for _ in 0..10 {
                buffer.append_line(Line::from("line"));
            }
            // With 10 lines and viewport 5, max scroll is 5 (shows lines 5-9)
            buffer.scroll_offset = 5;

            buffer.scroll_down(5);
            assert_eq!(buffer.scroll_offset, 5); // Should stay at max
        }

        #[test]
        fn scroll_top_resets_to_zero() {
            let mut buffer = IterationBuffer::new(1);
            for _ in 0..10 {
                buffer.append_line(Line::from("line"));
            }
            buffer.scroll_offset = 5;

            buffer.scroll_top();
            assert_eq!(buffer.scroll_offset, 0);
        }

        #[test]
        fn scroll_bottom_sets_to_max() {
            let mut buffer = IterationBuffer::new(1);
            for _ in 0..10 {
                buffer.append_line(Line::from("line"));
            }

            buffer.scroll_bottom(5); // viewport height 5
            assert_eq!(buffer.scroll_offset, 5); // max = 10 - 5 = 5
        }

        #[test]
        fn scroll_bottom_handles_small_content() {
            let mut buffer = IterationBuffer::new(1);
            for _ in 0..3 {
                buffer.append_line(Line::from("line"));
            }

            buffer.scroll_bottom(5); // viewport larger than content
            assert_eq!(buffer.scroll_offset, 0); // Can't scroll
        }

        #[test]
        fn scroll_down_handles_empty_buffer() {
            let mut buffer = IterationBuffer::new(1);
            buffer.scroll_down(5);
            assert_eq!(buffer.scroll_offset, 0);
        }

        // =====================================================================
        // Auto-scroll (following_bottom) Tests
        // =====================================================================

        #[test]
        fn following_bottom_is_true_initially() {
            let buffer = IterationBuffer::new(1);
            assert!(
                buffer.following_bottom,
                "New buffer should start with following_bottom = true"
            );
        }

        #[test]
        fn scroll_up_disables_following_bottom() {
            let mut buffer = IterationBuffer::new(1);
            for _ in 0..10 {
                buffer.append_line(Line::from("line"));
            }
            buffer.scroll_offset = 5;
            assert!(buffer.following_bottom);

            buffer.scroll_up();

            assert!(
                !buffer.following_bottom,
                "scroll_up should disable following_bottom"
            );
        }

        #[test]
        fn scroll_top_disables_following_bottom() {
            let mut buffer = IterationBuffer::new(1);
            for _ in 0..10 {
                buffer.append_line(Line::from("line"));
            }
            assert!(buffer.following_bottom);

            buffer.scroll_top();

            assert!(
                !buffer.following_bottom,
                "scroll_top should disable following_bottom"
            );
        }

        #[test]
        fn scroll_bottom_enables_following_bottom() {
            let mut buffer = IterationBuffer::new(1);
            for _ in 0..10 {
                buffer.append_line(Line::from("line"));
            }
            buffer.following_bottom = false;

            buffer.scroll_bottom(5);

            assert!(
                buffer.following_bottom,
                "scroll_bottom should enable following_bottom"
            );
        }

        #[test]
        fn scroll_down_to_bottom_enables_following_bottom() {
            let mut buffer = IterationBuffer::new(1);
            for _ in 0..10 {
                buffer.append_line(Line::from("line"));
            }
            buffer.scroll_offset = 4; // One away from max (5 with viewport 5)
            buffer.following_bottom = false;

            buffer.scroll_down(5); // Now at max (5)

            assert!(
                buffer.following_bottom,
                "scroll_down to bottom should enable following_bottom"
            );
        }

        #[test]
        fn scroll_down_not_at_bottom_keeps_following_false() {
            let mut buffer = IterationBuffer::new(1);
            for _ in 0..10 {
                buffer.append_line(Line::from("line"));
            }
            buffer.scroll_offset = 0;
            buffer.following_bottom = false;

            buffer.scroll_down(5); // Now at 1, max is 5

            assert!(
                !buffer.following_bottom,
                "scroll_down not reaching bottom should keep following_bottom false"
            );
        }

        #[test]
        fn autoscroll_scenario_content_grows_past_viewport() {
            // This tests the core bug fix: content growing from small to large
            let mut buffer = IterationBuffer::new(1);

            // Start with small content that fits in viewport
            for _ in 0..5 {
                buffer.append_line(Line::from("line"));
            }

            // Simulate initial state: following_bottom = true, scroll_offset = 0
            let viewport = 20;
            assert!(buffer.following_bottom);
            assert_eq!(buffer.scroll_offset, 0);

            // Simulate auto-scroll logic: if following_bottom, scroll to bottom
            if buffer.following_bottom {
                let max_scroll = buffer.line_count().saturating_sub(viewport);
                buffer.scroll_offset = max_scroll;
            }
            assert_eq!(buffer.scroll_offset, 0); // max_scroll is 0 when content < viewport

            // Content grows past viewport size
            for _ in 0..25 {
                buffer.append_line(Line::from("more content"));
            }
            // Now we have 30 lines, viewport is 20, max_scroll = 10

            // The bug was: scroll_offset = 0, but old logic checked if 0 >= 10-1 (false)
            // With following_bottom flag, we just check the flag:
            if buffer.following_bottom {
                let max_scroll = buffer.line_count().saturating_sub(viewport);
                buffer.scroll_offset = max_scroll;
            }

            // Now scroll_offset should be at the bottom
            assert_eq!(
                buffer.scroll_offset, 10,
                "Auto-scroll should move to bottom when content grows past viewport"
            );
        }
    }

    // ========================================================================
    // TuiState Tests (existing)
    // ========================================================================

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

    #[test]
    fn task_start_preserves_iterations_across_reset() {
        // Regression test: task.start used to do *self = Self::new() which wiped
        // iteration buffers, causing the header to show "iter 1/0" and losing all
        // previous iteration output.
        let mut state = TuiState::new();

        // Create 3 iterations with content
        state.start_new_iteration();
        state.start_new_iteration();
        state.start_new_iteration();
        assert_eq!(state.total_iterations(), 3);
        assert_eq!(state.current_view, 2); // following latest

        // Navigate back to review history
        state.navigate_prev();
        assert_eq!(state.current_view, 1);
        assert!(!state.following_latest);

        // When task.start fires (e.g., new task planning session)
        let event = Event::new("task.start", "New task");
        state.update(&event);

        // Then iterations are preserved
        assert_eq!(
            state.total_iterations(),
            3,
            "task.start should not wipe iteration buffers"
        );
        assert_eq!(
            state.current_view, 1,
            "task.start should preserve current_view position"
        );
        assert!(
            !state.following_latest,
            "task.start should preserve following_latest state"
        );
    }

    #[test]
    fn loop_terminate_freezes_iteration_timer() {
        // Given a running iteration with elapsed time
        let mut state = TuiState::new();
        let start_event = Event::new("build.task", "");
        state.update(&start_event);

        // Verify timer is running
        assert!(state.iteration_started.is_some());
        let elapsed_before = state.get_iteration_elapsed().unwrap();
        assert!(elapsed_before.as_nanos() > 0);

        // When loop.terminate is received
        let terminate_event = Event::new("loop.terminate", "");
        state.update(&terminate_event);

        // Then the timer is frozen
        assert!(state.loop_completed);
        assert!(state.final_iteration_elapsed.is_some());

        // The elapsed time should be frozen (not increasing)
        let frozen_elapsed = state.get_iteration_elapsed().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(10));
        let elapsed_after_sleep = state.get_iteration_elapsed().unwrap();

        assert_eq!(
            frozen_elapsed, elapsed_after_sleep,
            "Timer should be frozen after loop.terminate"
        );
    }

    #[test]
    fn loop_terminate_freezes_total_timer() {
        let mut state = TuiState::new();
        state.loop_started = Some(
            std::time::Instant::now()
                .checked_sub(std::time::Duration::from_secs(90))
                .unwrap(),
        );

        let before = state.get_loop_elapsed().unwrap();
        assert!(before.as_secs() >= 90);

        let terminate_event = Event::new("loop.terminate", "");
        state.update(&terminate_event);

        let frozen = state.get_loop_elapsed().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(10));
        let after = state.get_loop_elapsed().unwrap();

        assert_eq!(
            frozen, after,
            "Loop elapsed time should be frozen after termination"
        );
    }

    #[test]
    fn build_done_freezes_total_timer() {
        let mut state = TuiState::new();
        state.loop_started = Some(
            std::time::Instant::now()
                .checked_sub(std::time::Duration::from_secs(42))
                .unwrap(),
        );

        let before = state.get_loop_elapsed().unwrap();
        assert!(before.as_secs() >= 42);

        let done_event = Event::new("build.done", "");
        state.update(&done_event);

        let frozen = state.get_loop_elapsed().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(10));
        let after = state.get_loop_elapsed().unwrap();

        assert_eq!(
            frozen, after,
            "Loop elapsed time should be frozen after build.done"
        );
    }

    #[test]
    fn build_blocked_freezes_total_timer() {
        let mut state = TuiState::new();
        state.loop_started = Some(
            std::time::Instant::now()
                .checked_sub(std::time::Duration::from_secs(7))
                .unwrap(),
        );

        let before = state.get_loop_elapsed().unwrap();
        assert!(before.as_secs() >= 7);

        let blocked_event = Event::new("build.blocked", "");
        state.update(&blocked_event);

        let frozen = state.get_loop_elapsed().unwrap();
        std::thread::sleep(std::time::Duration::from_millis(10));
        let after = state.get_loop_elapsed().unwrap();

        assert_eq!(
            frozen, after,
            "Loop elapsed time should be frozen after build.blocked"
        );
    }

    // ========================================================================
    // TuiState Iteration Management Tests
    // ========================================================================

    mod tui_state_iterations {
        use super::*;

        #[test]
        fn start_new_iteration_creates_first_buffer() {
            // Given TuiState with 0 iterations
            let mut state = TuiState::new();
            assert_eq!(state.total_iterations(), 0);

            // When start_new_iteration() is called
            state.start_new_iteration();

            // Then iterations.len() == 1 and new IterationBuffer exists
            assert_eq!(state.total_iterations(), 1);
            assert_eq!(state.iterations[0].number, 1);
        }

        #[test]
        fn start_new_iteration_creates_subsequent_buffers() {
            let mut state = TuiState::new();
            state.start_new_iteration();
            state.start_new_iteration();
            state.start_new_iteration();

            assert_eq!(state.total_iterations(), 3);
            assert_eq!(state.iterations[0].number, 1);
            assert_eq!(state.iterations[1].number, 2);
            assert_eq!(state.iterations[2].number, 3);
        }

        #[test]
        fn current_iteration_returns_correct_buffer() {
            // Given TuiState with 3 iterations and current_view = 1
            let mut state = TuiState::new();
            state.start_new_iteration();
            state.start_new_iteration();
            state.start_new_iteration();
            state.current_view = 1;

            // When current_iteration() is called
            let current = state.current_iteration();

            // Then the buffer at index 1 is returned (iteration number 2)
            assert!(current.is_some());
            assert_eq!(current.unwrap().number, 2);
        }

        #[test]
        fn current_iteration_returns_none_when_empty() {
            let state = TuiState::new();
            assert!(state.current_iteration().is_none());
        }

        #[test]
        fn current_iteration_mut_allows_modification() {
            let mut state = TuiState::new();
            state.start_new_iteration();

            // Add a line via mutable reference
            if let Some(buffer) = state.current_iteration_mut() {
                buffer.append_line(Line::from("test line"));
            }

            // Verify modification persisted
            assert_eq!(state.current_iteration().unwrap().line_count(), 1);
        }

        #[test]
        fn navigate_next_increases_current_view() {
            // Given TuiState with current_view = 1 and 3 iterations
            let mut state = TuiState::new();
            state.start_new_iteration();
            state.start_new_iteration();
            state.start_new_iteration();
            state.current_view = 1;
            state.following_latest = false;

            // When navigate_next() is called
            state.navigate_next();

            // Then current_view == 2
            assert_eq!(state.current_view, 2);
        }

        #[test]
        fn navigate_prev_decreases_current_view() {
            // Given TuiState with current_view = 2
            let mut state = TuiState::new();
            state.start_new_iteration();
            state.start_new_iteration();
            state.start_new_iteration();
            state.current_view = 2;

            // When navigate_prev() is called
            state.navigate_prev();

            // Then current_view == 1
            assert_eq!(state.current_view, 1);
        }

        #[test]
        fn navigate_next_does_not_exceed_bounds() {
            // Given TuiState with current_view = 2 and 3 iterations (max index 2)
            let mut state = TuiState::new();
            state.start_new_iteration();
            state.start_new_iteration();
            state.start_new_iteration();
            state.current_view = 2;

            // When navigate_next() is called
            state.navigate_next();

            // Then current_view stays at 2
            assert_eq!(state.current_view, 2);
        }

        #[test]
        fn navigate_prev_does_not_go_below_zero() {
            // Given TuiState with current_view = 0
            let mut state = TuiState::new();
            state.start_new_iteration();
            state.current_view = 0;

            // When navigate_prev() is called
            state.navigate_prev();

            // Then current_view stays at 0
            assert_eq!(state.current_view, 0);
        }

        #[test]
        fn following_latest_initially_true() {
            // Given new TuiState
            // When created
            let state = TuiState::new();

            // Then following_latest == true
            assert!(state.following_latest);
        }

        #[test]
        fn following_latest_becomes_false_on_back_navigation() {
            // Given TuiState with following_latest = true and current_view = 2
            let mut state = TuiState::new();
            state.start_new_iteration();
            state.start_new_iteration();
            state.start_new_iteration();
            state.current_view = 2;
            state.following_latest = true;

            // When navigate_prev() is called
            state.navigate_prev();

            // Then following_latest == false
            assert!(!state.following_latest);
        }

        #[test]
        fn following_latest_restored_at_latest() {
            // Given TuiState with following_latest = false
            let mut state = TuiState::new();
            state.start_new_iteration();
            state.start_new_iteration();
            state.start_new_iteration();
            state.current_view = 1;
            state.following_latest = false;

            // When navigate_next() reaches the last iteration
            state.navigate_next(); // 1 -> 2 (last)

            // Then following_latest == true
            assert!(state.following_latest);
        }

        #[test]
        fn total_iterations_reports_count() {
            // Given TuiState with 3 iterations
            let mut state = TuiState::new();
            state.start_new_iteration();
            state.start_new_iteration();
            state.start_new_iteration();

            // When total_iterations() is called
            // Then 3 is returned
            assert_eq!(state.total_iterations(), 3);
        }

        #[test]
        fn start_new_iteration_auto_follows_latest() {
            let mut state = TuiState::new();
            state.following_latest = true;
            state.start_new_iteration();
            state.start_new_iteration();

            // When following latest, current_view should track new iterations
            assert_eq!(state.current_view, 1); // Index of second iteration
        }

        // ========================================================================
        // Per-Iteration Scroll Independence Tests (Task 08)
        // ========================================================================

        #[test]
        fn per_iteration_scroll_independence() {
            // Given iteration 1 with scroll_offset 5 and iteration 2 with scroll_offset 0
            let mut state = TuiState::new();
            state.start_new_iteration();
            state.start_new_iteration();

            // Set different scroll offsets for each iteration
            state.iterations[0].scroll_offset = 5;
            state.iterations[1].scroll_offset = 0;

            // When switching between iterations
            state.current_view = 0;
            assert_eq!(
                state.current_iteration().unwrap().scroll_offset,
                5,
                "iteration 1 should have scroll_offset 5"
            );

            state.navigate_next();
            assert_eq!(
                state.current_iteration().unwrap().scroll_offset,
                0,
                "iteration 2 should have scroll_offset 0"
            );

            // Then each iteration's scroll_offset is preserved
            state.navigate_prev();
            assert_eq!(
                state.current_iteration().unwrap().scroll_offset,
                5,
                "iteration 1 should still have scroll_offset 5 after switching back"
            );
        }

        #[test]
        fn scroll_within_iteration_does_not_affect_others() {
            // Given multiple iterations with different scroll offsets
            let mut state = TuiState::new();
            state.start_new_iteration();
            state.start_new_iteration();
            state.start_new_iteration();

            // Add content to each iteration
            for i in 0..3 {
                for j in 0..20 {
                    state.iterations[i].append_line(Line::from(format!(
                        "iter {} line {}",
                        i + 1,
                        j
                    )));
                }
            }

            // Set initial scroll offsets
            state.iterations[0].scroll_offset = 3;
            state.iterations[1].scroll_offset = 7;
            state.iterations[2].scroll_offset = 10;

            // When scrolling in iteration 2
            state.current_view = 1;
            state.current_iteration_mut().unwrap().scroll_down(10);

            // Then only iteration 2's scroll changed
            assert_eq!(
                state.iterations[0].scroll_offset, 3,
                "iteration 1 unchanged"
            );
            assert_eq!(
                state.iterations[1].scroll_offset, 8,
                "iteration 2 scrolled down"
            );
            assert_eq!(
                state.iterations[2].scroll_offset, 10,
                "iteration 3 unchanged"
            );
        }

        // ========================================================================
        // New Iteration Alert Tests (Task 07)
        // ========================================================================

        #[test]
        fn new_iteration_alert_set_when_not_following() {
            // Given following_latest = false and new iteration arrives
            let mut state = TuiState::new();
            state.start_new_iteration(); // Iteration 1
            state.start_new_iteration(); // Iteration 2
            state.navigate_prev(); // Go back to iteration 1, following_latest = false

            // When start_new_iteration() is called
            state.start_new_iteration(); // Iteration 3

            // Then new_iteration_alert is set to the new iteration number
            assert_eq!(state.new_iteration_alert, Some(3));
        }

        #[test]
        fn new_iteration_alert_not_set_when_following() {
            // Given following_latest = true
            let mut state = TuiState::new();
            state.following_latest = true;
            state.start_new_iteration();

            // When start_new_iteration() is called
            state.start_new_iteration();

            // Then new_iteration_alert remains None
            assert_eq!(state.new_iteration_alert, None);
        }

        #[test]
        fn alert_cleared_when_following_restored() {
            // Given new_iteration_alert = Some(5)
            let mut state = TuiState::new();
            state.start_new_iteration();
            state.start_new_iteration();
            state.start_new_iteration();
            state.current_view = 0;
            state.following_latest = false;
            state.new_iteration_alert = Some(3);

            // When navigation restores following_latest = true
            state.navigate_next(); // 0 -> 1
            state.navigate_next(); // 1 -> 2 (last, restores following)

            // Then new_iteration_alert is cleared to None
            assert_eq!(state.new_iteration_alert, None);
        }

        #[test]
        fn alert_not_cleared_on_partial_navigation() {
            // Given new_iteration_alert = Some(3) and not at last iteration
            let mut state = TuiState::new();
            state.start_new_iteration();
            state.start_new_iteration();
            state.start_new_iteration();
            state.current_view = 0;
            state.following_latest = false;
            state.new_iteration_alert = Some(3);

            // When navigate_next() but not reaching last
            state.navigate_next(); // 0 -> 1

            // Then alert is still set (not at latest yet)
            assert_eq!(state.new_iteration_alert, Some(3));
            assert!(!state.following_latest);
        }

        #[test]
        fn alert_updates_for_multiple_new_iterations() {
            // Given not following and multiple new iterations arrive
            let mut state = TuiState::new();
            state.start_new_iteration(); // 1
            state.start_new_iteration(); // 2
            state.navigate_prev(); // Go back, stop following

            state.start_new_iteration(); // 3 arrives
            assert_eq!(state.new_iteration_alert, Some(3));

            // When another iteration arrives
            state.start_new_iteration(); // 4 arrives

            // Then alert should show the newest
            assert_eq!(state.new_iteration_alert, Some(4));
        }
    }

    // ========================================================================
    // SearchState Tests (Task 09)
    // ========================================================================

    mod search_state {
        use super::*;

        #[test]
        fn search_finds_matches_in_lines() {
            // Given current iteration with "error" in 3 lines
            let mut state = TuiState::new();
            state.start_new_iteration();
            let buffer = state.current_iteration_mut().unwrap();
            buffer.append_line(Line::from("First error occurred"));
            buffer.append_line(Line::from("Normal line"));
            buffer.append_line(Line::from("Another error here"));
            buffer.append_line(Line::from("Final error message"));

            // When search("error") is called
            state.search("error");

            // Then matches.len() >= 3
            assert!(
                state.search_state.matches.len() >= 3,
                "expected at least 3 matches, got {}",
                state.search_state.matches.len()
            );
            assert_eq!(state.search_state.query, Some("error".to_string()));
        }

        #[test]
        fn search_is_case_insensitive() {
            // Given current iteration with "Error" and "error"
            let mut state = TuiState::new();
            state.start_new_iteration();
            let buffer = state.current_iteration_mut().unwrap();
            buffer.append_line(Line::from("Error in uppercase"));
            buffer.append_line(Line::from("error in lowercase"));
            buffer.append_line(Line::from("ERROR all caps"));

            // When search("error") is called
            state.search("error");

            // Then all 3 are found
            assert_eq!(
                state.search_state.matches.len(),
                3,
                "expected 3 case-insensitive matches"
            );
        }

        #[test]
        fn next_match_cycles_forward() {
            // Given 3 matches and current_match = 2 (last)
            let mut state = TuiState::new();
            state.start_new_iteration();
            let buffer = state.current_iteration_mut().unwrap();
            buffer.append_line(Line::from("match one"));
            buffer.append_line(Line::from("match two"));
            buffer.append_line(Line::from("match three"));
            state.search("match");
            state.search_state.current_match = 2;

            // When next_match() is called
            state.next_match();

            // Then current_match becomes 0 (cycles back)
            assert_eq!(state.search_state.current_match, 0);
        }

        #[test]
        fn prev_match_cycles_backward() {
            // Given 3 matches and current_match = 0 (first)
            let mut state = TuiState::new();
            state.start_new_iteration();
            let buffer = state.current_iteration_mut().unwrap();
            buffer.append_line(Line::from("match one"));
            buffer.append_line(Line::from("match two"));
            buffer.append_line(Line::from("match three"));
            state.search("match");
            state.search_state.current_match = 0;

            // When prev_match() is called
            state.prev_match();

            // Then current_match becomes 2 (cycles back)
            assert_eq!(state.search_state.current_match, 2);
        }

        #[test]
        fn search_jumps_to_match_line() {
            // Given match at line 50
            let mut state = TuiState::new();
            state.start_new_iteration();
            let buffer = state.current_iteration_mut().unwrap();
            for i in 0..60 {
                if i == 50 {
                    buffer.append_line(Line::from("target match here"));
                } else {
                    buffer.append_line(Line::from(format!("line {}", i)));
                }
            }

            // When search finds match at line 50
            state.search("target");

            // Then scroll_offset is updated so line 50 is visible
            let buffer = state.current_iteration().unwrap();
            // With viewport of ~20, scroll should position line 50 in view
            assert!(
                buffer.scroll_offset <= 50,
                "scroll_offset {} should position line 50 in view",
                buffer.scroll_offset
            );
        }

        #[test]
        fn clear_search_resets_state() {
            // Given active search
            let mut state = TuiState::new();
            state.start_new_iteration();
            let buffer = state.current_iteration_mut().unwrap();
            buffer.append_line(Line::from("search term here"));
            state.search("term");
            assert!(state.search_state.query.is_some());

            // When clear_search() is called
            state.clear_search();

            // Then query = None, matches cleared, search_mode = false
            assert!(state.search_state.query.is_none());
            assert!(state.search_state.matches.is_empty());
            assert!(!state.search_state.search_mode);
        }

        #[test]
        fn search_with_no_matches_sets_empty() {
            // Given iteration with no matching content
            let mut state = TuiState::new();
            state.start_new_iteration();
            let buffer = state.current_iteration_mut().unwrap();
            buffer.append_line(Line::from("hello world"));

            // When searching for non-existent term
            state.search("xyz");

            // Then matches is empty but query is set
            assert_eq!(state.search_state.query, Some("xyz".to_string()));
            assert!(state.search_state.matches.is_empty());
            assert_eq!(state.search_state.current_match, 0);
        }

        #[test]
        fn search_on_empty_iteration_handles_gracefully() {
            // Given empty iteration
            let mut state = TuiState::new();
            state.start_new_iteration();

            // When searching
            state.search("anything");

            // Then no panic, empty matches
            assert!(state.search_state.matches.is_empty());
        }

        #[test]
        fn next_match_with_no_matches_does_nothing() {
            // Given no active search or empty matches
            let mut state = TuiState::new();
            state.start_new_iteration();

            // When next_match is called
            state.next_match();

            // Then no panic, current_match stays 0
            assert_eq!(state.search_state.current_match, 0);
        }

        #[test]
        fn multiple_matches_on_same_line() {
            // Given line with multiple occurrences
            let mut state = TuiState::new();
            state.start_new_iteration();
            let buffer = state.current_iteration_mut().unwrap();
            buffer.append_line(Line::from("error error error"));

            // When searching
            state.search("error");

            // Then finds all 3 matches
            assert_eq!(
                state.search_state.matches.len(),
                3,
                "should find 3 matches on same line"
            );
        }

        #[test]
        fn next_match_updates_scroll_to_show_match() {
            // Given many lines with matches spread out
            let mut state = TuiState::new();
            state.start_new_iteration();
            let buffer = state.current_iteration_mut().unwrap();
            for i in 0..100 {
                if i % 30 == 0 {
                    buffer.append_line(Line::from("findme"));
                } else {
                    buffer.append_line(Line::from(format!("line {}", i)));
                }
            }
            state.search("findme");

            // Navigate to second match (at line 30)
            state.next_match();

            // Then scroll should position line 30 in view
            let buffer = state.current_iteration().unwrap();
            // Match at line 30, scroll should be adjusted
            assert!(buffer.scroll_offset <= 30, "scroll should show line 30");
        }

        #[test]
        fn latest_iteration_lines_handle_returns_newest_iteration() {
            // Given a user viewing iteration 1 while iteration 3 is executing
            let mut state = TuiState::new();
            state.start_new_iteration(); // iteration 1
            state.start_new_iteration(); // iteration 2
            state.start_new_iteration(); // iteration 3

            // User navigates back to iteration 1
            state.current_view = 0;
            state.following_latest = false;

            // When getting line handles
            let current_handle = state.current_iteration_lines_handle();
            let latest_handle = state.latest_iteration_lines_handle();

            // Then current_iteration_lines_handle returns iteration 1's buffer
            assert!(current_handle.is_some());
            // And latest_iteration_lines_handle returns iteration 3's buffer
            assert!(latest_handle.is_some());

            // Write to latest and verify it doesn't affect current view
            {
                let latest = latest_handle.unwrap();
                latest
                    .lock()
                    .unwrap()
                    .push(Line::from("output from iteration 3"));
            }

            // Current view (iteration 1) should be empty
            let current = state.current_iteration().unwrap();
            assert_eq!(
                current.lines.lock().unwrap().len(),
                0,
                "iteration 1 should have no lines"
            );

            // Latest (iteration 3) should have the output
            let latest_buffer = state.iterations.last().unwrap();
            assert_eq!(
                latest_buffer.lines.lock().unwrap().len(),
                1,
                "iteration 3 should have the output"
            );
        }

        #[test]
        fn output_goes_to_correct_iteration_when_user_reviewing_history() {
            // This reproduces the bug: user is on page 3 of 6, but active agent writes to page 3
            let mut state = TuiState::new();

            // Create 6 iterations
            for _ in 0..6 {
                state.start_new_iteration();
            }

            // User navigates to iteration 3 (index 2)
            state.current_view = 2;
            state.following_latest = false;

            // New iteration starts (iteration 7)
            state.start_new_iteration();

            // Get handle for writing output - MUST use latest, not current
            let lines_handle = state.latest_iteration_lines_handle();

            // Write output
            {
                let handle = lines_handle.unwrap();
                handle
                    .lock()
                    .unwrap()
                    .push(Line::from("iteration 7 output"));
            }

            // Verify: iteration 3 (what user is viewing) should be unaffected
            let iteration_3 = &state.iterations[2];
            assert_eq!(
                iteration_3.lines.lock().unwrap().len(),
                0,
                "iteration 3 (being viewed) should have no output"
            );

            // Verify: iteration 7 (latest) should have the output
            let iteration_7 = state.iterations.last().unwrap();
            assert_eq!(
                iteration_7.lines.lock().unwrap().len(),
                1,
                "iteration 7 (latest) should have the output"
            );
        }
    }
}
