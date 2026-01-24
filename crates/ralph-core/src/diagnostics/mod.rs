//! Diagnostic logging system for Ralph orchestration.
//!
//! Captures agent output, orchestration decisions, traces, performance metrics,
//! and errors to structured JSONL files when `RALPH_DIAGNOSTICS=1` is set.

mod agent_output;
mod errors;
mod orchestration;
mod performance;
mod stream_handler;
mod trace_layer;

#[cfg(test)]
mod integration_tests;

pub use agent_output::{AgentOutputContent, AgentOutputEntry, AgentOutputLogger};
pub use errors::{DiagnosticError, ErrorLogger};
pub use orchestration::{OrchestrationEvent, OrchestrationLogger};
pub use performance::{PerformanceLogger, PerformanceMetric};
pub use stream_handler::DiagnosticStreamHandler;
pub use trace_layer::{DiagnosticTraceLayer, TraceEntry};

use chrono::Local;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

/// Global storage for the shared session directory path.
/// This allows the trace layer and EventLoop to share the same session directory.
static SHARED_SESSION_DIR: OnceLock<PathBuf> = OnceLock::new();

/// Central coordinator for diagnostic logging.
///
/// Checks `RALPH_DIAGNOSTICS` environment variable and creates a timestamped
/// session directory if enabled.
pub struct DiagnosticsCollector {
    enabled: bool,
    session_dir: Option<PathBuf>,
    orchestration_logger: Option<Arc<Mutex<orchestration::OrchestrationLogger>>>,
    performance_logger: Option<Arc<Mutex<performance::PerformanceLogger>>>,
    error_logger: Option<Arc<Mutex<errors::ErrorLogger>>>,
}

impl DiagnosticsCollector {
    /// Creates a new diagnostics collector.
    ///
    /// If `RALPH_DIAGNOSTICS=1`, creates `.ralph/diagnostics/<timestamp>/` directory.
    /// If a shared session directory has been set via `set_shared_session_dir`, reuses
    /// that directory instead of creating a new one.
    pub fn new(base_path: &Path) -> std::io::Result<Self> {
        let enabled = std::env::var("RALPH_DIAGNOSTICS")
            .map(|v| v == "1")
            .unwrap_or(false);

        // Check if we should reuse an existing session directory
        if enabled
            && let Some(session_dir) = SHARED_SESSION_DIR.get()
            && session_dir.exists()
        {
            return Self::for_session(session_dir);
        }

        Self::with_enabled(base_path, enabled)
    }

    /// Sets the shared session directory for use by other components.
    ///
    /// This should be called once during initialization (e.g., in main() after
    /// setting up the trace layer) so that the EventLoop can reuse the same
    /// session directory.
    ///
    /// Returns `Ok(())` if the directory was set, or `Err` with the path if
    /// it was already set (OnceLock can only be set once).
    pub fn set_shared_session_dir(session_dir: PathBuf) -> Result<(), PathBuf> {
        SHARED_SESSION_DIR.set(session_dir)
    }

    /// Creates a diagnostics collector that uses an existing session directory.
    ///
    /// This is used when the session directory was already created (e.g., by the trace layer)
    /// and we want to share it with other components like the EventLoop.
    pub fn for_session(session_dir: &Path) -> std::io::Result<Self> {
        let orch_logger = orchestration::OrchestrationLogger::new(session_dir)?;
        let perf_logger = performance::PerformanceLogger::new(session_dir)?;
        let err_logger = errors::ErrorLogger::new(session_dir)?;

        Ok(Self {
            enabled: true,
            session_dir: Some(session_dir.to_path_buf()),
            orchestration_logger: Some(Arc::new(Mutex::new(orch_logger))),
            performance_logger: Some(Arc::new(Mutex::new(perf_logger))),
            error_logger: Some(Arc::new(Mutex::new(err_logger))),
        })
    }

    /// Creates a diagnostics collector with explicit enabled flag (for testing).
    pub fn with_enabled(base_path: &Path, enabled: bool) -> std::io::Result<Self> {
        let (session_dir, orchestration_logger, performance_logger, error_logger) = if enabled {
            let timestamp = Local::now().format("%Y-%m-%dT%H-%M-%S");
            let dir = base_path
                .join(".ralph")
                .join("diagnostics")
                .join(timestamp.to_string());
            fs::create_dir_all(&dir)?;

            let orch_logger = orchestration::OrchestrationLogger::new(&dir)?;
            let perf_logger = performance::PerformanceLogger::new(&dir)?;
            let err_logger = errors::ErrorLogger::new(&dir)?;
            (
                Some(dir),
                Some(Arc::new(Mutex::new(orch_logger))),
                Some(Arc::new(Mutex::new(perf_logger))),
                Some(Arc::new(Mutex::new(err_logger))),
            )
        } else {
            (None, None, None, None)
        };

        Ok(Self {
            enabled,
            session_dir,
            orchestration_logger,
            performance_logger,
            error_logger,
        })
    }

