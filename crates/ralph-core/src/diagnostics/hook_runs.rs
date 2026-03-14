use crate::hooks::{HookRunResult, HookStreamOutput, HookSuspendMode};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::Path;

/// Final outcome category for a hook invocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HookDisposition {
    Pass,
    Warn,
    Block,
    Suspend,
}

/// Structured diagnostics record persisted for each hook invocation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookRunTelemetryEntry {
    pub timestamp: DateTime<Utc>,
    pub loop_id: String,
    pub phase_event: String,
    pub hook_name: String,
    pub started_at: DateTime<Utc>,
    pub ended_at: DateTime<Utc>,
    pub duration_ms: u64,
    pub exit_code: Option<i32>,
    pub timed_out: bool,
    pub stdout: HookStreamOutput,
    pub stderr: HookStreamOutput,
    pub disposition: HookDisposition,
    pub suspend_mode: HookSuspendMode,
    pub retry_attempt: u32,
    pub retry_max_attempts: u32,
}

impl HookRunTelemetryEntry {
    /// Creates a telemetry record from executor output and lifecycle metadata.
    #[must_use]
    pub fn from_run_result(
        loop_id: impl Into<String>,
        phase_event: impl Into<String>,
        hook_name: impl Into<String>,
        disposition: HookDisposition,
        suspend_mode: HookSuspendMode,
        retry_attempt: u32,
        retry_max_attempts: u32,
        run_result: &HookRunResult,
    ) -> Self {
        Self {
            timestamp: Utc::now(),
            loop_id: loop_id.into(),
            phase_event: phase_event.into(),
            hook_name: hook_name.into(),
            started_at: run_result.started_at,
            ended_at: run_result.ended_at,
            duration_ms: run_result.duration_ms,
            exit_code: run_result.exit_code,
            timed_out: run_result.timed_out,
            stdout: run_result.stdout.clone(),
            stderr: run_result.stderr.clone(),
            disposition,
            suspend_mode,
            retry_attempt,
            retry_max_attempts,
        }
    }
}

/// JSONL writer for hook invocation telemetry (`hook-runs.jsonl`).
pub struct HookRunLogger {
    writer: BufWriter<File>,
}

impl HookRunLogger {
    pub fn new(session_dir: &Path) -> std::io::Result<Self> {
        let log_file = session_dir.join("hook-runs.jsonl");
        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(log_file)?;

        Ok(Self {
            writer: BufWriter::new(file),
        })
    }

    pub fn log(&mut self, entry: &HookRunTelemetryEntry) -> std::io::Result<()> {
        serde_json::to_writer(&mut self.writer, entry)?;
        self.writer.write_all(b"\n")?;
        self.writer.flush()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use std::fs;
    use tempfile::TempDir;

    fn fixed_time(hour: u32, minute: u32, second: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 2, 28, hour, minute, second)
            .single()
            .expect("fixed timestamp")
    }

    fn sample_entry(disposition: HookDisposition) -> HookRunTelemetryEntry {
        HookRunTelemetryEntry {
            timestamp: fixed_time(15, 30, 2),
            loop_id: "loop-1234-abcd".to_string(),
            phase_event: "pre.loop.start".to_string(),
            hook_name: "env-guard".to_string(),
            started_at: fixed_time(15, 30, 1),
            ended_at: fixed_time(15, 30, 2),
            duration_ms: 923,
            exit_code: Some(0),
            timed_out: false,
            stdout: HookStreamOutput {
                content: "hook-stdout".to_string(),
                truncated: false,
            },
            stderr: HookStreamOutput {
                content: "hook-stderr".to_string(),
                truncated: true,
            },
            disposition,
            suspend_mode: HookSuspendMode::RetryBackoff,
            retry_attempt: 2,
            retry_max_attempts: 4,
        }
    }

    #[test]
    fn hook_disposition_serializes_to_snake_case() {
        let variants = [
            (HookDisposition::Pass, "pass"),
            (HookDisposition::Warn, "warn"),
            (HookDisposition::Block, "block"),
            (HookDisposition::Suspend, "suspend"),
        ];

        for (disposition, expected) in variants {
            let serialized = serde_json::to_string(&disposition).expect("serialize disposition");
            assert_eq!(serialized, format!("\"{expected}\""));

            let parsed: HookDisposition =
                serde_json::from_str(&serialized).expect("deserialize disposition");
            assert_eq!(parsed, disposition);
        }
    }

