use anyhow::Result;
use std::fs;
use std::process::Command;
use tempfile::TempDir;

/// Integration tests for clean command acceptance criteria.
///
/// Tests the `ralph clean` command which removes Ralph-generated artifacts
/// from the `.agent/` directory.

#[test]
fn test_clean_basic_success() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let temp_path = temp_dir.path();

    // Create config file
    let config_content = r#"
core:
  scratchpad: ".agent/scratchpad.md"
"#;
    fs::write(temp_path.join("ralph.yml"), config_content)?;

    // Create .agent directory with files
    let agent_dir = temp_path.join(".agent");
    fs::create_dir_all(&agent_dir)?;
    fs::write(agent_dir.join("scratchpad.md"), "test content")?;
    fs::write(agent_dir.join("events.jsonl"), "{}")?;
    fs::write(agent_dir.join("summary.md"), "# Summary")?;

    // Verify directory exists before cleanup
    assert!(agent_dir.exists());
    assert!(agent_dir.join("scratchpad.md").exists());

    // Run ralph clean
    let output = Command::new(env!("CARGO_BIN_EXE_ralph"))
        .arg("clean")
        .arg("--config")
        .arg(temp_path.join("ralph.yml"))
        .current_dir(temp_path)
        .output()?;

    // Should succeed
    assert!(output.status.success(), "Command should succeed");

    // Directory should be deleted
    assert!(!agent_dir.exists(), ".agent directory should be deleted");

    // Output should contain success message
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("cleaned") || stdout.contains("Cleaned") || stdout.contains("✓"));

    Ok(())
}

#[test]
fn test_clean_with_custom_config() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let temp_path = temp_dir.path();

    // Create custom config with different scratchpad path
    let config_content = r#"
core:
  scratchpad: "custom-agent/state.md"
"#;
    fs::write(temp_path.join("custom.yml"), config_content)?;

    // Create custom directory structure
    let custom_dir = temp_path.join("custom-agent");
    fs::create_dir_all(&custom_dir)?;
    fs::write(custom_dir.join("state.md"), "custom state")?;
    fs::write(custom_dir.join("data.json"), "{}")?;

    // Verify directory exists
    assert!(custom_dir.exists());

    // Run ralph clean with custom config
    let output = Command::new(env!("CARGO_BIN_EXE_ralph"))
        .arg("clean")
        .arg("--config")
        .arg(temp_path.join("custom.yml"))
        .current_dir(temp_path)
        .output()?;

    // Should succeed
    assert!(output.status.success());

    // Custom directory should be deleted
    assert!(
        !custom_dir.exists(),
        "custom-agent directory should be deleted"
    );

    Ok(())
}

#[test]
fn test_clean_dry_run() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let temp_path = temp_dir.path();

    // Create config
    let config_content = r#"
core:
  scratchpad: ".agent/scratchpad.md"
"#;
    fs::write(temp_path.join("ralph.yml"), config_content)?;

    // Create .agent directory with files
    let agent_dir = temp_path.join(".agent");
    fs::create_dir_all(&agent_dir)?;
    fs::write(agent_dir.join("scratchpad.md"), "test")?;
    fs::write(agent_dir.join("events.jsonl"), "{}")?;

    // Run ralph clean with --dry-run
    let output = Command::new(env!("CARGO_BIN_EXE_ralph"))
        .arg("clean")
        .arg("--config")
        .arg(temp_path.join("ralph.yml"))
        .arg("--dry-run")
        .current_dir(temp_path)
        .output()?;

    // Should succeed
    assert!(output.status.success());

    // Directory should still exist
    assert!(
        agent_dir.exists(),
        ".agent directory should still exist after dry-run"
    );
    assert!(agent_dir.join("scratchpad.md").exists());

    // Output should mention dry-run or preview
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Would delete") || stdout.contains("dry run") || stdout.contains("Dry run"),
        "Output should indicate dry-run mode"
    );

    Ok(())
}

#[test]
fn test_clean_missing_directory() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let temp_path = temp_dir.path();

    // Create config
    let config_content = r#"
core:
  scratchpad: ".agent/scratchpad.md"
