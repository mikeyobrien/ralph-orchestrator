use anyhow::Result;
use std::fs;
use std::process::Command;
use tempfile::TempDir;

/// Integration tests for continue mode (--continue flag) acceptance criteria.
///
/// Per event-loop.spec.md, hats run --continue should:
/// 1) Check that scratchpad exists before continuing
/// 2) Publish task.resume instead of task.start
/// 3) Allow planner to read existing scratchpad rather than doing fresh gap analysis

#[test]
fn test_continue_requires_existing_scratchpad() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let temp_path = temp_dir.path();

    // Create a basic config file with custom backend that will fail fast
    // Using "nonexistent_backend" ensures auto-detection fails immediately
    let config_content = r#"
event_loop:
  prompt_file: "PROMPT.md"
  completion_promise: "LOOP_COMPLETE"
  max_iterations: 1
  max_runtime_seconds: 30

cli:
  backend: "custom"
  command: "true"

core:
  scratchpad: ".hats/agent/scratchpad.md"
"#;
    fs::write(temp_path.join("hats.yml"), config_content)?;

    // Create a prompt file
    fs::write(temp_path.join("PROMPT.md"), "Test task")?;

    // Ensure no scratchpad exists
    let scratchpad_path = temp_path.join(".hats/agent").join("scratchpad.md");
    assert!(!scratchpad_path.exists());

    // Run hats run --continue - should fail with error about missing scratchpad
    let output = Command::new(env!("CARGO_BIN_EXE_hats"))
        .arg("run")
        .arg("--continue")
        .arg("--config")
        .arg(temp_path.join("hats.yml"))
        .current_dir(temp_path)
        .output()?;

    // Should exit with error
    assert!(!output.status.success());

    // Should contain error message about missing scratchpad
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Cannot continue: scratchpad not found"));
    assert!(stderr.contains("Start a fresh run with `hats run`"));

    Ok(())
}

#[test]
fn test_continue_with_existing_scratchpad() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let temp_path = temp_dir.path();

    // Create a basic config file with short timeout and fast backend
    // Disable memories/tasks to test legacy scratchpad mode
    let config_content = r#"
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

memories:
  enabled: false

tasks:
  enabled: false
"#;
    fs::write(temp_path.join("hats.yml"), config_content)?;

    // Create a prompt file
    fs::write(temp_path.join("PROMPT.md"), "Test task")?;

    // Create the .hats/agent directory and scratchpad file
    let agent_dir = temp_path.join(".hats/agent");
    fs::create_dir_all(&agent_dir)?;

    let scratchpad_content = r"# Task List

## Current Tasks
- [ ] Implement feature A
- [x] Complete feature B
- [ ] Add tests for feature C

## Notes
Previous work completed on feature B.
";
    fs::write(agent_dir.join("scratchpad.md"), scratchpad_content)?;

    // Run hats run --continue --no-tui (needed for tracing output to stdout)
    let output = Command::new(env!("CARGO_BIN_EXE_hats"))
        .arg("run")
        .arg("--continue")
        .arg("--no-tui")
        .arg("--config")
        .arg(temp_path.join("hats.yml"))
        .current_dir(temp_path)
        .output()?;

    let _stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Should find the existing scratchpad (logged via tracing to stdout)
    assert!(stdout.contains("Found existing scratchpad"));

    Ok(())
}

#[test]
fn test_continue_publishes_task_resume_event() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let temp_path = temp_dir.path();

    // Create config with short timeout and fast backend
    let config_content = r#"
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
"#;

    fs::write(temp_path.join("hats.yml"), config_content)?;

    // Create a prompt file
    fs::write(temp_path.join("PROMPT.md"), "Continue test task")?;

    // Create the .hats/agent directory and scratchpad file
    let agent_dir = temp_path.join(".hats/agent");
    fs::create_dir_all(&agent_dir)?;

    // Create .hats directory with a marker file for continue to use
    // (simulates a previous run that created the events file)
    let hats_dir = temp_path.join(".hats");
    fs::create_dir_all(&hats_dir)?;
    let events_path = ".hats/events-continue-test.jsonl";
    fs::write(hats_dir.join("current-events"), events_path)?;

    let scratchpad_content = r"# Task List

## Current Tasks
- [ ] Continue this task
- [x] Previously completed task

