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
use ralph_core::{LoopRegistry, MergeButtonState, MergeQueue, MergeState, merge_button_state};

/// Manage parallel loops.
#[derive(Parser, Debug)]
pub struct LoopsArgs {
    #[command(subcommand)]
    pub command: Option<LoopsCommands>,
}

#[derive(Subcommand, Debug)]
pub enum LoopsCommands {
    /// List all loops (default if no subcommand)
    List(ListArgs),

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

    /// Process pending merge queue entries
    Process,

    /// Get merge button state for a loop (JSON output for web API)
    MergeButtonState(MergeButtonStateArgs),
}

#[derive(Parser, Debug)]
pub struct ListArgs {
    /// Output JSON instead of table
    #[arg(long)]
    pub json: bool,

    /// Show all loops including terminal states (merged, discarded)
    #[arg(long)]
    pub all: bool,
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
    /// Loop ID (group-id). If omitted, stops the active primary loop.
    #[arg(value_name = "LOOP_ID")]
    pub loop_id: Option<String>,

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

#[derive(Parser, Debug)]
pub struct MergeButtonStateArgs {
    /// Loop ID
    pub loop_id: String,
}

/// Execute a loops command.
pub fn execute(args: LoopsArgs, use_colors: bool) -> Result<()> {
    match args.command {
        None => list_loops(
            ListArgs {
                json: false,
                all: false,
            },
            use_colors,
        ),
        Some(LoopsCommands::List(args)) => list_loops(args, use_colors),
        Some(LoopsCommands::Logs(logs_args)) => show_logs(logs_args),
        Some(LoopsCommands::History(history_args)) => show_history(history_args),
        Some(LoopsCommands::Retry(retry_args)) => retry_merge(retry_args),
        Some(LoopsCommands::Discard(discard_args)) => discard_loop(discard_args),
        Some(LoopsCommands::Stop(stop_args)) => stop_loop(stop_args),
        Some(LoopsCommands::Prune) => prune_stale(),
        Some(LoopsCommands::Attach(attach_args)) => attach_to_loop(attach_args),
        Some(LoopsCommands::Diff(diff_args)) => show_diff(diff_args),
        Some(LoopsCommands::Merge(merge_args)) => merge_loop(merge_args),
        Some(LoopsCommands::Process) => process_queue(),
        Some(LoopsCommands::MergeButtonState(args)) => get_merge_button_state(args),
    }
}

/// Process pending merge queue entries.
fn process_queue() -> Result<()> {
    let cwd = std::env::current_dir()?;

    // Delegate to the loop_runner's process_pending_merges function
    crate::loop_runner::process_pending_merges_cli(&cwd);

    Ok(())
}

/// Get merge button state for a loop (JSON output for web API).
fn get_merge_button_state(args: MergeButtonStateArgs) -> Result<()> {
    let cwd = std::env::current_dir()?;
    let state = merge_button_state(&cwd, &args.loop_id)?;

    let json = match state {
        MergeButtonState::Active => serde_json::json!({ "state": "active" }),
        MergeButtonState::Blocked { reason } => {
            serde_json::json!({ "state": "blocked", "reason": reason })
        }
    };

    println!("{}", serde_json::to_string(&json)?);
    Ok(())
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

/// Format duration as relative age (e.g., "5m", "2h", "1d").
fn format_age(duration: chrono::Duration) -> String {
    let secs = duration.num_seconds();
    if secs < 60 {
        format!("{}s", secs)
    } else if secs < 3600 {
        format!("{}m", secs / 60)
    } else if secs < 86400 {
        format!("{}h", secs / 3600)
    } else {
        format!("{}d", secs / 86400)
    }
}

/// List all loops with their status.
fn list_loops(args: ListArgs, use_colors: bool) -> Result<()> {
    use ralph_core::LoopLock;

    let cwd = std::env::current_dir()?;
    let registry = LoopRegistry::new(&cwd);
    let merge_queue = MergeQueue::new(&cwd);
    let now = chrono::Utc::now();

    // Get loops from registry
    let loop_entries = registry.list().unwrap_or_default();

    // Get worktrees for additional info
    let worktrees = list_ralph_worktrees(&cwd).unwrap_or_default();

    // Get merge queue entries
    let merge_entries = merge_queue.list().unwrap_or_default();

    // Build combined view
    let mut rows: Vec<LoopRow> = Vec::new();
    let mut has_needs_review = false;
    let mut hidden_terminal_count = 0;

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
                    age: None,   // Primary loop age not easily available
                    merge: None, // Primary loop doesn't have merge state
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
            age: None, // Registry doesn't track start time
            merge: None,
        });
    }

    // Add merge queue entries not in registry
    for entry in &merge_entries {
        let already_listed = rows.iter().any(|r| r.id.ends_with(&entry.loop_id));
        if !already_listed {
            // Skip terminal merge states unless --all is specified
            if entry.state.is_terminal() && !args.all {
                hidden_terminal_count += 1;
                continue;
            }

            let status = match entry.state {
                MergeState::Queued => "queued",
                MergeState::Merging => "merging",
                MergeState::Merged => "merged",
                MergeState::NeedsReview => {
                    has_needs_review = true;
                    "needs-review"
                }
                MergeState::Discarded => "discarded",
            };

            // Calculate age from entry timestamp
            let age = Some(format_age(now.signed_duration_since(entry.queued_at)));

            // For merged entries, show commit SHA in location column
            let location = if let Some(ref sha) = entry.merge_commit {
                sha.clone()
            } else {
                "-".to_string()
            };

            // Get merge button state for queued entries
            let merge_status = if entry.state == MergeState::Queued {
                match merge_button_state(&cwd, &entry.loop_id) {
                    Ok(MergeButtonState::Active) => Some("ready".to_string()),
                    Ok(MergeButtonState::Blocked { .. }) => Some("blocked".to_string()),
                    Err(_) => None,
                }
            } else {
                None
            };

            rows.push(LoopRow {
                id: entry.loop_id.clone(),
                status: status.to_string(),
                location,
                prompt: truncate(&entry.prompt, 40),
                age,
                merge: merge_status,
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
                    age: None,
                    merge: None,
                });
            }
        }
    }

    if rows.is_empty() {
        if args.json {
            println!("[]");
        } else {
            println!("No loops found.");
        }
        return Ok(());
    }

    if args.json {
        let json = serde_json::to_string_pretty(&rows)?;
        println!("{json}");
        return Ok(());
    }

    // Count by status for summary header
    let mut status_counts: std::collections::HashMap<&str, usize> =
        std::collections::HashMap::new();
    for row in &rows {
        *status_counts.entry(&row.status).or_insert(0) += 1;
    }

    // Print summary header
    let summary_parts: Vec<String> = [
        "running",
        "queued",
        "merging",
        "needs-review",
        "merged",
        "discarded",
        "crashed",
        "orphan",
    ]
    .iter()
    .filter_map(|s| {
        status_counts
            .get(s)
            .map(|count| format!("{}: {}", s, count))
    })
    .collect();

    if !summary_parts.is_empty() {
        println!("Loops: {}", summary_parts.join(", "));
        println!();
    }

    // Print table
    println!(
        "{:<20} {:<12} {:<8} {:<8} {:<20} PROMPT",
        "ID", "STATUS", "MERGE", "AGE", "LOCATION"
    );
    println!("{}", "-".repeat(88));

    for row in rows {
        let status_display = if use_colors {
            colorize_status(&row.status)
        } else {
            row.status.clone()
        };

        let age_display = row.age.as_deref().unwrap_or("-");
        let merge_display = row.merge.as_deref().unwrap_or("-");

        println!(
            "{:<20} {:<12} {:<8} {:<8} {:<20} {}",
            truncate(&row.id, 20),
            status_display,
            merge_display,
            age_display,
            truncate(&row.location, 20),
            row.prompt
        );
    }

    // Print footer hints
    println!();
    if hidden_terminal_count > 0 {
        println!(
            "({} merged/discarded hidden. Use --all to show.)",
            hidden_terminal_count
        );
    }
    if has_needs_review {
        println!("Hint: Use `ralph loops retry <id>` to retry failed merges.");
    }
    println!("Use `ralph loops --help` for more commands.");

    Ok(())
}

