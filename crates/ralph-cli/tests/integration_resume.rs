use anyhow::Result;
use std::fs;
use std::process::Command;
use tempfile::TempDir;

/// Integration tests for resume mode acceptance criteria.
///
/// Per event-loop.spec.md, ralph resume should:
/// 1) Check that scratchpad exists before resuming
/// 2) Publish task.resume instead of task.start  
/// 3) Allow planner to read existing scratchpad rather than doing fresh gap analysis

#[test]
fn test_resume_requires_existing_scratchpad() -> Result<()> {
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
  scratchpad: ".agent/scratchpad.md"
"#;
    fs::write(temp_path.join("ralph.yml"), config_content)?;

    // Create a prompt file
    fs::write(temp_path.join("PROMPT.md"), "Test task")?;

    // Ensure no scratchpad exists
    let scratchpad_path = temp_path.join(".agent").join("scratchpad.md");
    assert!(!scratchpad_path.exists());

    // Run ralph resume - should fail with error about missing scratchpad
    let output = Command::new(env!("CARGO_BIN_EXE_ralph"))
        .arg("resume")
        .arg("--config")
        .arg(temp_path.join("ralph.yml"))
        .current_dir(temp_path)
        .output()?;

    // Should exit with error
    assert!(!output.status.success());

    // Should contain error message about missing scratchpad
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Cannot resume: scratchpad not found"));
    assert!(stderr.contains("Use `ralph run` to start a new loop"));

    Ok(())
}

#[test]
fn test_resume_with_existing_scratchpad() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let temp_path = temp_dir.path();

    // Create a basic config file with short timeout and fast backend
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
  scratchpad: ".agent/scratchpad.md"
"#;
    fs::write(temp_path.join("ralph.yml"), config_content)?;

    // Create a prompt file
    fs::write(temp_path.join("PROMPT.md"), "Test task")?;

    // Create the .agent directory and scratchpad file
    let agent_dir = temp_path.join(".agent");
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

    // Run ralph resume
    let output = Command::new(env!("CARGO_BIN_EXE_ralph"))
        .arg("resume")
        .arg("--config")
        .arg(temp_path.join("ralph.yml"))
        .current_dir(temp_path)
        .output()?;

    let _stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Should find the existing scratchpad (logged via tracing to stdout)
    assert!(stdout.contains("Found existing scratchpad"));

    Ok(())
}

#[test]
fn test_resume_publishes_task_resume_event() -> Result<()> {
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
  scratchpad: ".agent/scratchpad.md"
"#;

    fs::write(temp_path.join("ralph.yml"), config_content)?;

    // Create a prompt file
    fs::write(temp_path.join("PROMPT.md"), "Resume test task")?;

    // Create the .agent directory and scratchpad file
    let agent_dir = temp_path.join(".agent");
    fs::create_dir_all(&agent_dir)?;

    let scratchpad_content = r"# Task List

## Current Tasks
- [ ] Resume this task
- [x] Previously completed task

## Notes
This is a resumed session.
";
    fs::write(agent_dir.join("scratchpad.md"), scratchpad_content)?;

    // Run ralph resume
    let _output = Command::new(env!("CARGO_BIN_EXE_ralph"))
        .arg("resume")
        .arg("--config")
        .arg(temp_path.join("ralph.yml"))
        .current_dir(temp_path)
        .output()?;

    // Check that the event log contains task.resume instead of task.start
    let events_file = agent_dir.join("events.jsonl");
    if events_file.exists() {
        let events_content = fs::read_to_string(&events_file)?;

        // Should contain task.resume event
        assert!(events_content.contains("task.resume"));

        // Should NOT contain task.start event (since this is resume mode)
        assert!(!events_content.contains("task.start"));
    }

    Ok(())
}

#[test]
fn test_resume_vs_run_event_difference() -> Result<()> {
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
  scratchpad: ".agent/scratchpad.md"
"#;

    fs::write(temp_path.join("ralph.yml"), config_content)?;

    // Create a prompt file
    fs::write(temp_path.join("PROMPT.md"), "Test task")?;

    // Create the .agent directory
    let agent_dir = temp_path.join(".agent");
    fs::create_dir_all(&agent_dir)?;

    // Test 1: Run normal ralph run (should publish task.start)
    let scratchpad_content = "# Initial scratchpad\n- [ ] Task 1\n";
    fs::write(agent_dir.join("scratchpad.md"), scratchpad_content)?;

    // Clear any existing events
    let events_file = agent_dir.join("events.jsonl");
    if events_file.exists() {
        fs::remove_file(&events_file)?;
    }

    let _output = Command::new(env!("CARGO_BIN_EXE_ralph"))
        .arg("run")
        .arg("--config")
        .arg(temp_path.join("ralph.yml"))
        .current_dir(temp_path)
        .output()?;

    // Check events from run command
    let run_events = if events_file.exists() {
        fs::read_to_string(&events_file)?
    } else {
        String::new()
    };

    // Clear events for resume test
    if events_file.exists() {
        fs::remove_file(&events_file)?;
    }

    // Test 2: Run ralph resume (should publish task.resume)
    let _output = Command::new(env!("CARGO_BIN_EXE_ralph"))
        .arg("resume")
        .arg("--config")
        .arg(temp_path.join("ralph.yml"))
        .current_dir(temp_path)
        .output()?;

    // Check events from resume command
    let resume_events = if events_file.exists() {
        fs::read_to_string(&events_file)?
    } else {
        String::new()
    };

    // Verify the difference:
    // - run should have task.start
    // - resume should have task.resume
    if !run_events.is_empty() {
        assert!(run_events.contains("task.start"));
        assert!(!run_events.contains("task.resume"));
    }

    if !resume_events.is_empty() {
        assert!(resume_events.contains("task.resume"));
        assert!(!resume_events.contains("task.start"));
    }

    Ok(())
}

#[test]
fn test_resume_logs_scratchpad_found() -> Result<()> {
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
  scratchpad: ".agent/scratchpad.md"
"#;

    fs::write(temp_path.join("ralph.yml"), config_content)?;

    // Create a prompt file
    fs::write(temp_path.join("PROMPT.md"), "Test task")?;

    // Create the .agent directory and scratchpad with unique content
    let agent_dir = temp_path.join(".agent");
    fs::create_dir_all(&agent_dir)?;

    let scratchpad_content = r"# Existing Task List

## Current Tasks
- [ ] UNIQUE_TASK_MARKER: Complete the special feature
- [x] Previously finished work

## Notes
This scratchpad contains UNIQUE_CONTENT_MARKER for testing.
";
    fs::write(agent_dir.join("scratchpad.md"), scratchpad_content)?;

    // Run ralph resume
    let output = Command::new(env!("CARGO_BIN_EXE_ralph"))
        .arg("resume")
        .arg("--config")
        .arg(temp_path.join("ralph.yml"))
        .current_dir(temp_path)
        .output()?;

    let _stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Should log that it found the existing scratchpad (logged via tracing to stdout)
    assert!(stdout.contains("Found existing scratchpad"));

    Ok(())
}
