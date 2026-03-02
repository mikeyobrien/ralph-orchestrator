//! Integration tests for config source precedence semantics.
//!
//! Covers:
//! - A single combined `-c ralph.yml` that embeds hats works in dry-run
//! - When combined `-c` has hats and `-H hats.yml` is provided, `-H` hats win
//! - `event_loop.completion_promise` in the hats source overrides combined `-c`
//! - `core.specs_dir=...` CLI override takes final precedence alongside `-H`
//! - `-H builtin:<name>` beats hats embedded in combined `-c`

use std::fs;
use std::process::Command;
use tempfile::TempDir;

// ─────────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────────

/// Build a combined config YAML string (core + embedded hats).
fn combined_config(hat_name: &str, completion_promise: &str, specs_dir: &str) -> String {
    format!(
        r#"cli:
  backend: claude
event_loop:
  max_iterations: 10
  completion_promise: "{completion_promise}"
  prompt: "placeholder"
core:
  specs_dir: "{specs_dir}"
hats:
  {hat_name}:
    name: "{hat_name}"
    description: "A hat inside the combined config"
    triggers: ["{hat_name}.start"]
    publishes: ["{hat_name}.done"]
    default_publishes: "{hat_name}.done"
    instructions: |
      Do the thing for {hat_name}.
"#,
        hat_name = hat_name,
        completion_promise = completion_promise,
        specs_dir = specs_dir,
    )
}

/// Build a hats-only YAML (the kind used with `-H`).
fn hats_only_config(hat_name: &str, completion_promise: Option<&str>) -> String {
    let event_loop_block = match completion_promise {
        Some(promise) => format!(
            r#"event_loop:
  completion_promise: "{}"
"#,
            promise
        ),
        None => String::new(),
    };
    format!(
        r#"{event_loop_block}hats:
  {hat_name}:
    name: "{hat_name}"
    description: "A hat from the dedicated hats file"
    triggers: ["{hat_name}.start"]
    publishes: ["{hat_name}.done"]
    default_publishes: "{hat_name}.done"
    instructions: |
      Do the thing for {hat_name}.
"#,
        event_loop_block = event_loop_block,
        hat_name = hat_name,
    )
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

/// A combined `-c ralph.yml` that embeds hats must succeed in dry-run without
/// errors.  This verifies the single-file combined config path is healthy.
#[test]
fn test_combined_config_dry_run_succeeds() {
    let dir = TempDir::new().expect("temp dir");
    let config_path = dir.path().join("ralph.yml");
    fs::write(
        &config_path,
        combined_config("mybuilder", "LOOP_COMPLETE", "./specs/"),
    )
    .expect("write combined config");

    let out = Command::new(env!("CARGO_BIN_EXE_ralph"))
        .args([
            "--color",
            "never",
            "--config",
            config_path.to_str().unwrap(),
            "run",
            "--dry-run",
            "--skip-preflight",
            "--prompt",
            "hello combined",
            "--backend",
            "claude",
            "--no-tui",
        ])
        .current_dir(dir.path())
        .env("NO_COLOR", "1")
        .output()
        .expect("execute ralph");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success(),
        "expected success; stderr: {stderr}\nstdout: {stdout}"
    );
    assert!(
        stdout.contains("Dry run mode"),
        "expected 'Dry run mode' in stdout; got: {stdout}"
    );
}

/// When `-c combined.yml` contains hats and `-H hats.yml` is also supplied,
/// the hats from `-H` must be the effective hat set (the combined config's
/// embedded hats are replaced).
#[test]
fn test_hats_file_overrides_combined_config_hats() {
    let dir = TempDir::new().expect("temp dir");
    let config_path = dir.path().join("ralph.yml");
    let hats_path = dir.path().join("hats.yml");

    // Combined config embeds "mybuilder"
    fs::write(
        &config_path,
        combined_config("mybuilder", "LOOP_COMPLETE", "./specs/"),
    )
    .expect("write combined config");

    // Hats file defines "myreviewer"
    fs::write(&hats_path, hats_only_config("myreviewer", None)).expect("write hats file");

    let out = Command::new(env!("CARGO_BIN_EXE_ralph"))
        .args([
            "--color",
            "never",
            "--config",
            config_path.to_str().unwrap(),
            "--hats",
            hats_path.to_str().unwrap(),
            "hats",
            "list",
            "--format",
            "json",
        ])
        .current_dir(dir.path())
        .env("NO_COLOR", "1")
        .output()
        .expect("execute ralph");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success(),
        "expected success; stderr: {stderr}\nstdout: {stdout}"
    );

    // Parse the JSON array
    let json_start = stdout.find('[').expect("no JSON array start in stdout");
    let json_end = stdout.rfind(']').expect("no JSON array end in stdout");
    let json_str = &stdout[json_start..=json_end];
    let hats: serde_json::Value =
        serde_json::from_str(json_str).expect("expected valid JSON from 'hats list --format json'");
    let names: Vec<&str> = hats
        .as_array()
        .expect("hats JSON should be an array")
        .iter()
        .filter_map(|h| h["name"].as_str())
        .collect();

    assert!(
        names.contains(&"myreviewer"),
        "expected 'myreviewer' from hats file; got: {names:?}"
    );
    assert!(
        !names.contains(&"mybuilder"),
        "expected 'mybuilder' to be absent (replaced by hats file); got: {names:?}"
    );
}

