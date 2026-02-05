//! Integration tests for event isolation feature (Issue #82 fix).
//!
//! These tests verify that consecutive Hats runs get isolated events files,
//! preventing stale events from previous runs from polluting new runs.
//!
//! The event isolation mechanism:
//! 1. Fresh runs create `.hats/events-YYYYMMDD-HHMMSS.jsonl` timestamped files
//! 2. `.hats/current-events` marker file coordinates between Hats and `hats emit`
//! 3. Continue mode (`hats run --continue`) reuses the existing marker file
//! 4. Fallback to `.hats/events.jsonl` when no marker exists

use anyhow::Result;
use std::fs;
use std::process::Command;
use std::thread;
use std::time::Duration;
use tempfile::TempDir;

/// Creates a minimal config file and required directories for testing.
fn create_test_config(temp_path: &std::path::Path) -> Result<()> {
    let config = r#"
event_loop:
  prompt_file: "PROMPT.md"
  completion_promise: "LOOP_COMPLETE"
  max_iterations: 1
  max_runtime_seconds: 5

cli:
  backend: "custom"
  command: "true"

core:
  scratchpad: ".hats/agent/scratchpad.md"

features:
  preflight:
    enabled: false
"#;
    fs::write(temp_path.join("hats.yml"), config)?;
    fs::write(temp_path.join("PROMPT.md"), "Test task")?;
    Ok(())
}

/// Helper to get hats binary path.
fn hats_bin() -> &'static str {
    env!("CARGO_BIN_EXE_hats")
}

// =============================================================================
// Marker File Tests
// =============================================================================

#[test]
fn test_fresh_run_creates_marker_file() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let temp_path = temp_dir.path();

    create_test_config(temp_path)?;

    // Run hats
    let _output = Command::new(hats_bin())
        .arg("run")
        .arg("--config")
        .arg(temp_path.join("hats.yml"))
        .current_dir(temp_path)
        .output()?;

    // Verify .hats/current-events marker file exists
    let marker_path = temp_path.join(".hats/current-events");
    assert!(
        marker_path.exists(),
        ".hats/current-events marker file should exist after fresh run"
    );

    // Verify marker content points to a timestamped events file
    let marker_content = fs::read_to_string(&marker_path)?;
    let events_path = marker_content.trim();

    assert!(
        events_path.starts_with(".hats/events-"),
        "Marker should point to timestamped events file, got: {}",
        events_path
    );
    assert!(
        std::path::Path::new(events_path)
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("jsonl")),
        "Events file should have .jsonl extension, got: {}",
        events_path
    );

    Ok(())
}

#[test]
fn test_marker_file_contains_timestamped_path() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let temp_path = temp_dir.path();

    create_test_config(temp_path)?;

    // Run hats
    let _output = Command::new(hats_bin())
        .arg("run")
        .arg("--config")
        .arg(temp_path.join("hats.yml"))
        .current_dir(temp_path)
        .output()?;

    // Read the marker to find the events file path
    let marker_content = fs::read_to_string(temp_path.join(".hats/current-events"))?;
    let events_path = marker_content.trim();

    // Verify the marker contains a timestamped path pattern
    // Pattern: .hats/events-YYYYMMDD-HHMMSS.jsonl
    let re = regex::Regex::new(r"^\.hats/events-\d{8}-\d{6}\.jsonl$").unwrap();
    assert!(
        re.is_match(events_path),
        "Marker should contain path matching .hats/events-YYYYMMDD-HHMMSS.jsonl, got: {}",
        events_path
    );

    Ok(())
}

#[test]
fn test_hats_emit_creates_timestamped_events_file() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let temp_path = temp_dir.path();

    create_test_config(temp_path)?;

    // Run hats to create marker file
    let _output = Command::new(hats_bin())
        .arg("run")
        .arg("--config")
        .arg(temp_path.join("hats.yml"))
        .current_dir(temp_path)
        .output()?;

    // Read the marker to find the events file path
    let marker_content = fs::read_to_string(temp_path.join(".hats/current-events"))?;
    let events_path = marker_content.trim();

    // The timestamped file doesn't exist yet (EventLogger writes to default path)
    let timestamped_file = temp_path.join(events_path);

    // Use hats emit to write to the marker-specified file
    let output = Command::new(hats_bin())
        .arg("emit")
        .arg("test.topic")
        .arg("test payload")
        .current_dir(temp_path)
        .output()?;

    assert!(
        output.status.success(),
        "hats emit should succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Now the timestamped file should exist (hats emit reads marker)
    assert!(
        timestamped_file.exists(),
        "Timestamped events file should exist after hats emit: {}",
        timestamped_file.display()
    );

    // Verify the filename matches pattern
    let filename = timestamped_file
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("");

    let re = regex::Regex::new(r"^events-\d{8}-\d{6}\.jsonl$").unwrap();
    assert!(
        re.is_match(filename),
        "Events filename should match pattern events-YYYYMMDD-HHMMSS.jsonl, got: {}",
        filename
    );

    Ok(())
}

