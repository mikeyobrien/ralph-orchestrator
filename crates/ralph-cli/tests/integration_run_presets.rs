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
fn test_run_dry_run_with_builtin_preset() {
    let temp_dir = TempDir::new().expect("temp dir");
    let temp_path = temp_dir.path();

    let output = run_ralph(
        temp_path,
        &[
            "--color",
            "never",
            "--config",
            "builtin:feature",
            "run",
            "--dry-run",
            "--skip-preflight",
            "--prompt",
            "hello from preset",
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
fn test_run_dry_run_with_overrides_only() {
    let temp_dir = TempDir::new().expect("temp dir");
    let temp_path = temp_dir.path();

    let output = run_ralph(
        temp_path,
        &[
            "--color",
            "never",
            "--config",
            "core.scratchpad=.custom/scratchpad.md",
            "run",
            "--dry-run",
            "--skip-preflight",
            "--prompt",
            "hello from overrides",
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
    assert!(
        stdout.contains("Scratchpad: .custom/scratchpad.md"),
        "stdout: {stdout}"
    );
}

#[test]
fn test_run_dry_run_with_unknown_preset_fails() {
    let temp_dir = TempDir::new().expect("temp dir");
    let temp_path = temp_dir.path();

    let output = run_ralph(
        temp_path,
        &[
            "--color",
            "never",
            "--config",
            "builtin:not-a-preset",
            "run",
            "--dry-run",
            "--skip-preflight",
            "--prompt",
            "hello",
            "--backend",
            "claude",
            "--no-tui",
        ],
    );

    assert!(!output.status.success(), "run should fail");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined = format!("{stdout}{stderr}");
    assert!(
        combined.contains("Unknown preset"),
        "unexpected output: {combined}"
    );
}
