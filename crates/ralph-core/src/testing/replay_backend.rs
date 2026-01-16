//! Replay backend for deterministic testing using recorded JSONL sessions.
//!
//! `ReplayBackend` loads JSONL session files recorded by `SessionRecorder` and
//! replays terminal output as mock CLI responses. This enables deterministic
//! smoketesting without live API calls.
//!
//! # Example
//!
//! ```
//! use ralph_core::testing::ReplayBackend;
//! use std::io::Cursor;
//!
//! // Create a simple JSONL fixture
//! let jsonl = r#"{"ts":1000,"event":"ux.terminal.write","data":{"bytes":"SGVsbG8=","stdout":true,"offset_ms":0}}"#;
//! let mut backend = ReplayBackend::from_reader(Cursor::new(jsonl)).unwrap();
//!
//! // Get output chunks in order
//! let chunk = backend.next_output().unwrap();
//! assert_eq!(chunk, b"Hello");
//! ```

use crate::session_player::SessionPlayer;
use ralph_proto::UxEvent;
use std::io::{self, BufRead, BufReader};
use std::path::Path;
use std::time::Duration;

/// Timing mode for replay.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ReplayTimingMode {
    /// Serve all output immediately without timing delays.
    #[default]
    Instant,
    /// Honor recorded timing delays between outputs.
    Realistic,
}

/// A backend that replays recorded JSONL session output.
///
/// Loads session recordings from `SessionRecorder` and serves terminal output
/// chunks in the order they were recorded. Supports configurable timing modes
/// for fast tests or realistic replay.
#[derive(Debug)]
pub struct ReplayBackend {
    /// The underlying session player with parsed records.
    player: SessionPlayer,
    /// Current position in the terminal writes.
    position: usize,
    /// Timing mode for replay.
    timing_mode: ReplayTimingMode,
    /// Cached terminal write indices for efficient iteration.
    terminal_write_indices: Vec<usize>,
    /// Last offset for timing calculations.
    last_offset_ms: u64,
}

impl ReplayBackend {
    /// Creates a replay backend from a JSONL file.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be opened or contains invalid JSON.
    pub fn from_file(path: impl AsRef<Path>) -> io::Result<Self> {
        let file = std::fs::File::open(path.as_ref())?;
        let reader = BufReader::new(file);
        Self::from_reader(reader)
    }

    /// Creates a replay backend from a JSONL reader.
    ///
    /// # Errors
    ///
    /// Returns an error if the JSONL data is malformed.
    pub fn from_reader<R: BufRead>(reader: R) -> io::Result<Self> {
        let player = SessionPlayer::from_reader(reader)?;

        // Pre-compute indices of terminal write records for efficient iteration
        let terminal_write_indices: Vec<usize> = player
            .records()
            .iter()
            .enumerate()
            .filter(|(_, r)| r.record.event == "ux.terminal.write")
            .map(|(i, _)| i)
            .collect();

        Ok(Self {
            player,
            position: 0,
            timing_mode: ReplayTimingMode::default(),
            terminal_write_indices,
            last_offset_ms: 0,
        })
    }

    /// Creates a replay backend from raw JSONL bytes.
    ///
    /// # Errors
    ///
    /// Returns an error if the data is malformed.
    pub fn from_bytes(bytes: &[u8]) -> io::Result<Self> {
        Self::from_reader(io::BufReader::new(bytes))
    }

    /// Sets the timing mode for replay.
    pub fn with_timing(mut self, mode: ReplayTimingMode) -> Self {
        self.timing_mode = mode;
        self
    }

    /// Returns the next terminal output chunk, or `None` if exhausted.
    ///
    /// In `Realistic` timing mode, this will sleep for the recorded delay
    /// between outputs.
    pub fn next_output(&mut self) -> Option<Vec<u8>> {
        if self.position >= self.terminal_write_indices.len() {
            return None;
        }

        let record_idx = self.terminal_write_indices[self.position];
        let record = &self.player.records()[record_idx];

        // Handle timing in realistic mode
        if self.timing_mode == ReplayTimingMode::Realistic && self.position > 0 {
            let delay_ms = record.offset_ms.saturating_sub(self.last_offset_ms);
            if delay_ms > 0 {
                std::thread::sleep(Duration::from_millis(delay_ms));
            }
        }
        self.last_offset_ms = record.offset_ms;

        // Parse and decode the terminal write
        let bytes = self.parse_terminal_write(&record.record)?;
        self.position += 1;
        Some(bytes)
    }

