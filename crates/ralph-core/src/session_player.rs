//! Session player for replaying recorded JSONL sessions.
//!
//! `SessionPlayer` reads events from JSONL files and replays them with
//! configurable timing. Supports terminal output replay (with ANSI colors),
//! plain text mode (ANSI stripped), and step-through debugging.

use ralph_proto::{TerminalWrite, UxEvent};
use std::io::{self, BufRead, Write};
use std::time::Duration;

use crate::session_recorder::Record;

/// Replay mode for session playback.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplayMode {
    /// Re-render to terminal with timing and colors preserved.
    Terminal,
    /// Strip ANSI codes, output plain text.
    Text,
}

/// Configuration for session playback.
#[derive(Debug, Clone)]
pub struct PlayerConfig {
    /// Replay speed multiplier (1.0 = original speed, 2.0 = 2x faster).
    pub speed: f32,

    /// If true, pause after each event and wait for Enter.
    pub step_mode: bool,

    /// Output mode for UX events.
    pub replay_mode: ReplayMode,

    /// Filter to specific event types (empty = all events).
    pub event_filter: Vec<String>,
}

impl Default for PlayerConfig {
    fn default() -> Self {
        Self {
            speed: 1.0,
            step_mode: false,
            replay_mode: ReplayMode::Terminal,
            event_filter: Vec::new(),
        }
    }
}

impl PlayerConfig {
    /// Creates a new config with terminal replay mode.
    pub fn terminal() -> Self {
        Self::default()
    }

    /// Creates a new config with text replay mode (ANSI stripped).
    pub fn text() -> Self {
        Self {
            replay_mode: ReplayMode::Text,
            ..Default::default()
        }
    }

    /// Sets the speed multiplier.
    pub fn with_speed(mut self, speed: f32) -> Self {
        self.speed = speed.max(0.1); // Minimum 0.1x speed
        self
    }

    /// Enables step-through mode.
    pub fn with_step_mode(mut self) -> Self {
        self.step_mode = true;
        self
    }

    /// Filters to specific event types.
    pub fn with_filter(mut self, events: Vec<String>) -> Self {
        self.event_filter = events;
        self
    }
}

/// A parsed record with timing information for replay.
#[derive(Debug, Clone)]
pub struct TimestampedRecord {
    /// The original record.
    pub record: Record,

    /// Offset from session start in milliseconds.
    pub offset_ms: u64,
}

/// Plays back recorded sessions.
///
/// `SessionPlayer` reads JSONL records, extracts timing information,
/// and replays events with configurable speed and output modes.
///
/// # Example
///
/// ```
/// use ralph_core::{SessionPlayer, PlayerConfig};
/// use std::io::Cursor;
///
/// let jsonl = r#"{"ts":1000,"event":"ux.terminal.write","data":{"bytes":"SGVsbG8=","stdout":true,"offset_ms":0}}
/// {"ts":1100,"event":"ux.terminal.write","data":{"bytes":"V29ybGQ=","stdout":true,"offset_ms":100}}"#;
///
/// let reader = Cursor::new(jsonl);
/// let player = SessionPlayer::from_reader(reader).unwrap();
///
/// assert_eq!(player.record_count(), 2);
/// ```
#[derive(Debug)]
pub struct SessionPlayer {
    /// Parsed records with timing.
    records: Vec<TimestampedRecord>,

    /// Playback configuration.
    config: PlayerConfig,

    /// Current playback position.
    position: usize,
}

impl SessionPlayer {
    /// Creates a player from a JSONL reader.
    pub fn from_reader<R: BufRead>(reader: R) -> io::Result<Self> {
        let mut records = Vec::new();
        let mut first_ts: Option<u64> = None;

        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }

            let record: Record = serde_json::from_str(&line).map_err(|e| {
                io::Error::new(
                    io::ErrorKind::InvalidData,
                    format!("Invalid JSON record: {}", e),
                )
            })?;

            // Calculate offset from session start
            let ts = record.ts;
            let base_ts = *first_ts.get_or_insert(ts);
            let offset_ms = ts.saturating_sub(base_ts);