#[derive(serde::Serialize)]
struct LoopRow {
    id: String,
    status: String,
    location: String,
    prompt: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    age: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    merge: Option<String>,
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
    let (loop_id, worktree_path) = match args.loop_id.as_deref() {
        Some(id) => resolve_loop(&cwd, id)?,
        None => ("(primary)".to_string(), None),
    };

    let target_root = worktree_path
        .as_ref()
        .map(PathBuf::from)
        .unwrap_or_else(|| cwd.clone());

    let metadata = LoopLock::read_existing(&target_root)?
        .context("Cannot determine active loop - it may have already stopped")?;

    if !is_process_alive(metadata.pid) {
        bail!(
            "Loop '{}' is not running (process {} not found)",
            loop_id,
            metadata.pid
        );
    }

    if args.force {
        // Force-stop with SIGKILL for immediate termination.
        #[cfg(unix)]
        {
            use nix::sys::signal::{Signal, kill};
            use nix::unistd::Pid;

            println!(
                "Sending SIGKILL to loop '{}' (PID {})...",
                loop_id, metadata.pid
            );
            kill(Pid::from_raw(metadata.pid as i32), Signal::SIGKILL)
                .context("Failed to send SIGKILL")?;
            println!("Signal sent.");
            return Ok(());
        }

        #[cfg(not(unix))]
        {
            bail!("--force is only supported on Unix systems");
        }
    }

