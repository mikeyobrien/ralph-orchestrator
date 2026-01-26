//! CLI commands for the `ralph loops` namespace.
//!
//! Manage parallel Ralph loops running in git worktrees.
//!
//! Subcommands:
//! - `list`: Show all loops (active, merging, merged, needs-review)
//! - `logs`: View loop output
//! - `history`: Show event history
//! - `retry`: Re-run merge for failed loop
//! - `discard`: Abandon loop and cleanup
//! - `stop`: Terminate running loop
//! - `prune`: Clean up stale loops
//! - `attach`: Open shell in worktree
//! - `diff`: Show changes from merge-base

use std::path::PathBuf;
use std::process::Command;

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};

use ralph_core::worktree::{list_ralph_worktrees, remove_worktree};
use ralph_core::{LoopRegistry, MergeQueue, MergeState};

/// Manage parallel loops.
#[derive(Parser, Debug)]
pub struct LoopsArgs {
    #[command(subcommand)]
    pub command: Option<LoopsCommands>,
}

#[derive(Subcommand, Debug)]
pub enum LoopsCommands {
    /// List all loops (default if no subcommand)
    List,

    /// View loop output/logs
    Logs(LogsArgs),

    /// Show event history for a loop
    History(HistoryArgs),

    /// Re-run merge for a failed loop
    Retry(RetryArgs),

    /// Abandon loop and clean up worktree
    Discard(DiscardArgs),

    /// Stop a running loop
    Stop(StopArgs),

    /// Clean up stale loops (crashed processes)
    Prune,

    /// Open shell in loop's worktree
    Attach(AttachArgs),

    /// Show diff of loop's changes from merge-base
    Diff(DiffArgs),

    /// Merge a completed loop (or force retry)
    Merge(MergeArgs),
}

#[derive(Parser, Debug)]
pub struct LogsArgs {
    /// Loop ID (e.g., ralph-20250124-a3f2 or just a3f2)
    pub loop_id: String,

    /// Follow output in real-time
    #[arg(short, long)]
    pub follow: bool,
}

#[derive(Parser, Debug)]
pub struct HistoryArgs {
    /// Loop ID
    pub loop_id: String,

    /// Output raw JSONL instead of formatted table
    #[arg(long)]
    pub json: bool,
}

#[derive(Parser, Debug)]
pub struct RetryArgs {
    /// Loop ID
    pub loop_id: String,
}

#[derive(Parser, Debug)]
pub struct DiscardArgs {
    /// Loop ID
    pub loop_id: String,

    /// Skip confirmation prompt
    #[arg(short = 'y', long)]
    pub yes: bool,
}

#[derive(Parser, Debug)]
pub struct StopArgs {
    /// Loop ID
    pub loop_id: String,

    /// Use SIGKILL instead of SIGTERM
    #[arg(long)]
    pub force: bool,
}

#[derive(Parser, Debug)]
pub struct AttachArgs {
    /// Loop ID
    pub loop_id: String,
}

#[derive(Parser, Debug)]
pub struct DiffArgs {
    /// Loop ID
    pub loop_id: String,

    /// Show stat only (no diff content)
    #[arg(long)]
    pub stat: bool,
}

#[derive(Parser, Debug)]
pub struct MergeArgs {
    /// Loop ID
    pub loop_id: String,

    /// Force merge even if state is 'merging'
    #[arg(long)]
    pub force: bool,
}

/// Execute a loops command.
pub fn execute(args: LoopsArgs, use_colors: bool) -> Result<()> {
    match args.command {
        None | Some(LoopsCommands::List) => list_loops(use_colors),
        Some(LoopsCommands::Logs(logs_args)) => show_logs(logs_args),
        Some(LoopsCommands::History(history_args)) => show_history(history_args),
        Some(LoopsCommands::Retry(retry_args)) => retry_merge(retry_args),
        Some(LoopsCommands::Discard(discard_args)) => discard_loop(discard_args),
        Some(LoopsCommands::Stop(stop_args)) => stop_loop(stop_args),
        Some(LoopsCommands::Prune) => prune_stale(),
        Some(LoopsCommands::Attach(attach_args)) => attach_to_loop(attach_args),
        Some(LoopsCommands::Diff(diff_args)) => show_diff(diff_args),
        Some(LoopsCommands::Merge(merge_args)) => merge_loop(merge_args),
    }
}