// =============================================================================
// Consecutive Runs Isolation Tests
// =============================================================================

#[test]
fn test_consecutive_runs_get_isolated_marker_paths() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let temp_path = temp_dir.path();

    create_test_config(temp_path)?;

    // First run
    let _output1 = Command::new(hats_bin())
        .arg("run")
        .arg("--config")
        .arg(temp_path.join("hats.yml"))
        .current_dir(temp_path)
        .output()?;

    // Read first run's events path from marker
    let marker1_content = fs::read_to_string(temp_path.join(".hats/current-events"))?;
    let events_path1 = marker1_content.trim().to_string();

    // Small delay to ensure different timestamp
    thread::sleep(Duration::from_secs(1));

    // Second run
    let _output2 = Command::new(hats_bin())
        .arg("run")
        .arg("--config")
        .arg(temp_path.join("hats.yml"))
        .current_dir(temp_path)
        .output()?;

    // Read second run's events path from marker
    let marker2_content = fs::read_to_string(temp_path.join(".hats/current-events"))?;
    let events_path2 = marker2_content.trim().to_string();

    // Verify different events paths are in marker (isolation)
    assert_ne!(
        events_path1, events_path2,
        "Consecutive runs should create different marker paths.\nRun 1: {}\nRun 2: {}",
        events_path1, events_path2
    );

    // Verify both are timestamped paths
    let re = regex::Regex::new(r"^\.hats/events-\d{8}-\d{6}\.jsonl$").unwrap();
    assert!(
        re.is_match(&events_path1),
        "First run path should be timestamped: {}",
        events_path1
    );
    assert!(
        re.is_match(&events_path2),
        "Second run path should be timestamped: {}",
        events_path2
    );

    Ok(())
}

// =============================================================================
// Hats Emit Coordination Tests
// =============================================================================

#[test]
fn test_hats_emit_writes_to_marker_specified_file() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let temp_path = temp_dir.path();

    create_test_config(temp_path)?;

    // Run hats to create marker file
    let _output = Command::new(hats_bin())
        .arg("run")
        .arg("--config")
        .arg(temp_path.join("hats.yml"))
        .current_dir(temp_path)
        .output()?;

    // Read the marker to find the events file path
    let marker_content = fs::read_to_string(temp_path.join(".hats/current-events"))?;
    let events_path = marker_content.trim();

    // Get the events file content before emit
    let events_file = temp_path.join(events_path);
    let events_before = if events_file.exists() {
        fs::read_to_string(&events_file)?
    } else {
        String::new()
    };
    let lines_before = events_before.lines().count();

    // Use hats emit to write an event
    let output = Command::new(hats_bin())
        .arg("emit")
        .arg("test.topic")
        .arg("test payload")
        .current_dir(temp_path)
        .output()?;

    assert!(
        output.status.success(),
        "hats emit should succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Verify event was written to the marker-specified file
    let events_after = fs::read_to_string(&events_file)?;
    let lines_after = events_after.lines().count();

    assert!(
        lines_after > lines_before,
        "Event should be written to timestamped events file.\nBefore: {} lines\nAfter: {} lines",
        lines_before,
        lines_after
    );

    // Verify the event content
    assert!(
        events_after.contains("test.topic"),
        "Events file should contain the emitted topic"
    );
    assert!(
        events_after.contains("test payload"),
        "Events file should contain the emitted payload"
    );

    // Verify the event was NOT written to default fallback location
    let fallback_file = temp_path.join(".hats/events.jsonl");
    if fallback_file.exists() {
        let fallback_content = fs::read_to_string(&fallback_file)?;
        assert!(
            !fallback_content.contains("test.topic"),
            "Event should NOT be written to fallback .hats/events.jsonl"
        );
    }

    Ok(())
}

