//! Event reader for consuming events from `.agent/events.jsonl`.

use serde::{Deserialize, Deserializer, Serialize};
use std::fs::File;
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::PathBuf;
use tracing::warn;

/// Result of parsing events from a JSONL file.
///
/// Contains both successfully parsed events and information about lines
/// that failed to parse. This supports backpressure validation by allowing
/// the caller to respond to malformed events.
#[derive(Debug, Clone, Default)]
pub struct ParseResult {
    /// Successfully parsed events.
    pub events: Vec<Event>,
    /// Lines that failed to parse.
    pub malformed: Vec<MalformedLine>,
}

/// Information about a malformed JSONL line.
///
/// Used for backpressure feedback - when agents write invalid JSONL,
/// this provides details for the `event.malformed` system event.
#[derive(Debug, Clone, Serialize)]
pub struct MalformedLine {
    /// Line number in the file (1-indexed).
    pub line_number: u64,
    /// The raw content that failed to parse (truncated if very long).
    pub content: String,
    /// The parse error message.
    pub error: String,
}

impl MalformedLine {
    /// Maximum content length before truncation.
    const MAX_CONTENT_LEN: usize = 100;

    /// Creates a new MalformedLine, truncating content if needed.
    pub fn new(line_number: u64, content: &str, error: String) -> Self {
        let content = if content.len() > Self::MAX_CONTENT_LEN {
            format!("{}...", &content[..Self::MAX_CONTENT_LEN])
        } else {
            content.to_string()
        };
        Self {
            line_number,
            content,
            error,
        }
    }
}

/// Custom deserializer that accepts both String and structured JSON payloads.
///
/// Agents sometimes write structured data as JSON objects instead of strings.
/// This deserializer accepts both formats:
/// - `"payload": "string"` → `Some("string")`
/// - `"payload": {...}` → `Some("{...}")` (serialized to JSON string)
/// - `"payload": null` → `None`
/// - missing field → `None`
fn deserialize_flexible_payload<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum FlexiblePayload {
        String(String),
        Object(serde_json::Value),
    }

    let opt = Option::<FlexiblePayload>::deserialize(deserializer)?;
    Ok(opt.map(|flex| match flex {
        FlexiblePayload::String(s) => s,
        FlexiblePayload::Object(obj) => {
            // Serialize the object back to a JSON string
            serde_json::to_string(&obj).unwrap_or_else(|_| obj.to_string())
        }
    }))
}

/// A simplified event for reading from JSONL.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Event {
    pub topic: String,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        deserialize_with = "deserialize_flexible_payload"
    )]
    pub payload: Option<String>,
    pub ts: String,
}

/// Reads new events from `.agent/events.jsonl` since last read.
pub struct EventReader {
    path: PathBuf,
    position: u64,
}