/// Check if a process is alive.
fn is_process_alive(pid: u32) -> bool {
    #[cfg(unix)]
    {
        use nix::sys::signal::kill;
        use nix::unistd::Pid;

        // Signal 0 checks if process exists without sending a signal
        kill(Pid::from_raw(pid as i32), None).is_ok()
    }

    #[cfg(not(unix))]
    {
        let _ = pid;
        false
    }
}

/// List all loops with their status.
fn list_loops(use_colors: bool) -> Result<()> {
    use ralph_core::LoopLock;

    let cwd = std::env::current_dir()?;
    let registry = LoopRegistry::new(&cwd);
    let merge_queue = MergeQueue::new(&cwd);

    // Get loops from registry
    let loop_entries = registry.list().unwrap_or_default();

    // Get worktrees for additional info
    let worktrees = list_ralph_worktrees(&cwd).unwrap_or_default();

    // Get merge queue entries
    let merge_entries = merge_queue.list().unwrap_or_default();

    // Build combined view
    let mut rows: Vec<LoopRow> = Vec::new();

    // Check for primary loop holding the lock (not in a worktree)
    if let Ok(true) = LoopLock::is_locked(&cwd) {
        // Only show primary loop if it's not already tracked in the registry
        // (Registry entries with no worktree_path are primary loops)
        let primary_in_registry = loop_entries
            .iter()
            .any(|e| e.worktree_path.is_none() && e.is_alive());

        if !primary_in_registry && let Ok(Some(metadata)) = LoopLock::read_existing(&cwd) {
            // Verify the process is actually alive
            let is_alive = is_process_alive(metadata.pid);
            if is_alive {
                rows.push(LoopRow {
                    id: "(primary)".to_string(),
                    status: "running".to_string(),
                    location: "(in-place)".to_string(),
                    prompt: truncate(&metadata.prompt, 40),
                });
            }
        }
    }

    // Add running loops from registry
    for entry in &loop_entries {
        let status = if entry.is_alive() {
            "running"
        } else {
            "crashed"
        };

        let location = entry
            .worktree_path
            .as_ref()
            .map(|p| shorten_path(p))
            .unwrap_or_else(|| "(in-place)".to_string());

        rows.push(LoopRow {
            id: entry.id.clone(),
            status: status.to_string(),
            location,
            prompt: truncate(&entry.prompt, 40),
        });
    }

    // Add merge queue entries not in registry
    for entry in &merge_entries {
        let already_listed = rows.iter().any(|r| r.id.ends_with(&entry.loop_id));
        if !already_listed {
            let status = match entry.state {
                MergeState::Queued => "queued",
                MergeState::Merging => "merging",
                MergeState::Merged => "merged",
                MergeState::NeedsReview => "needs-review",
                MergeState::Discarded => "discarded",
            };

            rows.push(LoopRow {
                id: entry.loop_id.clone(),
                status: status.to_string(),
                location: "-".to_string(),
                prompt: truncate(&entry.prompt, 40),
            });
        }
    }

    // Add orphan worktrees (not in registry or merge queue)
    for wt in &worktrees {
        if wt.branch.starts_with("ralph/") {
            let loop_id = wt.branch.trim_start_matches("ralph/");
            let already_listed = rows.iter().any(|r| r.id.contains(loop_id));
            if !already_listed {
                rows.push(LoopRow {
                    id: loop_id.to_string(),
                    status: "orphan".to_string(),
                    location: shorten_path(&wt.path.to_string_lossy()),
                    prompt: String::new(),
                });
            }
        }
    }

    if rows.is_empty() {
        println!("No loops found.");
        return Ok(());
    }

    // Print table
    println!("{:<20} {:<12} {:<25} PROMPT", "ID", "STATUS", "LOCATION");
    println!("{}", "-".repeat(80));

    for row in rows {
        let status_display = if use_colors {
            colorize_status(&row.status)
        } else {
            row.status.clone()
        };

        println!(
            "{:<20} {:<12} {:<25} {}",
            truncate(&row.id, 20),
            status_display,
            truncate(&row.location, 25),
            row.prompt
        );
    }

    Ok(())
}

struct LoopRow {
    id: String,
    status: String,
    location: String,
    prompt: String,
}

