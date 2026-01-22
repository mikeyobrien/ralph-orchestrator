//! Agent output logger for diagnostic capture.

use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{BufWriter, Write};
use std::path::Path;

/// Logger for agent output events.
pub struct AgentOutputLogger {
    file: BufWriter<File>,
    iteration: u32,
    hat: String,
}

/// Single agent output entry in JSONL format.
#[derive(Debug, Serialize, Deserialize, PartialEq)]
pub struct AgentOutputEntry {
    pub ts: String,
    pub iteration: u32,
    pub hat: String,
    #[serde(flatten)]
    pub content: AgentOutputContent,
}

/// Types of agent output content.
#[derive(Debug, Serialize, Deserialize, PartialEq)]
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

impl AgentOutputLogger {
    /// Creates a new agent output logger.
    pub fn new(session_dir: &Path) -> std::io::Result<Self> {
        let file_path = session_dir.join("agent-output.jsonl");
        let file = File::create(file_path)?;

        Ok(Self {
            file: BufWriter::new(file),
            iteration: 0,
            hat: String::new(),
        })
    }

    /// Sets the current iteration and hat context.
    pub fn set_context(&mut self, iteration: u32, hat: &str) {
        self.iteration = iteration;
        self.hat = hat.to_string();
    }

    /// Logs an agent output event.
    pub fn log(&mut self, content: AgentOutputContent) -> std::io::Result<()> {
        let entry = AgentOutputEntry {
            ts: Utc::now().to_rfc3339(),
            iteration: self.iteration,
            hat: self.hat.clone(),
            content,
        };

        let json = serde_json::to_string(&entry)?;
        writeln!(self.file, "{}", json)?;
        self.file.flush()?;

        Ok(())
    }

    /// Flushes the output buffer.
    pub fn flush(&mut self) -> std::io::Result<()> {
        self.file.flush()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader};
    use tempfile::TempDir;

    #[test]
    fn test_logger_creates_file() {
        let temp = TempDir::new().unwrap();

        let _logger = AgentOutputLogger::new(temp.path()).unwrap();

        let file_path = temp.path().join("agent-output.jsonl");
        assert!(file_path.exists());
    }

    #[test]
    fn test_log_writes_valid_jsonl() {
        let temp = TempDir::new().unwrap();
        let mut logger = AgentOutputLogger::new(temp.path()).unwrap();
        logger.set_context(1, "ralph");

        logger
            .log(AgentOutputContent::Text {
                text: "Hello".to_string(),
            })
            .unwrap();

        logger
            .log(AgentOutputContent::ToolCall {
                name: "Read".to_string(),
                id: "tool_1".to_string(),
                input: serde_json::json!({"file": "test.rs"}),
            })
            .unwrap();

        // Read back and verify
        drop(logger);
        let file = File::open(temp.path().join("agent-output.jsonl")).unwrap();
        let reader = BufReader::new(file);
        let lines: Vec<String> = reader.lines().map(|l| l.unwrap()).collect();

        assert_eq!(lines.len(), 2);

        // Parse first line
        let entry1: AgentOutputEntry = serde_json::from_str(&lines[0]).unwrap();
        assert_eq!(entry1.iteration, 1);
        assert_eq!(entry1.hat, "ralph");
        assert!(matches!(entry1.content, AgentOutputContent::Text { .. }));

        // Parse second line
        let entry2: AgentOutputEntry = serde_json::from_str(&lines[1]).unwrap();
        assert_eq!(entry2.iteration, 1);
        assert_eq!(entry2.hat, "ralph");
        assert!(matches!(
            entry2.content,
            AgentOutputContent::ToolCall { .. }
        ));
    }

    #[test]
    fn test_immediate_flush() {
        let temp = TempDir::new().unwrap();
        let mut logger = AgentOutputLogger::new(temp.path()).unwrap();
        logger.set_context(1, "ralph");

        logger
            .log(AgentOutputContent::Text {
                text: "Test".to_string(),
            })
            .unwrap();

        // Don't drop logger - verify content is immediately available
        let file = File::open(temp.path().join("agent-output.jsonl")).unwrap();
        let reader = BufReader::new(file);
        let lines: Vec<String> = reader.lines().map(|l| l.unwrap()).collect();

        assert_eq!(lines.len(), 1);
    }

    #[test]
    fn test_all_content_types_serialize() {
        let temp = TempDir::new().unwrap();
        let mut logger = AgentOutputLogger::new(temp.path()).unwrap();
        logger.set_context(2, "builder");

        // Text
        logger
            .log(AgentOutputContent::Text {
                text: "Building...".to_string(),
            })
            .unwrap();

        // ToolCall
        logger
            .log(AgentOutputContent::ToolCall {
                name: "Execute".to_string(),
                id: "t1".to_string(),
                input: serde_json::json!({"cmd": "cargo test"}),
            })
            .unwrap();

        // ToolResult
        logger
            .log(AgentOutputContent::ToolResult {
                id: "t1".to_string(),
                output: "Tests passed".to_string(),
            })
            .unwrap();

        // Error
        logger
            .log(AgentOutputContent::Error {
                message: "Parse failed".to_string(),
            })
            .unwrap();

        // Complete
        logger
            .log(AgentOutputContent::Complete {
                input_tokens: Some(1500),
                output_tokens: Some(800),
            })
            .unwrap();

        drop(logger);

        // Verify all 5 lines parse correctly
        let file = File::open(temp.path().join("agent-output.jsonl")).unwrap();
        let reader = BufReader::new(file);
        let lines: Vec<String> = reader.lines().map(|l| l.unwrap()).collect();

        assert_eq!(lines.len(), 5);

        for line in lines {
            let entry: AgentOutputEntry = serde_json::from_str(&line).unwrap();
            assert_eq!(entry.iteration, 2);
            assert_eq!(entry.hat, "builder");
        }
    }
}