            records.push(TimestampedRecord { record, offset_ms });
        }

        Ok(Self {
            records,
            config: PlayerConfig::default(),
            position: 0,
        })
    }

    /// Creates a player from raw JSONL bytes.
    pub fn from_bytes(bytes: &[u8]) -> io::Result<Self> {
        Self::from_reader(io::BufReader::new(bytes))
    }

    /// Sets the playback configuration.
    pub fn with_config(mut self, config: PlayerConfig) -> Self {
        self.config = config;
        self
    }

    /// Returns the number of records.
    pub fn record_count(&self) -> usize {
        self.records.len()
    }

    /// Returns all records.
    pub fn records(&self) -> &[TimestampedRecord] {
        &self.records
    }

    /// Returns records filtered by event type.
    pub fn filter_by_event(&self, event_prefix: &str) -> Vec<&TimestampedRecord> {
        self.records
            .iter()
            .filter(|r| r.record.event.starts_with(event_prefix))
            .collect()
    }

    /// Returns only UX terminal write events.
    pub fn terminal_writes(&self) -> Vec<&TimestampedRecord> {
        self.filter_by_event("ux.terminal.write")
    }

    /// Returns only metadata events.
    pub fn metadata_events(&self) -> Vec<&TimestampedRecord> {
        self.filter_by_event("_meta.")
    }

    /// Returns only bus events.
    pub fn bus_events(&self) -> Vec<&TimestampedRecord> {
        self.filter_by_event("bus.")
    }

    /// Resets playback to the beginning.
    pub fn reset(&mut self) {
        self.position = 0;
    }

    /// Replays all UX terminal events to the given writer.
    ///
    /// This is a synchronous replay that respects timing delays adjusted
    /// by the speed multiplier. In step mode, it waits for Enter after
    /// each event.
    pub fn replay_terminal<W: Write>(&mut self, writer: &mut W) -> io::Result<()> {
        self.reset();
        let mut last_offset_ms: u64 = 0;

        let terminal_writes = self.terminal_writes();
        for record in terminal_writes {
            // Calculate delay from previous event
            let delay_ms = record.offset_ms.saturating_sub(last_offset_ms);
            last_offset_ms = record.offset_ms;

            // Apply speed multiplier
            if !self.config.step_mode && delay_ms > 0 && self.config.speed > 0.0 {
                let adjusted_delay = (delay_ms as f32 / self.config.speed) as u64;
                if adjusted_delay > 0 {
                    std::thread::sleep(Duration::from_millis(adjusted_delay));
                }
            }

            // Parse and output the terminal write
            if let Ok(UxEvent::TerminalWrite(write)) = Self::parse_ux_event(&record.record) {
                self.output_terminal_write(writer, &write)?;
            }

            // Step mode: wait for Enter
            if self.config.step_mode {
                writer.flush()?;
                let mut input = String::new();
                io::stdin().read_line(&mut input)?;
            }
        }

        writer.flush()
    }

    /// Outputs a terminal write event based on replay mode.
    fn output_terminal_write<W: Write>(
        &self,
        writer: &mut W,
        write: &TerminalWrite,
    ) -> io::Result<()> {
        let bytes = write.decode_bytes().map_err(|e| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Failed to decode base64: {}", e),
            )
        })?;

        match self.config.replay_mode {
            ReplayMode::Terminal => {
                // Output raw bytes (preserves ANSI sequences)
                writer.write_all(&bytes)?;
            }
            ReplayMode::Text => {
                // Strip ANSI sequences
                let stripped = strip_ansi(&bytes);
                writer.write_all(&stripped)?;
            }
        }

        Ok(())
    }

    /// Parses a Record's data field as a UxEvent.
    fn parse_ux_event(record: &Record) -> Result<UxEvent, serde_json::Error> {
        // The record stores data without the event tag, so we need to reconstruct
        // the tagged format for UxEvent deserialization
        let tagged = serde_json::json!({
            "event": record.event,
            "data": record.data,
        });
        serde_json::from_value(tagged)
    }

    /// Collects all terminal output as a single string (for snapshot testing).
    pub fn collect_terminal_output(&self) -> io::Result<String> {
        let mut output = Vec::new();

        for record in self.terminal_writes() {
            if let Ok(UxEvent::TerminalWrite(write)) = Self::parse_ux_event(&record.record) {
                let bytes = write.decode_bytes().map_err(|e| {
                    io::Error::new(
                        io::ErrorKind::InvalidData,
                        format!("Failed to decode base64: {}", e),
                    )
                })?;
                output.extend_from_slice(&bytes);
            }
        }

        String::from_utf8(output).map_err(|e| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Invalid UTF-8 in terminal output: {}", e),
            )
        })
    }

    /// Collects terminal output with ANSI codes stripped (for text snapshot testing).
    pub fn collect_text_output(&self) -> io::Result<String> {
        let raw = self.collect_terminal_output()?;
        Ok(String::from_utf8_lossy(&strip_ansi(raw.as_bytes())).into_owned())
    }

    /// Collects terminal output with ANSI codes escaped (for ANSI snapshot testing).
    pub fn collect_ansi_escaped(&self) -> io::Result<String> {
        let raw = self.collect_terminal_output()?;
        Ok(escape_ansi(&raw))
    }
}

