//! JSON-RPC stream handler for emitting orchestration events as JSON lines.
//!
//! This handler implements `StreamHandler` and writes one JSON line per event
//! to a configurable writer (typically stdout). It's the event producer side
//! of Ralph's JSON-RPC protocol, enabling machine-readable output for frontends.

use crate::{SessionResult, StreamHandler};
use ralph_proto::json_rpc::{RpcEvent, emit_event_line};
use serde_json::Value;
use std::io::{self, Write};
use std::sync::{Arc, Mutex};
use std::time::Instant;
use tracing::warn;

/// Stream handler that emits JSON-RPC events to a writer.
///
/// Each `StreamHandler` callback produces one JSON line using the event types
/// defined in `ralph_proto::json_rpc`. The handler tracks iteration context
/// and timing to populate event metadata.
pub struct JsonRpcStreamHandler<W: Write + Send> {
    /// Output writer (typically stdout, but configurable for testing).
    writer: Arc<Mutex<W>>,
    /// Current iteration number (1-indexed).
    iteration: u32,
    /// Current hat ID.
    hat: Option<String>,
    /// Current backend name.
    backend: Option<String>,
    /// Tool call start times for duration tracking.
    tool_start_times: std::collections::HashMap<String, Instant>,
    /// Set to true after a broken pipe; suppresses all further writes.
    poisoned: bool,
}

impl<W: Write + Send> JsonRpcStreamHandler<W> {
    /// Creates a new JSON-RPC handler writing to the given writer.
    ///
    /// # Arguments
    /// * `writer` - The output sink (wrapped in Arc<Mutex> for thread safety).
    /// * `iteration` - Current iteration number (1-indexed).
    /// * `hat` - Current hat ID (e.g., "builder", "planner").
    /// * `backend` - Backend name (e.g., "claude", "gemini").
    pub fn new(
        writer: Arc<Mutex<W>>,
        iteration: u32,
        hat: Option<String>,
        backend: Option<String>,
    ) -> Self {
        Self {
            writer,
            iteration,
            hat,
            backend,
            tool_start_times: std::collections::HashMap::new(),
            poisoned: false,
        }
    }

    /// Updates the iteration number for subsequent events.
    pub fn set_iteration(&mut self, iteration: u32) {
        self.iteration = iteration;
    }

    /// Updates the hat for subsequent events.
    pub fn set_hat(&mut self, hat: Option<String>) {
        self.hat = hat;
    }

    /// Updates the backend for subsequent events.
    pub fn set_backend(&mut self, backend: Option<String>) {
        self.backend = backend;
    }

    /// Writes an event to the output, handling errors gracefully.
    fn emit(&mut self, event: RpcEvent) {
        if self.poisoned {
            return;
        }
        let line = emit_event_line(&event);
        if let Ok(mut writer) = self.writer.lock() {
            if let Err(e) = writer.write_all(line.as_bytes()) {
                warn!(error = %e, "Failed to write JSON-RPC event");
                if e.kind() == io::ErrorKind::BrokenPipe {
                    self.poisoned = true;
                }
                return;
            }
            // Flush immediately to ensure events are delivered promptly
            if let Err(e) = writer.flush() {
                warn!(error = %e, "Failed to flush JSON-RPC event");
                if e.kind() == io::ErrorKind::BrokenPipe {
                    self.poisoned = true;
                }
            }
        }
    }
}

impl<W: Write + Send> StreamHandler for JsonRpcStreamHandler<W> {
    fn on_text(&mut self, text: &str) {
        self.emit(RpcEvent::TextDelta {
            iteration: self.iteration,
            delta: text.to_string(),
        });
    }

    fn on_tool_call(&mut self, name: &str, id: &str, input: &Value) {
        // Track start time for duration calculation
        self.tool_start_times.insert(id.to_string(), Instant::now());

        self.emit(RpcEvent::ToolCallStart {
            iteration: self.iteration,
            tool_name: name.to_string(),
            tool_call_id: id.to_string(),
            input: input.clone(),
        });
    }

    fn on_tool_result(&mut self, id: &str, output: &str) {
        // Calculate duration from start time
        let duration_ms = self
            .tool_start_times
            .remove(id)
            .map(|start| start.elapsed().as_millis() as u64)
            .unwrap_or(0);

        self.emit(RpcEvent::ToolCallEnd {
            iteration: self.iteration,
            tool_call_id: id.to_string(),
            output: output.to_string(),
            is_error: false,
            duration_ms,
        });
    }

