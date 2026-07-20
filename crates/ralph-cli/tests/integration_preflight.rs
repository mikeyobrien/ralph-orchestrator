//! Integration tests for `ralph preflight` CLI command.

use std::fs;
use std::process::Command;
use tempfile::TempDir;

fn ralph_preflight(temp_path: &std::path::Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_ralph"))
        .args(args)
        .current_dir(temp_path)
        .output()
        .expect("Failed to execute ralph preflight command")
}

fn rendered(output: &std::process::Output) -> String {
    format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
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

#[test]
fn configured_hooks_warning_is_visible_in_run_and_doctor_but_absent_without_hooks() {
    const WARNING: &str = "WARNING: lifecycle hooks are NOT executed under the autoloop engine pending the engine bridge.";

    let temp_dir = TempDir::new().expect("temp dir");
    let temp_path = temp_dir.path();
    let hooks_config = temp_path.join("hooks.yml");
    fs::write(
        &hooks_config,
        r#"
hooks:
  enabled: true
  events:
    pre.loop.start:
      - name: truthful-warning
        command: ["/usr/bin/true"]
        on_error: warn
features:
  preflight:
    enabled: true
"#,
    )
    .expect("write hooks config");

    let no_hooks_config = temp_path.join("no-hooks.yml");
    fs::write(
        &no_hooks_config,
        "features:\n  preflight:\n    enabled: true\n",
    )
    .expect("write no-hooks config");

    for (subcommand, trailing_args) in [
        ("run", vec!["--dry-run", "--no-tui", "--prompt", "test"]),
        ("doctor", vec![]),
    ] {
        let mut hooks_args = vec![
            "--color",
            "never",
            "--config",
            hooks_config.to_str().expect("utf8 hooks config"),
            subcommand,
        ];
        hooks_args.extend(trailing_args.iter().copied());
        let hooks_output = ralph_preflight(temp_path, &hooks_args);
        let hooks_text = rendered(&hooks_output);
        assert!(
            hooks_text.contains(WARNING),
            "{subcommand} output did not contain inert-hooks warning:\n{hooks_text}"
        );

        let mut no_hooks_args = vec![
            "--color",
            "never",
            "--config",
            no_hooks_config.to_str().expect("utf8 no-hooks config"),
            subcommand,
        ];
        no_hooks_args.extend(trailing_args.iter().copied());
        let no_hooks_output = ralph_preflight(temp_path, &no_hooks_args);
        let no_hooks_text = rendered(&no_hooks_output);
        assert!(
            !no_hooks_text.contains(WARNING),
            "{subcommand} output unexpectedly contained inert-hooks warning:\n{no_hooks_text}"
        );
    }
}

#[test]
fn test_preflight_config_resolves_hat_import_from_file() {
    let temp_dir = TempDir::new().expect("temp dir");
    let temp_path = temp_dir.path();
    let config_dir = temp_path.join("workflow");
    let shared_dir = temp_path.join("shared-hats");
    fs::create_dir_all(&config_dir).expect("create config dir");
    fs::create_dir_all(&shared_dir).expect("create shared hats dir");

    fs::write(
        shared_dir.join("builder.yml"),
        r#"
name: Imported Builder
description: Imported by preflight integration test
triggers: ["build.start"]
publishes: ["build.done"]
default_publishes: "build.done"
instructions: |
  Build the requested change.
"#,
    )
    .expect("write imported hat");

    let config_path = config_dir.join("ralph.yml");
    fs::write(
        &config_path,
        r"
cli:
  backend: claude
event_loop:
  completion_promise: LOOP_COMPLETE
hats:
  builder:
    import: ../shared-hats/builder.yml
",
    )
    .expect("write config");

    let output = ralph_preflight(
        temp_path,
        &[
            "--config",
            config_path.to_str().expect("utf8 config path"),
            "preflight",
            "--check",
            "config",
            "--format",
            "json",
        ],
    );

    assert!(
        output.status.success(),
        "preflight failed: stdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let json_start = stdout.find('{').expect("no json start");
    let json_end = stdout.rfind('}').expect("no json end");
    let json = &stdout[json_start..=json_end];
    let report: serde_json::Value = serde_json::from_str(json).expect("parse report");
    assert_eq!(report["passed"], true);
    assert_eq!(report["failures"], 0);
}