#[test]
fn test_hats_emit_fallback_without_marker() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let temp_path = temp_dir.path();

    create_test_config(temp_path)?;

    // Create .hats directory but NO marker file
    fs::create_dir_all(temp_path.join(".hats"))?;

    // Verify no marker file exists
    let marker_path = temp_path.join(".hats/current-events");
    assert!(
        !marker_path.exists(),
        "Marker file should not exist for this test"
    );

    // Use hats emit (should fall back to default)
    let output = Command::new(hats_bin())
        .arg("emit")
        .arg("fallback.topic")
        .arg("fallback payload")
        .current_dir(temp_path)
        .output()?;

    assert!(
        output.status.success(),
        "hats emit should succeed even without marker: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    // Verify event was written to fallback .hats/events.jsonl
    let fallback_file = temp_path.join(".hats/events.jsonl");
    assert!(
        fallback_file.exists(),
        "Fallback events.jsonl should be created"
    );

    let fallback_content = fs::read_to_string(&fallback_file)?;
    assert!(
        fallback_content.contains("fallback.topic"),
        "Fallback file should contain the emitted topic"
    );
    assert!(
        fallback_content.contains("fallback payload"),
        "Fallback file should contain the emitted payload"
    );

    Ok(())
}

// =============================================================================
// Continue Mode Tests
// =============================================================================

#[test]
fn test_continue_uses_existing_marker_file() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let temp_path = temp_dir.path();

    create_test_config(temp_path)?;

    // First: run hats to create marker file
    let _output = Command::new(hats_bin())
        .arg("run")
        .arg("--config")
        .arg(temp_path.join("hats.yml"))
        .current_dir(temp_path)
        .output()?;

    // Read the marker from first run
    let marker_content_after_run = fs::read_to_string(temp_path.join(".hats/current-events"))?;
    let events_path_after_run = marker_content_after_run.trim().to_string();

    // Create scratchpad for continue (required by continue mode)
    let agent_dir = temp_path.join(".agent");
    fs::create_dir_all(&agent_dir)?;
    fs::write(
        agent_dir.join("scratchpad.md"),
        "# Tasks\n- [ ] Test task\n",
    )?;

    // Continue - should NOT create a new marker/events file
    let _output = Command::new(hats_bin())
        .arg("run")
        .arg("--continue")
        .arg("--config")
        .arg(temp_path.join("hats.yml"))
        .current_dir(temp_path)
        .output()?;

    // Read the marker after continue
    let marker_content_after_continue =
        fs::read_to_string(temp_path.join(".hats/current-events"))?;
    let events_path_after_continue = marker_content_after_continue.trim().to_string();

    // Verify continue used the same events file
    assert_eq!(
        events_path_after_run, events_path_after_continue,
        "Continue should use the same events file as the original run.\nAfter run: {}\nAfter continue: {}",
        events_path_after_run, events_path_after_continue
    );

    Ok(())
}

#[test]
fn test_continue_preserves_marker_path() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let temp_path = temp_dir.path();

    create_test_config(temp_path)?;

    // First: run hats
    let _output = Command::new(hats_bin())
        .arg("run")
        .arg("--config")
        .arg(temp_path.join("hats.yml"))
        .current_dir(temp_path)
        .output()?;

    // Read the marker path from first run
    let marker_content = fs::read_to_string(temp_path.join(".hats/current-events"))?;
    let events_path_after_run = marker_content.trim().to_string();

    // Create scratchpad for continue
    let agent_dir = temp_path.join(".agent");
    fs::create_dir_all(&agent_dir)?;
    fs::write(
        agent_dir.join("scratchpad.md"),
        "# Tasks\n- [ ] Test task\n",
    )?;

    // Continue
    let _output = Command::new(hats_bin())
        .arg("run")
        .arg("--continue")
        .arg("--config")
        .arg(temp_path.join("hats.yml"))
        .current_dir(temp_path)
        .output()?;

    // Read the marker path after continue
    let marker_content_after_continue =
        fs::read_to_string(temp_path.join(".hats/current-events"))?;
    let events_path_after_continue = marker_content_after_continue.trim().to_string();

    // Continue should preserve the same marker path (not create a new one)
    assert_eq!(
        events_path_after_run, events_path_after_continue,
        "Continue should preserve the marker path.\nAfter run: {}\nAfter continue: {}",
        events_path_after_run, events_path_after_continue
    );

    Ok(())
}

// =============================================================================
// Regression Tests for Issue #82
// =============================================================================

