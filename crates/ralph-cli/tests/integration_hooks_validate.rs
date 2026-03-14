//! Integration tests for `ralph hooks validate` CLI command.

use serde_json::Value;
use std::fs;
use std::path::Path;
use std::process::{Command, Output};
use tempfile::TempDir;

fn ralph_hooks_validate(temp_path: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ralph"))
        .args(["--color", "never"])
        .args(args)
        .current_dir(temp_path)
        .env("NO_COLOR", "1")
        .output()
        .expect("Failed to execute ralph hooks validate command")
}

fn ralph_hooks_validate_with_home(temp_path: &Path, home_path: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ralph"))
        .args(["--color", "never"])
        .args(args)
        .current_dir(temp_path)
        .env("HOME", home_path)
        .env("USERPROFILE", home_path)
        .env("NO_COLOR", "1")
        .output()
        .expect("Failed to execute ralph hooks validate command")
}

fn parse_json_report(output: &Output) -> Value {
    let stdout = String::from_utf8_lossy(&output.stdout);
    let json_start = stdout.find('{').expect("no json start");
    let json_end = stdout.rfind('}').expect("no json end");
    serde_json::from_str(&stdout[json_start..=json_end]).expect("parse hooks validate report")
}

fn assert_report_shape(report: &Value) {
    let object = report.as_object().expect("report should be a JSON object");
    for key in [
        "pass",
        "source",
        "hooks_enabled",
        "checked_hooks",
        "diagnostics",
    ] {
        assert!(object.contains_key(key), "report missing key '{key}'");
    }

    assert!(report["pass"].is_boolean(), "pass should be bool");
    assert!(report["source"].is_string(), "source should be string");
    assert!(
        report["hooks_enabled"].is_boolean(),
        "hooks_enabled should be bool"
    );
    assert!(
        report["checked_hooks"].is_number(),
        "checked_hooks should be number"
    );

    let diagnostics = report["diagnostics"]
        .as_array()
        .expect("diagnostics should be an array");

    for diagnostic in diagnostics {
        assert!(
            diagnostic.get("code").and_then(Value::as_str).is_some(),
            "diagnostic code should be string"
        );
        assert!(
            diagnostic.get("message").and_then(Value::as_str).is_some(),
            "diagnostic message should be string"
        );
    }
}

fn make_executable(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let mut permissions = fs::metadata(path).expect("metadata").permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(path, permissions).expect("set executable permissions");
    }
}

#[test]
fn test_hooks_validate_json_success_report_and_exit_code() {
    let temp_dir = TempDir::new().expect("temp dir");
    let temp_path = temp_dir.path();

    let hook_path = temp_path.join("hook-ok.sh");
    fs::write(&hook_path, "#!/bin/sh\nexit 0\n").expect("write hook script");
    make_executable(&hook_path);

    let config_path = temp_path.join("ralph.yml");
    fs::write(
        &config_path,
        r#"
hooks:
  enabled: true
  events:
    pre.loop.start:
      - name: hook-ok
        command: ["./hook-ok.sh"]
        on_error: warn
"#,
    )
    .expect("write config");

    let output = ralph_hooks_validate(
        temp_path,
        &[
            "--config",
            config_path.to_str().expect("config path to str"),
            "hooks",
            "validate",
            "--format",
            "json",
        ],
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(0),
        "expected exit code 0; stderr: {stderr}\nstdout: {stdout}"
    );

    let report = parse_json_report(&output);
    assert_report_shape(&report);

    assert_eq!(report["pass"], true);
    assert_eq!(report["hooks_enabled"], true);
    assert_eq!(report["checked_hooks"], 1);

    let diagnostics = report["diagnostics"].as_array().expect("diagnostics array");
    assert!(diagnostics.is_empty(), "expected no diagnostics: {report}");
}

