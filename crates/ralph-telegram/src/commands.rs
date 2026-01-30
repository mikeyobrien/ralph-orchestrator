use std::path::Path;

use crate::bot::escape_html;

/// Check if a message is a bot command (starts with `/`).
pub fn is_command(text: &str) -> bool {
    text.starts_with('/')
}

/// Parse and execute a bot command, returning the response message.
///
/// Returns `Some(response)` if the text was a recognized command,
/// or `None` if the command was not recognized (so the caller can
/// treat it as a regular message).
pub fn handle_command(text: &str, workspace_root: &Path) -> Option<String> {
    let (command, _args) = parse_command(text);
    match command {
        "/help" => Some(cmd_help()),
        "/status" => Some(cmd_status(workspace_root)),
        "/tasks" => Some(cmd_tasks(workspace_root)),
        "/memories" => Some(cmd_memories(workspace_root)),
        "/tail" => Some(cmd_tail(workspace_root)),
        "/restart" => Some(cmd_restart(workspace_root)),
        _ => None,
    }
}

/// Split a command string into the command name and optional arguments.
fn parse_command(text: &str) -> (&str, &str) {
    // Handle @bot suffix: /status@ralph_bot -> /status
    if let Some((first, rest)) = text.split_once(char::is_whitespace) {
        let cmd = first.split('@').next().unwrap_or(first);
        (cmd, rest.trim())
    } else {
        let cmd = text.split('@').next().unwrap_or(text);
        (cmd, "")
    }
}

/// `/help` — List available commands.
fn cmd_help() -> String {
    [
        "<b>Ralph Bot Commands</b>",
        "",
        "/status — Current loop status",
        "/tasks — Open tasks",
        "/memories — Recent memories",
        "/tail — Last 20 events",
        "/restart — Restart the orchestration loop",
        "/help — This message",
    ]
    .join("\n")
}

/// `/status` — Current iteration, hat, elapsed time, loop ID.
fn cmd_status(workspace_root: &Path) -> String {
    let lock_path = workspace_root.join(".ralph/loop.lock");

    if !lock_path.exists() {
        return "No active loop (no lock file found).".to_string();
    }

    let lock_content = match std::fs::read_to_string(&lock_path) {
        Ok(c) => c,
        Err(e) => return format!("Failed to read lock file: {}", escape_html(&e.to_string())),
    };

    let lock: serde_json::Value = match serde_json::from_str(&lock_content) {
        Ok(v) => v,
        Err(e) => {
            return format!("Failed to parse lock file: {}", escape_html(&e.to_string()));
        }
    };

    let pid = lock.get("pid").and_then(|v| v.as_u64()).unwrap_or(0);
    let started = lock
        .get("started")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");

    let elapsed_str = if let Ok(started_dt) = chrono::DateTime::parse_from_rfc3339(started) {
        let elapsed = chrono::Utc::now().signed_duration_since(started_dt);
        let mins = elapsed.num_minutes();
        let secs = elapsed.num_seconds() % 60;
        if mins > 0 {
            format!("{}m {}s", mins, secs)
        } else {
            format!("{}s", secs)
        }
    } else {
        "unknown".to_string()
    };

    // Read current events file to count iterations
    let iteration_count = count_iterations(workspace_root);

    let prompt_preview = lock
        .get("prompt")
        .and_then(|v| v.as_str())
        .map(|p| {
            let preview: String = p.chars().take(100).collect();
            if p.len() > 100 {
                format!("{}...", preview)
            } else {
                preview
            }
        })
        .unwrap_or_else(|| "none".to_string());

    let mut lines = vec![
        "<b>Loop Status</b>".to_string(),
        String::new(),
        format!("PID: <code>{}</code>", pid),
        format!("Elapsed: <code>{}</code>", elapsed_str),
        format!("Iterations: <code>{}</code>", iteration_count),
        format!("Started: <code>{}</code>", escape_html(started)),
    ];

    lines.push(String::new());
    lines.push(format!("Prompt: {}", escape_html(&prompt_preview)));

    lines.join("\n")
}

