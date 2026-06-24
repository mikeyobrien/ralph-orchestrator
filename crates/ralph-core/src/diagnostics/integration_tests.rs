//! Integration tests for the diagnostics collector (engine-agnostic).

#[cfg(test)]
mod tests {
    use crate::diagnostics::{DiagnosticsCollector, HookDisposition, HookRunTelemetryEntry};
    use crate::hooks::{HookRunResult, HookStreamOutput, HookSuspendMode};
    use chrono::{TimeZone, Utc};
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
