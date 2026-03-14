//! Integration tests for diagnostics in EventLoop.

#[cfg(test)]
mod tests {
    use crate::config::RalphConfig;
    use crate::diagnostics::{DiagnosticsCollector, HookDisposition, HookRunTelemetryEntry};
    use crate::event_loop::EventLoop;
    use crate::hooks::{HookRunResult, HookStreamOutput, HookSuspendMode};
    use chrono::{TimeZone, Utc};
    use std::fs::File;
    use std::io::{BufRead, BufReader};
    use tempfile::TempDir;

    fn fixed_time(hour: u32, minute: u32, second: u32) -> chrono::DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 2, 28, hour, minute, second)
            .single()
            .expect("fixed timestamp")
    }

    fn sample_hook_telemetry_entry(disposition: HookDisposition) -> HookRunTelemetryEntry {
        let run_result = HookRunResult {
            started_at: fixed_time(15, 30, 1),
            ended_at: fixed_time(15, 30, 2),
            duration_ms: 923,
            exit_code: Some(13),
            timed_out: false,
            stdout: HookStreamOutput {
                content: "stdout-truncated".to_string(),
                truncated: true,
            },
            stderr: HookStreamOutput {
                content: "stderr-clean".to_string(),
                truncated: false,
            },
        };

        HookRunTelemetryEntry::from_run_result(
            "loop-telemetry-123",
            "pre.loop.start",
            "env-guard",
            disposition,
            HookSuspendMode::RetryBackoff,
            3,
            4,
            &run_result,
        )
    }

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

    /// Helper to write an event to a JSONL file for testing.
    fn write_event_to_jsonl(path: &std::path::Path, topic: &str, payload: &str) {
        use std::io::Write;
        let ts = chrono::Utc::now().to_rfc3339();
        let event_json = serde_json::json!({
            "topic": topic,
            "payload": payload,
            "ts": ts
        });
        let mut file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
            .unwrap();
        writeln!(file, "{}", event_json).unwrap();
    }

    #[test]
    fn test_event_loop_logs_event_published() {
        // Events now come from JSONL via `ralph emit`, not from XML in text output.
        let temp_dir = TempDir::new().unwrap();
        let events_path = temp_dir.path().join("events.jsonl");

        let config = RalphConfig::default();
        let diagnostics = DiagnosticsCollector::with_enabled(temp_dir.path(), true).unwrap();
        let mut event_loop = EventLoop::with_diagnostics(config, diagnostics);
        event_loop.event_reader = crate::event_reader::EventReader::new(&events_path);

        // Write event to JSONL file
        write_event_to_jsonl(&events_path, "build.start", "Starting build");
        let _ = event_loop.process_events_from_jsonl();

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
        // Events now come from JSONL via `ralph emit`.
        // build.done without backpressure evidence triggers backpressure.
        let temp_dir = TempDir::new().unwrap();
        let events_path = temp_dir.path().join("events.jsonl");

        let config = RalphConfig::default();
        let diagnostics = DiagnosticsCollector::with_enabled(temp_dir.path(), true).unwrap();
        let mut event_loop = EventLoop::with_diagnostics(config, diagnostics);
        event_loop.event_reader = crate::event_reader::EventReader::new(&events_path);

        // Write build.done event without backpressure evidence
        write_event_to_jsonl(&events_path, "build.done", "Done");
        let _ = event_loop.process_events_from_jsonl();

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

        let events_path = temp_dir.path().join("events.jsonl");
        event_loop.event_reader = crate::event_reader::EventReader::new(&events_path);

        let event_json = serde_json::json!({
            "topic": "LOOP_COMPLETE",
            "payload": "done",
            "ts": chrono::Utc::now().to_rfc3339()
        });
        std::fs::write(&events_path, format!("{event_json}\n")).unwrap();

        let _ = event_loop.process_events_from_jsonl();
        let _ = event_loop.check_completion_event();

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

    #[test]
    fn test_diagnostics_collector_logs_hook_run_telemetry() {
        let temp_dir = TempDir::new().unwrap();
        let collector = DiagnosticsCollector::with_enabled(temp_dir.path(), true).unwrap();

        collector.log_hook_run(sample_hook_telemetry_entry(HookDisposition::Block));

        let hook_runs_file = collector.session_dir().unwrap().join("hook-runs.jsonl");
        assert!(hook_runs_file.exists(), "hook-runs.jsonl should exist");

        let content = std::fs::read_to_string(hook_runs_file).unwrap();
        let lines: Vec<_> = content.lines().collect();
        assert_eq!(lines.len(), 1, "Should have one hook run telemetry entry");

        let entry: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
        for field in [
            "timestamp",
            "loop_id",
            "phase_event",
            "hook_name",
            "started_at",
            "ended_at",
            "duration_ms",
            "exit_code",
            "timed_out",
            "stdout",
            "stderr",
            "disposition",
            "suspend_mode",
            "retry_attempt",
            "retry_max_attempts",
        ] {
            assert!(
                entry.get(field).is_some(),
                "hook telemetry entry missing required field '{field}'"
            );
        }

        assert_eq!(entry["loop_id"], "loop-telemetry-123");
        assert_eq!(entry["phase_event"], "pre.loop.start");
        assert_eq!(entry["hook_name"], "env-guard");
        assert_eq!(entry["duration_ms"], 923);
        assert_eq!(entry["exit_code"], 13);
        assert_eq!(entry["timed_out"], false);
        assert_eq!(entry["stdout"]["content"], "stdout-truncated");
        assert_eq!(entry["stdout"]["truncated"], true);
        assert_eq!(entry["stderr"]["content"], "stderr-clean");
        assert_eq!(entry["stderr"]["truncated"], false);
        assert_eq!(entry["disposition"], "block");
        assert_eq!(entry["suspend_mode"], "retry_backoff");
        assert_eq!(entry["retry_attempt"], 3);
        assert_eq!(entry["retry_max_attempts"], 4);
    }

    #[test]
    fn test_diagnostics_collector_hook_run_logging_is_noop_when_disabled() {
        let temp_dir = TempDir::new().unwrap();
        let collector = DiagnosticsCollector::with_enabled(temp_dir.path(), false).unwrap();

        collector.log_hook_run(sample_hook_telemetry_entry(HookDisposition::Warn));

        assert!(collector.session_dir().is_none());
        assert!(
            !temp_dir.path().join(".ralph").join("diagnostics").exists(),
            "disabled diagnostics should not create diagnostics artifacts"
        );
    }
}
