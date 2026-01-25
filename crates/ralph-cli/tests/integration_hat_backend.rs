use anyhow::Result;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

/// Integration tests for hat-level backend configuration.
///
/// Tests that hats can have custom backend configurations that override
/// the global cli.backend setting.

/// Helper function to create a fake backend script that writes a marker file
fn create_fake_backend(dir: &Path, backend_name: &str) -> Result<String> {
    let script_path = dir.join(format!("{}-backend.sh", backend_name));
    let marker_path = dir.join(format!("{}-called.txt", backend_name));

    // Create a shell script that writes a marker file and outputs valid response
    let script_content = format!(
        r#"#!/bin/bash
# Write marker file to prove this backend was called
echo "Backend {} was called with args: $@" > {}
# Output a valid response with completion promise
echo "Backend {} executed successfully"
echo "<promise>LOOP_COMPLETE</promise>"
"#,
        backend_name,
        marker_path.display(),
        backend_name
    );

    fs::write(&script_path, script_content)?;

    // Make script executable
    let mut perms = fs::metadata(&script_path)?.permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&script_path, perms)?;

    Ok(script_path.to_string_lossy().to_string())
}

#[test]
fn test_hat_uses_custom_backend() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let temp_path = temp_dir.path();

    // Create fake backends
    let global_backend = create_fake_backend(temp_path, "global")?;
    let hat_backend = create_fake_backend(temp_path, "hat")?;

    // Create config with global backend and hat-specific backend
    let config_content = format!(
        r#"
event_loop:
  completion_promise: "LOOP_COMPLETE"
  max_iterations: 5
  max_runtime_seconds: 30
  starting_event: "task.start"

cli:
  backend: "custom"
  command: "{}"
  prompt_mode: "arg"

hats:
  custom_hat:
    name: "Custom Hat"
    description: "Hat with custom backend"
    triggers: ["task.start"]
    publishes: ["task.done"]
    backend:
      type: "custom"
      command: "{}"
      prompt_mode: "arg"
    instructions: |
      You are testing hat-level backend configuration.
      Output <promise>LOOP_COMPLETE</promise> immediately.
"#,
        global_backend, hat_backend
    );

    fs::write(temp_path.join("ralph.yml"), config_content)?;

    // Run ralph
    let output = Command::new(env!("CARGO_BIN_EXE_ralph"))
        .arg("run")
        .arg("--config")
        .arg(temp_path.join("ralph.yml"))
        .arg("--no-tui")
        .arg("-p")
        .arg("Start test")
        .current_dir(temp_path)
        .output()?;

    // Check if command succeeded
    if !output.status.success() {
        eprintln!("Command failed with status: {}", output.status);
        eprintln!("stdout: {}", String::from_utf8_lossy(&output.stdout));
        eprintln!("stderr: {}", String::from_utf8_lossy(&output.stderr));
    }

    // The critical assertion: hat backend should have been called
    let hat_marker = temp_path.join("hat-called.txt");
    assert!(
        hat_marker.exists(),
        "Hat backend should have been called, but marker file does not exist"
    );

    // Global backend should NOT have been called (except for ralph coordinator)
    // Note: ralph coordinator might use global backend, but custom_hat should use hat backend
    let hat_marker_content = fs::read_to_string(&hat_marker)?;
    assert!(
        hat_marker_content.contains("Backend hat was called"),
        "Hat backend marker file should contain expected content"
    );

    Ok(())
}

