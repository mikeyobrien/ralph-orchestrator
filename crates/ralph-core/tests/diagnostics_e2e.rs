use ralph_core::diagnostics::DiagnosticsCollector;
use std::fs;
use tempfile::TempDir;

#[test]
fn test_diagnostics_creates_log_files() {
    // Setup: Create temp directory and enable diagnostics
    let temp_dir = TempDir::new().unwrap();

    // Create diagnostics collector with explicit enabled flag
    let collector = DiagnosticsCollector::with_enabled(temp_dir.path(), true).unwrap();
    assert!(collector.is_enabled());

    let session_dir = collector.session_dir().unwrap();
    assert!(session_dir.exists());

    // Log entries to create files
    collector.log_orchestration(
        1,
        "test-hat",
        ralph_core::diagnostics::OrchestrationEvent::IterationStarted,
    );

    collector.log_performance(
        1,
        "test-hat",
        ralph_core::diagnostics::PerformanceMetric::IterationDuration { duration_ms: 100 },
    );

    collector.log_error(
        1,
        "test-hat",
        ralph_core::diagnostics::DiagnosticError::ParseError {
            source: "test".to_string(),
            message: "test error".to_string(),
            input: "bad input".to_string(),
        },
    );

    // Verify files exist
    let expected_files = vec!["orchestration.jsonl", "performance.jsonl", "errors.jsonl"];

    for file in &expected_files {
        let file_path = session_dir.join(file);
        assert!(
            file_path.exists(),
            "Expected file {} to exist",
            file_path.display()
        );
    }
}

#[test]
fn test_diagnostics_files_contain_valid_jsonl() {
    // Setup
    let temp_dir = TempDir::new().unwrap();

    let collector = DiagnosticsCollector::with_enabled(temp_dir.path(), true).unwrap();
    let session_dir = collector.session_dir().unwrap();

    // Write multiple entries to each file
    for i in 1..=3 {
        collector.log_orchestration(
            i,
            "test-hat",
            ralph_core::diagnostics::OrchestrationEvent::IterationStarted,
        );

        collector.log_performance(
            i,
            "test-hat",
            ralph_core::diagnostics::PerformanceMetric::IterationDuration {
                duration_ms: u64::from(i) * 100,
            },
        );

        collector.log_error(
            i,
            "test-hat",
            ralph_core::diagnostics::DiagnosticError::ParseError {
                source: "test".to_string(),
                message: format!("error {}", i),
                input: "bad".to_string(),
            },
        );
    }

    // Verify each file contains valid JSONL (3 lines each)
    let files = vec!["orchestration.jsonl", "performance.jsonl", "errors.jsonl"];

    for file in files {
        let file_path = session_dir.join(file);
        let content = fs::read_to_string(&file_path).unwrap();
        let lines: Vec<&str> = content.lines().collect();

        assert_eq!(
            lines.len(),
            3,
            "Expected 3 lines in {}, got {}",
            file,
            lines.len()
        );

        // Verify each line is valid JSON
        for (i, line) in lines.iter().enumerate() {
            serde_json::from_str::<serde_json::Value>(line)
                .unwrap_or_else(|e| panic!("Invalid JSON in {} line {}: {}", file, i + 1, e));
        }
    }
}

#[test]
fn test_diagnostics_disabled_when_env_not_set() {
    let temp_dir = TempDir::new().unwrap();
    let collector = DiagnosticsCollector::with_enabled(temp_dir.path(), false).unwrap();

    assert!(!collector.is_enabled());
    assert!(collector.session_dir().is_none());

    // Logging should be no-op when disabled
    collector.log_orchestration(
        1,
        "test-hat",
        ralph_core::diagnostics::OrchestrationEvent::IterationStarted,
    );

    // No files should be created
    let diagnostics_dir = temp_dir.path().join(".ralph/diagnostics");
    assert!(!diagnostics_dir.exists());
}