/// Count iterations from the current events file.
fn count_iterations(workspace_root: &Path) -> usize {
    // Read current-events pointer
    let pointer_path = workspace_root.join(".ralph/current-events");
    let events_path = if pointer_path.exists() {
        match std::fs::read_to_string(&pointer_path) {
            Ok(p) => workspace_root.join(p.trim()),
            Err(_) => return 0,
        }
    } else {
        workspace_root.join(".ralph/events.jsonl")
    };

    if !events_path.exists() {
        return 0;
    }

    let content = match std::fs::read_to_string(&events_path) {
        Ok(c) => c,
        Err(_) => return 0,
    };

    // Count events with "iteration" field to estimate iteration count
    let mut max_iteration: usize = 0;
    for line in content.lines() {
        if let Ok(event) = serde_json::from_str::<serde_json::Value>(line)
            && let Some(iter) = event.get("iteration").and_then(|v| v.as_u64())
        {
            let iter = iter as usize;
            if iter > max_iteration {
                max_iteration = iter;
            }
        }
    }
    max_iteration
}

/// `/tasks` — List open tasks from `.ralph/agent/tasks.jsonl`.
fn cmd_tasks(workspace_root: &Path) -> String {
    let tasks_path = workspace_root.join(".ralph/agent/tasks.jsonl");

    if !tasks_path.exists() {
        return "No tasks file found.".to_string();
    }

    let content = match std::fs::read_to_string(&tasks_path) {
        Ok(c) => c,
        Err(e) => return format!("Failed to read tasks: {}", escape_html(&e.to_string())),
    };

    let mut open_tasks: Vec<(String, String, u64)> = Vec::new(); // (id, title, priority)
    let mut closed_count = 0u32;

    for line in content.lines() {
        if line.trim().is_empty() {
            continue;
        }
        if let Ok(task) = serde_json::from_str::<serde_json::Value>(line) {
            let status = task.get("status").and_then(|v| v.as_str()).unwrap_or("");
            if status == "open" {
                let id = task
                    .get("id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("?")
                    .to_string();
                let title = task
                    .get("title")
                    .and_then(|v| v.as_str())
                    .unwrap_or("untitled")
                    .to_string();
                let priority = task.get("priority").and_then(|v| v.as_u64()).unwrap_or(3);
                open_tasks.push((id, title, priority));
            } else if status == "closed" {
                closed_count += 1;
            }
        }
    }

    // Sort by priority (lower = higher priority)
    open_tasks.sort_by_key(|t| t.2);

    if open_tasks.is_empty() {
        return format!("No open tasks. ({} completed)", closed_count);
    }

    let mut lines = vec![format!(
        "<b>Open Tasks</b> ({} open, {} closed)",
        open_tasks.len(),
        closed_count
    )];
    lines.push(String::new());

    for (id, title, priority) in &open_tasks {
        let priority_label = match priority {
            1 => "P1",
            2 => "P2",
            _ => "P3",
        };
        lines.push(format!(
            "{}  <code>{}</code>\n    {}",
            priority_label,
            escape_html(id),
            escape_html(title)
        ));
    }

    lines.join("\n")
}

/// `/memories` — Last N memories from `.ralph/agent/memories.md`.
fn cmd_memories(workspace_root: &Path) -> String {
    let memories_path = workspace_root.join(".ralph/agent/memories.md");

    if !memories_path.exists() {
        return "No memories file found.".to_string();
    }

    let content = match std::fs::read_to_string(&memories_path) {
        Ok(c) => c,
        Err(e) => return format!("Failed to read memories: {}", escape_html(&e.to_string())),
    };

    // Extract memory blocks (### mem-... sections)
    let mut memories: Vec<(String, String)> = Vec::new(); // (id, content_preview)

    let mut current_id = String::new();
    let mut current_content = String::new();

    for line in content.lines() {
        if let Some(id) = line.strip_prefix("### ") {
            // Save previous memory
            if !current_id.is_empty() {
                memories.push((current_id.clone(), current_content.trim().to_string()));
            }
            current_id = id.trim().to_string();
            current_content.clear();
        } else if !current_id.is_empty() {
            current_content.push_str(line);
            current_content.push('\n');
        }
    }

    // Save last memory
    if !current_id.is_empty() {
        memories.push((current_id, current_content.trim().to_string()));
    }

    if memories.is_empty() {
        return "No memories found.".to_string();
    }

    // Show last 5 memories
    let show_count = 5;
    let start = memories.len().saturating_sub(show_count);
    let shown = &memories[start..];

    let mut lines = vec![format!(
        "<b>Recent Memories</b> (showing {}/{})",
        shown.len(),
        memories.len()
    )];
    lines.push(String::new());

    for (id, content) in shown {
        // Extract blockquote content (> ...)
        let preview: String = content
            .lines()
            .filter(|l| l.starts_with('>'))
            .map(|l| {
                l.strip_prefix("> ")
                    .unwrap_or(l.strip_prefix('>').unwrap_or(l))
            })
            .collect::<Vec<_>>()
            .join(" ");

        let preview = if preview.len() > 120 {
            format!("{}...", &preview[..120])
        } else {
            preview
        };

        lines.push(format!(
            "<code>{}</code>\n  {}",
            escape_html(id),
            escape_html(&preview)
        ));
    }

    lines.join("\n")
}

/// `/restart` — Request a restart of the orchestration loop.
///
/// Writes a signal file (`.ralph/restart-requested`) that the event loop
/// checks at each iteration boundary. When detected, the loop terminates
/// and the process exec-replaces itself with the same CLI arguments.
fn cmd_restart(workspace_root: &Path) -> String {
    let restart_path = workspace_root.join(".ralph/restart-requested");

    // Check if a loop is actually running
    let lock_path = workspace_root.join(".ralph/loop.lock");
    if !lock_path.exists() {
        return "No active loop to restart.".to_string();
    }

    match std::fs::write(&restart_path, "") {
        Ok(()) => {
            "Restart requested. The loop will restart at the next iteration boundary.".to_string()
        }
        Err(e) => format!(
            "Failed to write restart signal: {}",
            escape_html(&e.to_string())
        ),
    }
}

/// `/tail` — Last 20 lines of the current events file.
fn cmd_tail(workspace_root: &Path) -> String {
    // Find current events file
    let pointer_path = workspace_root.join(".ralph/current-events");
    let events_path = if pointer_path.exists() {
        match std::fs::read_to_string(&pointer_path) {
            Ok(p) => workspace_root.join(p.trim()),
            Err(e) => {
                return format!(
                    "Failed to read current-events pointer: {}",
                    escape_html(&e.to_string())
                );
            }
        }
    } else {
        workspace_root.join(".ralph/events.jsonl")
    };

    if !events_path.exists() {
        return "No events file found.".to_string();
    }

    let content = match std::fs::read_to_string(&events_path) {
        Ok(c) => c,
        Err(e) => return format!("Failed to read events: {}", escape_html(&e.to_string())),
    };

    let all_lines: Vec<&str> = content.lines().filter(|l| !l.trim().is_empty()).collect();

    if all_lines.is_empty() {
        return "Events file is empty.".to_string();
    }

    let tail_count = 20;
    let start = all_lines.len().saturating_sub(tail_count);
    let tail = &all_lines[start..];

    let mut lines = vec![format!(
        "<b>Last {} Events</b> ({} total)",
        tail.len(),
        all_lines.len()
    )];
    lines.push(String::new());

    for event_line in tail {
        if let Ok(event) = serde_json::from_str::<serde_json::Value>(event_line) {
            let topic = event.get("topic").and_then(|v| v.as_str()).unwrap_or("?");
            let iteration = event
                .get("iteration")
                .and_then(|v| v.as_u64())
                .map(|i| format!("#{}", i))
                .unwrap_or_default();
            let hat = event.get("hat").and_then(|v| v.as_str()).unwrap_or("");
            let payload = event.get("payload").and_then(|v| v.as_str()).unwrap_or("");

            let payload_preview = if payload.len() > 60 {
                format!("{}...", &payload[..60])
            } else {
                payload.to_string()
            };

            let hat_str = if hat.is_empty() {
                String::new()
            } else {
                format!(" [{}]", hat)
            };

            lines.push(format!(
                "<code>{}{}</code> {}{}",
                escape_html(topic),
                iteration,
                hat_str,
                if payload_preview.is_empty() {
                    String::new()
                } else {
                    format!("\n  {}", escape_html(&payload_preview))
                }
            ));
        } else {
            // Non-JSON line, show raw (truncated)
            let preview = if event_line.len() > 80 {
                format!("{}...", &event_line[..80])
            } else {
                event_line.to_string()
            };
            lines.push(escape_html(&preview));
        }
    }

    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    fn setup_workspace(dir: &TempDir) {
        std::fs::create_dir_all(dir.path().join(".ralph/agent")).unwrap();
    }

    #[test]
    fn is_command_recognizes_slash() {
        assert!(is_command("/help"));
        assert!(is_command("/status"));
        assert!(!is_command("hello"));
        assert!(!is_command("use /help"));
    }

    #[test]
    fn parse_command_simple() {
        assert_eq!(parse_command("/help"), ("/help", ""));
        assert_eq!(parse_command("/status"), ("/status", ""));
    }

    #[test]
    fn parse_command_with_bot_suffix() {
        assert_eq!(parse_command("/help@ralph_bot"), ("/help", ""));
        assert_eq!(parse_command("/status@ralph_bot"), ("/status", ""));
    }

    #[test]
    fn parse_command_with_args() {
        assert_eq!(parse_command("/memories 10"), ("/memories", "10"));
    }

    #[test]
    fn handle_command_returns_none_for_unknown() {
        let dir = TempDir::new().unwrap();
        assert!(handle_command("/unknown", dir.path()).is_none());
    }

    #[test]
    fn handle_command_returns_some_for_known() {
        let dir = TempDir::new().unwrap();
        assert!(handle_command("/help", dir.path()).is_some());
    }

    #[test]
    fn cmd_help_lists_commands() {
        let result = cmd_help();
        assert!(result.contains("/status"));
        assert!(result.contains("/tasks"));
        assert!(result.contains("/memories"));
        assert!(result.contains("/tail"));
        assert!(result.contains("/help"));
    }

    #[test]
    fn cmd_status_no_lock_file() {
        let dir = TempDir::new().unwrap();
        setup_workspace(&dir);
        let result = cmd_status(dir.path());
        assert!(result.contains("No active loop"));
    }

    #[test]
    fn cmd_status_with_lock_file() {
        let dir = TempDir::new().unwrap();
        setup_workspace(&dir);

        let lock = serde_json::json!({
            "pid": 12345,
            "started": "2026-01-30T10:00:00Z",
            "prompt": "Build a feature"
        });
        let lock_path = dir.path().join(".ralph/loop.lock");
        std::fs::write(&lock_path, serde_json::to_string(&lock).unwrap()).unwrap();

        let result = cmd_status(dir.path());
        assert!(result.contains("12345"));
        assert!(result.contains("Build a feature"));
    }

    #[test]
    fn cmd_tasks_no_file() {
        let dir = TempDir::new().unwrap();
        setup_workspace(&dir);
        let result = cmd_tasks(dir.path());
        assert!(result.contains("No tasks file"));
    }

    #[test]
    fn cmd_tasks_with_open_and_closed() {
        let dir = TempDir::new().unwrap();
        setup_workspace(&dir);

        let tasks_path = dir.path().join(".ralph/agent/tasks.jsonl");
        let mut f = std::fs::File::create(&tasks_path).unwrap();
        writeln!(
            f,
            r#"{{"id":"task-1","title":"Add auth","status":"open","priority":1}}"#
        )
        .unwrap();
        writeln!(
            f,
            r#"{{"id":"task-2","title":"Fix bug","status":"closed","priority":2}}"#
        )
        .unwrap();
        writeln!(
            f,
            r#"{{"id":"task-3","title":"Add tests","status":"open","priority":2}}"#
        )
        .unwrap();

        let result = cmd_tasks(dir.path());
        assert!(result.contains("2 open"));
        assert!(result.contains("1 closed"));
        assert!(result.contains("Add auth"));
        assert!(result.contains("Add tests"));
        assert!(!result.contains("Fix bug")); // closed tasks not listed
    }

    #[test]
    fn cmd_tasks_all_closed() {
        let dir = TempDir::new().unwrap();
        setup_workspace(&dir);

        let tasks_path = dir.path().join(".ralph/agent/tasks.jsonl");
        let mut f = std::fs::File::create(&tasks_path).unwrap();
        writeln!(
            f,
            r#"{{"id":"task-1","title":"Done","status":"closed","priority":1}}"#
        )
        .unwrap();

        let result = cmd_tasks(dir.path());
        assert!(result.contains("No open tasks"));
        assert!(result.contains("1 completed"));
    }

    #[test]
    fn cmd_memories_no_file() {
        let dir = TempDir::new().unwrap();
        setup_workspace(&dir);
        let result = cmd_memories(dir.path());
        assert!(result.contains("No memories file"));
    }

    #[test]
    fn cmd_memories_with_entries() {
        let dir = TempDir::new().unwrap();
        setup_workspace(&dir);

        let mem_path = dir.path().join(".ralph/agent/memories.md");
        std::fs::write(
            &mem_path,
            "# Memories\n\n## Patterns\n\n### mem-001\n> First memory\n<!-- tags: test -->\n\n### mem-002\n> Second memory\n<!-- tags: test -->\n",
        )
        .unwrap();

        let result = cmd_memories(dir.path());
        assert!(result.contains("mem-001"));
        assert!(result.contains("mem-002"));
        assert!(result.contains("First memory"));
        assert!(result.contains("Second memory"));
    }

    #[test]
    fn cmd_tail_no_events() {
        let dir = TempDir::new().unwrap();
        setup_workspace(&dir);
        let result = cmd_tail(dir.path());
        assert!(result.contains("No events file"));
    }

    #[test]
    fn cmd_tail_with_events() {
        let dir = TempDir::new().unwrap();
        setup_workspace(&dir);

        let events_path = dir.path().join(".ralph/events.jsonl");
        let mut f = std::fs::File::create(&events_path).unwrap();
        for i in 0..5 {
            writeln!(
                f,
                r#"{{"topic":"work.start","iteration":{},"hat":"executor","payload":"task {}","ts":"2026-01-30T10:00:00Z"}}"#,
                i, i
            )
            .unwrap();
        }

        let result = cmd_tail(dir.path());
        assert!(result.contains("Last 5 Events"));
        assert!(result.contains("5 total"));
        assert!(result.contains("work.start"));
    }

    #[test]
    fn cmd_tail_with_current_events_pointer() {
        let dir = TempDir::new().unwrap();
        setup_workspace(&dir);

        // Create the events file with a timestamped name
        let events_file = ".ralph/events-20260130-100000.jsonl";
        let events_path = dir.path().join(events_file);
        let mut f = std::fs::File::create(&events_path).unwrap();
        writeln!(
            f,
            r#"{{"topic":"plan.start","iteration":1,"payload":"planning","ts":"2026-01-30T10:00:00Z"}}"#
        )
        .unwrap();

        // Create the pointer file
        let pointer_path = dir.path().join(".ralph/current-events");
        std::fs::write(&pointer_path, events_file).unwrap();

        let result = cmd_tail(dir.path());
        assert!(result.contains("plan.start"));
    }

    #[test]
    fn cmd_tail_truncates_long_payloads() {
        let dir = TempDir::new().unwrap();
        setup_workspace(&dir);

        let events_path = dir.path().join(".ralph/events.jsonl");
        let mut f = std::fs::File::create(&events_path).unwrap();
        let long_payload = "a".repeat(200);
        writeln!(
            f,
            r#"{{"topic":"work.done","iteration":1,"payload":"{}","ts":"2026-01-30T10:00:00Z"}}"#,
            long_payload
        )
        .unwrap();

        let result = cmd_tail(dir.path());
        assert!(result.contains("..."));
        assert!(result.len() < 300); // Truncated, not full 200 chars
    }

    #[test]
    fn cmd_restart_no_active_loop() {
        let dir = TempDir::new().unwrap();
        setup_workspace(&dir);
        let result = cmd_restart(dir.path());
        assert!(result.contains("No active loop"));
    }

    #[test]
    fn cmd_restart_writes_signal_file() {
        let dir = TempDir::new().unwrap();
        setup_workspace(&dir);

        // Create lock file to simulate active loop
        let lock = serde_json::json!({
            "pid": 12345,
            "started": "2026-01-30T10:00:00Z",
            "prompt": "Test prompt"
        });
        let lock_path = dir.path().join(".ralph/loop.lock");
        std::fs::write(&lock_path, serde_json::to_string(&lock).unwrap()).unwrap();

        let result = cmd_restart(dir.path());
        assert!(result.contains("Restart requested"));

        // Verify signal file was created
        let restart_path = dir.path().join(".ralph/restart-requested");
        assert!(restart_path.exists());
    }

    #[test]
    fn handle_command_recognizes_restart() {
        let dir = TempDir::new().unwrap();
        setup_workspace(&dir);
        assert!(handle_command("/restart", dir.path()).is_some());
    }

    #[test]
    fn cmd_help_lists_restart() {
        let result = cmd_help();
        assert!(result.contains("/restart"));
    }
}