/// Strips ANSI escape sequences from bytes.
///
/// Handles CSI sequences (\x1b[...m), OSC sequences (\x1b]...\x07),
/// and simple escape sequences (\x1b followed by a single char).
fn strip_ansi(bytes: &[u8]) -> Vec<u8> {
    let mut result = Vec::with_capacity(bytes.len());
    let mut i = 0;

    while i < bytes.len() {
        if bytes[i] == 0x1b {
            // ESC character - start of escape sequence
            i += 1;
            if i >= bytes.len() {
                break;
            }

            match bytes[i] {
                b'[' => {
                    // CSI sequence: ESC [ ... (final byte in 0x40-0x7E range)
                    i += 1;
                    while i < bytes.len() && !(0x40..=0x7E).contains(&bytes[i]) {
                        i += 1;
                    }
                    if i < bytes.len() {
                        i += 1; // Skip final byte
                    }
                }
                b']' => {
                    // OSC sequence: ESC ] ... (terminated by BEL or ST)
                    i += 1;
                    while i < bytes.len() {
                        if bytes[i] == 0x07 {
                            i += 1;
                            break;
                        }
                        if bytes[i] == 0x1b && i + 1 < bytes.len() && bytes[i + 1] == b'\\' {
                            i += 2;
                            break;
                        }
                        i += 1;
                    }
                }
                _ => {
                    // Simple escape sequence: ESC + single char
                    i += 1;
                }
            }
        } else {
            result.push(bytes[i]);
            i += 1;
        }
    }

    result
}

