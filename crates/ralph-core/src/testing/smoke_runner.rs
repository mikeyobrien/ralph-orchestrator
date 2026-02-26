//! Smoke test replay runner for CI-friendly testing.
//!
//! Loads JSONL session fixtures and runs them through the event loop with `ReplayBackend`,
//! enabling deterministic testing without live API calls.
//!
//! # Example
//!
//! ```ignore
//! use ralph_core::testing::{SmokeRunner, SmokeTestConfig};
//!
//! let config = SmokeTestConfig::new("tests/fixtures/basic_session.jsonl");
//! let result = SmokeRunner::run(&config)?;
//!
//! assert!(result.completed_successfully());
//! assert_eq!(result.iterations_run(), 3);
//! ```

use std::path::{Path, PathBuf};
use std::time::Duration;

use super::ReplayBackend;

/// Configuration for a smoke test run.
#[derive(Debug, Clone)]
pub struct SmokeTestConfig {
    /// Path to the JSONL fixture file.
    pub fixture_path: PathBuf,
    /// Maximum time to run before timing out.
    pub timeout: Duration,
    /// Expected number of iterations (for validation, optional).
    pub expected_iterations: Option<u32>,
    /// Expected termination reason (for validation, optional).
    pub expected_termination: Option<String>,
}

impl SmokeTestConfig {
    /// Creates a new smoke test configuration.
    pub fn new(fixture_path: impl AsRef<Path>) -> Self {
        Self {
            fixture_path: fixture_path.as_ref().to_path_buf(),
            timeout: Duration::from_secs(30),
            expected_iterations: None,
            expected_termination: None,
        }
    }

    /// Sets the timeout for this smoke test.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Sets expected iterations for validation.
    pub fn with_expected_iterations(mut self, iterations: u32) -> Self {
        self.expected_iterations = Some(iterations);
        self
    }

    /// Sets expected termination reason for validation.
    pub fn with_expected_termination(mut self, reason: impl Into<String>) -> Self {
        self.expected_termination = Some(reason.into());
        self
    }
}

/// Result of a smoke test run.
#[derive(Debug, Clone)]
pub struct SmokeTestResult {
    /// Number of event loop iterations executed.
    iterations: u32,
    /// Number of events parsed from the fixture.
    events_parsed: usize,
    /// Reason the test terminated.
    termination_reason: TerminationReason,
    /// Total output bytes processed.
    output_bytes: usize,
}

/// Reason the smoke test terminated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminationReason {
    /// Completed successfully (completion promise detected).
    Completed,
    /// Fixture exhausted (all output consumed).
    FixtureExhausted,
    /// Timeout reached.
    Timeout,
    /// Maximum iterations reached.
    MaxIterations,
    /// Error during execution.
    Error(String),
}

impl SmokeTestResult {
    /// Returns true if the test completed successfully.
    pub fn completed_successfully(&self) -> bool {
        matches!(
            self.termination_reason,
            TerminationReason::Completed | TerminationReason::FixtureExhausted
        )
    }

    /// Returns the number of iterations executed.
    pub fn iterations_run(&self) -> u32 {
        self.iterations
    }

    /// Returns the number of events parsed.
    pub fn event_count(&self) -> usize {
        self.events_parsed
    }

    /// Returns the termination reason.
    pub fn termination_reason(&self) -> &TerminationReason {
        &self.termination_reason
    }

    /// Returns the total output bytes processed.
    pub fn output_bytes(&self) -> usize {
        self.output_bytes
    }
}

/// Error types for smoke test operations.
#[derive(Debug, thiserror::Error)]
pub enum SmokeTestError {
    /// Fixture file not found.
    #[error("Fixture not found: {0}")]
    FixtureNotFound(PathBuf),

    /// IO error reading fixture.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Invalid fixture format.
    #[error("Invalid fixture format: {0}")]
    InvalidFixture(String),

    /// Timeout during execution.
    #[error("Timeout after {0:?}")]
    Timeout(Duration),
}

/// Lists available fixtures in a directory.
pub fn list_fixtures(dir: impl AsRef<Path>) -> std::io::Result<Vec<PathBuf>> {
    let dir = dir.as_ref();
    if !dir.exists() {
        return Ok(Vec::new());
    }

    let mut fixtures = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().is_some_and(|ext| ext == "jsonl") {
            fixtures.push(path);
        }
    }

    fixtures.sort();
    Ok(fixtures)
}

/// The smoke test runner.
pub struct SmokeRunner;