    /// Creates a disabled diagnostics collector without any I/O (for testing).
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            session_dir: None,
            orchestration_logger: None,
            performance_logger: None,
            error_logger: None,
        }
    }

    /// Returns whether diagnostics are enabled.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Returns the session directory if diagnostics are enabled.
    pub fn session_dir(&self) -> Option<&Path> {
        self.session_dir.as_deref()
    }

    /// Wraps a stream handler with diagnostic logging.
    ///
    /// Returns the original handler if diagnostics are disabled.
    pub fn wrap_stream_handler<H>(&self, handler: H) -> Result<DiagnosticStreamHandler<H>, H> {
        if let Some(session_dir) = &self.session_dir {
            match AgentOutputLogger::new(session_dir) {
                Ok(logger) => {
                    let logger = Arc::new(Mutex::new(logger));
                    Ok(DiagnosticStreamHandler::new(handler, logger))
                }
                Err(_) => Err(handler), // Return original handler on error
            }
        } else {
            Err(handler) // Diagnostics disabled, return original
        }
    }

    /// Logs an orchestration event.
    ///
    /// Does nothing if diagnostics are disabled.
    pub fn log_orchestration(&self, iteration: u32, hat: &str, event: OrchestrationEvent) {
        if let Some(logger) = &self.orchestration_logger
            && let Ok(mut logger) = logger.lock()
        {
            let _ = logger.log(iteration, hat, event);
        }
    }

    /// Logs a performance metric.
    ///
    /// Does nothing if diagnostics are disabled.
    pub fn log_performance(&self, iteration: u32, hat: &str, metric: PerformanceMetric) {
        if let Some(logger) = &self.performance_logger
            && let Ok(mut logger) = logger.lock()
        {
            let _ = logger.log(iteration, hat, metric);
        }
    }

    /// Logs an error.
    ///
    /// Does nothing if diagnostics are disabled.
    pub fn log_error(&self, iteration: u32, hat: &str, error: DiagnosticError) {
        if let Some(logger) = &self.error_logger
            && let Ok(mut logger) = logger.lock()
        {
            logger.set_context(iteration, hat);
            logger.log(error);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_diagnostics_disabled_by_default() {
        let temp = TempDir::new().unwrap();

        let collector = DiagnosticsCollector::with_enabled(temp.path(), false).unwrap();

        assert!(!collector.is_enabled());
        assert!(collector.session_dir().is_none());
    }

    #[test]
    fn test_diagnostics_enabled_creates_directory() {
        let temp = TempDir::new().unwrap();

        let collector = DiagnosticsCollector::with_enabled(temp.path(), true).unwrap();

        assert!(collector.is_enabled());
        assert!(collector.session_dir().is_some());
        assert!(collector.session_dir().unwrap().exists());
    }

    #[test]
    fn test_session_directory_format() {
        let temp = TempDir::new().unwrap();

        let collector = DiagnosticsCollector::with_enabled(temp.path(), true).unwrap();

        let session_dir = collector.session_dir().unwrap();
        let dir_name = session_dir.file_name().unwrap().to_str().unwrap();

        // Verify format: YYYY-MM-DDTHH-MM-SS
        assert!(dir_name.len() == 19); // 2024-01-21T08-49-56
        assert!(dir_name.chars().nth(4) == Some('-'));
        assert!(dir_name.chars().nth(7) == Some('-'));
        assert!(dir_name.chars().nth(10) == Some('T'));
        assert!(dir_name.chars().nth(13) == Some('-'));
        assert!(dir_name.chars().nth(16) == Some('-'));
    }

    #[test]
    fn test_performance_logger_integration() {
        let temp = TempDir::new().unwrap();
        let collector = DiagnosticsCollector::with_enabled(temp.path(), true).unwrap();

        // Log some performance metrics
        collector.log_performance(
            1,
            "ralph",
            PerformanceMetric::IterationDuration { duration_ms: 1500 },
        );
        collector.log_performance(
            1,
            "builder",
            PerformanceMetric::AgentLatency { duration_ms: 800 },
        );
        collector.log_performance(
            1,
            "builder",
            PerformanceMetric::TokenCount {
                input: 1000,
                output: 500,
            },
        );

        // Verify file exists
        let perf_file = collector.session_dir().unwrap().join("performance.jsonl");
        assert!(perf_file.exists(), "performance.jsonl should exist");

        // Verify content
        let content = std::fs::read_to_string(perf_file).unwrap();
        let lines: Vec<_> = content.lines().collect();
        assert_eq!(lines.len(), 3, "Should have 3 performance entries");

        // Verify each line is valid JSON
        for line in lines {
            let _: performance::PerformanceEntry = serde_json::from_str(line).unwrap();
        }
    }

    #[test]
    fn test_error_logger_integration() {
        let temp = TempDir::new().unwrap();
        let collector = DiagnosticsCollector::with_enabled(temp.path(), true).unwrap();

        // Log some errors
        collector.log_error(
            1,
            "ralph",
            DiagnosticError::ParseError {
                source: "agent_output".to_string(),
                message: "Invalid JSON".to_string(),
                input: "{invalid".to_string(),
            },
        );
        collector.log_error(
            2,
            "builder",
            DiagnosticError::ValidationFailure {
                rule: "tests_required".to_string(),
                message: "Missing test evidence".to_string(),
                evidence: "tests: missing".to_string(),
            },
        );

        // Verify file exists
        let error_file = collector.session_dir().unwrap().join("errors.jsonl");
        assert!(error_file.exists(), "errors.jsonl should exist");

        // Verify content
        let content = std::fs::read_to_string(error_file).unwrap();
        let lines: Vec<_> = content.lines().collect();
        assert_eq!(lines.len(), 2, "Should have 2 error entries");

        // Verify each line is valid JSON
        for line in lines {
            let parsed: serde_json::Value = serde_json::from_str(line).unwrap();
            assert!(parsed.get("error_type").is_some());
            assert!(parsed.get("message").is_some());
            assert!(parsed.get("context").is_some());
        }
    }
}
