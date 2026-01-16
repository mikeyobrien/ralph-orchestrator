//! Session recorder for writing events to JSONL files.
//!
//! `SessionRecorder` captures events from both the EventBus (routing events)
//! and UX captures (terminal output) into a unified JSONL format for replay
//! and analysis.

use ralph_proto::{Event, UxEvent};
use serde::{Deserialize, Serialize};
use std::io::{self, Write};
use std::sync::Mutex;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

/// A timestamped record in the JSONL session file.
///
/// Records use internal tagging to distinguish event types while maintaining
/// a flat structure for easy parsing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Record {
    /// Unix timestamp in milliseconds when the event was recorded.
    pub ts: u64,

    /// The event type discriminator (e.g., "bus.publish", "ux.terminal.write").
    pub event: String,

    /// The event data, serialized based on event type.
    pub data: serde_json::Value,
}

impl Record {
    /// Creates a new record with the current timestamp.
    pub fn new(event: impl Into<String>, data: impl Serialize) -> Self {
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        Self {
            ts,
            event: event.into(),
            data: serde_json::to_value(data).unwrap_or(serde_json::Value::Null),
        }
    }

    /// Creates a record for an EventBus event.
    pub fn from_bus_event(event: &Event) -> Self {
        Self::new("bus.publish", event)
    }

    /// Creates a record for a UX event.
    pub fn from_ux_event(ux_event: &UxEvent) -> Self {
        // Extract the event type from the UxEvent's serde tag
        let event_type = match ux_event {
            UxEvent::TerminalWrite(_) => "ux.terminal.write",
            UxEvent::TerminalResize(_) => "ux.terminal.resize",
            UxEvent::TerminalColorMode(_) => "ux.terminal.color_mode",
            UxEvent::TuiFrame(_) => "ux.tui.frame",
        };
        Self::new(event_type, ux_event)
    }

    /// Creates a metadata record for loop start.
    pub fn meta_loop_start(prompt_file: &str, max_iterations: u32, ux_mode: Option<&str>) -> Self {
        Self::new(
            "_meta.loop_start",
            serde_json::json!({
                "prompt_file": prompt_file,
                "max_iterations": max_iterations,
                "ux_mode": ux_mode.unwrap_or("cli"),
            }),
        )
    }

    /// Creates a metadata record for an iteration.
    pub fn meta_iteration(iteration: u32, elapsed_ms: u64, hat: &str) -> Self {
        Self::new(
            "_meta.iteration",
            serde_json::json!({
                "n": iteration,
                "elapsed_ms": elapsed_ms,
                "hat": hat,
            }),
        )
    }

    /// Creates a metadata record for termination.
    pub fn meta_termination(
        reason: &str,
        iterations: u32,
        elapsed_secs: f64,
        ux_writes: u32,
    ) -> Self {
        Self::new(
            "_meta.termination",
            serde_json::json!({
                "reason": reason,
                "iterations": iterations,
                "elapsed_secs": elapsed_secs,
                "ux_writes": ux_writes,
            }),
        )
    }
}

/// Records session events to a JSONL output.
///
/// The recorder is thread-safe and can be used as an EventBus observer.
/// It writes each event as a JSON line immediately for crash resilience.
///
/// # Example
///
/// ```
/// use ralph_core::SessionRecorder;
/// use ralph_proto::Event;
///
/// let mut output = Vec::new();
/// let recorder = SessionRecorder::new(&mut output);
///
/// // Record a bus event
/// let event = Event::new("task.start", "Begin implementation");
/// recorder.record_bus_event(&event);
///
/// // Flush and check output
/// drop(recorder);
/// let output_str = String::from_utf8_lossy(&output);
/// assert!(output_str.contains("bus.publish"));
/// ```
pub struct SessionRecorder<W> {
    /// The output writer, wrapped in a mutex for thread-safe access.
    writer: Mutex<W>,

    /// Start time for calculating session-relative offsets.
    start_time: Instant,

    /// Counter for UX write events recorded.
    ux_write_count: Mutex<u32>,
}

impl<W: Write> SessionRecorder<W> {
    /// Creates a new session recorder writing to the given output.
    pub fn new(writer: W) -> Self {
        Self {
            writer: Mutex::new(writer),
            start_time: Instant::now(),
            ux_write_count: Mutex::new(0),
        }
    }

    /// Records an EventBus event.
    pub fn record_bus_event(&self, event: &Event) {
        let record = Record::from_bus_event(event);
        self.write_record(&record);
    }

    /// Records a UX event.
    pub fn record_ux_event(&self, ux_event: &UxEvent) {
        if matches!(ux_event, UxEvent::TerminalWrite(_))
            && let Ok(mut count) = self.ux_write_count.lock()
        {
            *count += 1;
        }
        let record = Record::from_ux_event(ux_event);
        self.write_record(&record);
    }

    /// Records multiple UX events.
    pub fn record_ux_events(&self, events: &[UxEvent]) {
        for event in events {
            self.record_ux_event(event);
        }
    }

    /// Records a metadata event.
    pub fn record_meta(&self, record: Record) {
        self.write_record(&record);
    }