    /// Returns `true` if all output has been served.
    pub fn is_exhausted(&self) -> bool {
        self.position >= self.terminal_write_indices.len()
    }

    /// Returns the total number of terminal write events.
    pub fn output_count(&self) -> usize {
        self.terminal_write_indices.len()
    }

    /// Returns the number of outputs already served.
    pub fn outputs_served(&self) -> usize {
        self.position
    }

    /// Resets the replay to the beginning.
    pub fn reset(&mut self) {
        self.position = 0;
        self.last_offset_ms = 0;
    }

    /// Collects all remaining output into a single byte vector.
    ///
    /// This consumes all remaining output chunks. The backend will be
    /// exhausted after calling this method.
    pub fn collect_remaining(&mut self) -> Vec<u8> {
        let mut result = Vec::new();
        while let Some(chunk) = self.next_output() {
            result.extend(chunk);
        }
        result
    }

    /// Collects all output (from beginning) into a single byte vector.
    ///
    /// Resets position before collecting.
    pub fn collect_all(&mut self) -> Vec<u8> {
        self.reset();
        self.collect_remaining()
    }

    /// Parses a Record's data field as terminal write bytes.
    fn parse_terminal_write(&self, record: &crate::session_recorder::Record) -> Option<Vec<u8>> {
        // Reconstruct the tagged format for UxEvent deserialization
        let tagged = serde_json::json!({
            "event": record.event,
            "data": record.data,
        });

        let ux_event: UxEvent = serde_json::from_value(tagged).ok()?;

        if let UxEvent::TerminalWrite(write) = ux_event {
            write.decode_bytes().ok()
        } else {
            None
        }
    }
}

/// Iterator implementation for streaming output chunks.
impl Iterator for ReplayBackend {
    type Item = Vec<u8>;

    fn next(&mut self) -> Option<Self::Item> {
        self.next_output()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session_recorder::Record;
    use ralph_proto::TerminalWrite;

    /// Helper to create a JSONL line for a terminal write event.
    fn make_write_record(bytes: &[u8], stdout: bool, offset_ms: u64, base_ts: u64) -> String {
        let write = TerminalWrite::new(bytes, stdout, offset_ms);
        let record = Record {
            ts: base_ts + offset_ms,
            event: "ux.terminal.write".to_string(),
            data: serde_json::to_value(&write).unwrap(),
        };
        serde_json::to_string(&record).unwrap()
    }

    #[test]
    fn test_from_reader_loads_valid_jsonl() {
        let line1 = make_write_record(b"Hello", true, 0, 1000);
        let line2 = make_write_record(b" World", true, 100, 1000);
        let jsonl = format!("{}\n{}\n", line1, line2);

        let backend = ReplayBackend::from_bytes(jsonl.as_bytes()).unwrap();

        assert_eq!(backend.output_count(), 2);
        assert!(!backend.is_exhausted());
    }

    #[test]
    fn test_from_file_error_on_missing_file() {
        let result = ReplayBackend::from_file("/nonexistent/path/to/file.jsonl");
        assert!(result.is_err());

        let err = result.unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::NotFound);
    }

    #[test]
    fn test_from_reader_empty_input() {
        let backend = ReplayBackend::from_bytes(b"").unwrap();

        assert_eq!(backend.output_count(), 0);
        assert!(backend.is_exhausted());
    }

    #[test]
    fn test_next_output_returns_bytes_in_order() {
        let line1 = make_write_record(b"First", true, 0, 1000);
        let line2 = make_write_record(b"Second", true, 50, 1000);
        let line3 = make_write_record(b"Third", true, 100, 1000);
        let jsonl = format!("{}\n{}\n{}\n", line1, line2, line3);

        let mut backend = ReplayBackend::from_bytes(jsonl.as_bytes()).unwrap();

        assert_eq!(backend.next_output().unwrap(), b"First");
        assert_eq!(backend.next_output().unwrap(), b"Second");
        assert_eq!(backend.next_output().unwrap(), b"Third");
        assert!(backend.next_output().is_none());
    }