"#;
    fs::write(temp_path.join("ralph.yml"), config_content)?;

    // Don't create .agent directory
    let agent_dir = temp_path.join(".agent");
    assert!(!agent_dir.exists());

    // Run ralph clean
    let output = Command::new(env!("CARGO_BIN_EXE_ralph"))
        .arg("clean")
        .arg("--config")
        .arg(temp_path.join("ralph.yml"))
        .current_dir(temp_path)
        .output()?;

    // Should succeed (not an error)
    assert!(
        output.status.success(),
        "Command should succeed even when directory doesn't exist"
    );

    // Output should indicate nothing to clean
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("not found")
            || stdout.contains("does not exist")
            || stdout.contains("Nothing to clean"),
        "Output should indicate directory doesn't exist"
    );

    Ok(())
}

#[test]
fn test_clean_color_output_never() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let temp_path = temp_dir.path();

    // Create config and .agent directory
    let config_content = r#"
core:
  scratchpad: ".agent/scratchpad.md"
"#;
    fs::write(temp_path.join("ralph.yml"), config_content)?;

    let agent_dir = temp_path.join(".agent");
    fs::create_dir_all(&agent_dir)?;
    fs::write(agent_dir.join("scratchpad.md"), "test")?;

    // Run ralph clean with --color never
    let output = Command::new(env!("CARGO_BIN_EXE_ralph"))
        .arg("clean")
        .arg("--config")
        .arg(temp_path.join("ralph.yml"))
        .arg("--color")
        .arg("never")
        .current_dir(temp_path)
        .output()?;

    assert!(output.status.success());

    // Output should not contain ANSI color codes
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains("\x1b["),
        "Output should not contain ANSI escape codes when --color never is used"
    );

    Ok(())
}

#[test]
fn test_clean_color_output_always() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let temp_path = temp_dir.path();

    // Create config and .agent directory
    let config_content = r#"
core:
  scratchpad: ".agent/scratchpad.md"
"#;
    fs::write(temp_path.join("ralph.yml"), config_content)?;

    let agent_dir = temp_path.join(".agent");
    fs::create_dir_all(&agent_dir)?;
    fs::write(agent_dir.join("scratchpad.md"), "test")?;

    // Run ralph clean with --color always
    let output = Command::new(env!("CARGO_BIN_EXE_ralph"))
        .arg("clean")
        .arg("--config")
        .arg(temp_path.join("ralph.yml"))
        .arg("--color")
        .arg("always")
        .current_dir(temp_path)
        .output()?;

    assert!(output.status.success());

    // Output should contain ANSI color codes
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("\x1b["),
        "Output should contain ANSI escape codes when --color always is used"
    );

    Ok(())
}

#[test]
#[cfg(unix)]
fn test_clean_permission_error() -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let temp_dir = TempDir::new()?;
    let temp_path = temp_dir.path();

    // Create config
    let config_content = r#"
core:
  scratchpad: ".agent/scratchpad.md"
"#;
    fs::write(temp_path.join("ralph.yml"), config_content)?;

    // Create .agent directory with files
    let agent_dir = temp_path.join(".agent");
    fs::create_dir_all(&agent_dir)?;
    fs::write(agent_dir.join("scratchpad.md"), "test")?;

    // Make directory read-only (remove write permissions)
    let mut perms = fs::metadata(&agent_dir)?.permissions();
    perms.set_mode(0o444); // Read-only
    fs::set_permissions(&agent_dir, perms)?;

    // Run ralph clean
    let output = Command::new(env!("CARGO_BIN_EXE_ralph"))
        .arg("clean")
        .arg("--config")
        .arg(temp_path.join("ralph.yml"))
        .current_dir(temp_path)
        .output()?;

    // Restore permissions for cleanup
    let mut perms = fs::metadata(&agent_dir)?.permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&agent_dir, perms)?;

    // Should fail with non-zero exit code
    assert!(
        !output.status.success(),
        "Command should fail with permission error"
    );

    // Should contain error message
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("permission") || stderr.contains("Permission") || stderr.contains("Failed"),
        "Error message should mention permission issue"
    );

    Ok(())
}
