//! Event-sourced loop history for crash recovery and debugging.
//!
//! Each loop maintains an append-only event log in `.ralph/history.jsonl`.
//! This provides:
//! - **Crash recovery**: Resume from last known state after crash
//! - **Debugging**: Replay loop execution to understand failures
//! - **Auditing**: Complete trace of what happened and when
//! - **Source of truth**: Registry state can be derived from history

use std::fs::{File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::file_lock::FileLock;

/// Errors that can occur during history operations.
#[derive(Debug, Error)]
pub enum HistoryError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON serialization error: {0}")]
    Json(#[from] serde_json::Error),
}

/// A single event in the loop history.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HistoryEvent {
    /// Timestamp when the event occurred.
    #[serde(rename = "ts")]
    pub timestamp: DateTime<Utc>,

    /// Type of the event.
    #[serde(rename = "type")]
    pub event_type: HistoryEventType,

    /// Optional additional data.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

impl HistoryEvent {
    /// Create a new history event with current timestamp.
    pub fn new(event_type: HistoryEventType) -> Self {
        Self {
            timestamp: Utc::now(),
            event_type,
            data: None,
        }
    }

    /// Create a new history event with data.
    pub fn with_data(event_type: HistoryEventType, data: serde_json::Value) -> Self {
        Self {
            timestamp: Utc::now(),
            event_type,
            data: Some(data),
        }
    }
}

/// Types of events that can be recorded in loop history.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum HistoryEventType {
    /// Loop started with given prompt.
    LoopStarted { prompt: String },

    /// Iteration started.
    IterationStarted { iteration: u32 },

    /// An event was published during the iteration.
    EventPublished { topic: String, payload: String },

    /// Iteration completed.
    IterationCompleted { iteration: u32, success: bool },

    /// Loop completed successfully.
    LoopCompleted { reason: String },

    /// Loop was resumed from a previous state.
    LoopResumed { from_iteration: u32 },

    /// Loop was terminated (SIGTERM or similar).
    LoopTerminated { signal: String },

    /// Loop was queued for merge.
    MergeQueued,

    /// Merge-ralph started.
    MergeStarted { pid: u32 },

    /// Merge completed successfully.
    MergeCompleted { commit: String },

    /// Merge failed.
    MergeFailed { reason: String },

    /// Loop was discarded.
    LoopDiscarded { reason: String },
}

/// Loop history manager for a single loop.
///
/// Wraps an append-only JSONL file for recording loop events.
pub struct LoopHistory {
    path: PathBuf,
}