impl EventReader {
    /// Creates a new event reader for the given path.
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            position: 0,
        }
    }

    /// Reads new events since the last read.
    ///
    /// Returns a `ParseResult` containing both successfully parsed events
    /// and information about malformed lines. This enables backpressure
    /// validation - the caller can emit `event.malformed` events and
    /// track consecutive failures.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be opened or read.
    pub fn read_new_events(&mut self) -> std::io::Result<ParseResult> {
        if !self.path.exists() {
            return Ok(ParseResult::default());
        }

        let mut file = File::open(&self.path)?;
        file.seek(SeekFrom::Start(self.position))?;

        let reader = BufReader::new(file);
        let mut result = ParseResult::default();
        let mut current_pos = self.position;
        let mut line_number = self.count_lines_before_position();

        for line in reader.lines() {
            let line = line?;
            let line_bytes = line.len() as u64 + 1; // +1 for newline
            line_number += 1;

            if line.trim().is_empty() {
                current_pos += line_bytes;
                continue;
            }

            match serde_json::from_str::<Event>(&line) {
                Ok(event) => result.events.push(event),
                Err(e) => {
                    warn!(error = %e, line_number = line_number, "Malformed JSON line");
                    result
                        .malformed
                        .push(MalformedLine::new(line_number, &line, e.to_string()));
                }
            }

            current_pos += line_bytes;
        }

        self.position = current_pos;
        Ok(result)
    }

    /// Counts lines before the current position (for line numbering).
    fn count_lines_before_position(&self) -> u64 {
        if self.position == 0 || !self.path.exists() {
            return 0;
        }
        // Read file up to position and count newlines
        if let Ok(file) = File::open(&self.path) {
            let reader = BufReader::new(file);
            let mut count = 0u64;
            let mut bytes_read = 0u64;
            for line in reader.lines() {
                if let Ok(line) = line {
                    bytes_read += line.len() as u64 + 1;
                    if bytes_read > self.position {
                        break;
                    }
                    count += 1;
                } else {
                    break;
                }
            }
            count
        } else {
            0
        }
    }

    /// Returns the current file position.
    pub fn position(&self) -> u64 {
        self.position
    }

    /// Resets the position to the start of the file.
    pub fn reset(&mut self) {
        self.position = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_read_new_events() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(
            file,
            r#"{{"topic":"test","payload":"hello","ts":"2024-01-01T00:00:00Z"}}"#
        )
        .unwrap();
        writeln!(file, r#"{{"topic":"test2","ts":"2024-01-01T00:00:01Z"}}"#).unwrap();
        file.flush().unwrap();

        let mut reader = EventReader::new(file.path());
        let result = reader.read_new_events().unwrap();

        assert_eq!(result.events.len(), 2);
        assert_eq!(result.events[0].topic, "test");
        assert_eq!(result.events[0].payload, Some("hello".to_string()));
        assert_eq!(result.events[1].topic, "test2");
        assert_eq!(result.events[1].payload, None);
        assert!(result.malformed.is_empty());
    }

    #[test]
    fn test_tracks_position() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, r#"{{"topic":"first","ts":"2024-01-01T00:00:00Z"}}"#).unwrap();
        file.flush().unwrap();

        let mut reader = EventReader::new(file.path());
        let result = reader.read_new_events().unwrap();
        assert_eq!(result.events.len(), 1);

        // Add more events
        writeln!(file, r#"{{"topic":"second","ts":"2024-01-01T00:00:01Z"}}"#).unwrap();
        file.flush().unwrap();

        // Should only read new events
        let result = reader.read_new_events().unwrap();
        assert_eq!(result.events.len(), 1);
        assert_eq!(result.events[0].topic, "second");
    }

    #[test]
    fn test_missing_file() {
        let mut reader = EventReader::new("/nonexistent/path.jsonl");
        let result = reader.read_new_events().unwrap();
        assert!(result.events.is_empty());
        assert!(result.malformed.is_empty());
    }

    #[test]
    fn test_captures_malformed_lines() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, r#"{{"topic":"good","ts":"2024-01-01T00:00:00Z"}}"#).unwrap();
        writeln!(file, r"{{corrupt json}}").unwrap();
        writeln!(
            file,
            r#"{{"topic":"also_good","ts":"2024-01-01T00:00:01Z"}}"#
        )
        .unwrap();
        file.flush().unwrap();

        let mut reader = EventReader::new(file.path());
        let result = reader.read_new_events().unwrap();

        // Good events should be parsed
        assert_eq!(result.events.len(), 2);
        assert_eq!(result.events[0].topic, "good");
        assert_eq!(result.events[1].topic, "also_good");

        // Malformed line should be captured
        assert_eq!(result.malformed.len(), 1);
        assert_eq!(result.malformed[0].line_number, 2);
        assert!(result.malformed[0].content.contains("corrupt json"));
        assert!(!result.malformed[0].error.is_empty());
    }

    #[test]
    fn test_empty_file() {
        let file = NamedTempFile::new().unwrap();
        let mut reader = EventReader::new(file.path());
        let result = reader.read_new_events().unwrap();
        assert!(result.events.is_empty());
        assert!(result.malformed.is_empty());
    }

    #[test]
    fn test_reset_position() {
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, r#"{{"topic":"test","ts":"2024-01-01T00:00:00Z"}}"#).unwrap();
        file.flush().unwrap();

        let mut reader = EventReader::new(file.path());
        reader.read_new_events().unwrap();
        assert!(reader.position() > 0);

        reader.reset();
        assert_eq!(reader.position(), 0);

        let result = reader.read_new_events().unwrap();
        assert_eq!(result.events.len(), 1);
    }

    #[test]
    fn test_structured_payload_as_object() {
        // Test that JSON objects in payload field are converted to strings
        let mut file = NamedTempFile::new().unwrap();
        writeln!(
            file,
            r#"{{"topic":"review.done","payload":{{"status":"approved","files":["a.rs","b.rs"]}},"ts":"2024-01-01T00:00:00Z"}}"#
        )
        .unwrap();
        file.flush().unwrap();

        let mut reader = EventReader::new(file.path());
        let result = reader.read_new_events().unwrap();

        assert_eq!(result.events.len(), 1);
        assert_eq!(result.events[0].topic, "review.done");

        // Payload should be stringified JSON
        let payload = result.events[0].payload.as_ref().unwrap();
        assert!(payload.contains("\"status\""));
        assert!(payload.contains("\"approved\""));
        assert!(payload.contains("\"files\""));

        // Verify it can be parsed back as JSON
        let parsed: serde_json::Value = serde_json::from_str(payload).unwrap();
        assert_eq!(parsed["status"], "approved");
    }

    #[test]
    fn test_mixed_payload_formats() {
        // Test mixing string and object payloads in same file
        let mut file = NamedTempFile::new().unwrap();

        // String payload
        writeln!(
            file,
            r#"{{"topic":"task.start","payload":"Start work","ts":"2024-01-01T00:00:00Z"}}"#
        )
        .unwrap();

        // Object payload
        writeln!(
            file,
            r#"{{"topic":"task.done","payload":{{"result":"success"}},"ts":"2024-01-01T00:00:01Z"}}"#
        )
        .unwrap();

        // No payload
        writeln!(
            file,
            r#"{{"topic":"heartbeat","ts":"2024-01-01T00:00:02Z"}}"#
        )
        .unwrap();

        file.flush().unwrap();

        let mut reader = EventReader::new(file.path());
        let result = reader.read_new_events().unwrap();

        assert_eq!(result.events.len(), 3);

        // First event: string payload
        assert_eq!(result.events[0].payload, Some("Start work".to_string()));

        // Second event: object payload converted to string
        let payload2 = result.events[1].payload.as_ref().unwrap();
        assert!(payload2.contains("\"result\""));

        // Third event: no payload
        assert_eq!(result.events[2].payload, None);
    }

    #[test]
    fn test_nested_object_payload() {
        // Test deeply nested objects are handled correctly
        let mut file = NamedTempFile::new().unwrap();
        writeln!(
            file,
            r#"{{"topic":"analysis","payload":{{"issues":[{{"file":"test.rs","line":42,"severity":"major"}}],"approval":"conditional"}},"ts":"2024-01-01T00:00:00Z"}}"#
        )
        .unwrap();
        file.flush().unwrap();

        let mut reader = EventReader::new(file.path());
        let result = reader.read_new_events().unwrap();

        assert_eq!(result.events.len(), 1);

        // Should serialize nested structure
        let payload = result.events[0].payload.as_ref().unwrap();
        let parsed: serde_json::Value = serde_json::from_str(payload).unwrap();
        assert_eq!(parsed["issues"][0]["file"], "test.rs");
        assert_eq!(parsed["issues"][0]["line"], 42);
        assert_eq!(parsed["approval"], "conditional");
    }

    #[test]
    fn test_mixed_valid_invalid_handling() {
        // Test that valid events are captured alongside malformed ones
        let mut file = NamedTempFile::new().unwrap();
        writeln!(file, r#"{{"topic":"valid1","ts":"2024-01-01T00:00:00Z"}}"#).unwrap();
        writeln!(file, "not valid json at all").unwrap();
        writeln!(file, r#"{{"topic":"valid2","ts":"2024-01-01T00:00:01Z"}}"#).unwrap();
        file.flush().unwrap();

        let mut reader = EventReader::new(file.path());
        let result = reader.read_new_events().unwrap();

        assert_eq!(result.events.len(), 2);
        assert_eq!(result.malformed.len(), 1);
        assert_eq!(result.events[0].topic, "valid1");
        assert_eq!(result.events[1].topic, "valid2");
    }
}
