//! File watcher for Ralph diagnostic JSONL files.
//!
//! Watches `.ralph/diagnostics/` for changes and broadcasts parsed events
//! to subscribers via a broadcast channel.

use notify::{Config, RecommendedWatcher, RecursiveMode, Watcher};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use tokio::sync::broadcast;

/// Events broadcast from the file watcher.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "source")]
pub enum DiagnosticEvent {
    /// Agent output event (text, tool call, tool result, etc.)
    #[serde(rename = "agent_output")]
    AgentOutput(AgentOutputEvent),

    /// Orchestration event (iteration started, hat selected, etc.)
    #[serde(rename = "orchestration")]
    Orchestration(OrchestrationEvent),
}

/// Agent output event parsed from agent-output.jsonl.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AgentOutputEvent {
    /// Timestamp of the event (field name is "ts" in JSONL)
    #[serde(rename = "ts")]
    pub timestamp: String,
    /// Iteration number
    pub iteration: u32,
    /// Active hat
    pub hat: String,
    /// Event type
    #[serde(flatten)]
    pub content: AgentOutputContent,
}

/// Types of agent output content.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum AgentOutputContent {
    #[serde(rename = "text")]
    Text { text: String },

    #[serde(rename = "tool_call")]
    ToolCall {
        name: String,
        id: String,
        input: serde_json::Value,
    },

    #[serde(rename = "tool_result")]
    ToolResult { id: String, output: String },

    #[serde(rename = "error")]
    Error { message: String },

    #[serde(rename = "complete")]
    Complete {
        input_tokens: Option<u64>,
        output_tokens: Option<u64>,
    },
}

/// Orchestration event parsed from orchestration.jsonl.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OrchestrationEvent {
    /// Timestamp of the event
    pub timestamp: String,
    /// Iteration number
    pub iteration: u32,
    /// Active hat
    pub hat: String,
    /// Event details (nested under "event" field in JSONL)
    pub event: OrchestrationEventType,
}

/// Types of orchestration events.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum OrchestrationEventType {
    IterationStarted,
    HatSelected { hat: String, reason: String },
    EventPublished { topic: String },
    BackpressureTriggered { reason: String },
    LoopTerminated { reason: String },
    TaskAbandoned { reason: String },
}

/// Parse a single line of agent-output.jsonl.
pub fn parse_agent_output(line: &str) -> Result<AgentOutputEvent, serde_json::Error> {
    serde_json::from_str(line)
}

/// Parse a single line of orchestration.jsonl.
pub fn parse_orchestration_event(line: &str) -> Result<OrchestrationEvent, serde_json::Error> {
    serde_json::from_str(line)
}

/// Tracks file positions to avoid re-reading content.
#[derive(Debug, Default)]
pub struct FilePositionTracker {
    positions: HashMap<PathBuf, u64>,
}

impl FilePositionTracker {
    /// Creates a new position tracker.
    pub fn new() -> Self {
        Self {
            positions: HashMap::new(),
        }
    }

    /// Gets the current position for a file.
    pub fn get_position(&self, path: &Path) -> u64 {
        self.positions.get(path).copied().unwrap_or(0)
    }

    /// Updates the position for a file.
    pub fn set_position(&mut self, path: PathBuf, position: u64) {
        self.positions.insert(path, position);
    }

    /// Reads new lines from a file starting at the tracked position.
    ///
    /// Returns the new lines and updates the position.
    pub fn read_new_lines(&mut self, path: &Path) -> std::io::Result<Vec<String>> {
        let current_pos = self.get_position(path);
        let mut file = File::open(path)?;

        // Seek to tracked position
        file.seek(SeekFrom::Start(current_pos))?;

        let reader = BufReader::new(&file);
        let lines: Vec<String> = reader.lines().collect::<Result<_, _>>()?;

        // Update position to end of file
        let new_pos = file.seek(SeekFrom::End(0))?;
        self.set_position(path.to_path_buf(), new_pos);

        Ok(lines)
    }
}