/// Escapes ANSI sequences for visibility in snapshots.
///
/// Converts \x1b to `\x1b` literal string for readable diff comparisons.
fn escape_ansi(s: &str) -> String {
    s.replace('\x1b', "\\x1b")
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn test_player_from_reader() {
        let line1 = make_write_record(b"Hello", true, 0, 1000);
        let line2 = make_write_record(b"World", true, 100, 1000);
        let jsonl = format!("{}\n{}\n", line1, line2);

        let player = SessionPlayer::from_bytes(jsonl.as_bytes()).unwrap();

        assert_eq!(player.record_count(), 2);
        assert_eq!(player.records[0].offset_ms, 0);
        assert_eq!(player.records[1].offset_ms, 100);
    }

    #[test]
    fn test_filter_by_event() {
        let write = make_write_record(b"test", true, 0, 1000);
        let meta = r#"{"ts":1000,"event":"_meta.loop_start","data":{"prompt_file":"PROMPT.md"}}"#;
        let bus = r#"{"ts":1050,"event":"bus.publish","data":{"topic":"task.start"}}"#;

        let jsonl = format!("{}\n{}\n{}\n", write, meta, bus);
        let player = SessionPlayer::from_bytes(jsonl.as_bytes()).unwrap();

        assert_eq!(player.terminal_writes().len(), 1);
        assert_eq!(player.metadata_events().len(), 1);
        assert_eq!(player.bus_events().len(), 1);
    }

    #[test]
    fn test_collect_terminal_output() {
        let line1 = make_write_record(b"Hello, ", true, 0, 1000);
        let line2 = make_write_record(b"World!", true, 50, 1000);
        let jsonl = format!("{}\n{}\n", line1, line2);

        let player = SessionPlayer::from_bytes(jsonl.as_bytes()).unwrap();
        let output = player.collect_terminal_output().unwrap();

        assert_eq!(output, "Hello, World!");
    }

    #[test]
    fn test_strip_ansi() {
        let input = b"Hello, \x1b[32mWorld\x1b[0m!";
        let stripped = strip_ansi(input);
        assert_eq!(stripped, b"Hello, World!");
    }

    #[test]
    fn test_strip_ansi_complex() {
        // Multiple CSI sequences
        let input = b"\x1b[1m\x1b[32mBold Green\x1b[0m Normal";
        let stripped = strip_ansi(input);
        assert_eq!(stripped, b"Bold Green Normal");
    }

    #[test]
    fn test_escape_ansi() {
        let input = "Hello \x1b[32mWorld\x1b[0m";
        let escaped = escape_ansi(input);
        assert_eq!(escaped, "Hello \\x1b[32mWorld\\x1b[0m");
    }

    #[test]
    fn test_collect_text_output() {
        let line = make_write_record(b"Hello \x1b[32mWorld\x1b[0m", true, 0, 1000);
        let player = SessionPlayer::from_bytes(line.as_bytes()).unwrap();

        let text = player.collect_text_output().unwrap();
        assert_eq!(text, "Hello World");
    }

    #[test]
    fn test_collect_ansi_escaped() {
        let line = make_write_record(b"Hello \x1b[32mWorld\x1b[0m", true, 0, 1000);
        let player = SessionPlayer::from_bytes(line.as_bytes()).unwrap();

        let escaped = player.collect_ansi_escaped().unwrap();
        assert_eq!(escaped, "Hello \\x1b[32mWorld\\x1b[0m");
    }

    #[test]
    fn test_replay_terminal() {
        let line1 = make_write_record(b"Hello", true, 0, 1000);
        let line2 = make_write_record(b" World", true, 10, 1000);
        let jsonl = format!("{}\n{}\n", line1, line2);

        let mut player = SessionPlayer::from_bytes(jsonl.as_bytes())
            .unwrap()
            .with_config(PlayerConfig::terminal().with_speed(100.0)); // Fast replay

        let mut output = Vec::new();
        player.replay_terminal(&mut output).unwrap();

        assert_eq!(String::from_utf8(output).unwrap(), "Hello World");
    }

    #[test]
    fn test_replay_text_mode() {
        let line = make_write_record(b"\x1b[32mGreen\x1b[0m", true, 0, 1000);
        let mut player = SessionPlayer::from_bytes(line.as_bytes())
            .unwrap()
            .with_config(PlayerConfig::text());

        let mut output = Vec::new();
        player.replay_terminal(&mut output).unwrap();

        assert_eq!(String::from_utf8(output).unwrap(), "Green");
    }

    #[test]
    fn test_player_config_builder() {
        let config = PlayerConfig::terminal()
            .with_speed(2.0)
            .with_step_mode()
            .with_filter(vec!["ux.".to_string()]);

        assert!((config.speed - 2.0).abs() < f32::EPSILON);
        assert!(config.step_mode);
        assert_eq!(config.event_filter, vec!["ux."]);
    }

    #[test]
    fn test_empty_input() {
        let player = SessionPlayer::from_bytes(b"").unwrap();
        assert_eq!(player.record_count(), 0);
    }

    #[test]
    fn test_whitespace_lines_skipped() {
        let line = make_write_record(b"test", true, 0, 1000);
        let jsonl = format!("\n  \n{}\n\n", line);

        let player = SessionPlayer::from_bytes(jsonl.as_bytes()).unwrap();
        assert_eq!(player.record_count(), 1);
    }
}
