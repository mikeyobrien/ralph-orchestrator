//! Mock CLI backend for deterministic testing.

use std::sync::{Arc, Mutex};

/// Mock backend that returns pre-scripted responses.
#[derive(Debug, Clone)]
pub struct MockBackend {
    responses: Arc<Mutex<MockState>>,
}

#[derive(Debug)]
struct MockState {
    responses: Vec<String>,
    current: usize,
    executions: Vec<ExecutionRecord>,
}

/// Record of a mock execution.
#[derive(Debug, Clone)]
pub struct ExecutionRecord {
    pub prompt: String,
    pub response: String,
}

impl MockBackend {
    /// Creates a new mock backend with scripted responses.
    pub fn new(responses: Vec<String>) -> Self {
        Self {
            responses: Arc::new(Mutex::new(MockState {
                responses,
                current: 0,
                executions: Vec::new(),
            })),
        }
    }

    /// Executes a prompt, returning the next scripted response.
    pub fn execute(&self, prompt: &str) -> String {
        let mut state = self.responses.lock().unwrap();
        let response = state
            .responses
            .get(state.current)
            .cloned()
            .unwrap_or_else(String::new);

        state.executions.push(ExecutionRecord {
            prompt: prompt.to_string(),
            response: response.clone(),
        });

        state.current += 1;
        response
    }

    /// Returns the number of times execute was called.
    pub fn execution_count(&self) -> usize {
        self.responses.lock().unwrap().executions.len()
    }

    /// Returns all execution records.
    pub fn executions(&self) -> Vec<ExecutionRecord> {
        self.responses.lock().unwrap().executions.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mock_backend_returns_scripted_responses() {
        let backend = MockBackend::new(vec!["response 1".into(), "response 2".into()]);

        assert_eq!(backend.execute("prompt 1"), "response 1");
        assert_eq!(backend.execute("prompt 2"), "response 2");
        assert_eq!(backend.execution_count(), 2);
    }

    #[test]
    fn test_mock_backend_tracks_executions() {
        let backend = MockBackend::new(vec!["ok".into()]);
        backend.execute("test prompt");

        let executions = backend.executions();
        assert_eq!(executions.len(), 1);
        assert_eq!(executions[0].prompt, "test prompt");
        assert_eq!(executions[0].response, "ok");
    }
}
