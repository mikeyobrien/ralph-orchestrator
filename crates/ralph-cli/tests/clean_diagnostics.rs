use std::fs;
use tempfile::TempDir;

/// Test that clean_diagnostics removes the .ralph/diagnostics directory
#[test]
fn test_clean_diagnostics_removes_directory() {
    let temp = TempDir::new().unwrap();
    let diagnostics_dir = temp.path().join(".ralph/diagnostics");

    // Create diagnostics directory with some session data
    fs::create_dir_all(&diagnostics_dir).unwrap();
    let session_dir = diagnostics_dir.join("2024-01-15T10-23-45");
    fs::create_dir_all(&session_dir).unwrap();
    fs::write(session_dir.join("agent-output.jsonl"), "test data").unwrap();

    assert!(diagnostics_dir.exists());

    // Clean diagnostics (use_colors=false, dry_run=false)
    ralph_cli::clean_diagnostics(temp.path(), false, false).unwrap();

    // Directory should be gone
    assert!(!diagnostics_dir.exists());
}

/// Test that clean_diagnostics handles non-existent directory gracefully
#[test]
fn test_clean_diagnostics_handles_missing_directory() {
    let temp = TempDir::new().unwrap();
    let diagnostics_dir = temp.path().join(".ralph/diagnostics");

    assert!(!diagnostics_dir.exists());

    // Should not error
    let result = ralph_cli::clean_diagnostics(temp.path(), false, false);
    assert!(result.is_ok());
}

/// Test that clean_diagnostics only removes diagnostics, not other .ralph contents
#[test]
fn test_clean_diagnostics_preserves_other_ralph_contents() {
    let temp = TempDir::new().unwrap();
    let ralph_dir = temp.path().join(".ralph");
    let diagnostics_dir = ralph_dir.join("diagnostics");
    let events_file = ralph_dir.join("events.jsonl");

    // Create both diagnostics and other .ralph contents
    fs::create_dir_all(&diagnostics_dir).unwrap();
    fs::write(&events_file, "event data").unwrap();

    assert!(diagnostics_dir.exists());
    assert!(events_file.exists());

    // Clean diagnostics
    ralph_cli::clean_diagnostics(temp.path(), false, false).unwrap();

    // Diagnostics gone, events preserved
    assert!(!diagnostics_dir.exists());
    assert!(events_file.exists());
}