    #[test]
    fn telemetry_entry_serializes_required_fields() {
        let entry = sample_entry(HookDisposition::Pass);
        let value = serde_json::to_value(&entry).expect("serialize telemetry entry");

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
                value.get(field).is_some(),
                "serialized entry missing '{field}'"
            );
        }

        assert_eq!(value["phase_event"], "pre.loop.start");
        assert_eq!(value["hook_name"], "env-guard");
        assert_eq!(value["duration_ms"], 923);
        assert_eq!(value["disposition"], "pass");
        assert_eq!(value["stdout"]["content"], "hook-stdout");
        assert_eq!(value["stdout"]["truncated"], false);
        assert_eq!(value["stderr"]["content"], "hook-stderr");
        assert_eq!(value["stderr"]["truncated"], true);
        assert_eq!(value["suspend_mode"], "retry_backoff");
        assert_eq!(value["retry_attempt"], 2);
        assert_eq!(value["retry_max_attempts"], 4);
    }

    #[test]
    fn from_run_result_maps_hook_runtime_fields() {
        let run_result = HookRunResult {
            started_at: fixed_time(16, 0, 0),
            ended_at: fixed_time(16, 0, 2),
            duration_ms: 2000,
            exit_code: Some(17),
            timed_out: true,
            stdout: HookStreamOutput {
                content: "captured-stdout".to_string(),
                truncated: true,
            },
            stderr: HookStreamOutput {
                content: "captured-stderr".to_string(),
                truncated: false,
            },
        };

        let timestamp_before = Utc::now();
        let entry = HookRunTelemetryEntry::from_run_result(
            "loop-777",
            "post.iteration.start",
            "manual-gate",
            HookDisposition::Block,
            HookSuspendMode::WaitThenRetry,
            2,
            2,
            &run_result,
        );
        let timestamp_after = Utc::now();

        assert_eq!(entry.loop_id, "loop-777");
        assert_eq!(entry.phase_event, "post.iteration.start");
        assert_eq!(entry.hook_name, "manual-gate");
        assert_eq!(entry.started_at, run_result.started_at);
        assert_eq!(entry.ended_at, run_result.ended_at);
        assert_eq!(entry.duration_ms, run_result.duration_ms);
        assert_eq!(entry.exit_code, run_result.exit_code);
        assert_eq!(entry.timed_out, run_result.timed_out);
        assert_eq!(entry.stdout.content, run_result.stdout.content);
        assert_eq!(entry.stdout.truncated, run_result.stdout.truncated);
        assert_eq!(entry.stderr.content, run_result.stderr.content);
        assert_eq!(entry.stderr.truncated, run_result.stderr.truncated);
        assert_eq!(entry.disposition, HookDisposition::Block);
        assert_eq!(entry.suspend_mode, HookSuspendMode::WaitThenRetry);
        assert_eq!(entry.retry_attempt, 2);
        assert_eq!(entry.retry_max_attempts, 2);
        assert!(entry.timestamp >= timestamp_before);
        assert!(entry.timestamp <= timestamp_after);
    }

    #[test]
    fn hook_run_logger_persists_jsonl_entries() {
        let temp_dir = TempDir::new().expect("temp dir");
        let mut logger = HookRunLogger::new(temp_dir.path()).expect("create logger");

        let entry = sample_entry(HookDisposition::Warn);
        logger.log(&entry).expect("write telemetry entry");
        drop(logger);

        let content = fs::read_to_string(temp_dir.path().join("hook-runs.jsonl"))
            .expect("read hook-runs.jsonl");
        let lines: Vec<_> = content.lines().collect();
        assert_eq!(lines.len(), 1);

        let parsed: HookRunTelemetryEntry =
            serde_json::from_str(lines[0]).expect("parse logged telemetry entry");
        assert_eq!(parsed.loop_id, "loop-1234-abcd");
        assert_eq!(parsed.phase_event, "pre.loop.start");
        assert_eq!(parsed.hook_name, "env-guard");
        assert_eq!(parsed.disposition, HookDisposition::Warn);
        assert_eq!(parsed.suspend_mode, HookSuspendMode::RetryBackoff);
        assert_eq!(parsed.retry_attempt, 2);
        assert_eq!(parsed.retry_max_attempts, 4);
        assert_eq!(parsed.stdout.content, "hook-stdout");
        assert_eq!(parsed.stderr.content, "hook-stderr");
        assert!(parsed.stderr.truncated);
    }

    #[test]
    fn hook_run_logger_flushes_on_each_write() {
        let temp_dir = TempDir::new().expect("temp dir");
        let mut logger = HookRunLogger::new(temp_dir.path()).expect("create logger");

        logger
            .log(&sample_entry(HookDisposition::Suspend))
            .expect("write telemetry entry");

        // Validate flush behavior without dropping logger.
        let content = fs::read_to_string(temp_dir.path().join("hook-runs.jsonl"))
            .expect("read hook-runs.jsonl");
        assert_eq!(content.lines().count(), 1);
    }
}
