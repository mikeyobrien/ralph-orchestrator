use anyhow::Result;
use std::fs;
use std::process::Command;
use tempfile::TempDir;

#[test]
fn test_list_presets_includes_confession_loop() -> Result<()> {
    let output = Command::new(env!("CARGO_BIN_EXE_ralph"))
        .arg("init")
        .arg("--list-presets")
        .output()?;

    assert!(output.status.success(), "Expected command to succeed");

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("confession-loop"),
        "Expected preset list to include confession-loop, got:\n{stdout}"
    );

    Ok(())
}
#[test]
fn test_init_from_confession_loop_preset_creates_config() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let temp_path = temp_dir.path();

    let output = Command::new(env!("CARGO_BIN_EXE_ralph"))
        .arg("init")
        .arg("--preset")
        .arg("confession-loop")
        .current_dir(temp_path)
        .output()?;

    assert!(
        output.status.success(),
        "Expected init to succeed, stderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let config = fs::read_to_string(temp_path.join("ralph.yml"))?;
    assert!(
        config.contains("confession.issues_found") || config.contains("confession.clean"),
        "Expected config to define confession events"
    );

    Ok(())
}
