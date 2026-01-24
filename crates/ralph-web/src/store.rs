//! Session storage for Ralph web dashboard.
//!
//! Loads and caches session data from `.ralph/diagnostics/` directories.

use crate::models::{
    Event, HatInfo, Iteration, IterationContent, OutputLine, OutputLineType, Session, SessionId,
    SessionStatus, SessionSummary,
};
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// In-memory session store backed by filesystem.
#[derive(Debug)]
pub struct SessionStore {
    /// Cached sessions
    sessions: HashMap<SessionId, Session>,
    /// Base path for diagnostics (e.g., `.ralph/diagnostics/`)
    base_path: PathBuf,
}

impl SessionStore {
    /// Creates a new session store for the given base path.
    pub fn new(base_path: impl Into<PathBuf>) -> Self {
        Self {
            sessions: HashMap::new(),
            base_path: base_path.into(),
        }
    }

    /// Loads sessions from disk.
    ///
    /// Scans `.ralph/diagnostics/` for session directories and parses their contents.
    pub fn load_from_disk(&mut self) -> std::io::Result<()> {
        self.sessions.clear();

        if !self.base_path.exists() {
            return Ok(());
        }

        let entries = fs::read_dir(&self.base_path)?;

        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir()
                && let Some(session) = self.parse_session(&path)
            {
                self.sessions.insert(session.id.clone(), session);
            }
        }

        Ok(())
    }

    /// Lists all sessions, sorted by date (newest first).
    pub fn list(&self) -> Vec<SessionSummary> {
        let mut summaries: Vec<SessionSummary> = self
            .sessions
            .values()
            .map(|s| SessionSummary {
                id: s.id.clone(),
                started_at: s.started_at.clone(),
                iteration_count: s.iterations.len() as u32,
                status: s.status,
            })
            .collect();

        // Sort by started_at descending (newest first)
        summaries.sort_by(|a, b| b.started_at.cmp(&a.started_at));

        summaries
    }

    /// Gets a session by ID.
    ///
    /// If the session is not in the cache, attempts to load it from disk.
    pub fn get(&mut self, id: &SessionId) -> Option<&Session> {
        // Check cache first
        if self.sessions.contains_key(id) {
            return self.sessions.get(id);
        }

        // Try to load from disk if not cached
        let session_path = self.base_path.join(id);
        if session_path.is_dir()
            && let Some(session) = self.parse_session(&session_path)
        {
            self.sessions.insert(id.clone(), session);
            return self.sessions.get(id);
        }

        None
    }

    /// Gets iteration content for a specific iteration.
    pub fn get_iteration_content(
        &mut self,
        session_id: &SessionId,
        iteration_num: u32,
    ) -> Option<IterationContent> {
        // First ensure session is loaded
        let _ = self.get(session_id)?;
        let session = self.sessions.get(session_id)?;

        // Find the iteration
        let iteration = session
            .iterations
            .iter()
            .find(|i| i.number == iteration_num)?;

        // Parse agent output for this iteration
        let lines = self.parse_agent_output(&session.path, iteration_num);

        Some(IterationContent {
            lines,
            events: iteration.events.clone(),
        })
    }

    /// Parses a session from a directory.
    fn parse_session(&self, path: &Path) -> Option<Session> {
        let dir_name = path.file_name()?.to_str()?;

        // Parse directory name as timestamp (e.g., "2026-01-21T10-33-47")
        let started_at = parse_session_timestamp(dir_name)?;

        // Parse orchestration.jsonl for iterations and events
        let iterations = self.parse_orchestration(path);

        // Determine status from iterations
        let status = determine_session_status(&iterations, path);

        Some(Session {
            id: dir_name.to_string(),
            started_at,
            path: path.to_path_buf(),
            iterations,
            status,
        })
    }

    /// Parses orchestration.jsonl for iterations and events.
    fn parse_orchestration(&self, session_path: &Path) -> Vec<Iteration> {
        let orch_path = session_path.join("orchestration.jsonl");

        if !orch_path.exists() {
            return Vec::new();
        }

        let content = match fs::read_to_string(&orch_path) {
            Ok(c) => c,
            Err(_) => return Vec::new(),
        };

        let mut iterations: HashMap<u32, Iteration> = HashMap::new();

        for line in content.lines() {
            if line.is_empty() {
                continue;
            }

            if let Ok(entry) = serde_json::from_str::<OrchestrationEntry>(line) {
                let iteration = iterations
                    .entry(entry.iteration)
                    .or_insert_with(|| Iteration {
                        number: entry.iteration,
                        hat: None,
                        events: Vec::new(),
                    });

                // Update hat info if this is a hat selection event
                if let OrchestrationEventType::HatSelected { hat, .. } = &entry.event {
                    iteration.hat = Some(HatInfo {
                        id: hat.clone(),
                        display: format_hat_display(hat),
                    });
                }

                // Add event if it's a published event
                if let OrchestrationEventType::EventPublished { topic } = &entry.event {
                    iteration.events.push(Event {
                        topic: topic.clone(),
                        payload: None,
                        timestamp: entry.timestamp.clone(),
                    });
                }
            }
        }

        let mut result: Vec<Iteration> = iterations.into_values().collect();
        result.sort_by_key(|i| i.number);
        result
    }

    /// Parses agent-output.jsonl for a specific iteration.
    fn parse_agent_output(&self, session_path: &Path, iteration_num: u32) -> Vec<OutputLine> {
        let output_path = session_path.join("agent-output.jsonl");

        if !output_path.exists() {
            return Vec::new();
        }

        let content = match fs::read_to_string(&output_path) {
            Ok(c) => c,
            Err(_) => return Vec::new(),
        };

        let mut lines = Vec::new();

        for line in content.lines() {
            if line.is_empty() {
                continue;
            }

            if let Ok(entry) = serde_json::from_str::<AgentOutputEntry>(line)
                && entry.iteration == iteration_num
            {
                let (text, line_type) = match entry.content {
                    AgentOutputContent::Text { text } => (text, OutputLineType::Text),
                    AgentOutputContent::ToolCall { name, .. } => {
                        (format!("[Tool Call: {}]", name), OutputLineType::ToolCall)
                    }
                    AgentOutputContent::ToolResult { output, .. } => {
                        (output, OutputLineType::ToolResult)
                    }
                    AgentOutputContent::Error { message } => (message, OutputLineType::Error),
                    AgentOutputContent::Complete { .. } => continue,
                };

                lines.push(OutputLine {
                    text,
                    line_type,
                    timestamp: entry.ts,
                });
            }
        }

        lines
    }
}