/// File watcher for diagnostic JSONL files.
pub struct FileWatcher {
    #[allow(dead_code)]
    watcher: RecommendedWatcher,
    tx: broadcast::Sender<DiagnosticEvent>,
    #[allow(dead_code)]
    positions: Arc<Mutex<FilePositionTracker>>,
}

impl FileWatcher {
    /// Creates a new file watcher for the given diagnostics directory.
    ///
    /// Watches for changes to JSONL files and broadcasts parsed events.
    pub fn new(path: PathBuf) -> notify::Result<Self> {
        let (tx, _) = broadcast::channel(1024);
        let positions = Arc::new(Mutex::new(FilePositionTracker::new()));

        let tx_clone = tx.clone();
        let positions_clone = Arc::clone(&positions);
        let path_clone = path.clone();

        let mut watcher = RecommendedWatcher::new(
            move |res: notify::Result<notify::Event>| {
                if let Ok(event) = res {
                    Self::handle_event(&path_clone, &event, &tx_clone, &positions_clone);
                }
            },
            Config::default(),
        )?;

        watcher.watch(&path, RecursiveMode::Recursive)?;

        Ok(Self {
            watcher,
            tx,
            positions,
        })
    }

    /// Subscribe to diagnostic events.
    pub fn subscribe(&self) -> broadcast::Receiver<DiagnosticEvent> {
        self.tx.subscribe()
    }

    /// Handle a file system event.
    fn handle_event(
        base_path: &Path,
        event: &notify::Event,
        tx: &broadcast::Sender<DiagnosticEvent>,
        positions: &Arc<Mutex<FilePositionTracker>>,
    ) {
        use notify::EventKind;

        // Only handle modify/create events
        if !matches!(event.kind, EventKind::Modify(_) | EventKind::Create(_)) {
            return;
        }

        for path in &event.paths {
            // Only process .jsonl files
            if path.extension().is_some_and(|ext| ext == "jsonl") {
                Self::process_jsonl_file(base_path, path, tx, positions);
            }
        }
    }

