//! Data models for Ralph web dashboard.
//!
//! These models represent sessions, iterations, and events parsed from
//! Ralph diagnostic files.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Unique identifier for a session (timestamp-based directory name).
pub type SessionId = String;

/// Summary of a session for list views.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SessionSummary {
    /// Session ID (directory name, e.g., "2026-01-21T10-33-47")
    pub id: SessionId,
    /// When the session started (parsed from directory name)
    pub started_at: String,
    /// Total number of iterations
    pub iteration_count: u32,
    /// Session status
    pub status: SessionStatus,
}

/// Full session detail.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Session {
    /// Session ID
    pub id: SessionId,
    /// When the session started
    pub started_at: String,
    /// Path to the session directory (internal use only, not serialized)
    #[serde(skip, default)]
    pub path: PathBuf,
    /// Iterations in this session
    pub iterations: Vec<Iteration>,
    /// Session status
    pub status: SessionStatus,
}

/// Session status.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Default)]
#[serde(rename_all = "lowercase")]
pub enum SessionStatus {
    /// Session is still running
    #[default]
    Running,
    /// Session completed successfully
    Completed,
    /// Session failed
    Failed,
    /// Session was cancelled
    Cancelled,
}

/// A single iteration within a session.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Iteration {
    /// Iteration number (1-based)
    pub number: u32,
    /// Hat active during this iteration
    pub hat: Option<HatInfo>,
    /// Events that occurred during this iteration
    pub events: Vec<Event>,
}

/// Hat information.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct HatInfo {
    /// Hat identifier
    pub id: String,
    /// Display name with emoji
    pub display: String,
}

/// An event that occurred during orchestration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Event {
    /// Event topic (e.g., "build.start", "task.complete")
    pub topic: String,
    /// Event payload
    pub payload: Option<String>,
    /// Timestamp
    pub timestamp: String,
}

/// Content of a single iteration (agent output, events).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IterationContent {
    /// Lines of agent output
    pub lines: Vec<OutputLine>,
    /// Events during this iteration
    pub events: Vec<Event>,
}

/// A single line of agent output.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutputLine {
    /// Line content
    pub text: String,
    /// Line type
    pub line_type: OutputLineType,
    /// Timestamp
    pub timestamp: String,
}

/// Type of output line.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutputLineType {
    /// Regular text output
    Text,
    /// Tool call
    ToolCall,
    /// Tool result
    ToolResult,
    /// Error
    Error,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_session_summary_serializes() {
        let summary = SessionSummary {
            id: "2026-01-21T10-33-47".to_string(),
            started_at: "2026-01-21T10:33:47".to_string(),
            iteration_count: 5,
            status: SessionStatus::Completed,
        };

        let json = serde_json::to_string(&summary).unwrap();
        let parsed: SessionSummary = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed, summary);
    }

    #[test]
    fn test_session_status_serializes_lowercase() {
        assert_eq!(
            serde_json::to_string(&SessionStatus::Running).unwrap(),
            "\"running\""
        );
        assert_eq!(
            serde_json::to_string(&SessionStatus::Completed).unwrap(),
            "\"completed\""
        );
        assert_eq!(
            serde_json::to_string(&SessionStatus::Failed).unwrap(),
            "\"failed\""
        );
        assert_eq!(
            serde_json::to_string(&SessionStatus::Cancelled).unwrap(),
            "\"cancelled\""
        );
    }

    #[test]
    fn test_iteration_serializes() {
        let iteration = Iteration {
            number: 1,
            hat: Some(HatInfo {
                id: "builder".to_string(),
                display: "⚙️ Builder".to_string(),
            }),
            events: vec![Event {
                topic: "build.start".to_string(),
                payload: None,
                timestamp: "2026-01-21T10:33:47Z".to_string(),
            }],
        };

        let json = serde_json::to_string(&iteration).unwrap();
        let parsed: Iteration = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed, iteration);
    }
}