#[test]
fn test_hooks_validate_malformed_config_fails_with_json_diagnostic() {
    let temp_dir = TempDir::new().expect("temp dir");
    let temp_path = temp_dir.path();

    let config_path = temp_path.join("ralph.yml");
    fs::write(&config_path, "hooks: [\n").expect("write malformed config");

    let output = ralph_hooks_validate(
        temp_path,
        &[
            "--config",
            config_path.to_str().expect("config path to str"),
            "hooks",
            "validate",
            "--format",
            "json",
        ],
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(1),
        "expected exit code 1; stderr: {stderr}\nstdout: {stdout}"
    );

    let report = parse_json_report(&output);
    assert_report_shape(&report);

    assert_eq!(report["pass"], false);
    assert_eq!(report["hooks_enabled"], false);
    assert_eq!(report["checked_hooks"], 0);

    let diagnostics = report["diagnostics"].as_array().expect("diagnostics array");
    assert!(
        !diagnostics.is_empty(),
        "expected diagnostic in report: {report}"
    );
    assert_eq!(diagnostics[0]["code"], "config.load");

    let message = diagnostics[0]["message"]
        .as_str()
        .expect("diagnostic message string");
    assert!(
        message.contains("Failed to parse YAML") || message.contains("YAML parse error"),
        "expected parse-yaml diagnostic, got: {message}"
    );
}

#[test]
fn test_hooks_validate_unknown_phase_event_fails_with_json_diagnostic() {
    let temp_dir = TempDir::new().expect("temp dir");
    let temp_path = temp_dir.path();

    let config_path = temp_path.join("ralph.yml");
    fs::write(
        &config_path,
        r#"
hooks:
  enabled: true
  events:
    pre.loop.launch:
      - name: bad-phase
        command: ["./hook-ok.sh"]
        on_error: warn
"#,
    )
    .expect("write config");

    let output = ralph_hooks_validate(
        temp_path,
        &[
            "--config",
            config_path.to_str().expect("config path to str"),
            "hooks",
            "validate",
            "--format",
            "json",
        ],
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(1),
        "expected exit code 1; stderr: {stderr}\nstdout: {stdout}"
    );

    let report = parse_json_report(&output);
    assert_report_shape(&report);

    assert_eq!(report["pass"], false);
    let diagnostics = report["diagnostics"].as_array().expect("diagnostics array");
    assert!(
        !diagnostics.is_empty(),
        "expected diagnostic in report: {report}"
    );
    assert_eq!(diagnostics[0]["code"], "config.load");

    let message = diagnostics[0]["message"]
        .as_str()
        .expect("diagnostic message string");
    assert!(
        message.contains("Failed to parse merged core config"),
        "expected merged-config parse failure diagnostic, got: {message}"
    );
}

#[test]
fn test_hooks_validate_uses_user_scoped_hooks_when_local_config_missing() {
    let temp_dir = TempDir::new().expect("temp dir");
    let home_dir = TempDir::new().expect("temp home");
    let temp_path = temp_dir.path();
    let hook_path = temp_path.join("hook-ok.sh");
    fs::write(&hook_path, "#!/bin/sh\nexit 0\n").expect("write hook script");
    make_executable(&hook_path);

    let user_config_dir = home_dir.path().join(".ralph");
    fs::create_dir_all(&user_config_dir).expect("create user config dir");
    fs::write(
        user_config_dir.join("config.yml"),
        r#"
hooks:
  enabled: true
  events:
    pre.loop.start:
      - name: user-hook
        command: ["./hook-ok.sh"]
        on_error: warn
"#,
    )
    .expect("write user config");

    let output = ralph_hooks_validate_with_home(
        temp_path,
        home_dir.path(),
        &["hooks", "validate", "--format", "json"],
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        output.status.code(),
        Some(0),
        "expected exit code 0; stderr: {stderr}\nstdout: {stdout}"
    );

    let report = parse_json_report(&output);
    assert_report_shape(&report);
    assert_eq!(report["pass"], true);
    assert_eq!(report["hooks_enabled"], true);
    assert_eq!(report["checked_hooks"], 1);
    let source = report["source"].as_str().expect("source should be string");
    assert!(
        source.contains(".ralph/config.yml"),
        "expected global config source, got: {source}"
    );
}
