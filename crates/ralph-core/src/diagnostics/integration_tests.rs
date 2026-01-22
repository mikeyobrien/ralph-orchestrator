//! Integration tests for diagnostics in EventLoop.

#[cfg(test)]
mod tests {
    use crate::config::RalphConfig;
    use crate::diagnostics::DiagnosticsCollector;
    use crate::event_loop::EventLoop;
    use std::fs::File;
    use std::io::{BufRead, BufReader};
    use tempfile::TempDir;

    #[test]
    fn test_event_loop_logs_iteration_started() {
        let temp_dir = TempDir::new().unwrap();

        let config = RalphConfig::default();
        let diagnostics = DiagnosticsCollector::with_enabled(temp_dir.path(), true).unwrap();
        let mut event_loop = EventLoop::with_diagnostics(config, diagnostics);

        // Simulate processing output (which increments iteration)
        event_loop.process_output(&"ralph".into(), "some output", true);

        // Verify orchestration.jsonl was created and contains IterationStarted
        let diagnostics_dir = temp_dir.path().join(".ralph").join("diagnostics");

        // Find the session directory (timestamped)
        let session_dirs: Vec<_> = std::fs::read_dir(&diagnostics_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .collect();

        assert_eq!(
            session_dirs.len(),
            1,
            "Expected exactly one session directory"
        );

        let session_dir = session_dirs[0].path();
        let orchestration_file = session_dir.join("orchestration.jsonl");
        assert!(
            orchestration_file.exists(),
            "orchestration.jsonl should exist"
        );

        // Read and verify entries
        let file = File::open(orchestration_file).unwrap();
        let reader = BufReader::new(file);
        let lines: Vec<_> = reader.lines().map(|l| l.unwrap()).collect();

        assert!(!lines.is_empty(), "Should have at least one log entry");

        // First entry should be IterationStarted
        let first_entry: serde_json::Value = serde_json::from_str(&lines[0]).unwrap();
        assert_eq!(first_entry["event"]["type"], "iteration_started");
        assert_eq!(first_entry["iteration"], 1);
    }

    #[test]
    fn test_event_loop_logs_hat_selected() {
        let temp_dir = TempDir::new().unwrap();

        let config = RalphConfig::default();
        let diagnostics = DiagnosticsCollector::with_enabled(temp_dir.path(), true).unwrap();
        let mut event_loop = EventLoop::with_diagnostics(config, diagnostics);

        // Process output which should trigger hat selection logging
        event_loop.process_output(&"ralph".into(), "some output", true);

        let diagnostics_dir = temp_dir.path().join(".ralph").join("diagnostics");
        let session_dirs: Vec<_> = std::fs::read_dir(&diagnostics_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .collect();
        let session_dir = session_dirs[0].path();
        let orchestration_file = session_dir.join("orchestration.jsonl");

        let file = File::open(orchestration_file).unwrap();
        let reader = BufReader::new(file);
        let lines: Vec<_> = reader.lines().map(|l| l.unwrap()).collect();

        // Should have HatSelected event
        let has_hat_selected = lines.iter().any(|line| {
            let entry: serde_json::Value = serde_json::from_str(line).unwrap();
            entry["event"]["type"] == "hat_selected"
        });

        assert!(has_hat_selected, "Should log hat_selected event");
    }

    #[test]
    fn test_event_loop_logs_event_published() {
        let temp_dir = TempDir::new().unwrap();

        let config = RalphConfig::default();
        let diagnostics = DiagnosticsCollector::with_enabled(temp_dir.path(), true).unwrap();
        let mut event_loop = EventLoop::with_diagnostics(config, diagnostics);

        // Process output with an event
        let output = r#"<event topic="build.start">Starting build</event>"#;
        event_loop.process_output(&"ralph".into(), output, true);

        let diagnostics_dir = temp_dir.path().join(".ralph").join("diagnostics");
        let session_dirs: Vec<_> = std::fs::read_dir(&diagnostics_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .collect();
        let session_dir = session_dirs[0].path();
        let orchestration_file = session_dir.join("orchestration.jsonl");

        let file = File::open(orchestration_file).unwrap();
        let reader = BufReader::new(file);
        let lines: Vec<_> = reader.lines().map(|l| l.unwrap()).collect();

        // Should have EventPublished
        let has_event_published = lines.iter().any(|line| {
            let entry: serde_json::Value = serde_json::from_str(line).unwrap();
            entry["event"]["type"] == "event_published" && entry["event"]["topic"] == "build.start"
        });

        assert!(has_event_published, "Should log event_published");
    }

    #[test]
    fn test_event_loop_logs_backpressure_triggered() {
        let temp_dir = TempDir::new().unwrap();

        let config = RalphConfig::default();
        let diagnostics = DiagnosticsCollector::with_enabled(temp_dir.path(), true).unwrap();
        let mut event_loop = EventLoop::with_diagnostics(config, diagnostics);

        // Process output with build.done but missing backpressure evidence
        let output = r#"<event topic="build.done">Done</event>"#;
        event_loop.process_output(&"builder".into(), output, true);

        let diagnostics_dir = temp_dir.path().join(".ralph").join("diagnostics");
        let session_dirs: Vec<_> = std::fs::read_dir(&diagnostics_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .collect();
        let session_dir = session_dirs[0].path();
        let orchestration_file = session_dir.join("orchestration.jsonl");

        let file = File::open(orchestration_file).unwrap();
        let reader = BufReader::new(file);
        let lines: Vec<_> = reader.lines().map(|l| l.unwrap()).collect();

        // Should have BackpressureTriggered
        let has_backpressure = lines.iter().any(|line| {
            let entry: serde_json::Value = serde_json::from_str(line).unwrap();
            entry["event"]["type"] == "backpressure_triggered"
        });

        assert!(has_backpressure, "Should log backpressure_triggered");
    }

    #[test]
    fn test_event_loop_logs_loop_terminated() {
        let temp_dir = TempDir::new().unwrap();

        // Create a scratchpad with no pending tasks (all done) in temp directory
        let agent_dir = temp_dir.path().join(".agent");
        std::fs::create_dir_all(&agent_dir).unwrap();
        let scratchpad_path = agent_dir.join("scratchpad.md");
        std::fs::write(&scratchpad_path, "- [x] Task 1 done\n- [x] Task 2 done\n").unwrap();

        // Configure event loop to use temp directory scratchpad
        let mut config = RalphConfig::default();
        config.core.scratchpad = scratchpad_path.to_string_lossy().to_string();

        let diagnostics = DiagnosticsCollector::with_enabled(temp_dir.path(), true).unwrap();
        let mut event_loop = EventLoop::with_diagnostics(config, diagnostics);

        // Process output with completion promise twice (requires consecutive confirmation)
        event_loop.process_output(&"ralph".into(), "LOOP_COMPLETE", true);
        event_loop.process_output(&"ralph".into(), "LOOP_COMPLETE", true);

        let diagnostics_dir = temp_dir.path().join(".ralph").join("diagnostics");
        let session_dirs: Vec<_> = std::fs::read_dir(&diagnostics_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .collect();
        let session_dir = session_dirs[0].path();
        let orchestration_file = session_dir.join("orchestration.jsonl");

        let file = File::open(orchestration_file).unwrap();
        let reader = BufReader::new(file);
        let lines: Vec<_> = reader.lines().map(|l| l.unwrap()).collect();

        // Should have LoopTerminated
        let has_terminated = lines.iter().any(|line| {
            let entry: serde_json::Value = serde_json::from_str(line).unwrap();
            entry["event"]["type"] == "loop_terminated"
        });

        assert!(has_terminated, "Should log loop_terminated");
    }
}
