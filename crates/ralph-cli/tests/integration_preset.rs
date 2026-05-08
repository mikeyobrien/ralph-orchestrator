//! Integration tests for preset import via the `-H` flag.
//!
//! Covers:
//! - `-H <dir>` where the directory is a TOML multi-file preset (auto-detect).
//! - `-H <name>` where `<name>` resolves via cwd-local `presets/`.
//! - Error messaging when a preset name fails to resolve.

use std::fs;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

const MIN_AUTOLOOPS_TOML: &str = r#"
event_loop.max_iterations = 17
event_loop.completion_event = "task.complete"
event_loop.required_events = ["review.passed"]
"#;

const MIN_TOPOLOGY_TOML: &str = r#"
name = "it-test"
completion = "task.complete"

[[role]]
id = "planner"
emits = ["tasks.ready"]
prompt_file = "roles/planner.md"

[[role]]
id = "builder"
emits = ["review.ready"]
prompt_file = "roles/builder.md"

[[role]]
id = "critic"
emits = ["review.passed"]
prompt_file = "roles/critic.md"

[handoff]
"loop.start" = ["planner"]
"tasks.ready" = ["builder"]
"review.ready" = ["critic"]
"#;

fn write_toml_preset(root: &Path, name: &str) -> std::path::PathBuf {
    let dir = root.join("presets").join(name);
    fs::create_dir_all(dir.join("roles")).unwrap();
    fs::write(dir.join("autoloops.toml"), MIN_AUTOLOOPS_TOML).unwrap();
    fs::write(dir.join("topology.toml"), MIN_TOPOLOGY_TOML).unwrap();
    fs::write(dir.join("roles/planner.md"), "Plan.").unwrap();
    fs::write(dir.join("roles/builder.md"), "Build.").unwrap();
    fs::write(dir.join("roles/critic.md"), "Criticize.").unwrap();
    fs::write(
        dir.join("harness.md"),
        "Harness: always verify before completing.",
    )
    .unwrap();
    dir
}

fn run_ralph(cwd: &Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_ralph"))
        .args(args)
        .current_dir(cwd)
        .output()
        .expect("execute ralph")
}

#[test]
fn dry_run_detects_toml_preset_dir_passed_as_path() {
    let tmp = TempDir::new().unwrap();
    let preset = write_toml_preset(tmp.path(), "my-preset");

    let output = run_ralph(
        tmp.path(),
        &[
            "--color",
            "never",
            "--hats",
            preset.to_str().unwrap(),
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

    assert!(
        output.status.success(),
        "run failed: stdout={}\nstderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Dry run mode"), "stdout: {stdout}");
}

#[test]
fn bare_preset_name_resolves_from_cwd_presets_dir() {
    let tmp = TempDir::new().unwrap();
    write_toml_preset(tmp.path(), "my-preset");

    let output = run_ralph(
        tmp.path(),
        &[
            "--color",
            "never",
            "--hats",
            "my-preset",
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

    assert!(
        output.status.success(),
        "run failed: stdout={}\nstderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn unresolved_preset_name_fails_with_helpful_error() {
    let tmp = TempDir::new().unwrap();

    let output = run_ralph(
        tmp.path(),
        &[
            "--color",
            "never",
            "--hats",
            "definitely-not-here",
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

    assert!(
        !output.status.success(),
        "expected failure for missing preset"
    );
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        combined.contains("definitely-not-here"),
        "error should mention the missing name: {combined}"
    );
}