#[test]
fn test_stale_events_dont_pollute_new_runs() -> Result<()> {
    // This test verifies the fix for issue #82:
    // Stale events from previous runs should not pollute new runs
    let temp_dir = TempDir::new()?;
    let temp_path = temp_dir.path();

    create_test_config(temp_path)?;

    // Simulate a previous run with stale events by creating a marker and events file
    fs::create_dir_all(temp_path.join(".hats"))?;

    // Create a "stale" events file with events from a previous (different) config
    let stale_events_file = temp_path.join(".hats/events-20260119-120000.jsonl");
    let stale_events = r#"{"topic":"archaeology.start","payload":"old preset","ts":"2026-01-19T12:00:00Z"}
{"topic":"map.created","payload":"stale map event","ts":"2026-01-19T12:00:01Z"}
{"topic":"artifact.found","payload":"stale artifact","ts":"2026-01-19T12:00:02Z"}
"#;
    fs::write(&stale_events_file, stale_events)?;

    // Point the marker to the stale events (simulating previous run)
    fs::write(
        temp_path.join(".hats/current-events"),
        ".hats/events-20260119-120000.jsonl",
    )?;

    // Now run hats fresh - it should create a NEW events file
    let _output = Command::new(hats_bin())
        .arg("run")
        .arg("--config")
        .arg(temp_path.join("hats.yml"))
        .current_dir(temp_path)
        .output()?;

    // Read the new marker
    let new_marker_content = fs::read_to_string(temp_path.join(".hats/current-events"))?;
    let new_events_path = new_marker_content.trim();

    // Verify a NEW events file was created (different from the stale one)
    assert_ne!(
        new_events_path, ".hats/events-20260119-120000.jsonl",
        "Fresh run should create new events file, not reuse stale one"
    );

    // Verify the new events file does NOT contain stale events
    let new_events_file = temp_path.join(new_events_path);
    if new_events_file.exists() {
        let new_events_content = fs::read_to_string(&new_events_file)?;

        assert!(
            !new_events_content.contains("archaeology.start"),
            "New run should NOT contain stale 'archaeology.start' events"
        );
        assert!(
            !new_events_content.contains("map.created"),
            "New run should NOT contain stale 'map.created' events"
        );
        assert!(
            !new_events_content.contains("artifact.found"),
            "New run should NOT contain stale 'artifact.found' events"
        );
    }

    // Verify the stale events file still exists (wasn't deleted)
    assert!(
        stale_events_file.exists(),
        "Stale events file should be preserved (not deleted)"
    );

    Ok(())
}

#[test]
fn test_new_run_ignores_stale_marker() -> Result<()> {
    // Another regression test for issue #82:
    // A fresh `hats run` should create a new marker even if one exists
    let temp_dir = TempDir::new()?;
    let temp_path = temp_dir.path();

    create_test_config(temp_path)?;

    // Create a stale marker pointing to an old events file
    fs::create_dir_all(temp_path.join(".hats"))?;
    fs::write(
        temp_path.join(".hats/current-events"),
        ".hats/events-old.jsonl",
    )?;
    fs::write(temp_path.join(".hats/events-old.jsonl"), "{}")?;

    // Run hats fresh
    let _output = Command::new(hats_bin())
        .arg("run")
        .arg("--config")
        .arg(temp_path.join("hats.yml"))
        .current_dir(temp_path)
        .output()?;

    // Read the new marker
    let new_marker_content = fs::read_to_string(temp_path.join(".hats/current-events"))?;
    let new_events_path = new_marker_content.trim();

    // Fresh run should have created a new timestamped events file
    assert_ne!(
        new_events_path, ".hats/events-old.jsonl",
        "Fresh run should create new events file, not reuse stale marker path"
    );

    // Should match the timestamped pattern
    assert!(
        new_events_path.starts_with(".hats/events-")
            && std::path::Path::new(new_events_path)
                .extension()
                .is_some_and(|ext| ext.eq_ignore_ascii_case("jsonl"))
            && new_events_path != ".hats/events-old.jsonl",
        "Fresh run should create timestamped events file: {}",
        new_events_path
    );

    Ok(())
}

// =============================================================================
// Directory Structure Tests
// =============================================================================

#[test]
fn test_hats_directory_created() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let temp_path = temp_dir.path();

    create_test_config(temp_path)?;

    // Verify .hats does not exist before run
    let hats_dir = temp_path.join(".hats");
    assert!(!hats_dir.exists(), ".hats should not exist before run");

    // Run hats
    let _output = Command::new(hats_bin())
        .arg("run")
        .arg("--config")
        .arg(temp_path.join("hats.yml"))
        .current_dir(temp_path)
        .output()?;

    // Verify .hats directory was created
    assert!(
        hats_dir.exists(),
        ".hats directory should be created by run"
    );
    assert!(
        hats_dir.is_dir(),
        ".hats should be a directory, not a file"
    );

    Ok(())
}