    fn on_error(&mut self, error: &str) {
        self.emit(RpcEvent::Error {
            iteration: self.iteration,
            code: "EXECUTION_ERROR".to_string(),
            message: error.to_string(),
            recoverable: true,
        });
    }

    fn on_complete(&mut self, result: &SessionResult) {
        self.emit(RpcEvent::IterationEnd {
            iteration: self.iteration,
            duration_ms: result.duration_ms,
            cost_usd: result.total_cost_usd,
            input_tokens: result.input_tokens,
            output_tokens: result.output_tokens,
            cache_read_tokens: result.cache_read_tokens,
            cache_write_tokens: result.cache_write_tokens,
            loop_complete_triggered: false, // Determined externally by orchestration
        });
    }
}

/// Creates a JsonRpcStreamHandler writing to stdout.
pub fn stdout_json_rpc_handler(
    iteration: u32,
    hat: Option<String>,
    backend: Option<String>,
) -> JsonRpcStreamHandler<io::Stdout> {
    JsonRpcStreamHandler::new(Arc::new(Mutex::new(io::stdout())), iteration, hat, backend)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn capture_handler() -> (JsonRpcStreamHandler<Vec<u8>>, Arc<Mutex<Vec<u8>>>) {
        let buffer = Arc::new(Mutex::new(Vec::new()));
        let handler = JsonRpcStreamHandler::new(
            buffer.clone(),
            3,
            Some("builder".to_string()),
            Some("claude".to_string()),
        );
        (handler, buffer)
    }

    fn get_output(buffer: &Arc<Mutex<Vec<u8>>>) -> String {
        let guard = buffer.lock().unwrap();
        String::from_utf8_lossy(&guard).to_string()
    }

    fn parse_json_line(line: &str) -> serde_json::Value {
        serde_json::from_str(line).expect("should be valid JSON")
    }

    #[test]
    fn test_text_delta_event() {
        let (mut handler, buffer) = capture_handler();

        handler.on_text("hello world");

        let output = get_output(&buffer);
        let json = parse_json_line(output.trim());

        assert_eq!(json["type"], "text_delta");
        assert_eq!(json["iteration"], 3);
        assert_eq!(json["delta"], "hello world");
    }

    #[test]
    fn test_tool_call_start_event() {
        let (mut handler, buffer) = capture_handler();

        handler.on_tool_call("Bash", "call-1", &json!({"command": "ls -la"}));

        let output = get_output(&buffer);
        let json = parse_json_line(output.trim());

        assert_eq!(json["type"], "tool_call_start");
        assert_eq!(json["iteration"], 3);
        assert_eq!(json["tool_name"], "Bash");
        assert_eq!(json["tool_call_id"], "call-1");
        assert_eq!(json["input"]["command"], "ls -la");
    }

    #[test]
    fn test_tool_call_end_event() {
        let (mut handler, buffer) = capture_handler();

        // Simulate a tool call followed by result
        handler.on_tool_call("Read", "call-2", &json!({"file_path": "/tmp/test"}));
        handler.on_tool_result("call-2", "file contents here");

        let output = get_output(&buffer);
        let lines: Vec<&str> = output.trim().lines().collect();
        assert_eq!(lines.len(), 2);

        let end_json = parse_json_line(lines[1]);
        assert_eq!(end_json["type"], "tool_call_end");
        assert_eq!(end_json["iteration"], 3);
        assert_eq!(end_json["tool_call_id"], "call-2");
        assert_eq!(end_json["output"], "file contents here");
        assert_eq!(end_json["is_error"], false);
    }

    #[test]
    fn test_error_event() {
        let (mut handler, buffer) = capture_handler();

        handler.on_error("Connection timeout");

        let output = get_output(&buffer);
        let json = parse_json_line(output.trim());

        assert_eq!(json["type"], "error");
        assert_eq!(json["iteration"], 3);
        assert_eq!(json["code"], "EXECUTION_ERROR");
        assert_eq!(json["message"], "Connection timeout");
        assert_eq!(json["recoverable"], true);
    }

    #[test]
    fn test_iteration_end_event() {
        let (mut handler, buffer) = capture_handler();

        let result = SessionResult {
            duration_ms: 5432,
            total_cost_usd: 0.0054,
            num_turns: 3,
            is_error: false,
            ..Default::default()
        };
        handler.on_complete(&result);

        let output = get_output(&buffer);
        let json = parse_json_line(output.trim());

        assert_eq!(json["type"], "iteration_end");
        assert_eq!(json["iteration"], 3);
        assert_eq!(json["duration_ms"], 5432);
        assert_eq!(json["cost_usd"], 0.0054);
    }

    #[test]
    fn test_one_line_per_event() {
        let (mut handler, buffer) = capture_handler();

        handler.on_text("first");
        handler.on_text("second");
        handler.on_tool_call("Grep", "t1", &json!({"pattern": "test"}));

        let output = get_output(&buffer);
        let lines: Vec<&str> = output.trim().lines().collect();
        assert_eq!(lines.len(), 3);

        // Each line should be valid JSON
        for line in lines {
            let _ = parse_json_line(line);
        }
    }

    #[test]
    fn test_iteration_metadata_included() {
        let (mut handler, buffer) = capture_handler();

        // All events should include the iteration number
        handler.on_text("test");
        handler.on_error("error");

        let output = get_output(&buffer);
        for line in output.trim().lines() {
            let json = parse_json_line(line);
            assert_eq!(json["iteration"], 3, "iteration should be present");
        }
    }

    #[test]
    fn test_set_iteration_updates_subsequent_events() {
        let (mut handler, buffer) = capture_handler();

        handler.on_text("at iter 3");
        handler.set_iteration(4);
        handler.on_text("at iter 4");

        let output = get_output(&buffer);
        let lines: Vec<&str> = output.trim().lines().collect();

        let first = parse_json_line(lines[0]);
        let second = parse_json_line(lines[1]);

        assert_eq!(first["iteration"], 3);
        assert_eq!(second["iteration"], 4);
    }

    #[test]
    fn test_tool_duration_tracking() {
        let (mut handler, buffer) = capture_handler();

        handler.on_tool_call("Bash", "slow-call", &json!({"command": "sleep 0.01"}));
        std::thread::sleep(std::time::Duration::from_millis(10));
        handler.on_tool_result("slow-call", "done");

        let output = get_output(&buffer);
        let lines: Vec<&str> = output.trim().lines().collect();
        let end_json = parse_json_line(lines[1]);

        // Duration should be > 0 (we slept for 10ms)
        let duration = end_json["duration_ms"].as_u64().unwrap();
        assert!(duration >= 10, "duration should be at least 10ms");
    }

    #[test]
    fn test_unknown_tool_result_has_zero_duration() {
        let (mut handler, buffer) = capture_handler();

        // Result without prior call
        handler.on_tool_result("unknown-id", "output");

        let output = get_output(&buffer);
        let json = parse_json_line(output.trim());

        assert_eq!(json["duration_ms"], 0);
    }

    /// A writer that returns BrokenPipe on every write, simulating a disconnected consumer.
    struct BrokenPipeWriter {
        write_attempts: std::cell::Cell<u32>,
    }

    impl BrokenPipeWriter {
        fn new() -> Self {
            Self {
                write_attempts: std::cell::Cell::new(0),
            }
        }

        fn attempts(&self) -> u32 {
            self.write_attempts.get()
        }
    }

    impl Write for BrokenPipeWriter {
        fn write(&mut self, _buf: &[u8]) -> io::Result<usize> {
            self.write_attempts.set(self.write_attempts.get() + 1);
            Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "Broken pipe (os error 32)",
            ))
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn test_broken_pipe_stops_emitting_after_first_failure() {
        let writer = Arc::new(Mutex::new(BrokenPipeWriter::new()));
        let mut handler = JsonRpcStreamHandler::new(
            writer.clone(),
            1,
            Some("builder".to_string()),
            Some("claude".to_string()),
        );

        // Emit many events — simulates the log spam from the bug report
        for i in 0..10 {
            handler.on_text(&format!("event {i}"));
        }

        let attempts = writer.lock().unwrap().attempts();
        // BUG: currently all 10 writes are attempted, producing 10 WARN logs.
        // After fix, only 1 write should be attempted before the handler stops.
        assert_eq!(
            attempts, 1,
            "should stop writing after first broken pipe, but attempted {attempts} writes"
        );
    }
}
