//! Diagnostic stream handler wrapper.

use crate::diagnostics::agent_output::AgentOutputLogger;
use std::sync::{Arc, Mutex};

/// Wrapper that logs agent output while delegating to inner handler.
///
/// Note: Fields are used in the test module's StreamHandler implementation.
/// This struct is scaffolded for future production integration with ralph-adapters.
#[allow(dead_code)]
pub struct DiagnosticStreamHandler<H> {
    inner: H,
    logger: Arc<Mutex<AgentOutputLogger>>,
}

impl<H> DiagnosticStreamHandler<H> {
    /// Creates a new diagnostic stream handler wrapper.
    pub fn new(inner: H, logger: Arc<Mutex<AgentOutputLogger>>) -> Self {
        Self { inner, logger }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostics::agent_output::{AgentOutputContent, AgentOutputEntry};
    use std::fs::File;
    use std::io::{BufRead, BufReader};
    use tempfile::TempDir;

    // Mock StreamHandler for testing
    struct MockStreamHandler {
        text_calls: Arc<Mutex<Vec<String>>>,
        tool_calls: Arc<Mutex<Vec<String>>>,
        tool_results: Arc<Mutex<Vec<String>>>,
        errors: Arc<Mutex<Vec<String>>>,
        completes: Arc<Mutex<usize>>,
    }

    impl MockStreamHandler {
        fn new() -> Self {
            Self {
                text_calls: Arc::new(Mutex::new(Vec::new())),
                tool_calls: Arc::new(Mutex::new(Vec::new())),
                tool_results: Arc::new(Mutex::new(Vec::new())),
                errors: Arc::new(Mutex::new(Vec::new())),
                completes: Arc::new(Mutex::new(0)),
            }
        }
    }

    // We need to import StreamHandler trait - but it's in ralph-adapters
    // For now, define a minimal trait for testing
    trait StreamHandler: Send {
        fn on_text(&mut self, text: &str);
        fn on_tool_call(&mut self, name: &str, id: &str, input: &serde_json::Value);
        fn on_tool_result(&mut self, id: &str, output: &str);
        fn on_error(&mut self, error: &str);
        fn on_complete(&mut self, result: &SessionResult);
    }

    /// Mock session result for testing - fields used via trait signature.
    #[derive(Debug)]
    #[allow(dead_code)]
    struct SessionResult {
        is_error: bool,
        duration_ms: u64,
        total_cost_usd: f64,
        num_turns: u32,
    }

    impl StreamHandler for MockStreamHandler {
        fn on_text(&mut self, text: &str) {
            self.text_calls.lock().unwrap().push(text.to_string());
        }

        fn on_tool_call(&mut self, name: &str, id: &str, _input: &serde_json::Value) {
            self.tool_calls
                .lock()
                .unwrap()
                .push(format!("{}:{}", name, id));
        }

        fn on_tool_result(&mut self, id: &str, output: &str) {
            self.tool_results
                .lock()
                .unwrap()
                .push(format!("{}:{}", id, output));
        }

        fn on_error(&mut self, error: &str) {
            self.errors.lock().unwrap().push(error.to_string());
        }

        fn on_complete(&mut self, _result: &SessionResult) {
            *self.completes.lock().unwrap() += 1;
        }
    }

    // NOTE: DiagnosticStreamHandler<H: StreamHandler> implementation will go here
    // after tests fail (GREEN phase)

    impl<H: StreamHandler> StreamHandler for DiagnosticStreamHandler<H> {
        fn on_text(&mut self, text: &str) {
            let _ = self.logger.lock().unwrap().log(AgentOutputContent::Text {
                text: text.to_string(),
            });
            self.inner.on_text(text);
        }

        fn on_tool_call(&mut self, name: &str, id: &str, input: &serde_json::Value) {
            let _ = self
                .logger
                .lock()
                .unwrap()
                .log(AgentOutputContent::ToolCall {
                    name: name.to_string(),
                    id: id.to_string(),
                    input: input.clone(),
                });
            self.inner.on_tool_call(name, id, input);
        }

        fn on_tool_result(&mut self, id: &str, output: &str) {
            let _ = self
                .logger
                .lock()
                .unwrap()
                .log(AgentOutputContent::ToolResult {
                    id: id.to_string(),
                    output: output.to_string(),
                });
            self.inner.on_tool_result(id, output);
        }

        fn on_error(&mut self, error: &str) {
            let _ = self.logger.lock().unwrap().log(AgentOutputContent::Error {
                message: error.to_string(),
            });
            self.inner.on_error(error);
        }

        fn on_complete(&mut self, result: &SessionResult) {
            let _ = self
                .logger
                .lock()
                .unwrap()
                .log(AgentOutputContent::Complete {
                    input_tokens: None,
                    output_tokens: None,
                });
            self.inner.on_complete(result);
        }
    }

    #[test]
    fn test_wrapper_calls_inner_handler() {
        let temp = TempDir::new().unwrap();
        let logger = Arc::new(Mutex::new(AgentOutputLogger::new(temp.path()).unwrap()));
        logger.lock().unwrap().set_context(1, "ralph");

        let mock = MockStreamHandler::new();
        let text_calls = mock.text_calls.clone();
        let tool_calls = mock.tool_calls.clone();
        let errors = mock.errors.clone();

        let mut wrapper = DiagnosticStreamHandler::new(mock, logger);

        wrapper.on_text("Hello");
        wrapper.on_tool_call("Read", "t1", &serde_json::json!({"file": "test.rs"}));
        wrapper.on_error("Failed");

        // Verify inner handler was called
        assert_eq!(text_calls.lock().unwrap().len(), 1);
        assert_eq!(text_calls.lock().unwrap()[0], "Hello");

        assert_eq!(tool_calls.lock().unwrap().len(), 1);
        assert_eq!(tool_calls.lock().unwrap()[0], "Read:t1");

        assert_eq!(errors.lock().unwrap().len(), 1);
        assert_eq!(errors.lock().unwrap()[0], "Failed");
    }

    #[test]
    fn test_wrapper_logs_all_events() {
        let temp = TempDir::new().unwrap();
        let logger = Arc::new(Mutex::new(AgentOutputLogger::new(temp.path()).unwrap()));
        logger.lock().unwrap().set_context(1, "ralph");

        let mock = MockStreamHandler::new();
        let mut wrapper = DiagnosticStreamHandler::new(mock, logger);

        wrapper.on_text("Building");
        wrapper.on_tool_call("Execute", "t1", &serde_json::json!({"cmd": "cargo test"}));
        wrapper.on_tool_result("t1", "Tests passed");
        wrapper.on_error("Parse error");
        wrapper.on_complete(&SessionResult {
            is_error: false,
            duration_ms: 1000,
            total_cost_usd: 0.05,
            num_turns: 3,
        });

        drop(wrapper);

        // Verify all events were logged
        let file = File::open(temp.path().join("agent-output.jsonl")).unwrap();
        let reader = BufReader::new(file);
        let lines: Vec<String> = reader.lines().map(|l| l.unwrap()).collect();

        assert_eq!(lines.len(), 5);

        // Verify types
        let entries: Vec<AgentOutputEntry> = lines
            .iter()
            .map(|l| serde_json::from_str(l).unwrap())
            .collect();

        assert!(matches!(
            entries[0].content,
            AgentOutputContent::Text { .. }
        ));
        assert!(matches!(
            entries[1].content,
            AgentOutputContent::ToolCall { .. }
        ));
        assert!(matches!(
            entries[2].content,
            AgentOutputContent::ToolResult { .. }
        ));
        assert!(matches!(
            entries[3].content,
            AgentOutputContent::Error { .. }
        ));
        assert!(matches!(
            entries[4].content,
            AgentOutputContent::Complete { .. }
        ));
    }

    #[test]
    fn test_thread_safety() {
        use std::thread;

        let temp = TempDir::new().unwrap();
        let logger = Arc::new(Mutex::new(AgentOutputLogger::new(temp.path()).unwrap()));
        logger.lock().unwrap().set_context(1, "ralph");

        let logger1 = logger.clone();
        let logger2 = logger.clone();

        let handle1 = thread::spawn(move || {
            let mock = MockStreamHandler::new();
            let mut wrapper = DiagnosticStreamHandler::new(mock, logger1);
            for i in 0..10 {
                wrapper.on_text(&format!("Thread1-{}", i));
            }
        });

        let handle2 = thread::spawn(move || {
            let mock = MockStreamHandler::new();
            let mut wrapper = DiagnosticStreamHandler::new(mock, logger2);
            for i in 0..10 {
                wrapper.on_text(&format!("Thread2-{}", i));
            }
        });

        handle1.join().unwrap();
        handle2.join().unwrap();

        // Verify no panics and all entries logged
        let file = File::open(temp.path().join("agent-output.jsonl")).unwrap();
        let reader = BufReader::new(file);
        let lines: Vec<String> = reader.lines().map(|l| l.unwrap()).collect();

        assert_eq!(lines.len(), 20);
    }
}