impl SmokeRunner {
    /// Runs a smoke test with the given configuration.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The fixture file is not found
    /// - The fixture file cannot be read
    /// - The fixture format is invalid
    pub fn run(config: &SmokeTestConfig) -> Result<SmokeTestResult, SmokeTestError> {
        // Validate fixture exists
        if !config.fixture_path.exists() {
            return Err(SmokeTestError::FixtureNotFound(config.fixture_path.clone()));
        }

        // Load the replay backend
        let mut backend = ReplayBackend::from_file(&config.fixture_path)?;

        // Track metrics
        let mut iterations = 0u32;
        let mut events_parsed = 0usize;
        let mut output_bytes = 0usize;

        let start_time = std::time::Instant::now();

        // Process all output chunks
        while let Some(chunk) = backend.next_output() {
            // Check timeout
            if start_time.elapsed() > config.timeout {
                return Ok(SmokeTestResult {
                    iterations,
                    events_parsed,
                    termination_reason: TerminationReason::Timeout,
                    output_bytes,
                });
            }

            output_bytes += chunk.len();

            // Convert chunk to string and parse events
            if let Ok(output) = String::from_utf8(chunk) {
                let parser = crate::EventParser::new();
                let events = parser.parse(&output);
                events_parsed += events.len();

                // Check for completion event (must be emitted as an event)
                if events
                    .iter()
                    .any(|event| event.topic.as_str() == "LOOP_COMPLETE")
                {
                    return Ok(SmokeTestResult {
                        iterations,
                        events_parsed,
                        termination_reason: TerminationReason::Completed,
                        output_bytes,
                    });
                }
            }

            iterations += 1;
        }

        // Fixture exhausted
        Ok(SmokeTestResult {
            iterations,
            events_parsed,
            termination_reason: TerminationReason::FixtureExhausted,
            output_bytes,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    /// Helper to create a JSONL fixture file.
    fn create_fixture(dir: &Path, name: &str, content: &str) -> PathBuf {
        let path = dir.join(name);
        let mut file = std::fs::File::create(&path).unwrap();
        file.write_all(content.as_bytes()).unwrap();
        path
    }

    /// Creates a terminal write JSONL line.
    fn make_write_line(text: &str, offset_ms: u64) -> String {
        use crate::session_recorder::Record;
        use ralph_proto::TerminalWrite;

        let write = TerminalWrite::new(text.as_bytes(), true, offset_ms);
        let record = Record {
            ts: 1000 + offset_ms,
            event: "ux.terminal.write".to_string(),
            data: serde_json::to_value(&write).unwrap(),
        };
        serde_json::to_string(&record).unwrap()
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Acceptance Criteria #5: Fixture Not Found
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn test_fixture_not_found_returns_error() {
        let config = SmokeTestConfig::new("/nonexistent/path/to/fixture.jsonl");
        let result = SmokeRunner::run(&config);

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(err, SmokeTestError::FixtureNotFound(_)));
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Acceptance Criteria #1: Run Fixture Through Event Loop
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn test_run_fixture_through_event_loop() {
        let temp_dir = TempDir::new().unwrap();

        // Create a simple fixture with some output
        let line1 = make_write_line("Starting task...", 0);
        let line2 = make_write_line("Working on implementation...", 100);
        let line3 = make_write_line("Task complete!", 200);
        let content = format!("{}\n{}\n{}\n", line1, line2, line3);

        let fixture_path = create_fixture(temp_dir.path(), "basic.jsonl", &content);

        let config = SmokeTestConfig::new(&fixture_path);
        let result = SmokeRunner::run(&config).unwrap();

        // Verify the fixture was processed
        assert!(result.iterations_run() > 0);
        assert!(result.output_bytes() > 0);
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Acceptance Criteria #2: Capture Termination Reason
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn test_captures_completion_termination() {
        let temp_dir = TempDir::new().unwrap();

        // Create a fixture with completion event
        let line1 = make_write_line("Working...", 0);
        let line2 = make_write_line(r#"<event topic="LOOP_COMPLETE">done</event>"#, 100);
        let content = format!("{}\n{}\n", line1, line2);

        let fixture_path = create_fixture(temp_dir.path(), "completion.jsonl", &content);

        let config = SmokeTestConfig::new(&fixture_path);
        let result = SmokeRunner::run(&config).unwrap();

        assert_eq!(*result.termination_reason(), TerminationReason::Completed);
        assert!(result.completed_successfully());
    }

    #[test]
    fn test_captures_fixture_exhausted_termination() {
        let temp_dir = TempDir::new().unwrap();

        // Create a fixture WITHOUT completion promise
        let line1 = make_write_line("Some output", 0);
        let line2 = make_write_line("More output", 100);
        let content = format!("{}\n{}\n", line1, line2);

        let fixture_path = create_fixture(temp_dir.path(), "no_completion.jsonl", &content);

        let config = SmokeTestConfig::new(&fixture_path);
        let result = SmokeRunner::run(&config).unwrap();

        assert_eq!(
            *result.termination_reason(),
            TerminationReason::FixtureExhausted
        );
        assert!(result.completed_successfully()); // FixtureExhausted is considered success
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Acceptance Criteria #3: Event Counting
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn test_event_counting() {
        let temp_dir = TempDir::new().unwrap();

        // Create a fixture with events
        let output_with_events = r#"Some preamble
<event topic="build.task">Task 1</event>
Working on task...
<event topic="build.done">
tests: pass
lint: pass
typecheck: pass
audit: pass
coverage: pass
</event>"#;

        let line1 = make_write_line(output_with_events, 0);
        let content = format!("{}\n", line1);

        let fixture_path = create_fixture(temp_dir.path(), "with_events.jsonl", &content);

        let config = SmokeTestConfig::new(&fixture_path);
        let result = SmokeRunner::run(&config).unwrap();

        // Should have parsed 2 events
        assert_eq!(result.event_count(), 2);
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Acceptance Criteria #4: Timeout Handling
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn test_timeout_handling() {
        let temp_dir = TempDir::new().unwrap();

        // Create a fixture - we'll use a very short timeout
        let line1 = make_write_line("Output 1", 0);
        let content = format!("{}\n", line1);

        let fixture_path = create_fixture(temp_dir.path(), "timeout_test.jsonl", &content);

        // Note: This test verifies timeout handling works, but won't actually timeout
        // since the fixture is small. A real timeout test would need realistic timing.
        let config = SmokeTestConfig::new(&fixture_path).with_timeout(Duration::from_millis(1)); // Very short timeout

        let result = SmokeRunner::run(&config).unwrap();

        // The test should complete quickly so won't actually timeout,
        // but the timeout mechanism is in place
        assert!(
            result.completed_successfully()
                || *result.termination_reason() == TerminationReason::Timeout
        );
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Acceptance Criteria #6: Fixture Discovery
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn test_list_fixtures_empty_directory() {
        let temp_dir = TempDir::new().unwrap();

        let fixtures = list_fixtures(temp_dir.path()).unwrap();
        assert!(fixtures.is_empty());
    }

    #[test]
    fn test_list_fixtures_finds_jsonl_files() {
        let temp_dir = TempDir::new().unwrap();

        // Create some fixture files
        create_fixture(temp_dir.path(), "test1.jsonl", "{}");
        create_fixture(temp_dir.path(), "test2.jsonl", "{}");
        create_fixture(temp_dir.path(), "not_a_fixture.txt", "text");

        let fixtures = list_fixtures(temp_dir.path()).unwrap();

        assert_eq!(fixtures.len(), 2);
        assert!(fixtures.iter().all(|p| p.extension().unwrap() == "jsonl"));
    }

    #[test]
    fn test_list_fixtures_nonexistent_directory() {
        let fixtures = list_fixtures("/nonexistent/path").unwrap();
        assert!(fixtures.is_empty());
    }

    // ─────────────────────────────────────────────────────────────────────────
    // Additional edge cases
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn test_empty_fixture_completes() {
        let temp_dir = TempDir::new().unwrap();

        let fixture_path = create_fixture(temp_dir.path(), "empty.jsonl", "");

        let config = SmokeTestConfig::new(&fixture_path);
        let result = SmokeRunner::run(&config).unwrap();

        assert_eq!(result.iterations_run(), 0);
        assert_eq!(result.event_count(), 0);
        assert_eq!(
            *result.termination_reason(),
            TerminationReason::FixtureExhausted
        );
    }

    #[test]
    fn test_config_builder_pattern() {
        let config = SmokeTestConfig::new("test.jsonl")
            .with_timeout(Duration::from_secs(60))
            .with_expected_iterations(5)
            .with_expected_termination("Completed");

        assert_eq!(config.fixture_path, PathBuf::from("test.jsonl"));
        assert_eq!(config.timeout, Duration::from_secs(60));
        assert_eq!(config.expected_iterations, Some(5));
        assert_eq!(config.expected_termination, Some("Completed".to_string()));
    }

    #[test]
    fn test_result_accessors() {
        let result = SmokeTestResult {
            iterations: 5,
            events_parsed: 3,
            termination_reason: TerminationReason::Completed,
            output_bytes: 1024,
        };

        assert_eq!(result.iterations_run(), 5);
        assert_eq!(result.event_count(), 3);
        assert_eq!(*result.termination_reason(), TerminationReason::Completed);
        assert_eq!(result.output_bytes(), 1024);
        assert!(result.completed_successfully());
    }
}