/// Orchestration entry from orchestration.jsonl.
#[derive(Debug, Deserialize)]
struct OrchestrationEntry {
    timestamp: String,
    iteration: u32,
    #[allow(dead_code)]
    hat: String,
    event: OrchestrationEventType,
}

/// Orchestration event types.
#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum OrchestrationEventType {
    IterationStarted,
    HatSelected {
        hat: String,
        #[allow(dead_code)]
        reason: String,
    },
    EventPublished {
        topic: String,
    },
    BackpressureTriggered {
        #[allow(dead_code)]
        reason: String,
    },
    LoopTerminated {
        reason: String,
    },
    TaskAbandoned {
        #[allow(dead_code)]
        reason: String,
    },
}

/// Agent output entry from agent-output.jsonl.
#[derive(Debug, Deserialize)]
struct AgentOutputEntry {
    ts: String,
    iteration: u32,
    #[allow(dead_code)]
    hat: String,
    #[serde(flatten)]
    content: AgentOutputContent,
}

/// Agent output content types.
#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
enum AgentOutputContent {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "tool_call")]
    ToolCall {
        name: String,
        #[allow(dead_code)]
        id: String,
        #[allow(dead_code)]
        input: serde_json::Value,
    },
    #[serde(rename = "tool_result")]
    ToolResult {
        #[allow(dead_code)]
        id: String,
        output: String,
    },
    #[serde(rename = "error")]
    Error { message: String },
    #[serde(rename = "complete")]
    Complete {
        #[allow(dead_code)]
        input_tokens: Option<u64>,
        #[allow(dead_code)]
        output_tokens: Option<u64>,
    },
}