impl LoopHistory {
    /// Create a new loop history at the given path.
    pub fn new(path: impl AsRef<Path>) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
        }
    }

    /// Create a loop history from a loop context.
    pub fn from_context(context: &crate::LoopContext) -> Self {
        Self::new(context.history_path())
    }

    /// Get the path to the history file.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Append an event to the history file.
    ///
    /// This is thread-safe via file locking.
    pub fn append(&self, event: HistoryEvent) -> Result<(), HistoryError> {
        // Ensure parent directory exists
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        // Acquire exclusive lock
        let file_lock = FileLock::new(&self.path)?;
        let _lock = file_lock.exclusive()?;

        // Open file in append mode
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;

        // Serialize and write
        let json = serde_json::to_string(&event)?;
        writeln!(file, "{}", json)?;
        file.flush()?;

        Ok(())
    }

    /// Read all events from the history file.
    pub fn read_all(&self) -> Result<Vec<HistoryEvent>, HistoryError> {
        if !self.path.exists() {
            return Ok(Vec::new());
        }

        // Acquire shared lock
        let file_lock = FileLock::new(&self.path)?;
        let _lock = file_lock.shared()?;

        let file = File::open(&self.path)?;
        let reader = BufReader::new(file);

        let mut events = Vec::new();
        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }

            // Skip malformed lines (best-effort parsing)
            if let Ok(event) = serde_json::from_str::<HistoryEvent>(&line) {
                events.push(event);
            }
        }

        Ok(events)
    }

    /// Find the last completed iteration number.
    ///
    /// Returns None if no iterations have been completed.
    pub fn last_iteration(&self) -> Result<Option<u32>, HistoryError> {
        let events = self.read_all()?;

        let mut last_completed = None;

        for event in events {
            if let HistoryEventType::IterationCompleted { iteration, .. } = event.event_type {
                last_completed = Some(iteration);
            }
        }

        Ok(last_completed)
    }

    /// Check if the loop completed successfully.
    pub fn is_completed(&self) -> Result<bool, HistoryError> {
        let events = self.read_all()?;

        for event in events.iter().rev() {
            match &event.event_type {
                HistoryEventType::LoopCompleted { .. } => return Ok(true),
                HistoryEventType::LoopTerminated { .. } => return Ok(false),
                HistoryEventType::LoopDiscarded { .. } => return Ok(false),
                _ => {}
            }
        }

        Ok(false)
    }

    /// Get the original prompt that started the loop.
    pub fn get_prompt(&self) -> Result<Option<String>, HistoryError> {
        let events = self.read_all()?;

        for event in events {
            if let HistoryEventType::LoopStarted { prompt } = event.event_type {
                return Ok(Some(prompt));
            }
        }

        Ok(None)
    }

    /// Get summary statistics about the loop.
    pub fn summary(&self) -> Result<HistorySummary, HistoryError> {
        let events = self.read_all()?;

        let mut summary = HistorySummary::default();

        for event in &events {
            match &event.event_type {
                HistoryEventType::LoopStarted { prompt } => {
                    summary.prompt = Some(prompt.clone());
                    summary.started_at = Some(event.timestamp);
                }
                HistoryEventType::IterationCompleted { iteration, success } => {
                    summary.iterations_completed = *iteration;
                    if !success {
                        summary.iterations_failed += 1;
                    }
                }
                HistoryEventType::EventPublished { .. } => {
                    summary.events_published += 1;
                }
                HistoryEventType::LoopCompleted { reason } => {
                    summary.completed = true;
                    summary.completion_reason = Some(reason.clone());
                    summary.ended_at = Some(event.timestamp);
                }
                HistoryEventType::LoopTerminated { signal } => {
                    summary.terminated = true;
                    summary.termination_signal = Some(signal.clone());
                    summary.ended_at = Some(event.timestamp);
                }
                HistoryEventType::MergeCompleted { commit } => {
                    summary.merge_commit = Some(commit.clone());
                }
                HistoryEventType::MergeFailed { reason } => {
                    summary.merge_failed = true;
                    summary.merge_failure_reason = Some(reason.clone());
                }
                _ => {}
            }
        }

        Ok(summary)
    }

    /// Record loop started event.
    pub fn record_started(&self, prompt: &str) -> Result<(), HistoryError> {
        self.append(HistoryEvent::new(HistoryEventType::LoopStarted {
            prompt: prompt.to_string(),
        }))
    }

    /// Record iteration started event.
    pub fn record_iteration_started(&self, iteration: u32) -> Result<(), HistoryError> {
        self.append(HistoryEvent::new(HistoryEventType::IterationStarted {
            iteration,
        }))
    }

    /// Record event published event.
    pub fn record_event_published(&self, topic: &str, payload: &str) -> Result<(), HistoryError> {
        self.append(HistoryEvent::new(HistoryEventType::EventPublished {
            topic: topic.to_string(),
            payload: payload.to_string(),
        }))
    }

    /// Record iteration completed event.
    pub fn record_iteration_completed(
        &self,
        iteration: u32,
        success: bool,
    ) -> Result<(), HistoryError> {
        self.append(HistoryEvent::new(HistoryEventType::IterationCompleted {
            iteration,
            success,
        }))
    }

    /// Record loop completed event.
    pub fn record_completed(&self, reason: &str) -> Result<(), HistoryError> {
        self.append(HistoryEvent::new(HistoryEventType::LoopCompleted {
            reason: reason.to_string(),
        }))
    }

    /// Record loop resumed event.
    pub fn record_resumed(&self, from_iteration: u32) -> Result<(), HistoryError> {
        self.append(HistoryEvent::new(HistoryEventType::LoopResumed {
            from_iteration,
        }))
    }

    /// Record loop terminated event.
    pub fn record_terminated(&self, signal: &str) -> Result<(), HistoryError> {
        self.append(HistoryEvent::new(HistoryEventType::LoopTerminated {
            signal: signal.to_string(),
        }))
    }

    /// Record merge queued event.
    pub fn record_merge_queued(&self) -> Result<(), HistoryError> {
        self.append(HistoryEvent::new(HistoryEventType::MergeQueued))
    }

    /// Record merge started event.
    pub fn record_merge_started(&self, pid: u32) -> Result<(), HistoryError> {
        self.append(HistoryEvent::new(HistoryEventType::MergeStarted { pid }))
    }

    /// Record merge completed event.
    pub fn record_merge_completed(&self, commit: &str) -> Result<(), HistoryError> {
        self.append(HistoryEvent::new(HistoryEventType::MergeCompleted {
            commit: commit.to_string(),
        }))
    }

    /// Record merge failed event.
    pub fn record_merge_failed(&self, reason: &str) -> Result<(), HistoryError> {
        self.append(HistoryEvent::new(HistoryEventType::MergeFailed {
            reason: reason.to_string(),
        }))
    }

    /// Record loop discarded event.
    pub fn record_discarded(&self, reason: &str) -> Result<(), HistoryError> {
        self.append(HistoryEvent::new(HistoryEventType::LoopDiscarded {
            reason: reason.to_string(),
        }))
    }
}

