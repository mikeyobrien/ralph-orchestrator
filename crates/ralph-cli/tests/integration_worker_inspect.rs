//! Integration tests for `ralph worker inspect` CLI command.

use std::path::Path;
use std::process::Command;
use tempfile::TempDir;

fn ralph_worker(temp_path: &Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_ralph"))
        .arg("worker")
        .args(args)
        .arg("--root")
        .arg(temp_path)
        .current_dir(temp_path)
        .output()
        .expect("Failed to execute ralph worker command")
}

fn ralph_worker_ok(temp_path: &Path, args: &[&str]) -> String {
    let output = ralph_worker(temp_path, args);
    assert!(
        output.status.success(),
        "Command 'ralph worker {}' failed: {}",
        args.join(" "),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).to_string()
}

/// Write a workers.json with a single worker record.
fn write_worker_json(root: &Path, worker_id: &str, task_id: Option<&str>, workspace_root: &str) {
    let ralph_dir = root.join(".ralph");
    std::fs::create_dir_all(&ralph_dir).expect("create .ralph");

    let current_task_field = match task_id {
        Some(tid) => format!(r#""currentTaskId": "{tid}","#),
        None => String::new(),
    };

    let json = format!(
        r#"{{
  "workers": [
    {{
      "workerId": "{worker_id}",
      "workerName": "test-worker",
      "loopId": "loop-test-1",
      "backend": "claude",
      "workspaceRoot": "{workspace_root}",
      {current_task_field}
      "status": "busy",
      "lastHeartbeatAt": "2026-03-16T00:00:00.000Z"
    }}
  ]
}}"#
    );

    std::fs::write(ralph_dir.join("workers.json"), json).expect("write workers.json");
}