fn colorize_status(status: &str) -> String {
    match status {
        "running" => format!("\x1b[32m{}\x1b[0m", status), // green
        "merging" => format!("\x1b[33m{}\x1b[0m", status), // yellow
        "merged" => format!("\x1b[34m{}\x1b[0m", status),  // blue
        "needs-review" => format!("\x1b[31m{}\x1b[0m", status), // red
        "crashed" => format!("\x1b[31m{}\x1b[0m", status), // red
        "orphan" => format!("\x1b[90m{}\x1b[0m", status),  // gray
        "queued" => format!("\x1b[36m{}\x1b[0m", status),  // cyan
        "discarded" => format!("\x1b[90m{}\x1b[0m", status), // gray
        _ => status.to_string(),
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}...", &s[..max.saturating_sub(3)])
    }
}

fn shorten_path(path: &str) -> String {
    // Show just the last component or relative path
    if let Some(last) = std::path::Path::new(path).file_name() {
        last.to_string_lossy().to_string()
    } else {
        path.to_string()
    }
}

/// Show logs for a loop.
fn show_logs(args: LogsArgs) -> Result<()> {
    let cwd = std::env::current_dir()?;
    let (loop_id, worktree_path) = resolve_loop(&cwd, &args.loop_id)?;

    let base_path = if let Some(ref wt_path) = worktree_path {
        PathBuf::from(wt_path)
    } else {
        cwd.clone()
    };

    let events_path = base_path.join(".ralph/events.jsonl");

    if !events_path.exists() {
        // Fallback: show history file instead
        let history_path = base_path.join(".ralph/history.jsonl");

        if history_path.exists() {
            eprintln!(
                "Note: Events file not found for loop '{}', showing history instead",
                loop_id
            );
            let contents =
                std::fs::read_to_string(&history_path).context("Failed to read history file")?;
            for line in contents.lines() {
                println!("{}", line);
            }
            return Ok(());
        }

        bail!(
            "No events file found for loop '{}' (may have crashed before publishing events)",
            loop_id
        );
    }

    if args.follow {
        // Use tail -f for following
        let status = Command::new("tail")
            .args(["-f", events_path.to_string_lossy().as_ref()])
            .status()
            .context("Failed to run tail")?;

        if !status.success() {
            bail!("tail exited with error");
        }
    } else {
        // Just cat the file
        let contents =
            std::fs::read_to_string(&events_path).context("Failed to read events file")?;
        print!("{}", contents);
    }

    Ok(())
}

/// Show history for a loop.
fn show_history(args: HistoryArgs) -> Result<()> {
    let cwd = std::env::current_dir()?;
    let (loop_id, worktree_path) = resolve_loop(&cwd, &args.loop_id)?;

    let history_path = if let Some(wt_path) = worktree_path {
        PathBuf::from(wt_path).join(".ralph/history.jsonl")
    } else {
        cwd.join(".ralph/history.jsonl")
    };

    if !history_path.exists() {
        bail!("No history file found for loop '{}'", loop_id);
    }

    let contents = std::fs::read_to_string(&history_path).context("Failed to read history file")?;

    if args.json {
        print!("{}", contents);
    } else {
        // Format as table
        println!("{:<25} {:<20} DATA", "TIMESTAMP", "TYPE");
        println!("{}", "-".repeat(80));

        for line in contents.lines() {
            if line.trim().is_empty() {
                continue;
            }
            if let Ok(event) = serde_json::from_str::<serde_json::Value>(line) {
                let ts = event.get("ts").and_then(|v| v.as_str()).unwrap_or("-");
                let event_type = event.get("type").and_then(|v| v.as_str()).unwrap_or("-");
                let data = event.get("data").map(|v| v.to_string()).unwrap_or_default();
                println!(
                    "{:<25} {:<20} {}",
                    truncate(ts, 25),
                    event_type,
                    truncate(&data, 35)
                );
            }
        }
    }

    Ok(())
}

/// Retry merge for a failed loop.
fn retry_merge(args: RetryArgs) -> Result<()> {
    let cwd = std::env::current_dir()?;
    let merge_queue = MergeQueue::new(&cwd);

    let entry = merge_queue
        .get_entry(&args.loop_id)?
        .context(format!("Loop '{}' not found in merge queue", args.loop_id))?;

    if entry.state != MergeState::NeedsReview {
        bail!(
            "Loop '{}' is in state {:?}, can only retry 'needs-review' loops",
            args.loop_id,
            entry.state
        );
    }

    spawn_merge_ralph(&cwd, &args.loop_id)
}

