use std::process::Command;
use tempfile::TempDir;

fn run_ralph(temp_path: &std::path::Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_ralph"))
        .args(args)
        .current_dir(temp_path)
        .output()
        .expect("execute ralph")
}

fn run_ralph_with_home(
    temp_path: &std::path::Path,
    home_path: &std::path::Path,
    args: &[&str],
) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_ralph"))
        .args(args)
        .current_dir(temp_path)
        .env("HOME", home_path)
        .env("USERPROFILE", home_path)
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
fn test_run_dry_run_uses_user_scoped_config_defaults() {
    let temp_dir = TempDir::new().expect("temp dir");
    let home_dir = TempDir::new().expect("temp home");
    let temp_path = temp_dir.path();
    let user_config_dir = home_dir.path().join(".ralph");
    std::fs::create_dir_all(&user_config_dir).expect("create user config dir");
    std::fs::write(
        user_config_dir.join("config.yml"),
        r"
cli:
  backend: claude
event_loop:
  max_iterations: 7
",
    )
    .expect("write user config");

    let output = run_ralph_with_home(
        temp_path,
        home_dir.path(),
        &[
            "run",
            "--dry-run",
            "--skip-preflight",
            "--prompt",
            "hello world",
            "--no-tui",
        ],
    );

    assert!(
        output.status.success(),
        "run failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Backend: claude"), "stdout: {stdout}");
    assert!(stdout.contains("Max iterations: 7"), "stdout: {stdout}");
}

#[test]
fn test_run_dry_run_local_config_overrides_user_scoped_defaults() {
    let temp_dir = TempDir::new().expect("temp dir");
    let home_dir = TempDir::new().expect("temp home");
    let temp_path = temp_dir.path();
    let user_config_dir = home_dir.path().join(".ralph");
    std::fs::create_dir_all(&user_config_dir).expect("create user config dir");
    std::fs::write(
        user_config_dir.join("config.yml"),
        r"
cli:
  backend: claude
event_loop:
  max_iterations: 7
",
    )
    .expect("write user config");
    std::fs::write(
        temp_path.join("ralph.yml"),
        r"
cli:
  backend: gemini
event_loop:
  max_iterations: 3
",
    )
    .expect("write local config");

    let output = run_ralph_with_home(
        temp_path,
        home_dir.path(),
        &[
            "run",
            "--dry-run",
            "--skip-preflight",
            "--prompt",
            "hello world",
            "--no-tui",
        ],
    );

    assert!(
        output.status.success(),
        "run failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Backend: gemini"), "stdout: {stdout}");
    assert!(stdout.contains("Max iterations: 3"), "stdout: {stdout}");
}

#[test]
fn test_run_continue_does_not_require_scratchpad() {
    let temp_dir = TempDir::new().expect("temp dir");
    let temp_path = temp_dir.path();
    assert!(!temp_path.join(".ralph/scratchpad.md").exists());

    let output = run_ralph(
        temp_path,
        &["run", "--continue", "--dry-run", "--skip-preflight"],
    );

    assert!(
        output.status.success(),
        "run failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(!stderr.contains("scratchpad not found"), "stderr: {stderr}");
}