    /// Process a JSONL file and broadcast new events.
    fn process_jsonl_file(
        _base_path: &Path,
        path: &Path,
        tx: &broadcast::Sender<DiagnosticEvent>,
        positions: &Arc<Mutex<FilePositionTracker>>,
    ) {
        let mut tracker = match positions.lock() {
            Ok(t) => t,
            Err(_) => return,
        };

        let lines = match tracker.read_new_lines(path) {
            Ok(lines) => lines,
            Err(_) => return,
        };

        // Determine file type from name
        let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

        for line in lines {
            if line.trim().is_empty() {
                continue;
            }

            let event = match file_name {
                "agent-output.jsonl" => parse_agent_output(&line)
                    .ok()
                    .map(DiagnosticEvent::AgentOutput),
                "orchestration.jsonl" => parse_orchestration_event(&line)
                    .ok()
                    .map(DiagnosticEvent::Orchestration),
                _ => None,
            };

            if let Some(e) = event {
                let _ = tx.send(e);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;
    use tempfile::TempDir;

    // ==================== JSONL Parsing Tests ====================

    #[test]
    fn test_parse_agent_output_text() {
        let json = r#"{"ts":"2026-01-23T10:00:00Z","iteration":1,"hat":"ralph","type":"text","text":"Hello world"}"#;
        let event = parse_agent_output(json).unwrap();

        assert_eq!(event.iteration, 1);
        assert_eq!(event.hat, "ralph");
        assert!(matches!(
            event.content,
            AgentOutputContent::Text { text } if text == "Hello world"
        ));
    }

    #[test]
    fn test_parse_agent_output_tool_call() {
        let json = r#"{"ts":"2026-01-23T10:00:00Z","iteration":2,"hat":"builder","type":"tool_call","name":"Read","id":"t1","input":{"file":"test.rs"}}"#;
        let event = parse_agent_output(json).unwrap();

        assert_eq!(event.iteration, 2);
        assert_eq!(event.hat, "builder");
        assert!(matches!(
            event.content,
            AgentOutputContent::ToolCall { name, id, .. } if name == "Read" && id == "t1"
        ));
    }

    #[test]
    fn test_parse_agent_output_tool_result() {
        let json = r#"{"ts":"2026-01-23T10:00:00Z","iteration":2,"hat":"builder","type":"tool_result","id":"t1","output":"file contents"}"#;
        let event = parse_agent_output(json).unwrap();

        assert!(matches!(
            event.content,
            AgentOutputContent::ToolResult { id, output } if id == "t1" && output == "file contents"
        ));
    }

    #[test]
    fn test_parse_agent_output_error() {
        let json = r#"{"ts":"2026-01-23T10:00:00Z","iteration":3,"hat":"validator","type":"error","message":"Test failed"}"#;
        let event = parse_agent_output(json).unwrap();

        assert!(matches!(
            event.content,
            AgentOutputContent::Error { message } if message == "Test failed"
        ));
    }

    #[test]
    fn test_parse_agent_output_complete() {
        let json = r#"{"ts":"2026-01-23T10:00:00Z","iteration":1,"hat":"ralph","type":"complete","input_tokens":1500,"output_tokens":800}"#;
        let event = parse_agent_output(json).unwrap();

        assert!(matches!(
            event.content,
            AgentOutputContent::Complete {
                input_tokens: Some(1500),
                output_tokens: Some(800)
            }
        ));
    }

    #[test]
    fn test_parse_orchestration_event_iteration_started() {
        // Uses ralph-core's actual format with nested "event" object
        let json = r#"{"timestamp":"2026-01-23T10:00:00+00:00","iteration":1,"hat":"ralph","event":{"type":"iteration_started"}}"#;
        let event = parse_orchestration_event(json).unwrap();

        assert_eq!(event.iteration, 1);
        assert_eq!(event.hat, "ralph");
        assert!(matches!(
            event.event,
            OrchestrationEventType::IterationStarted
        ));
    }

    #[test]
    fn test_parse_orchestration_event_hat_selected() {
        let json = r#"{"timestamp":"2026-01-23T10:00:00+00:00","iteration":2,"hat":"builder","event":{"type":"hat_selected","hat":"builder","reason":"tasks_ready"}}"#;
        let event = parse_orchestration_event(json).unwrap();

        assert!(matches!(
            event.event,
            OrchestrationEventType::HatSelected { hat, reason } if hat == "builder" && reason == "tasks_ready"
        ));
    }

    #[test]
    fn test_parse_orchestration_event_backpressure() {
        let json = r#"{"timestamp":"2026-01-23T10:00:00+00:00","iteration":3,"hat":"builder","event":{"type":"backpressure_triggered","reason":"tests failed"}}"#;
        let event = parse_orchestration_event(json).unwrap();

        assert!(matches!(
            event.event,
            OrchestrationEventType::BackpressureTriggered { reason } if reason == "tests failed"
        ));
    }

    // ==================== FilePositionTracker Tests ====================

    #[test]
    fn test_position_tracker_new_file() {
        let tracker = FilePositionTracker::new();
        let path = Path::new("/tmp/test.jsonl");

        assert_eq!(tracker.get_position(path), 0);
    }

    #[test]
    fn test_position_tracker_set_get() {
        let mut tracker = FilePositionTracker::new();
        let path = PathBuf::from("/tmp/test.jsonl");

        tracker.set_position(path.clone(), 100);
        assert_eq!(tracker.get_position(&path), 100);

        tracker.set_position(path.clone(), 200);
        assert_eq!(tracker.get_position(&path), 200);
    }

    #[test]
    fn test_position_tracker_read_new_lines() {
        let temp = TempDir::new().unwrap();
        let file_path = temp.path().join("test.jsonl");

        // Write initial content
        {
            let mut file = File::create(&file_path).unwrap();
            writeln!(file, "line 1").unwrap();
            writeln!(file, "line 2").unwrap();
        }

        let mut tracker = FilePositionTracker::new();

        // First read should get all lines
        let lines = tracker.read_new_lines(&file_path).unwrap();
        assert_eq!(lines, vec!["line 1", "line 2"]);

        // Second read should get nothing (no new content)
        let lines = tracker.read_new_lines(&file_path).unwrap();
        assert!(lines.is_empty());

        // Append more content
        {
            let mut file = fs::OpenOptions::new()
                .append(true)
                .open(&file_path)
                .unwrap();
            writeln!(file, "line 3").unwrap();
        }

        // Third read should get only new line
        let lines = tracker.read_new_lines(&file_path).unwrap();
        assert_eq!(lines, vec!["line 3"]);
    }

    // ==================== FileWatcher Tests ====================

    #[tokio::test]
    async fn test_file_watcher_detects_new_lines() {
        let temp = TempDir::new().unwrap();
        let diag_dir = temp
            .path()
            .join(".ralph")
            .join("diagnostics")
            .join("2026-01-23T10-00-00");
        fs::create_dir_all(&diag_dir).unwrap();

        // Create the file first (before watcher starts)
        let agent_output_path = diag_dir.join("agent-output.jsonl");
        File::create(&agent_output_path).unwrap();

        // Create watcher for the diagnostics parent directory
        let watch_path = temp.path().join(".ralph").join("diagnostics");
        let watcher = FileWatcher::new(watch_path).unwrap();
        let mut rx = watcher.subscribe();

        // Give the watcher time to start
        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

        // Now append to the file (this should trigger a modify event)
        {
            let mut file = fs::OpenOptions::new()
                .append(true)
                .open(&agent_output_path)
                .unwrap();
            writeln!(
                file,
                r#"{{"ts":"2026-01-23T10:00:00Z","iteration":1,"hat":"ralph","type":"text","text":"Hello"}}"#
            )
            .unwrap();
            file.flush().unwrap();
            file.sync_all().unwrap();
        }

        // Wait for the event with timeout
        let result = tokio::time::timeout(tokio::time::Duration::from_secs(5), rx.recv()).await;

        assert!(result.is_ok(), "Should receive event within timeout");
        let event = result.unwrap().unwrap();

        assert!(matches!(
            event,
            DiagnosticEvent::AgentOutput(AgentOutputEvent {
                iteration: 1,
                hat,
                ..
            }) if hat == "ralph"
        ));
    }

    /// Test that watcher detects files in subdirectories created AFTER watcher starts.
    /// This is the key scenario for ralph-web: the watcher starts when the server starts,
    /// then later a loop is started which creates a new session subdirectory.
    #[tokio::test]
    async fn test_file_watcher_detects_new_subdirectory() {
        let temp = TempDir::new().unwrap();

        // Create ONLY the base diagnostics directory (no session subdirectory yet)
        let watch_path = temp.path().join(".ralph").join("diagnostics");
        fs::create_dir_all(&watch_path).unwrap();

        // Start watcher on the base directory
        let watcher = FileWatcher::new(watch_path.clone()).unwrap();
        let mut rx = watcher.subscribe();

        // Give the watcher time to start
        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;

        // Now create a new session subdirectory (simulating a new loop starting)
        let session_dir = watch_path.join("2026-01-24T10-00-00");
        fs::create_dir_all(&session_dir).unwrap();

        // Create and write to a file in the new subdirectory
        let agent_output_path = session_dir.join("agent-output.jsonl");
        {
            let mut file = File::create(&agent_output_path).unwrap();
            writeln!(
                file,
                r#"{{"ts":"2026-01-24T10:00:00Z","iteration":1,"hat":"ralph","type":"text","text":"New session"}}"#
            )
            .unwrap();
            file.flush().unwrap();
            file.sync_all().unwrap();
        }

        // Wait for the event with timeout
        let result = tokio::time::timeout(tokio::time::Duration::from_secs(5), rx.recv()).await;

        assert!(result.is_ok(), "Should receive event from new subdirectory");
        let event = result.unwrap().unwrap();

        assert!(matches!(
            event,
            DiagnosticEvent::AgentOutput(AgentOutputEvent {
                iteration: 1,
                content: AgentOutputContent::Text { text },
                ..
            }) if text == "New session"
        ));
    }
}