/// Discard a loop and clean up.
fn discard_loop(args: DiscardArgs) -> Result<()> {
    let cwd = std::env::current_dir()?;
    let (loop_id, worktree_path) = resolve_loop(&cwd, &args.loop_id)?;

    // Confirmation unless -y
    if !args.yes {
        eprintln!(
            "This will permanently discard loop '{}' and delete its worktree.",
            loop_id
        );
        eprintln!("Continue? [y/N] ");

        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        if !input.trim().eq_ignore_ascii_case("y") {
            println!("Aborted.");
            return Ok(());
        }
    }

    // Update merge queue
    let merge_queue = MergeQueue::new(&cwd);
    if let Ok(Some(_)) = merge_queue.get_entry(&loop_id) {
        merge_queue.discard(&loop_id, Some("User requested discard"))?;
    }

    // Deregister from registry
    let registry = LoopRegistry::new(&cwd);
    let _ = registry.deregister(&loop_id);

    // Remove worktree if exists
    if let Some(wt_path) = worktree_path {
        println!("Removing worktree at {}...", wt_path);
        remove_worktree(&cwd, &wt_path)?;
    }

    println!("Loop '{}' discarded.", loop_id);
    Ok(())
}

/// Stop a running loop.
fn stop_loop(args: StopArgs) -> Result<()> {
    use ralph_core::LoopLock;

    let cwd = std::env::current_dir()?;
    let (loop_id, worktree_path) = resolve_loop(&cwd, &args.loop_id)?;

    let registry = LoopRegistry::new(&cwd);

    // Try registry first for PID
    let pid = if let Ok(Some(entry)) = registry.get(&loop_id) {
        if !entry.is_alive() {
            bail!(
                "Loop '{}' is not running (process {} not found)",
                loop_id,
                entry.pid
            );
        }
        Some(entry.pid)
    } else if let Some(wt_path) = &worktree_path {
        // Fall back to reading PID from worktree's lock file
        read_pid_from_worktree(wt_path)?
    } else {
        // Try reading from primary loop's lock file
        if let Ok(Some(metadata)) = LoopLock::read_existing(&cwd) {
            Some(metadata.pid)
        } else {
            None
        }
    };

    let pid = pid.context(format!(
        "Cannot determine PID for loop '{}' - it may have already stopped",
        loop_id
    ))?;

    // Verify process is alive before sending signal
    #[cfg(unix)]
    {
        use nix::sys::signal::{Signal, kill};
        use nix::unistd::Pid;

        // Check if process exists (signal 0)
        if kill(Pid::from_raw(pid as i32), None).is_err() {
            bail!(
                "Loop '{}' is not running (process {} not found)",
                loop_id,
                pid
            );
        }

        let signal = if args.force { "SIGKILL" } else { "SIGTERM" };
        println!("Sending {} to loop '{}' (PID {})...", signal, loop_id, pid);

        let sig = if args.force {
            Signal::SIGKILL
        } else {
            Signal::SIGTERM
        };

        kill(Pid::from_raw(pid as i32), sig).context("Failed to send signal")?;
    }

    #[cfg(not(unix))]
    {
        let _ = pid;
        bail!("Stopping loops is only supported on Unix systems");
    }

    println!("Signal sent.");
    Ok(())
}

/// Read PID from a worktree's loop lock file.
fn read_pid_from_worktree(worktree_path: &str) -> Result<Option<u32>> {
    use ralph_core::LoopLock;

    let wt_path = PathBuf::from(worktree_path);
    if let Ok(Some(metadata)) = LoopLock::read_existing(&wt_path) {
        Ok(Some(metadata.pid))
    } else {
        Ok(None)
    }
}