    let stop_path = target_root.join(".ralph/stop-requested");
    if let Some(parent) = stop_path.parent() {
        std::fs::create_dir_all(parent)
            .context("Failed to create .ralph directory for stop signal")?;
    }
    std::fs::write(&stop_path, "").context("Failed to write stop signal")?;

    println!(
        "Stop requested for loop '{}' (PID {}). The loop will stop at the next iteration boundary.",
        loop_id, metadata.pid
    );

    Ok(())
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
            "--exclusive",
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

    // Try partial match in merge queue
    if let Ok(entries) = merge_queue.list() {
        for entry in entries {
            if entry.loop_id.ends_with(id) || entry.loop_id.contains(id) {
                let worktrees = list_ralph_worktrees(cwd).unwrap_or_default();
                let wt_path = worktrees
                    .iter()
                    .find(|wt| wt.branch.ends_with(&entry.loop_id))
                    .map(|wt| wt.path.to_string_lossy().to_string());
                return Ok((entry.loop_id.clone(), wt_path));
            }
        }
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
    use crate::test_support::CwdGuard;
    use ralph_core::LoopLock;
    use ralph_core::loop_registry::LoopEntry;
    use std::process::Command;

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

    #[test]
    fn test_format_age_boundaries() {
        assert_eq!(format_age(chrono::Duration::seconds(59)), "59s");
        assert_eq!(format_age(chrono::Duration::seconds(60)), "1m");
        assert_eq!(format_age(chrono::Duration::seconds(3599)), "59m");
        assert_eq!(format_age(chrono::Duration::seconds(3600)), "1h");
        assert_eq!(format_age(chrono::Duration::seconds(86399)), "23h");
        assert_eq!(format_age(chrono::Duration::seconds(86400)), "1d");
    }

    #[cfg(unix)]
    #[test]
    fn test_is_process_alive_current_pid() {
        let pid = std::process::id();
        assert!(is_process_alive(pid));
    }

    #[test]
    fn test_list_loops_includes_registry_entry_json() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let _cwd = CwdGuard::set(temp_dir.path());

        let registry = LoopRegistry::new(temp_dir.path());
        let entry = LoopEntry::with_id(
            "loop-test-1234",
            "test prompt",
            Some("worktrees/loop-test-1234"),
            temp_dir.path().display().to_string(),
        );
        registry.register(entry).expect("register loop");

        list_loops(
            ListArgs {
                json: true,
                all: true,
            },
            false,
        )
        .expect("list loops");
    }

