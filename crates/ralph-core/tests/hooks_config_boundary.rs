use ralph_core::{ConfigError, RalphConfig};
use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;

fn write_config_file(temp_dir: &TempDir, yaml: &str) -> PathBuf {
    let path = temp_dir.path().join("ralph.yml");
    fs::write(&path, yaml).expect("failed to write temporary config file");
    path
}

#[test]
fn test_hooks_config_boundary_accepts_valid_file() {
    let temp_dir = TempDir::new().expect("failed to create temporary directory");
    let config_path = write_config_file(
        &temp_dir,
        r"
hooks:
  enabled: true
  defaults:
    timeout_seconds: 45
    max_output_bytes: 16384
    suspend_mode: wait_for_resume
  events:
    pre.loop.start:
      - name: env-guard
        command: [./scripts/hooks/env-guard.sh, --check]
        on_error: block
",
    );

    let config =
        RalphConfig::from_file(&config_path).expect("expected valid hooks config to parse");
    let warnings = config
        .validate()
        .expect("expected valid hooks config to pass validation");

    assert!(warnings.is_empty());
    assert!(config.hooks.enabled);
    assert_eq!(config.hooks.events.len(), 1);
}

#[test]
fn test_hooks_config_boundary_rejects_invalid_phase_event_key() {
    let temp_dir = TempDir::new().expect("failed to create temporary directory");
    let config_path = write_config_file(
        &temp_dir,
        r"
hooks:
  enabled: true
  events:
    pre.loop.launch:
      - name: invalid-phase
        command: [./scripts/hooks/invalid-phase.sh]
        on_error: warn
",
    );

    let err = RalphConfig::from_file(&config_path).expect_err("expected invalid phase-event key");
    assert!(matches!(
        err,
        ConfigError::InvalidHookPhaseEvent { phase_event }
            if phase_event == "pre.loop.launch"
    ));
}

#[test]
fn test_hooks_config_boundary_rejects_non_v1_scope_field() {
    let temp_dir = TempDir::new().expect("failed to create temporary directory");
    let config_path = write_config_file(
        &temp_dir,
        r"
hooks:
  enabled: true
  events:
    pre.loop.start:
      - name: scope-field
        command: [./scripts/hooks/scope.sh]
        on_error: warn
        scope: global
",
    );

    let config = RalphConfig::from_file(&config_path).expect("expected hooks YAML to parse");
    let err = config
        .validate()
        .expect_err("expected unsupported scope field to fail validation");

    assert!(matches!(
        err,
        ConfigError::UnsupportedHookField { field, .. }
            if field == "hooks.events.pre.loop.start[0].scope"
    ));
}

#[test]
fn test_hooks_config_boundary_rejects_mutation_non_json_format() {
    let temp_dir = TempDir::new().expect("failed to create temporary directory");
    let config_path = write_config_file(
        &temp_dir,
        r"
hooks:
  enabled: true
  events:
    pre.loop.start:
      - name: mutate-format
        command: [./scripts/hooks/mutate.sh]
        on_error: warn
        mutate:
          enabled: true
          format: xml
",
    );

    let config = RalphConfig::from_file(&config_path).expect("expected hooks YAML to parse");
    let err = config
        .validate()
        .expect_err("expected non-json mutate format to fail validation");

    assert!(matches!(
        err,
        ConfigError::HookValidation { field, .. }
            if field == "hooks.events.pre.loop.start[0].mutate.format"
    ));
}

#[test]
fn test_hooks_config_boundary_preserves_backwards_compat_without_hooks() {
    let temp_dir = TempDir::new().expect("failed to create temporary directory");
    let config_path = write_config_file(&temp_dir, "agent: claude\n");

    let config = RalphConfig::from_file(&config_path).expect("expected legacy config to parse");
    let warnings = config
        .validate()
        .expect("expected legacy config to pass validation");

    assert!(warnings.is_empty());
    assert!(!config.hooks.enabled);
    assert!(config.hooks.events.is_empty());
}