/// Prune stale loops.
fn prune_stale() -> Result<()> {
    let cwd = std::env::current_dir()?;
    let registry = LoopRegistry::new(&cwd);

    let count = registry.clean_stale()?;

    if count == 0 {
        println!("No stale loops found.");
    } else {
        println!("Cleaned up {} stale loop(s).", count);
    }

    // Also check for orphan worktrees
    let worktrees = list_ralph_worktrees(&cwd).unwrap_or_default();
    let loop_entries = registry.list().unwrap_or_default();

    let mut orphan_count = 0;
    for wt in worktrees {
        if wt.branch.starts_with("ralph/") {
            let loop_id = wt.branch.trim_start_matches("ralph/");
            let in_registry = loop_entries.iter().any(|e| e.id.contains(loop_id));
            if !in_registry {
                println!(
                    "Found orphan worktree: {} (branch: {})",
                    wt.path.display(),
                    wt.branch
                );
                orphan_count += 1;
            }
        }
    }

    if orphan_count > 0 {
        println!("\nTo remove orphan worktrees, use: ralph loops discard <id>");
    }

    Ok(())
}

/// Attach to a loop's worktree.
fn attach_to_loop(args: AttachArgs) -> Result<()> {
    let cwd = std::env::current_dir()?;
    let (loop_id, worktree_path) = resolve_loop(&cwd, &args.loop_id)?;

    let wt_path = worktree_path.context(format!(
        "Loop '{}' is not a worktree-based loop (it runs in-place)",
        loop_id
    ))?;

    println!("Attaching to loop '{}' at {}...", loop_id, wt_path);
    println!("Type 'exit' to return.\n");

    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string());

    let status = Command::new(&shell)
        .current_dir(&wt_path)
        .status()
        .context("Failed to spawn shell")?;

    if !status.success() {
        bail!("Shell exited with error");
    }

    Ok(())
}

/// Show diff for a loop.
fn show_diff(args: DiffArgs) -> Result<()> {
    let cwd = std::env::current_dir()?;
    let (loop_id, _worktree_path) = resolve_loop(&cwd, &args.loop_id)?;

    // Find the branch
    let branch = format!("ralph/{}", loop_id);

    // Check if branch exists
    let output = Command::new("git")
        .args(["rev-parse", "--verify", &branch])
        .current_dir(&cwd)
        .output()
        .context("Failed to run git")?;

    if !output.status.success() {
        bail!("Branch '{}' not found", branch);
    }

    // Show diff from merge-base
    // Note: three-dot syntax requires both refs in a single argument: "main...branch"
    let diff_range = format!("main...{}", branch);
    let mut git_args = vec!["diff", &diff_range];

    if args.stat {
        git_args.push("--stat");
    }

    let status = Command::new("git")
        .args(&git_args)
        .current_dir(&cwd)
        .status()
        .context("Failed to run git diff")?;

    if !status.success() {
        bail!("git diff failed");
    }

    Ok(())
}

/// Merge a completed loop (or force retry).
fn merge_loop(args: MergeArgs) -> Result<()> {
    let cwd = std::env::current_dir()?;
    let registry = LoopRegistry::new(&cwd);
    let merge_queue = MergeQueue::new(&cwd);

    // Try to find the loop in various places
    let (loop_id, worktree_path) = resolve_loop(&cwd, &args.loop_id)?;

    // 1. Check if it's running
    if let Ok(Some(entry)) = registry.get(&loop_id)
        && entry.is_alive()
    {
        bail!("Loop '{}' is still running. Stop it first.", loop_id);
    }

    // 2. Check merge queue state
    if let Ok(Some(entry)) = merge_queue.get_entry(&loop_id) {
        match entry.state {
            MergeState::Merged => bail!("Loop '{}' is already merged.", loop_id),
            MergeState::Discarded => bail!("Loop '{}' is discarded.", loop_id),
            MergeState::Merging => {
                if !args.force {
                    bail!(
                        "Loop '{}' is currently merging (PID {:?}). Use --force to override.",
                        loop_id,
                        entry.merge_pid
                    );
                }
                println!("Force-merging loop '{}'...", loop_id);
            }
            MergeState::Queued | MergeState::NeedsReview => {
                println!("Merging loop '{}'...", loop_id);
            }
        }
    } else {
        // 3. Not in queue - check if it's an orphan worktree
        let worktrees = list_ralph_worktrees(&cwd).unwrap_or_default();
        let is_orphan = worktrees
            .iter()
            .any(|wt| wt.branch == format!("ralph/{}", loop_id));

        if is_orphan {
            println!(
                "Found orphan worktree for loop '{}'. Queueing for merge...",
                loop_id
            );
            // We need a prompt for the queue entry. Since it's an orphan, we might not have it easily.
            // Try to read it from the worktree's loop lock if available, or use a placeholder.
            let prompt = if let Some(wt_path) = worktree_path {
                use ralph_core::LoopLock;
                LoopLock::read_existing(std::path::Path::new(&wt_path))
                    .ok()
                    .flatten()
                    .map(|m| m.prompt)
                    .unwrap_or_else(|| "Orphan loop (recovered)".to_string())
            } else {
                "Orphan loop (recovered)".to_string()
            };

            merge_queue.enqueue(&loop_id, &prompt)?;
        } else {
            bail!(
                "Loop '{}' is not ready for merge (not in queue and not an orphan worktree).",
                loop_id
            );
        }
    }

    spawn_merge_ralph(&cwd, &loop_id)
}