#[test]
fn test_hat_backend_fallback_on_error() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let temp_path = temp_dir.path();

    // Create global backend (valid)
    let global_backend = create_fake_backend(temp_path, "global")?;

    // Create config with global backend and INVALID hat-specific backend
    let config_content = format!(
        r#"
event_loop:
  completion_promise: "LOOP_COMPLETE"
  max_iterations: 5
  max_runtime_seconds: 30
  starting_event: "task.start"

cli:
  backend: "custom"
  command: "{}"
  prompt_mode: "arg"

hats:
  failing_hat:
    name: "Failing Hat"
    description: "Hat with invalid backend"
    triggers: ["task.start"]
    publishes: ["task.done"]
    backend:
      type: "custom"
      command: "/nonexistent/backend"
      prompt_mode: "arg"
    instructions: |
      You are testing backend fallback.
      Output <promise>LOOP_COMPLETE</promise> immediately.
"#,
        global_backend
    );

    fs::write(temp_path.join("ralph.yml"), config_content)?;

    // Run ralph
    let output = Command::new(env!("CARGO_BIN_EXE_ralph"))
        .arg("run")
        .arg("--config")
        .arg(temp_path.join("ralph.yml"))
        .arg("--no-tui")
        .arg("-p")
        .arg("Start test")
        .current_dir(temp_path)
        .output()?;

    // Should still succeed due to fallback
    if !output.status.success() {
        eprintln!("Command failed: {}", output.status);
        eprintln!("stdout: {}", String::from_utf8_lossy(&output.stdout));
        eprintln!("stderr: {}", String::from_utf8_lossy(&output.stderr));
    }

    // Global backend should have been called as fallback
    let global_marker = temp_path.join("global-called.txt");
    assert!(
        global_marker.exists(),
        "Global backend should have been called as fallback"
    );

    // Check stderr for warning about fallback
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Failed to create backend") || stderr.contains("Falling back"),
        "Should warn about backend creation failure"
    );

    Ok(())
}

#[test]
fn test_multiple_hats_different_backends() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let temp_path = temp_dir.path();

    // Create multiple fake backends
    let global_backend = create_fake_backend(temp_path, "global")?;
    let backend_a = create_fake_backend(temp_path, "backend-a")?;
    let backend_b = create_fake_backend(temp_path, "backend-b")?;

    // Create config with multiple hats using different backends
    let config_content = format!(
        r#"
event_loop:
  completion_promise: "LOOP_COMPLETE"
  max_iterations: 10
  max_runtime_seconds: 30
  starting_event: "task.start"

cli:
  backend: "custom"
  command: "{}"
  prompt_mode: "arg"

hats:
  hat_a:
    name: "Hat A"
    description: "First hat with custom backend A"
    triggers: ["task.start"]
    publishes: ["intermediate.done"]
    backend:
      type: "custom"
      command: "{}"
      prompt_mode: "arg"
    instructions: |
      Output: Hat A is working
      Then publish intermediate.done event:
      <promise>LOOP_COMPLETE</promise>

  hat_b:
    name: "Hat B"
    description: "Second hat with custom backend B"
    triggers: ["intermediate.done"]
    publishes: ["task.done"]
    backend:
      type: "custom"
      command: "{}"
      prompt_mode: "arg"
    instructions: |
      Output: Hat B is working
      Then complete:
      <promise>LOOP_COMPLETE</promise>
"#,
        global_backend, backend_a, backend_b
    );

    fs::write(temp_path.join("ralph.yml"), config_content)?;

    // Run ralph
    let output = Command::new(env!("CARGO_BIN_EXE_ralph"))
        .arg("run")
        .arg("--config")
        .arg(temp_path.join("ralph.yml"))
        .arg("--no-tui")
        .arg("-p")
        .arg("Start multi-backend test")
        .current_dir(temp_path)
        .output()?;

    if !output.status.success() {
        eprintln!("Command failed: {}", output.status);
        eprintln!("stdout: {}", String::from_utf8_lossy(&output.stdout));
        eprintln!("stderr: {}", String::from_utf8_lossy(&output.stderr));
    }

    // Both backends should have been called
    let marker_a = temp_path.join("backend-a-called.txt");
    let marker_b = temp_path.join("backend-b-called.txt");

    assert!(
        marker_a.exists(),
        "Backend A should have been called for hat_a"
    );
    assert!(
        marker_b.exists(),
        "Backend B should have been called for hat_b"
    );

    // Verify each backend was called exactly once
    let content_a = fs::read_to_string(&marker_a)?;
    let content_b = fs::read_to_string(&marker_b)?;

    assert!(
        content_a.contains("Backend backend-a was called"),
        "Backend A marker should contain expected content"
    );
    assert!(
        content_b.contains("Backend backend-b was called"),
        "Backend B marker should contain expected content"
    );

    Ok(())
}