/// Summary statistics for a loop history.
#[derive(Debug, Default)]
pub struct HistorySummary {
    /// Original prompt.
    pub prompt: Option<String>,

    /// When the loop started.
    pub started_at: Option<DateTime<Utc>>,

    /// When the loop ended.
    pub ended_at: Option<DateTime<Utc>>,

    /// Number of completed iterations.
    pub iterations_completed: u32,

    /// Number of failed iterations.
    pub iterations_failed: u32,

    /// Number of events published.
    pub events_published: u32,

    /// Whether the loop completed successfully.
    pub completed: bool,

    /// Completion reason (if completed).
    pub completion_reason: Option<String>,

    /// Whether the loop was terminated.
    pub terminated: bool,

    /// Termination signal (if terminated).
    pub termination_signal: Option<String>,

    /// Merge commit SHA (if merged).
    pub merge_commit: Option<String>,

    /// Whether merge failed.
    pub merge_failed: bool,

    /// Merge failure reason (if failed).
    pub merge_failure_reason: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn temp_history() -> (TempDir, LoopHistory) {
        let dir = TempDir::new().unwrap();
        let history = LoopHistory::new(dir.path().join("history.jsonl"));
        (dir, history)
    }

    #[test]
    fn test_append_and_read() {
        let (_dir, history) = temp_history();

        history.record_started("test prompt").unwrap();
        history.record_iteration_started(1).unwrap();
        history.record_iteration_completed(1, true).unwrap();
        history.record_completed("completion_promise").unwrap();

        let events = history.read_all().unwrap();
        assert_eq!(events.len(), 4);

        assert!(matches!(
            events[0].event_type,
            HistoryEventType::LoopStarted { .. }
        ));
        assert!(matches!(
            events[1].event_type,
            HistoryEventType::IterationStarted { iteration: 1 }
        ));
        assert!(matches!(
            events[2].event_type,
            HistoryEventType::IterationCompleted {
                iteration: 1,
                success: true
            }
        ));
        assert!(matches!(
            events[3].event_type,
            HistoryEventType::LoopCompleted { .. }
        ));
    }