/// Parses a session directory name to an ISO 8601 timestamp.
fn parse_session_timestamp(dir_name: &str) -> Option<String> {
    // Format: 2026-01-21T10-33-47 -> 2026-01-21T10:33:47
    if dir_name.len() != 19 {
        return None;
    }

    // Validate format
    let chars: Vec<char> = dir_name.chars().collect();
    if chars[4] != '-'
        || chars[7] != '-'
        || chars[10] != 'T'
        || chars[13] != '-'
        || chars[16] != '-'
    {
        return None;
    }

    // Convert to ISO 8601
    let mut result = dir_name.to_string();
    result.replace_range(13..14, ":");
    result.replace_range(16..17, ":");

    Some(result)
}

/// Formats a hat ID into a display string with emoji.
fn format_hat_display(hat_id: &str) -> String {
    match hat_id.to_lowercase().as_str() {
        "ralph" => "Ralph".to_string(),
        "builder" => "⚙️ Builder".to_string(),
        "validator" => "✅ Validator".to_string(),
        "planner" => "📋 Planner".to_string(),
        "committer" => "📦 Committer".to_string(),
        _ => hat_id.to_string(),
    }
}

/// Determines session status from iterations and files.
fn determine_session_status(iterations: &[Iteration], session_path: &Path) -> SessionStatus {
    // Check for termination event
    let orch_path = session_path.join("orchestration.jsonl");
    if let Ok(content) = fs::read_to_string(&orch_path) {
        for line in content.lines().rev() {
            if line.is_empty() {
                continue;
            }

            if let Ok(entry) = serde_json::from_str::<OrchestrationEntry>(line)
                && let OrchestrationEventType::LoopTerminated { reason } = entry.event
            {
                return match reason.as_str() {
                    "completion_promise" | "LOOP_COMPLETE" => SessionStatus::Completed,
                    "cancelled" | "user_cancelled" => SessionStatus::Cancelled,
                    _ => SessionStatus::Failed,
                };
            }
        }
    }

    // If no termination event found, check if there are any iterations
    if iterations.is_empty() {
        SessionStatus::Running
    } else {
        // Assume running if not terminated
        SessionStatus::Running
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    fn create_test_session(dir: &Path, name: &str) -> PathBuf {
        let session_dir = dir.join(name);
        fs::create_dir_all(&session_dir).unwrap();
        session_dir
    }

    fn write_orchestration_jsonl(session_dir: &Path, entries: &[&str]) {
        let path = session_dir.join("orchestration.jsonl");
        let mut file = fs::File::create(path).unwrap();
        for entry in entries {
            writeln!(file, "{}", entry).unwrap();
        }
    }

    #[test]
    fn test_session_store_load_from_disk_empty() {
        let temp = TempDir::new().unwrap();
        let mut store = SessionStore::new(temp.path());

        store.load_from_disk().unwrap();

        assert!(store.list().is_empty());
    }

    #[test]
    fn test_session_store_load_from_disk_with_sessions() {
        let temp = TempDir::new().unwrap();

        // Create two session directories
        create_test_session(temp.path(), "2026-01-21T10-33-47");
        create_test_session(temp.path(), "2026-01-21T14-28-02");

        let mut store = SessionStore::new(temp.path());
        store.load_from_disk().unwrap();

        let sessions = store.list();
        assert_eq!(sessions.len(), 2);
    }

    #[test]
    fn test_session_store_list_returns_sorted() {
        let temp = TempDir::new().unwrap();

        // Create sessions in non-sorted order
        create_test_session(temp.path(), "2026-01-21T10-33-47");
        create_test_session(temp.path(), "2026-01-21T14-28-02");
        create_test_session(temp.path(), "2026-01-20T08-00-00");

        let mut store = SessionStore::new(temp.path());
        store.load_from_disk().unwrap();

        let sessions = store.list();
        assert_eq!(sessions.len(), 3);

        // Should be sorted newest first
        assert_eq!(sessions[0].id, "2026-01-21T14-28-02");
        assert_eq!(sessions[1].id, "2026-01-21T10-33-47");
        assert_eq!(sessions[2].id, "2026-01-20T08-00-00");
    }

    #[test]
    fn test_session_detail_includes_iterations() {
        let temp = TempDir::new().unwrap();
        let session_dir = create_test_session(temp.path(), "2026-01-21T10-33-47");

        // Write orchestration data with iterations
        write_orchestration_jsonl(
            &session_dir,
            &[
                r#"{"timestamp":"2026-01-21T10:33:47Z","iteration":1,"hat":"ralph","event":{"type":"iteration_started"}}"#,
                r#"{"timestamp":"2026-01-21T10:33:48Z","iteration":1,"hat":"ralph","event":{"type":"hat_selected","hat":"planner","reason":"build.start"}}"#,
                r#"{"timestamp":"2026-01-21T10:33:50Z","iteration":2,"hat":"planner","event":{"type":"iteration_started"}}"#,
                r#"{"timestamp":"2026-01-21T10:33:51Z","iteration":2,"hat":"planner","event":{"type":"event_published","topic":"tasks.ready"}}"#,
            ],
        );

        let mut store = SessionStore::new(temp.path());
        store.load_from_disk().unwrap();

        let session = store.get(&"2026-01-21T10-33-47".to_string()).unwrap();
        assert_eq!(session.iterations.len(), 2);
        assert_eq!(session.iterations[0].number, 1);
        assert_eq!(session.iterations[1].number, 2);

        // Check hat info was parsed
        assert_eq!(session.iterations[0].hat.as_ref().unwrap().id, "planner");

        // Check events were parsed
        assert_eq!(session.iterations[1].events.len(), 1);
        assert_eq!(session.iterations[1].events[0].topic, "tasks.ready");
    }

    #[test]
    fn test_session_store_get_unknown_returns_none() {
        let temp = TempDir::new().unwrap();
        let mut store = SessionStore::new(temp.path());
        store.load_from_disk().unwrap();

        assert!(store.get(&"nonexistent".to_string()).is_none());
    }

    #[test]
    fn test_parse_session_timestamp() {
        assert_eq!(
            parse_session_timestamp("2026-01-21T10-33-47"),
            Some("2026-01-21T10:33:47".to_string())
        );

        // Invalid formats
        assert_eq!(parse_session_timestamp("invalid"), None);
        assert_eq!(parse_session_timestamp("2026-01-21"), None);
    }

    #[test]
    fn test_session_status_completed() {
        let temp = TempDir::new().unwrap();
        let session_dir = create_test_session(temp.path(), "2026-01-21T10-33-47");

        write_orchestration_jsonl(
            &session_dir,
            &[
                r#"{"timestamp":"2026-01-21T10:33:47Z","iteration":1,"hat":"ralph","event":{"type":"iteration_started"}}"#,
                r#"{"timestamp":"2026-01-21T10:34:00Z","iteration":1,"hat":"ralph","event":{"type":"loop_terminated","reason":"completion_promise"}}"#,
            ],
        );

        let mut store = SessionStore::new(temp.path());
        store.load_from_disk().unwrap();

        let session = store.get(&"2026-01-21T10-33-47".to_string()).unwrap();
        assert_eq!(session.status, SessionStatus::Completed);
    }

    #[test]
    fn test_format_hat_display() {
        assert_eq!(format_hat_display("builder"), "⚙️ Builder");
        assert_eq!(format_hat_display("validator"), "✅ Validator");
        assert_eq!(format_hat_display("planner"), "📋 Planner");
        assert_eq!(format_hat_display("committer"), "📦 Committer");
        assert_eq!(format_hat_display("ralph"), "Ralph");
        assert_eq!(format_hat_display("custom"), "custom");
    }
}