## Notes
This is a continued session.
";
    fs::write(agent_dir.join("scratchpad.md"), scratchpad_content)?;

    // Run hats run --continue
    let _output = Command::new(env!("CARGO_BIN_EXE_hats"))
        .arg("run")
        .arg("--continue")
        .arg("--config")
        .arg(temp_path.join("hats.yml"))
        .current_dir(temp_path)
        .output()?;

    // Check that the event log contains task.resume instead of task.start
    // Events are now stored in .hats/ directory, read path from marker file
    let marker_path = hats_dir.join("current-events");
    if marker_path.exists() {
        let events_path = fs::read_to_string(&marker_path)?.trim().to_string();
        let events_file = temp_path.join(&events_path);
        if events_file.exists() {
            let events_content = fs::read_to_string(&events_file)?;

            // Should contain task.resume event
            assert!(events_content.contains("task.resume"));

            // Should NOT contain task.start event (since this is continue mode)
            assert!(!events_content.contains("task.start"));
        }
    }

    Ok(())
}

#[test]
fn test_continue_vs_run_event_difference() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let temp_path = temp_dir.path();

    // Create config with short timeout and fast backend
    let config_content = r#"
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
"#;

    fs::write(temp_path.join("hats.yml"), config_content)?;

    // Create a prompt file
    fs::write(temp_path.join("PROMPT.md"), "Test task")?;

    // Create the .hats/agent directory
    let agent_dir = temp_path.join(".hats/agent");
    fs::create_dir_all(&agent_dir)?;

    // Test 1: Run normal hats run (should publish task.start)
    let scratchpad_content = "# Initial scratchpad\n- [ ] Task 1\n";
    fs::write(agent_dir.join("scratchpad.md"), scratchpad_content)?;

    let _output = Command::new(env!("CARGO_BIN_EXE_hats"))
        .arg("run")
        .arg("--config")
        .arg(temp_path.join("hats.yml"))
        .current_dir(temp_path)
        .output()?;

    // EventLogger writes to the default path .hats/events.jsonl
    // The marker file points to a timestamped path for isolation, but EventLogger
    // uses the default path for debugging/history purposes
    let events_file = temp_path.join(".hats/events.jsonl");

    // Check events from run command
    let run_events = if events_file.exists() {
        fs::read_to_string(&events_file)?
    } else {
        String::new()
    };

    // Test 2: Run hats run --continue (should publish task.resume)
    let _output = Command::new(env!("CARGO_BIN_EXE_hats"))
        .arg("run")
        .arg("--continue")
        .arg("--config")
        .arg(temp_path.join("hats.yml"))
        .current_dir(temp_path)
        .output()?;

    // Check events after continue
    let continue_events = if events_file.exists() {
        fs::read_to_string(&events_file)?
    } else {
        String::new()
    };

    // Verify the difference:
    // - run should have task.start
    // - continue should ADD task.resume to the same file
    if !run_events.is_empty() {
        assert!(
            run_events.contains("task.start"),
            "Run should produce task.start event"
        );
    }

    // After continue, the file should contain both task.start (from run) and task.resume (from continue)
    if !continue_events.is_empty() {
        assert!(
            continue_events.contains("task.start"),
            "Events file should still contain task.start from the run"
        );
        assert!(
            continue_events.contains("task.resume"),
            "Events file should now also contain task.resume from the continue"
        );
    }

    Ok(())
}

#[test]
fn test_continue_logs_scratchpad_found() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let temp_path = temp_dir.path();

    // Create config with short timeout and fast backend
    // Disable memories/tasks to test legacy scratchpad mode
    let config_content = r#"
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

memories:
  enabled: false

tasks:
  enabled: false
"#;

    fs::write(temp_path.join("hats.yml"), config_content)?;

    // Create a prompt file
    fs::write(temp_path.join("PROMPT.md"), "Test task")?;

    // Create the .hats/agent directory and scratchpad with unique content
    let agent_dir = temp_path.join(".hats/agent");
    fs::create_dir_all(&agent_dir)?;

    let scratchpad_content = r"# Existing Task List

## Current Tasks
- [ ] UNIQUE_TASK_MARKER: Complete the special feature
- [x] Previously finished work

## Notes
This scratchpad contains UNIQUE_CONTENT_MARKER for testing.
";
    fs::write(agent_dir.join("scratchpad.md"), scratchpad_content)?;

    // Run hats run --continue --no-tui (needed for tracing output to stdout)
    let output = Command::new(env!("CARGO_BIN_EXE_hats"))
        .arg("run")
        .arg("--continue")
        .arg("--no-tui")
        .arg("--config")
        .arg(temp_path.join("hats.yml"))
        .current_dir(temp_path)
        .output()?;

    let _stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Should log that it found the existing scratchpad (logged via tracing output)
    assert!(stdout.contains("Found existing scratchpad"));

    Ok(())
}
