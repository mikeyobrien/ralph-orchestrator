use std::process::Command;
use tempfile::TempDir;

fn run_ralph(temp_path: &std::path::Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_ralph"))
        .args(args)
        .current_dir(temp_path)
        .output()
        .expect("execute ralph")
}

#[test]
fn test_run_dry_run_succeeds() {
    let temp_dir = TempDir::new().expect("temp dir");
    let temp_path = temp_dir.path();

    let output = run_ralph(
        temp_path,
        &[
            "run",
            "--dry-run",
            "--skip-preflight",
            "--prompt",
            "hello world",
            "--completion-promise",
            "done",
            "--max-iterations",
            "1",
            "--backend",
            "claude",
            "--no-tui",
        ],
    );

    assert!(
        output.status.success(),
        "run failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Dry run mode"), "stdout: {stdout}");
}

#[test]
fn test_run_continue_requires_scratchpad() {
    let temp_dir = TempDir::new().expect("temp dir");
    let temp_path = temp_dir.path();

    let output = run_ralph(
        temp_path,
        &["run", "--continue", "--dry-run", "--skip-preflight"],
    );

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Cannot continue: scratchpad not found"),
        "stderr: {stderr}"
    );
}
