//! Integration tests: OMP-spawned process trees are fully reaped on
//! timeout/interrupt, on BOTH the no-TUI `CliExecutor` and the TUI
//! `PtyExecutor`.
//!
//! Validates OMP step-4 AC2: a long-lived **grandchild** (a separate pid in the
//! direct child's process group) outlives a single `kill(direct_pid, SIGTERM)`,
//! so it can only be cleaned up by a process-**GROUP** kill (`kill(-pgid)`).
//! Under single-PID termination the grandchild survives and the test fails; it
//! passes only once both executors kill the whole group.
//!
//! Run with: cargo test -p ralph-adapters --test omp_process_cleanup

#![cfg(unix)]

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::Duration;

use ralph_adapters::{CliBackend, CliExecutor, OutputFormat, PromptMode, PtyConfig, PtyExecutor};
use tempfile::TempDir;
use tokio::time::timeout;

/// Overall per-test wall-clock cap (well above the ~2s terminate grace period).
const TEST_CAP: Duration = Duration::from_secs(20);

fn pid_alive(pid: u32) -> bool {
    // kill(pid, 0) checks process existence without delivering a signal.
    nix::sys::signal::kill(nix::unistd::Pid::from_raw(pid as i32), None).is_ok()
}

/// Mock OMP script: spawns a long-lived grandchild, records
/// `script_pid:grandchild_pid` to a file, then stalls on a foreground `sleep`.
///
/// The grandchild (`nohup sleep 300`) ignores SIGHUP (so closing the PTY master
/// does not reap it via the terminal-hangup backstop) but still dies to SIGTERM.
/// It is a distinct pid in the script's process group, so it outlives a single
/// `kill(script_pid, SIGTERM)` AND the PTY-close SIGHUP — proving whether
/// `terminate_child` kills the whole process group. Under single-PID
/// termination the grandchild survives (test fails); it dies only under a
/// group kill.
fn create_stalling_omp_script(dir: &Path) -> (String, PathBuf) {
    let script_path = dir.join("mock-omp.sh");
    let pid_file = dir.join("omp-pids.txt");
    let script = format!(
        r#"#!/usr/bin/env bash
# Long-lived grandchild simulating an OMP-spawned descendant. nohup makes it
# ignore SIGHUP so the PTY-close hangup backstop cannot reap it; it still
# dies to SIGTERM/SIGKILL delivered to its process group.
nohup sleep 300 </dev/null >/dev/null 2>&1 &
GRANDCHILD=$!
echo "$$:$GRANDCHILD" >> {pid_file}
# Stall so the executor's timeout/interrupt fires while the grandchild lives.
sleep 300
"#,
        pid_file = pid_file.display()
    );
    fs::write(&script_path, &script).unwrap();
    fs::set_permissions(&script_path, fs::Permissions::from_mode(0o755)).unwrap();
    (script_path.to_string_lossy().to_string(), pid_file)
}

fn read_grandchild(pid_file: &Path) -> Option<u32> {
    let content = fs::read_to_string(pid_file).ok()?;
    content
        .lines()
        .rfind(|l| !l.is_empty())
        .and_then(|line| line.split(':').nth(1).and_then(|s| s.parse::<u32>().ok()))
}

/// Wait until the script has recorded a grandchild pid.
async fn await_grandchild(pid_file: &Path) -> u32 {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
    loop {
        if let Some(gc) = read_grandchild(pid_file) {
            return gc;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "mock OMP script never recorded a grandchild pid"
        );
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
}

/// Poll until the grandchild is reaped; fail loudly if it survives (orphan leak).
async fn assert_grandchild_reaped(grandchild: u32) {
    let deadline = tokio::time::Instant::now() + Duration::from_secs(8);
    loop {
        if !pid_alive(grandchild) {
            return;
        }
        assert!(
            tokio::time::Instant::now() < deadline,
            "OMP grandchild {grandchild} still alive — process-GROUP kill did \
             not reach the descendant (single-PID termination leaves an orphan)"
        );
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}

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

/// CliExecutor (OMP no-TUI path) reaps the grandchild via process-group kill on
/// the inactivity timeout. `CliExecutor` already sets `process_group(0)` and
/// group-kills, so this is a regression guard for OMP's no-TUI lifecycle.
#[tokio::test]
async fn cli_executor_reaps_omp_grandchild_on_inactivity_timeout() {
    let temp_dir = TempDir::new().unwrap();
    let (script, pid_file) = create_stalling_omp_script(temp_dir.path());
    let executor = CliExecutor::new(omp_backend(&script));

    // The stalling script emits nothing, so the 500ms inactivity timeout fires
    // and triggers process-group termination.
    let _ = timeout(
        TEST_CAP,
        executor.execute_capture_with_timeout("review", Some(Duration::from_millis(500))),
    )
    .await
    .expect("CliExecutor execute hung");

    let grandchild = await_grandchild(&pid_file).await;
    assert_grandchild_reaped(grandchild).await;
}

/// PtyExecutor (OMP TUI path) reaps the grandchild via process-group kill on a
/// user interrupt. RED under single-PID `terminate_child` (grandchild survives),
/// GREEN once terminate kills the whole group.
#[tokio::test]
async fn pty_executor_reaps_omp_grandchild_on_interrupt() {
    let temp_dir = TempDir::new().unwrap();
    let (script, pid_file) = create_stalling_omp_script(temp_dir.path());
    let config = PtyConfig {
        interactive: false,
        idle_timeout_secs: 0,
        cols: 80,
        rows: 24,
        workspace_root: temp_dir.path().to_path_buf(),
    };
    let executor = PtyExecutor::new(omp_backend(&script), config);
    let (interrupt_tx, interrupt_rx) = tokio::sync::watch::channel(false);

    let run = tokio::spawn(async move {
        let _ = executor.run_observe("review", interrupt_rx).await;
    });

    // Let the script spawn its grandchild, then interrupt → terminate_child.
    let grandchild = await_grandchild(&pid_file).await;
    let _ = interrupt_tx.send(true);
    let _ = timeout(TEST_CAP, run).await;

    assert_grandchild_reaped(grandchild).await;
}