/// When `-H hats.yml` provides an `event_loop.completion_promise`, that value
/// takes precedence over the one embedded in the combined `-c`.
#[test]
fn test_hats_file_event_loop_completion_promise_overrides_combined_config() {
    let dir = TempDir::new().expect("temp dir");
    let config_path = dir.path().join("ralph.yml");
    let hats_path = dir.path().join("hats.yml");

    // Combined config says COMBINED_DONE
    fs::write(
        &config_path,
        combined_config("mybuilder", "COMBINED_DONE", "./specs/"),
    )
    .expect("write combined config");

    // Hats file overrides to HATS_DONE
    fs::write(
        &hats_path,
        hats_only_config("myreviewer", Some("HATS_DONE")),
    )
    .expect("write hats file");

    let out = Command::new(env!("CARGO_BIN_EXE_ralph"))
        .args([
            "--color",
            "never",
            "--config",
            config_path.to_str().unwrap(),
            "--hats",
            hats_path.to_str().unwrap(),
            "run",
            "--dry-run",
            "--skip-preflight",
            "--prompt",
            "test promise override",
            "--backend",
            "claude",
            "--no-tui",
        ])
        .current_dir(dir.path())
        .env("NO_COLOR", "1")
        .output()
        .expect("execute ralph");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success(),
        "expected success; stderr: {stderr}\nstdout: {stdout}"
    );
    assert!(
        stdout.contains("Completion promise: HATS_DONE"),
        "expected 'Completion promise: HATS_DONE'; got stdout: {stdout}"
    );
    assert!(
        !stdout.contains("COMBINED_DONE"),
        "expected COMBINED_DONE to be absent (overridden by hats file); got: {stdout}"
    );
}

/// A `core.specs_dir=...` CLI override must be applied last (after both `-c`
/// and `-H` are merged), overriding whatever value was in the combined config.
#[test]
fn test_core_specs_dir_cli_override_applies_last() {
    let dir = TempDir::new().expect("temp dir");
    let config_path = dir.path().join("ralph.yml");
    let hats_path = dir.path().join("hats.yml");

    // Combined config sets specs_dir to ./original-specs/
    fs::write(
        &config_path,
        combined_config("mybuilder", "LOOP_COMPLETE", "./original-specs/"),
    )
    .expect("write combined config");

    fs::write(&hats_path, hats_only_config("myreviewer", None)).expect("write hats file");

    let out = Command::new(env!("CARGO_BIN_EXE_ralph"))
        .args([
            "--color",
            "never",
            "--config",
            config_path.to_str().unwrap(),
            "--config",
            "core.specs_dir=./custom-specs/",
            "--hats",
            hats_path.to_str().unwrap(),
            "run",
            "--dry-run",
            "--skip-preflight",
            "--prompt",
            "test specs_dir override",
            "--backend",
            "claude",
            "--no-tui",
        ])
        .current_dir(dir.path())
        .env("NO_COLOR", "1")
        .output()
        .expect("execute ralph");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success(),
        "expected success; stderr: {stderr}\nstdout: {stdout}"
    );
    assert!(
        stdout.contains("Specs dir: ./custom-specs/"),
        "expected 'Specs dir: ./custom-specs/'; got stdout: {stdout}"
    );
    assert!(
        !stdout.contains("./original-specs/"),
        "expected original specs_dir to be absent; got: {stdout}"
    );
}

/// `-H builtin:<name>` must replace the hats embedded in a combined `-c`, just
/// like a file-based hats source does.
#[test]
fn test_builtin_hats_source_overrides_combined_config_hats() {
    let dir = TempDir::new().expect("temp dir");
    let config_path = dir.path().join("ralph.yml");

    // Combined config embeds a "myplanner" hat
    fs::write(
        &config_path,
        combined_config("myplanner", "LOOP_COMPLETE", "./specs/"),
    )
    .expect("write combined config");

    let out = Command::new(env!("CARGO_BIN_EXE_ralph"))
        .args([
            "--color",
            "never",
            "--config",
            config_path.to_str().unwrap(),
            "--hats",
            "builtin:feature",
            "hats",
            "list",
            "--format",
            "json",
        ])
        .current_dir(dir.path())
        .env("NO_COLOR", "1")
        .output()
        .expect("execute ralph");

    let stdout = String::from_utf8_lossy(&out.stdout);
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(
        out.status.success(),
        "expected success; stderr: {stderr}\nstdout: {stdout}"
    );

    let json_start = stdout.find('[').expect("no JSON array start in stdout");
    let json_end = stdout.rfind(']').expect("no JSON array end in stdout");
    let json_str = &stdout[json_start..=json_end];
    let hats: serde_json::Value =
        serde_json::from_str(json_str).expect("expected valid JSON from 'hats list --format json'");
    let names: Vec<&str> = hats
        .as_array()
        .expect("hats JSON should be an array")
        .iter()
        .filter_map(|h| h["name"].as_str())
        .collect();

    // builtin:feature defines "Builder" and "Reviewer"
    assert!(
        names.contains(&"Builder"),
        "expected 'Builder' from builtin:feature; got: {names:?}"
    );
    assert!(
        names.contains(&"Reviewer"),
        "expected 'Reviewer' from builtin:feature; got: {names:?}"
    );

    // The combined config's "myplanner" hat must have been replaced
    assert!(
        !names.contains(&"myplanner"),
        "expected 'myplanner' to be absent (replaced by builtin:feature); got: {names:?}"
    );
}