#[test]
fn test_inspect_worker_not_found() {
    let temp_dir = TempDir::new().expect("temp dir");
    let temp_path = temp_dir.path();

    // Create empty workers.json
    let ralph_dir = temp_path.join(".ralph");
    std::fs::create_dir_all(&ralph_dir).expect("create .ralph");
    std::fs::write(ralph_dir.join("workers.json"), r#"{"workers": []}"#)
        .expect("write workers.json");

    let output = ralph_worker(temp_path, &["inspect", "nonexistent-worker"]);
    assert!(
        !output.status.success(),
        "Expected failure for nonexistent worker"
    );
}

#[test]
fn test_inspect_worker_no_current_task() {
    let temp_dir = TempDir::new().expect("temp dir");
    let temp_path = temp_dir.path();

    write_worker_json(temp_path, "idle-worker", None, temp_path.to_str().unwrap());

    let stdout = ralph_worker_ok(temp_path, &["inspect", "idle-worker"]);
    assert!(
        stdout.contains("no current task"),
        "Expected 'no current task' message, got:\n{stdout}"
    );
}

#[test]
fn test_inspect_worker_worktree_missing() {
    let temp_dir = TempDir::new().expect("temp dir");
    let temp_path = temp_dir.path();

    write_worker_json(
        temp_path,
        "busy-worker",
        Some("task-missing-123"),
        temp_path.to_str().unwrap(),
    );

    let stdout = ralph_worker_ok(temp_path, &["inspect", "busy-worker"]);
    assert!(
        stdout.contains("Worktree not found"),
        "Expected 'Worktree not found' message, got:\n{stdout}"
    );
}

#[test]
fn test_inspect_worker_with_agent_files() {
    let temp_dir = TempDir::new().expect("temp dir");
    let temp_path = temp_dir.path();

    let task_id = "task-1234567890-abcd";
    write_worker_json(
        temp_path,
        "file-worker",
        Some(task_id),
        temp_path.to_str().unwrap(),
    );

    // Create worktree with agent files
    let agent_dir = temp_path
        .join(".worktrees")
        .join(task_id)
        .join(".ralph")
        .join("agent");
    std::fs::create_dir_all(&agent_dir).expect("create agent dir");

    std::fs::write(
        agent_dir.join("scratchpad.md"),
        "# My scratchpad\nSome notes here.",
    )
    .expect("write scratchpad");
    std::fs::write(
        agent_dir.join("decisions.md"),
        "# Decisions\nDEC-001: chose X over Y",
    )
    .expect("write decisions");

    // Create a subdirectory with another md file
    let sub_dir = agent_dir.join("research");
    std::fs::create_dir_all(&sub_dir).expect("create research dir");
    std::fs::write(sub_dir.join("notes.md"), "# Research notes").expect("write notes");

    let stdout = ralph_worker_ok(temp_path, &["inspect", "file-worker"]);

    // Verify agent files header
    assert!(
        stdout.contains("Agent Files (3)"),
        "Expected 3 agent files, got:\n{stdout}"
    );
    // Verify file contents are shown
    assert!(
        stdout.contains("My scratchpad"),
        "Expected scratchpad content, got:\n{stdout}"
    );
    assert!(
        stdout.contains("DEC-001"),
        "Expected decisions content, got:\n{stdout}"
    );
    assert!(
        stdout.contains("Research notes"),
        "Expected research notes, got:\n{stdout}"
    );
    // Verify no tasks.jsonl message
    assert!(
        stdout.contains("no tasks.jsonl"),
        "Expected 'no tasks.jsonl' message, got:\n{stdout}"
    );
}

#[test]
fn test_inspect_worker_with_subtasks() {
    let temp_dir = TempDir::new().expect("temp dir");
    let temp_path = temp_dir.path();

    let task_id = "task-9999999999-ffff";
    write_worker_json(
        temp_path,
        "task-worker",
        Some(task_id),
        temp_path.to_str().unwrap(),
    );

    // Create worktree with tasks.jsonl
    let agent_dir = temp_path
        .join(".worktrees")
        .join(task_id)
        .join(".ralph")
        .join("agent");
    std::fs::create_dir_all(&agent_dir).expect("create agent dir");

    // Write JSONL tasks — each line is a complete Task JSON object
    let tasks_jsonl = concat!(
        r#"{"id":"task-0001-aaaa","title":"Write unit tests","status":"open","priority":1,"blocked_by":[],"created":"2026-03-16T00:00:00Z"}"#,
        "\n",
        r#"{"id":"task-0002-bbbb","title":"Fix lint errors","status":"closed","priority":2,"blocked_by":[],"created":"2026-03-16T00:01:00Z"}"#,
    );
    std::fs::write(agent_dir.join("tasks.jsonl"), tasks_jsonl).expect("write tasks.jsonl");

    let stdout = ralph_worker_ok(temp_path, &["inspect", "task-worker"]);

    assert!(
        stdout.contains("Subtasks (2)"),
        "Expected 2 subtasks, got:\n{stdout}"
    );
    assert!(
        stdout.contains("Write unit tests"),
        "Expected task title, got:\n{stdout}"
    );
    assert!(
        stdout.contains("Fix lint errors"),
        "Expected task title, got:\n{stdout}"
    );
}

#[test]
fn test_inspect_worker_truncates_long_files() {
    let temp_dir = TempDir::new().expect("temp dir");
    let temp_path = temp_dir.path();

    let task_id = "task-5555555555-eeee";
    write_worker_json(
        temp_path,
        "verbose-worker",
        Some(task_id),
        temp_path.to_str().unwrap(),
    );

    let agent_dir = temp_path
        .join(".worktrees")
        .join(task_id)
        .join(".ralph")
        .join("agent");
    std::fs::create_dir_all(&agent_dir).expect("create agent dir");

    // Create a file with 100 lines — should be truncated to 50
    let long_content = (1..=100).fold(String::new(), |mut acc, i| {
        use std::fmt::Write;
        writeln!(acc, "Line {i}").unwrap();
        acc
    });
    std::fs::write(agent_dir.join("scratchpad.md"), &long_content).expect("write long file");

    let stdout = ralph_worker_ok(temp_path, &["inspect", "verbose-worker"]);

    assert!(
        stdout.contains("Line 50"),
        "Expected line 50 to be shown, got:\n{stdout}"
    );
    assert!(
        !stdout.contains("Line 51"),
        "Expected line 51 to be truncated, got:\n{stdout}"
    );
    assert!(
        stdout.contains("50 more lines"),
        "Expected truncation indicator, got:\n{stdout}"
    );
}
