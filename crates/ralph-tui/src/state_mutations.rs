//! Shared TuiState mutation helpers.
//!
//! Both [`crate::rpc_source`] (subprocess RPC) and [`crate::rpc_bridge`]
//! (HTTP/WS RPC) translate incoming events into identical `TuiState` changes.
//! Rather than duplicating that logic, each module calls these helpers and
//! keeps only the input-specific parsing on its own side.

use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};

use crate::state::{TaskSummary, TuiState};

/// Marks a task as active (running) in TUI state.
///
/// Overwrites the current active task regardless of what was previously set.
pub(crate) fn apply_task_active(state: &mut TuiState, id: &str, title: &str, status: &str) {
    state.set_active_task(Some(TaskSummary::new(id, title, status)));
}

/// Marks a task as closed: increments closed count, decrements open count,
/// and clears the active task pointer if it matches `task_id`.
pub(crate) fn apply_task_close(state: &mut TuiState, task_id: &str) {
    let mut counts = state.task_counts.clone();
    counts.closed += 1;
    if counts.open > 0 {
        counts.open -= 1;
    }
    state.set_task_counts(counts);

    if state.get_active_task().is_some_and(|t| t.id == task_id) {
        state.set_active_task(None);
    }
}

/// Marks the loop as completed: sets the `loop_completed` flag, captures
/// final iteration and loop elapsed durations, and finishes the latest
/// iteration buffer.
pub(crate) fn apply_loop_completed(state: &mut TuiState) {
    state.loop_completed = true;
    state.final_iteration_elapsed = state.iteration_started.map(|start| start.elapsed());
    state.final_loop_elapsed = state.loop_started.map(|start| start.elapsed());
    state.finish_latest_iteration();
}

/// Appends a styled error line (`⚠ [code] message`) to the latest iteration buffer.
pub(crate) fn append_error_line(state: &mut TuiState, code: &str, message: &str) {
    if let Some(handle) = state.latest_iteration_lines_handle()
        && let Ok(mut lines) = handle.lock()
    {
        lines.push(Line::from(vec![
            Span::styled(
                format!("\u{26A0} [{code}] "),
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(message.to_string()),
        ]));
    }
}
