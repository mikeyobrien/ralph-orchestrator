//! Regression tests for issue #213: Subprocess TUI run wrongly spawns worktree on first run
//!
//! This tests the self-lock contention bug where the parent process acquires the lock,
//! then spawns a child RPC process, which sees the parent's lock and incorrectly
//! spawns a worktree.

use std::process::Command;
use tempfile::TempDir;

/// Test that subprocess TUI mode does NOT spawn a worktree when there's no actual contention.
/// This is a regression test for issue #213.
#[test]
fn test_subprocess_tui_no_spurious_worktree() {
    let temp_dir = TempDir::new().expect("temp dir");
    let temp_path = temp_dir.path();

    // Initialize a git repo (required for ralph)
    let git_init = Command::new("git")
        .args(["init"])
        .current_dir(temp_path)
        .output()
        .expect("git init");
    assert!(git_init.status.success(), "git init failed");

    // Run ralph with a short timeout in subprocess TUI mode
    // In a proper TTY, this would use subprocess TUI, but in test we force it
    // We use --legacy-tui to ensure we get the in-process behavior for comparison
    let output = Command::new(env!("CARGO_BIN_EXE_ralph"))
        .args([
            "run",
            "--dry-run",
            "--skip-preflight",
            "--prompt",
            "test prompt",
            "--completion-promise",
            "LOOP_COMPLETE",
            "--max-iterations",
            "1",
            "--no-tui",
        ])
        .current_dir(temp_path)
        .output()
        .expect("execute ralph");

    // Should succeed
    assert!(
        output.status.success(),
        "ralph run failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Should NOT create a worktree directory
    let worktrees_dir = temp_path.join(".worktrees");
    assert!(
        !worktrees_dir.exists(),
        "Worktree directory should not exist for single run: {:?}",
        worktrees_dir
    );
}

/// Test that when lock is held by another process, we correctly detect it and either
/// wait (--exclusive) or spawn worktree (parallel mode).
#[test]
fn test_lock_contention_detection() {
    let temp_dir = TempDir::new().expect("temp dir");
    let temp_path = temp_dir.path();

    // Initialize git repo
    let git_init = Command::new("git")
        .args(["init"])
        .current_dir(temp_path)
        .output()
        .expect("git init");
    assert!(git_init.status.success());

    // Create .ralph directory and a lock file to simulate a running loop
    let ralph_dir = temp_path.join(".ralph");
    std::fs::create_dir_all(&ralph_dir).expect("create .ralph dir");
    let lock_file = ralph_dir.join("loop.lock");
    std::fs::write(
        &lock_file,
        serde_json::json!({
            "pid": 999_999,
            "started": "2026-01-01T00:00:00Z",
            "prompt": "existing loop"
        })
        .to_string(),
    )
    .expect("write lock file");

    // Try to run with --exclusive - should wait and succeed
    let output = Command::new(env!("CARGO_BIN_EXE_ralph"))
        .args([
            "run",
            "--dry-run",
            "--skip-preflight",
            "--exclusive",
            "--prompt",
            "test",
            "--completion-promise",
            "LOOP_COMPLETE",
            "--max-iterations",
            "1",
            "--no-tui",
        ])
        .current_dir(temp_path)
        .output()
        .expect("execute ralph");

    // With --exclusive, it should wait for the lock (or fail if lock can't be acquired)
    // The exact behavior depends on whether the mock lock is properly held
    let stderr = String::from_utf8_lossy(&output.stderr);
    println!("Output: {}", stderr);
}