    #[test]
    fn test_is_exhausted_true_after_all_output() {
        let line = make_write_record(b"Only", true, 0, 1000);
        let mut backend = ReplayBackend::from_bytes(line.as_bytes()).unwrap();

        assert!(!backend.is_exhausted());
        assert_eq!(backend.outputs_served(), 0);

        backend.next_output();

        assert!(backend.is_exhausted());
        assert_eq!(backend.outputs_served(), 1);
    }

    #[test]
    fn test_instant_mode_serves_all_immediately() {
        let line1 = make_write_record(b"A", true, 0, 1000);
        let line2 = make_write_record(b"B", true, 1000, 1000); // 1 second delay
        let jsonl = format!("{}\n{}\n", line1, line2);

        let mut backend = ReplayBackend::from_bytes(jsonl.as_bytes())
            .unwrap()
            .with_timing(ReplayTimingMode::Instant);

        // Should be instant, not delayed
        let start = std::time::Instant::now();
        backend.next_output();
        backend.next_output();
        let elapsed = start.elapsed();

        // Should complete in well under 1 second (the recorded delay)
        assert!(
            elapsed.as_millis() < 100,
            "Should be instant, took {:?}",
            elapsed
        );
    }

    #[test]
    fn test_iterator_yields_all_chunks() {
        let line1 = make_write_record(b"One", true, 0, 1000);
        let line2 = make_write_record(b"Two", true, 10, 1000);
        let jsonl = format!("{}\n{}\n", line1, line2);

        let backend = ReplayBackend::from_bytes(jsonl.as_bytes()).unwrap();
        let chunks: Vec<Vec<u8>> = backend.collect();

        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0], b"One");
        assert_eq!(chunks[1], b"Two");
    }

    #[test]
    fn test_collect_all_concatenates_output() {
        let line1 = make_write_record(b"Hello, ", true, 0, 1000);
        let line2 = make_write_record(b"World!", true, 50, 1000);
        let jsonl = format!("{}\n{}\n", line1, line2);

        let mut backend = ReplayBackend::from_bytes(jsonl.as_bytes()).unwrap();
        let all = backend.collect_all();

        assert_eq!(all, b"Hello, World!");
    }

    #[test]
    fn test_reset_allows_replay() {
        let line = make_write_record(b"Replay", true, 0, 1000);
        let mut backend = ReplayBackend::from_bytes(line.as_bytes()).unwrap();

        // First pass
        assert_eq!(backend.next_output().unwrap(), b"Replay");
        assert!(backend.is_exhausted());

        // Reset and replay
        backend.reset();
        assert!(!backend.is_exhausted());
        assert_eq!(backend.next_output().unwrap(), b"Replay");
    }

    #[test]
    fn test_filters_non_terminal_write_events() {
        let write = make_write_record(b"output", true, 0, 1000);
        let meta = r#"{"ts":1000,"event":"_meta.loop_start","data":{"prompt_file":"PROMPT.md"}}"#;
        let bus = r#"{"ts":1050,"event":"bus.publish","data":{"topic":"task.start"}}"#;

        let jsonl = format!("{}\n{}\n{}\n", write, meta, bus);
        let backend = ReplayBackend::from_bytes(jsonl.as_bytes()).unwrap();

        // Should only have the terminal write event
        assert_eq!(backend.output_count(), 1);
    }

    #[test]
    fn test_handles_whitespace_lines() {
        let line = make_write_record(b"data", true, 0, 1000);
        let jsonl = format!("\n  \n{}\n\n", line);

        let backend = ReplayBackend::from_bytes(jsonl.as_bytes()).unwrap();
        assert_eq!(backend.output_count(), 1);
    }

    #[test]
    fn test_malformed_json_returns_error() {
        let result = ReplayBackend::from_bytes(b"not valid json");
        assert!(result.is_err());
    }
}
