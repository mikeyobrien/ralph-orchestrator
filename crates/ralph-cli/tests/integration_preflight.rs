//! Integration tests for `ralph preflight` CLI command.

use std::process::Command;
use tempfile::TempDir;

fn ralph_preflight(temp_path: &std::path::Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_ralph"))
        .args(args)
        .current_dir(temp_path)
        .output()
        .expect("Failed to execute ralph preflight command")
}

#[test]
fn test_preflight_check_config_json() {
    let temp_dir = TempDir::new().expect("temp dir");
    let temp_path = temp_dir.path();

    let output = ralph_preflight(
        temp_path,
        &["preflight", "--check", "config", "--format", "json"],
    );

    assert!(
        output.status.success(),
        "preflight failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let json_start = stdout.find('{').expect("no json start");
    let json_end = stdout.rfind('}').expect("no json end");
    let json = &stdout[json_start..=json_end];
    let report: serde_json::Value = serde_json::from_str(json).expect("parse report");
    assert_eq!(report["passed"], true);
    assert_eq!(report["failures"], 0);
    let checks = report["checks"].as_array().expect("checks array");
    assert_eq!(checks.len(), 1);
    assert_eq!(checks[0]["name"], "config");
}

#[test]
fn test_preflight_unknown_check_fails() {
    let temp_dir = TempDir::new().expect("temp dir");
    let temp_path = temp_dir.path();

    let output = ralph_preflight(
        temp_path,
        &["preflight", "--check", "nope", "--format", "json"],
    );

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Unknown check"), "stderr: {}", stderr);
}