    #[test]
    fn test_last_iteration() {
        let (_dir, history) = temp_history();

        assert_eq!(history.last_iteration().unwrap(), None);

        history.record_started("test").unwrap();
        history.record_iteration_started(1).unwrap();
        history.record_iteration_completed(1, true).unwrap();
        assert_eq!(history.last_iteration().unwrap(), Some(1));

        history.record_iteration_started(2).unwrap();
        history.record_iteration_completed(2, true).unwrap();
        assert_eq!(history.last_iteration().unwrap(), Some(2));

        history.record_iteration_started(3).unwrap();
        history.record_iteration_completed(3, false).unwrap();
        assert_eq!(history.last_iteration().unwrap(), Some(3));
    }

    #[test]
    fn test_is_completed() {
        let (_dir, history) = temp_history();

        assert!(!history.is_completed().unwrap());

        history.record_started("test").unwrap();
        assert!(!history.is_completed().unwrap());

        history.record_completed("done").unwrap();
        assert!(history.is_completed().unwrap());
    }

    #[test]
    fn test_is_completed_terminated() {
        let (_dir, history) = temp_history();

        history.record_started("test").unwrap();
        history.record_terminated("SIGTERM").unwrap();
        assert!(!history.is_completed().unwrap());
    }

    #[test]
    fn test_get_prompt() {
        let (_dir, history) = temp_history();

        assert!(history.get_prompt().unwrap().is_none());

        history.record_started("my test prompt").unwrap();
        assert_eq!(
            history.get_prompt().unwrap(),
            Some("my test prompt".to_string())
        );
    }

    #[test]
    fn test_summary() {
        let (_dir, history) = temp_history();

        history.record_started("test prompt").unwrap();
        history.record_iteration_started(1).unwrap();
        history
            .record_event_published("build.task", "task 1")
            .unwrap();
        history.record_iteration_completed(1, true).unwrap();
        history.record_iteration_started(2).unwrap();
        history
            .record_event_published("build.done", "done")
            .unwrap();
        history.record_iteration_completed(2, true).unwrap();
        history.record_completed("completion_promise").unwrap();

        let summary = history.summary().unwrap();
        assert_eq!(summary.prompt, Some("test prompt".to_string()));
        assert_eq!(summary.iterations_completed, 2);
        assert_eq!(summary.events_published, 2);
        assert!(summary.completed);
        assert_eq!(
            summary.completion_reason,
            Some("completion_promise".to_string())
        );
    }

    #[test]
    fn test_empty_file() {
        let (_dir, history) = temp_history();

        let events = history.read_all().unwrap();
        assert!(events.is_empty());
    }

    #[test]
    fn test_merge_events() {
        let (_dir, history) = temp_history();

        history.record_merge_queued().unwrap();
        history.record_merge_started(12345).unwrap();
        history.record_merge_completed("abc123").unwrap();

        let events = history.read_all().unwrap();
        assert_eq!(events.len(), 3);

        assert!(matches!(
            events[0].event_type,
            HistoryEventType::MergeQueued
        ));
        assert!(matches!(
            events[1].event_type,
            HistoryEventType::MergeStarted { pid: 12345 }
        ));
        assert!(matches!(
            events[2].event_type,
            HistoryEventType::MergeCompleted { .. }
        ));
    }

    #[test]
    fn test_serialization_format() {
        let event = HistoryEvent::new(HistoryEventType::LoopStarted {
            prompt: "test".to_string(),
        });

        let json = serde_json::to_string(&event).unwrap();
        // Check that it contains expected fields
        assert!(json.contains("\"ts\""));
        assert!(json.contains("\"type\""));
        assert!(json.contains("\"kind\":\"loop_started\""));
        assert!(json.contains("\"prompt\":\"test\""));

        // Verify it can be deserialized back
        let parsed: HistoryEvent = serde_json::from_str(&json).unwrap();
        assert!(matches!(
            parsed.event_type,
            HistoryEventType::LoopStarted { prompt } if prompt == "test"
        ));
    }
}
