//! Integration tests for `ralph tools task` CLI commands.

use ralph_core::{Task, TaskStatus};
use std::process::Command;
use tempfile::TempDir;

fn ralph_task(temp_path: &std::path::Path, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_ralph"))
        .arg("tools")
        .arg("task")
        .args(args)
        .arg("--root")
        .arg(temp_path)
        .current_dir(temp_path)
        .output()
        .expect("Failed to execute ralph tools task command")
}

fn ralph_task_ok(temp_path: &std::path::Path, args: &[&str]) -> String {
    let output = ralph_task(temp_path, args);
    assert!(
        output.status.success(),
        "Command 'ralph tools task {}' failed: {}",
        args.join(" "),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).to_string()
}

fn list_tasks(temp_path: &std::path::Path, extra_args: &[&str]) -> Vec<Task> {
    let mut args = vec!["list", "--format", "json"];
    args.extend_from_slice(extra_args);
    let stdout = ralph_task_ok(temp_path, &args);
    serde_json::from_str(&stdout).expect("Failed to parse task list JSON")
}

#[test]
fn test_task_add_and_list_json() {
    let temp_dir = TempDir::new().expect("temp dir");
    let temp_path = temp_dir.path();

    ralph_task_ok(
        temp_path,
        &["add", "First task", "-p", "2", "-d", "Test description"],
    );

    let tasks = list_tasks(temp_path, &["--all"]);
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].title, "First task");
    assert_eq!(tasks[0].priority, 2);
    assert_eq!(tasks[0].description.as_deref(), Some("Test description"));
}

#[test]
fn test_task_add_quiet_outputs_id() {
    let temp_dir = TempDir::new().expect("temp dir");
    let temp_path = temp_dir.path();

    let stdout = ralph_task_ok(temp_path, &["add", "Quiet task", "--format", "quiet"]);
    let id = stdout.trim();
    assert!(id.starts_with("task-"), "Expected task id, got: {}", id);

    let tasks = list_tasks(temp_path, &["--all"]);
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].id, id);
}

#[test]
fn test_task_ready_filters_by_loop_id() {
    let temp_dir = TempDir::new().expect("temp dir");
    let temp_path = temp_dir.path();

    let ralph_dir = temp_path.join(".ralph");
    std::fs::create_dir_all(&ralph_dir).expect("create .ralph");

    std::fs::write(ralph_dir.join("current-loop-id"), "loop-a").expect("write loop a");
    ralph_task_ok(temp_path, &["add", "Task A"]);

    std::fs::write(ralph_dir.join("current-loop-id"), "loop-b").expect("write loop b");
    ralph_task_ok(temp_path, &["add", "Task B"]);

    let stdout = ralph_task_ok(temp_path, &["ready", "--format", "json"]);
    let tasks: Vec<Task> = serde_json::from_str(&stdout).expect("parse ready JSON");

    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].title, "Task B");
    assert_eq!(tasks[0].loop_id.as_deref(), Some("loop-b"));
}

#[test]
fn test_task_ready_respects_blockers() {
    let temp_dir = TempDir::new().expect("temp dir");
    let temp_path = temp_dir.path();

    ralph_task_ok(temp_path, &["add", "Blocker"]);
    let tasks = list_tasks(temp_path, &["--all"]);
    let blocker_id = tasks[0].id.clone();

    ralph_task_ok(temp_path, &["add", "Blocked", "--blocked-by", &blocker_id]);

    let stdout = ralph_task_ok(temp_path, &["ready", "--format", "json"]);
    let ready: Vec<Task> = serde_json::from_str(&stdout).expect("parse ready JSON");
    assert_eq!(ready.len(), 1);
    assert_eq!(ready[0].title, "Blocker");

    ralph_task_ok(temp_path, &["close", &blocker_id]);

    let stdout = ralph_task_ok(temp_path, &["ready", "--format", "json"]);
    let ready: Vec<Task> = serde_json::from_str(&stdout).expect("parse ready JSON");
    assert_eq!(ready.len(), 1);
    assert_eq!(ready[0].title, "Blocked");
}

#[test]
fn test_task_close_and_fail_update_status() {
    let temp_dir = TempDir::new().expect("temp dir");
    let temp_path = temp_dir.path();

    ralph_task_ok(temp_path, &["add", "Close me"]);
    ralph_task_ok(temp_path, &["add", "Fail me"]);

    let tasks = list_tasks(temp_path, &["--all"]);
    let close_id = tasks[0].id.clone();
    let fail_id = tasks[1].id.clone();

    ralph_task_ok(temp_path, &["close", &close_id]);
    ralph_task_ok(temp_path, &["fail", &fail_id]);

    let tasks = list_tasks(temp_path, &["--all"]);
    let status_by_id: std::collections::HashMap<String, TaskStatus> =
        tasks.into_iter().map(|t| (t.id, t.status)).collect();

    assert_eq!(status_by_id.get(&close_id), Some(&TaskStatus::Closed));
    assert_eq!(status_by_id.get(&fail_id), Some(&TaskStatus::Failed));
}

#[test]
fn test_task_show_json() {
    let temp_dir = TempDir::new().expect("temp dir");
    let temp_path = temp_dir.path();

    ralph_task_ok(temp_path, &["add", "Show me"]);
    let tasks = list_tasks(temp_path, &["--all"]);
    let task_id = tasks[0].id.clone();

    let stdout = ralph_task_ok(temp_path, &["show", &task_id, "--format", "json"]);
    let task: Task = serde_json::from_str(&stdout).expect("parse task JSON");
    assert_eq!(task.id, task_id);
    assert_eq!(task.title, "Show me");
}
