//! Integration tests: OMP backends are subject to the no-TUI executor's
//! per-worker/inactivity timeout — a stalling OMP process is classified as
//! timed-out (not silently treated as success, not left hanging), and a healthy
//! OMP process that emits output and exits within the deadline is NOT falsely
//! timed out.
//!
//! Validates OMP step-4 focused filter `omp_timeout` (AC3/AC6): the adapter-
//! level enforcement of the per-worker deadline on the ordinary `CliExecutor`
//! path. The R13 precedence-chain *resolution* (`hat.timeout` → override →
//! default → 300 s) is unit-tested in `ralph-core::config`
//! (`per_worker_timeout_secs`); GAP3 aggregator-wait in
//! `ralph-core::wave_detection` (`resolve_aggregate_wait`). These tests prove
//! the resolved `Duration` is actually enforced for an `OmpStreamJson` backend.
//!
//! Run with: cargo test -p ralph-adapters --test omp_timeout

#![cfg(unix)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::time::Duration;

use ralph_adapters::{CliBackend, CliExecutor, OutputFormat, PromptMode};
use tempfile::TempDir;
use tokio::time::timeout;

/// Overall per-test wall-clock cap (well above the 500 ms inactivity window and
/// the ~2 s terminate grace period).
const TEST_CAP: Duration = Duration::from_secs(20);

fn omp_backend(script: &str) -> CliBackend {
    CliBackend {
        command: script.to_string(),
        args: vec![],
        prompt_mode: PromptMode::Arg,
        prompt_flag: None,
        output_format: OutputFormat::OmpStreamJson,
        env_vars: vec![],
    }
}

fn write_executable(dir: &Path, name: &str, body: &str) -> String {
    let path = dir.join(name);
    let script = format!("#!/bin/sh\n{body}\n");
    fs::write(&path, &script).unwrap();
    let mut perms = fs::metadata(&path).unwrap().permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&path, perms).unwrap();
    path.to_string_lossy().to_string()
}

/// A stalling OMP that emits NO output so the inactivity timeout fires, and
/// outlives the timeout window.
fn stalling_omp_script(dir: &Path) -> String {
    write_executable(dir, "stalling-omp.sh", "sleep 30")
}

/// A healthy OMP that emits a valid Pi-family NDJSON stream and exits 0.
fn healthy_omp_script(dir: &Path) -> String {
    write_executable(
        dir,
        "healthy-omp.sh",
        r#"printf '%s\n' '{"type":"message_update","assistantMessageEvent":{"type":"text_delta","contentIndex":0,"delta":"omp timeout healthy"}}'
printf '%s\n' '{"type":"turn_end","message":{"content":[],"usage":{"input":1,"output":1,"cacheRead":0,"cacheWrite":0,"cost":{"total":0.0}}}}'"#,
    )
}

/// A stalling OMP backend is classified as timed-out by the CliExecutor's
/// inactivity timeout — the resolved per-worker deadline enforced on the
/// ordinary no-TUI path. The result indicates timeout rather than hanging or
/// silent success.
#[tokio::test]
async fn test_omp_timeout_cli_executor_classifies_inactivity() {
    let temp_dir = TempDir::new().unwrap();
    let script = stalling_omp_script(temp_dir.path());
    let executor = CliExecutor::new(omp_backend(&script));

    let result = timeout(
        TEST_CAP,
        executor.execute_capture_with_timeout("review", Some(Duration::from_millis(500))),
    )
    .await
    .expect("CliExecutor execute hung")
    .expect("executor returned IO error");

    assert!(
        result.timed_out,
        "stalling OMP should be classified as timed-out: {result:?}"
    );
    assert!(
        !result.success,
        "timed-out OMP should not report success: {result:?}"
    );
}

/// A healthy OMP backend that emits output and exits within the deadline is NOT
/// falsely timed out — the inactivity window resets on each output line.
#[tokio::test]
async fn test_omp_timeout_cli_executor_no_false_fire_on_completion() {
    let temp_dir = TempDir::new().unwrap();
    let script = healthy_omp_script(temp_dir.path());
    let executor = CliExecutor::new(omp_backend(&script));

    let result = timeout(
        TEST_CAP,
        executor.execute_capture_with_timeout("review", Some(Duration::from_secs(5))),
    )
    .await
    .expect("CliExecutor execute hung")
    .expect("executor returned IO error");

    assert!(
        !result.timed_out,
        "healthy OMP should not be timed-out: {result:?}"
    );
    assert!(
        !result.output.is_empty(),
        "healthy OMP should produce output: {result:?}"
    );
}