    /// Returns the number of UX write events recorded.
    pub fn ux_write_count(&self) -> u32 {
        self.ux_write_count.lock().map(|g| *g).unwrap_or(0)
    }

    /// Returns the elapsed time since recording started.
    pub fn elapsed(&self) -> std::time::Duration {
        self.start_time.elapsed()
    }

    /// Writes a record to the output.
    fn write_record(&self, record: &Record) {
        if let Ok(mut writer) = self.writer.lock() {
            // Ignore write errors - recording should not interrupt execution
            if let Ok(json) = serde_json::to_string(record) {
                let _ = writeln!(writer, "{}", json);
            }
        }
    }

    /// Flushes the underlying writer.
    pub fn flush(&self) -> io::Result<()> {
        self.writer
            .lock()
            .map_err(|_| io::Error::new(io::ErrorKind::Other, "Failed to acquire writer lock"))?
            .flush()
    }
}

impl<W: Write + Send + 'static> SessionRecorder<W> {
    /// Creates an observer closure suitable for EventBus::set_observer.
    ///
    /// The returned closure holds a reference to this recorder and calls
    /// `record_bus_event` for each event received.
    ///
    /// # Example
    ///
    /// ```ignore
    /// let recorder = Arc::new(SessionRecorder::new(file));
    /// let observer = SessionRecorder::make_observer(Arc::clone(&recorder));
    /// event_bus.set_observer(observer);
    /// ```
    pub fn make_observer(recorder: std::sync::Arc<Self>) -> impl Fn(&Event) + Send + 'static {
        move |event| {
            recorder.record_bus_event(event);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_record_bus_event() {
        let mut output = Vec::new();
        {
            let recorder = SessionRecorder::new(&mut output);
            let event = Event::new("task.start", "Begin work");
            recorder.record_bus_event(&event);
        }

        let output_str = String::from_utf8_lossy(&output);
        assert!(output_str.contains("bus.publish"));
        assert!(output_str.contains("task.start"));
        assert!(output_str.contains("Begin work"));
    }

    #[test]
    fn test_record_ux_event() {
        use ralph_proto::TerminalWrite;

        let mut output = Vec::new();
        {
            let recorder = SessionRecorder::new(&mut output);
            let ux_event = UxEvent::TerminalWrite(TerminalWrite::new(b"Hello", true, 100));
            recorder.record_ux_event(&ux_event);
        }

        let output_str = String::from_utf8_lossy(&output);
        assert!(output_str.contains("ux.terminal.write"));
        assert!(output_str.contains("SGVsbG8=")); // "Hello" in base64
    }

    #[test]
    fn test_record_metadata() {
        let mut output = Vec::new();
        {
            let recorder = SessionRecorder::new(&mut output);
            recorder.record_meta(Record::meta_loop_start("PROMPT.md", 100, Some("cli")));
            recorder.record_meta(Record::meta_iteration(1, 5000, "default"));
            recorder.record_meta(Record::meta_termination("CompletionPromise", 3, 25.5, 42));
        }

        let output_str = String::from_utf8_lossy(&output);
        assert!(output_str.contains("_meta.loop_start"));
        assert!(output_str.contains("_meta.iteration"));
        assert!(output_str.contains("_meta.termination"));
        assert!(output_str.contains("PROMPT.md"));
        assert!(output_str.contains("CompletionPromise"));
    }

    #[test]
    fn test_jsonl_format() {
        let mut output = Vec::new();
        {
            let recorder = SessionRecorder::new(&mut output);
            recorder.record_bus_event(&Event::new("test.1", "First"));
            recorder.record_bus_event(&Event::new("test.2", "Second"));
        }

        let output_str = String::from_utf8_lossy(&output);
        let lines: Vec<&str> = output_str.lines().collect();

        // Should have exactly 2 lines (JSONL format)
        assert_eq!(lines.len(), 2);

        // Each line should be valid JSON
        for line in lines {
            let parsed: Result<serde_json::Value, _> = serde_json::from_str(line);
            assert!(parsed.is_ok(), "Line should be valid JSON: {}", line);
        }
    }

    #[test]
    fn test_ux_write_count() {
        use ralph_proto::{TerminalResize, TerminalWrite};

        let output = Vec::new();
        let recorder = SessionRecorder::new(output);

        // Record some UX events
        recorder.record_ux_event(&UxEvent::TerminalWrite(TerminalWrite::new(b"a", true, 0)));
        recorder.record_ux_event(&UxEvent::TerminalResize(TerminalResize::new(80, 24, 10)));
        recorder.record_ux_event(&UxEvent::TerminalWrite(TerminalWrite::new(b"b", true, 20)));

        // Only TerminalWrite events should be counted
        assert_eq!(recorder.ux_write_count(), 2);
    }

    #[test]
    fn test_record_roundtrip() {
        let event = Event::new("task.done", "Finished");
        let record = Record::from_bus_event(&event);

        // Serialize and deserialize
        let json = serde_json::to_string(&record).unwrap();
        let parsed: Record = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.event, "bus.publish");
        assert!(parsed.ts > 0);
    }
}