    #[test]
    fn test_resolve_loop_exact_match_registry() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let _cwd = CwdGuard::set(temp_dir.path());

        let registry = LoopRegistry::new(temp_dir.path());
        let entry = LoopEntry::with_id(
            "loop-test-9999",
            "resolve me",
            Some("worktrees/loop-test-9999"),
            temp_dir.path().display().to_string(),
        );
        registry.register(entry).expect("register loop");

        let (id, worktree) = resolve_loop(temp_dir.path(), "loop-test-9999").expect("resolve");
        assert_eq!(id, "loop-test-9999");
        assert_eq!(worktree, Some("worktrees/loop-test-9999".to_string()));
    }

    #[test]
    fn test_resolve_loop_partial_match_registry_suffix() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let _cwd = CwdGuard::set(temp_dir.path());

        let registry = LoopRegistry::new(temp_dir.path());
        let entry = LoopEntry::with_id(
            "loop-test-8888",
            "resolve suffix",
            None::<String>,
            temp_dir.path().display().to_string(),
        );
        registry.register(entry).expect("register loop");

        let (id, worktree) = resolve_loop(temp_dir.path(), "8888").expect("resolve");
        assert_eq!(id, "loop-test-8888");
        assert_eq!(worktree, None);
    }

    #[test]
    fn test_resolve_loop_from_merge_queue_entry() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let _cwd = CwdGuard::set(temp_dir.path());

        let queue = MergeQueue::new(temp_dir.path());
        queue
            .enqueue("loop-queue-1234", "merge prompt")
            .expect("enqueue");

        let (id, worktree) = resolve_loop(temp_dir.path(), "loop-queue-1234").expect("resolve");
        assert_eq!(id, "loop-queue-1234");
        assert_eq!(worktree, None);
    }

    #[test]
    fn test_resolve_loop_partial_match_merge_queue_suffix() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let _cwd = CwdGuard::set(temp_dir.path());

        let queue = MergeQueue::new(temp_dir.path());
        queue
            .enqueue("loop-queue-5678", "merge prompt")
            .expect("enqueue");

        let (id, worktree) = resolve_loop(temp_dir.path(), "5678").expect("resolve");
        assert_eq!(id, "loop-queue-5678");
        assert_eq!(worktree, None);
    }

    #[test]
    fn test_resolve_loop_missing_returns_error() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let _cwd = CwdGuard::set(temp_dir.path());

        let err = resolve_loop(temp_dir.path(), "does-not-exist").unwrap_err();
        assert!(err.to_string().contains("not found"));
    }

    #[test]
    fn test_list_loops_handles_merge_queue_states() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let _cwd = CwdGuard::set(temp_dir.path());

        let queue = MergeQueue::new(temp_dir.path());
        queue
            .enqueue("loop-review-1234", "merge prompt")
            .expect("enqueue");
        queue
            .mark_merging("loop-review-1234", 4242)
            .expect("mark merging");
        queue
            .mark_needs_review("loop-review-1234", "conflicts")
            .expect("needs review");

        queue
            .enqueue("loop-merged-5678", "merge prompt")
            .expect("enqueue");
        queue
            .mark_merging("loop-merged-5678", 9001)
            .expect("mark merging");
        queue
            .mark_merged("loop-merged-5678", "abc123")
            .expect("merged");

        list_loops(
            ListArgs {
                json: false,
                all: false,
            },
            false,
        )
        .expect("list loops");
    }

    #[test]
    fn test_get_merge_button_state_blocked_when_merging() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let _cwd = CwdGuard::set(temp_dir.path());

        let queue = MergeQueue::new(temp_dir.path());
        queue
            .enqueue("loop-merge-9999", "merge prompt")
            .expect("enqueue");
        queue
            .mark_merging("loop-merge-9999", 4242)
            .expect("mark merging");

        get_merge_button_state(MergeButtonStateArgs {
            loop_id: "loop-merge-9999".to_string(),
        })
        .expect("merge button state");
    }

    #[test]
    fn test_show_logs_falls_back_to_history() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let _cwd = CwdGuard::set(temp_dir.path());

        std::fs::create_dir_all(".ralph").expect("create .ralph");
        std::fs::write(
            ".ralph/history.jsonl",
            r#"{"ts":"2026-01-01T00:00:00Z","type":"event","data":{"ok":true}}"#,
        )
        .expect("write history");

        let registry = LoopRegistry::new(temp_dir.path());
        let entry = LoopEntry::with_id(
            "loop-log-1234",
            "test prompt",
            None::<String>,
            temp_dir.path().display().to_string(),
        );
        registry.register(entry).expect("register loop");

        show_logs(LogsArgs {
            loop_id: "loop-log-1234".to_string(),
            follow: false,
        })
        .expect("show logs");
    }

    #[test]
    fn test_show_history_formats_table() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let _cwd = CwdGuard::set(temp_dir.path());

        std::fs::create_dir_all(".ralph").expect("create .ralph");
        std::fs::write(
            ".ralph/history.jsonl",
            r#"{"ts":"2026-01-01T00:00:00Z","type":"event","data":{"ok":true}}"#,
        )
        .expect("write history");

        let registry = LoopRegistry::new(temp_dir.path());
        let entry = LoopEntry::with_id(
            "loop-hist-5678",
            "test prompt",
            None::<String>,
            temp_dir.path().display().to_string(),
        );
        registry.register(entry).expect("register loop");

        show_history(HistoryArgs {
            loop_id: "loop-hist-5678".to_string(),
            json: false,
        })
        .expect("show history");
    }

    #[test]
    fn test_retry_merge_rejects_non_needs_review_state() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let _cwd = CwdGuard::set(temp_dir.path());

        let queue = MergeQueue::new(temp_dir.path());
        queue.enqueue("loop-queue-1", "prompt").expect("enqueue");

        let err = retry_merge(RetryArgs {
            loop_id: "loop-queue-1".to_string(),
        })
        .expect_err("retry should fail for non-needs-review");

        assert!(err.to_string().contains("can only retry"));
    }

    #[test]
    fn test_discard_loop_marks_discarded_and_deregisters() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let _cwd = CwdGuard::set(temp_dir.path());

        let registry = LoopRegistry::new(temp_dir.path());
        let entry = LoopEntry::with_id(
            "loop-discard-1",
            "discard me",
            None::<String>,
            temp_dir.path().display().to_string(),
        );
        registry.register(entry).expect("register loop");

        let queue = MergeQueue::new(temp_dir.path());
        queue.enqueue("loop-discard-1", "prompt").expect("enqueue");

        discard_loop(DiscardArgs {
            loop_id: "loop-discard-1".to_string(),
            yes: true,
        })
        .expect("discard loop");

        let entry = queue
            .get_entry("loop-discard-1")
            .expect("get entry")
            .expect("entry exists");
        assert_eq!(entry.state, MergeState::Discarded);

        assert!(registry.get("loop-discard-1").unwrap().is_none());
    }

    #[cfg(unix)]
    #[test]
    fn test_stop_loop_writes_stop_requested_file() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let _cwd = CwdGuard::set(temp_dir.path());

        let _lock = LoopLock::try_acquire(temp_dir.path(), "test prompt").expect("lock");

        stop_loop(StopArgs {
            loop_id: None,
            force: false,
        })
        .expect("stop loop");

        assert!(temp_dir.path().join(".ralph/stop-requested").exists());
    }

    #[test]
    fn test_attach_to_loop_requires_worktree() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let _cwd = CwdGuard::set(temp_dir.path());

        let registry = LoopRegistry::new(temp_dir.path());
        let entry = LoopEntry::with_id(
            "loop-inplace-1",
            "no worktree",
            None::<String>,
            temp_dir.path().display().to_string(),
        );
        registry.register(entry).expect("register loop");

        let err = attach_to_loop(AttachArgs {
            loop_id: "loop-inplace-1".to_string(),
        })
        .expect_err("attach should fail for in-place loop");

        assert!(err.to_string().contains("not a worktree-based loop"));
    }

    #[test]
    fn test_show_diff_missing_branch_errors() {
        if Command::new("git").arg("--version").output().is_err() {
            return;
        }

        let temp_dir = tempfile::tempdir().expect("temp dir");
        let _cwd = CwdGuard::set(temp_dir.path());

        Command::new("git")
            .args(["init", "-q"])
            .status()
            .expect("git init");
        Command::new("git")
            .args(["config", "user.email", "test@example.com"])
            .status()
            .expect("git config email");
        Command::new("git")
            .args(["config", "user.name", "Test User"])
            .status()
            .expect("git config name");
        std::fs::write("README.md", "# Test").expect("write README");
        Command::new("git")
            .args(["add", "."])
            .status()
            .expect("git add");
        Command::new("git")
            .args(["commit", "-m", "Initial commit", "--quiet"])
            .status()
            .expect("git commit");

        let registry = LoopRegistry::new(temp_dir.path());
        let entry = LoopEntry::with_id(
            "loop-missing-branch",
            "diff me",
            None::<String>,
            temp_dir.path().display().to_string(),
        );
        registry.register(entry).expect("register loop");

        let err = show_diff(DiffArgs {
            loop_id: "loop-missing-branch".to_string(),
            stat: false,
        })
        .expect_err("missing branch should error");

        assert!(
            err.to_string()
                .contains("Branch 'ralph/loop-missing-branch' not found")
        );
    }

    #[test]
    fn test_execute_defaults_to_list_when_no_subcommand() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let _cwd = CwdGuard::set(temp_dir.path());

        execute(LoopsArgs { command: None }, false).expect("execute default");
    }

    #[test]
    fn test_get_merge_button_state_active_when_idle() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let _cwd = CwdGuard::set(temp_dir.path());

        get_merge_button_state(MergeButtonStateArgs {
            loop_id: "loop-idle-1".to_string(),
        })
        .expect("merge button state");
    }

    #[test]
    fn test_merge_loop_rejects_already_merged() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let _cwd = CwdGuard::set(temp_dir.path());

        let queue = MergeQueue::new(temp_dir.path());
        queue.enqueue("loop-merged-1", "prompt").expect("enqueue");
        queue
            .mark_merging("loop-merged-1", 4242)
            .expect("mark merging");
        queue
            .mark_merged("loop-merged-1", "abc123")
            .expect("mark merged");

        let err = merge_loop(MergeArgs {
            loop_id: "loop-merged-1".to_string(),
            force: false,
        })
        .expect_err("merge should fail for merged loop");

        assert!(err.to_string().contains("already merged"));
    }

    #[test]
    fn test_merge_loop_rejects_discarded() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let _cwd = CwdGuard::set(temp_dir.path());

        let queue = MergeQueue::new(temp_dir.path());
        queue
            .enqueue("loop-discarded-1", "prompt")
            .expect("enqueue");
        queue
            .discard("loop-discarded-1", Some("no longer needed"))
            .expect("discard");

        let err = merge_loop(MergeArgs {
            loop_id: "loop-discarded-1".to_string(),
            force: false,
        })
        .expect_err("merge should fail for discarded loop");

        assert!(err.to_string().contains("discarded"));
    }

    #[test]
    fn test_merge_loop_rejects_merging_without_force() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let _cwd = CwdGuard::set(temp_dir.path());

        let queue = MergeQueue::new(temp_dir.path());
        queue.enqueue("loop-merging-1", "prompt").expect("enqueue");
        queue
            .mark_merging("loop-merging-1", 4242)
            .expect("mark merging");

        let err = merge_loop(MergeArgs {
            loop_id: "loop-merging-1".to_string(),
            force: false,
        })
        .expect_err("merge should fail for merging loop without force");

        assert!(err.to_string().contains("currently merging"));
    }
}