/// Helper to spawn merge-ralph
fn spawn_merge_ralph(cwd: &std::path::Path, loop_id: &str) -> Result<()> {
    // Get the merge-loop preset and write to config file
    let preset = crate::presets::get_preset("merge-loop").context("merge-loop preset not found")?;

    let config_path = cwd.join(".ralph/merge-loop-config.yml");
    std::fs::write(&config_path, preset.content).context("Failed to write merge config file")?;

    // Spawn merge-ralph
    println!("Spawning merge-ralph for loop '{}'...", loop_id);

    let status = Command::new("ralph")
        .args([
            "run",
            "-c",
            ".ralph/merge-loop-config.yml",
            "-p",
            &format!("Merge loop {} from branch ralph/{}", loop_id, loop_id),
        ])
        .env("RALPH_MERGE_LOOP_ID", loop_id)
        .status()
        .context("Failed to spawn merge-ralph")?;

    if !status.success() {
        bail!("merge-ralph exited with error");
    }

    Ok(())
}

/// Resolve a loop ID to its full ID and worktree path (if any).
fn resolve_loop(cwd: &std::path::Path, id: &str) -> Result<(String, Option<String>)> {
    let registry = LoopRegistry::new(cwd);
    let merge_queue = MergeQueue::new(cwd);

    // Try exact match in registry
    if let Ok(Some(entry)) = registry.get(id) {
        return Ok((entry.id.clone(), entry.worktree_path.clone()));
    }

    // Try partial match (e.g., "a3f2" matches "ralph-20250124-143052-a3f2")
    if let Ok(entries) = registry.list() {
        for entry in entries {
            if entry.id.ends_with(id) || entry.id.contains(id) {
                return Ok((entry.id.clone(), entry.worktree_path.clone()));
            }
        }
    }

    // Try merge queue
    if let Ok(Some(entry)) = merge_queue.get_entry(id) {
        // Look up worktree from worktrees list
        let worktrees = list_ralph_worktrees(cwd).unwrap_or_default();
        let wt_path = worktrees
            .iter()
            .find(|wt| wt.branch.ends_with(&entry.loop_id))
            .map(|wt| wt.path.to_string_lossy().to_string());

        return Ok((entry.loop_id.clone(), wt_path));
    }

    // Try worktrees directly
    let worktrees = list_ralph_worktrees(cwd).unwrap_or_default();
    for wt in worktrees {
        if wt.branch.ends_with(id) || wt.branch.contains(id) {
            let loop_id = wt.branch.trim_start_matches("ralph/").to_string();
            return Ok((loop_id, Some(wt.path.to_string_lossy().to_string())));
        }
    }

    bail!("Loop '{}' not found", id);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_truncate() {
        assert_eq!(truncate("hello", 10), "hello");
        assert_eq!(truncate("hello world", 8), "hello...");
        assert_eq!(truncate("hi", 2), "hi");
    }

    #[test]
    fn test_colorize_status() {
        // Just verify it returns something with escape codes for colored statuses
        assert!(colorize_status("running").contains("\x1b["));
        assert!(colorize_status("merged").contains("\x1b["));
        // Unknown status returns as-is
        assert_eq!(colorize_status("unknown"), "unknown");
    }

    #[test]
    fn test_shorten_path() {
        assert_eq!(shorten_path("/foo/bar/baz"), "baz");
        assert_eq!(shorten_path("./worktrees/ralph-abc"), "ralph-abc");
    }
}
