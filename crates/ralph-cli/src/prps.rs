//! CLI commands for the `ralph prps` namespace.
//!
//! Manage the native PRP (Pull Request Process) queue.
//!
//! Subcommands:
//! - `import`: Import PRPs from the configured import directory
//! - `list`: List all PRPs in the queue
//! - `show`: Show detailed information about a specific PRP
//! - `process`: Process the next queued PRP
//! - `resume`: Resume a specific PRP
//! - `retry`: Retry a PRP in needs_review state
//! - `discard`: Discard a PRP from the queue

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{Context, Result, bail};
use clap::{Parser, Subcommand};

use ralph_core::{
    prp_files::{definition_of_done_complete, discover_prps},
    prp_queue::{PrpQueue, PrpState, PrpPhase},
    worktree::{create_worktree_with_spec, worktree_exists, WorktreeConfig, WorktreeSpec, WorktreeError},
    PrpsConfig,
};

/// Manage the PRP queue.
#[derive(Parser, Debug)]
pub struct PrpsArgs {
    #[command(subcommand)]
    pub command: Option<PrpsCommands>,
}

/// PRP queue subcommands.
#[derive(Subcommand, Debug)]
pub enum PrpsCommands {
    /// Import new PRPs from the configured import directory.
    Import,

    /// List all PRPs in the queue (default).
    List(ListPrpsArgs),

    /// Show detailed information about a PRP.
    Show(ShowPrpsArgs),

    /// Process the next queued PRP (implementation + integration).
    Process,

    /// Resume a specific PRP.
    Resume(ResumePrpsArgs),

    /// Retry a PRP in needs_review state.
    Retry(RetryPrpsArgs),

    /// Discard a PRP from the queue.
    Discard(DiscardPrpsArgs),
}

#[derive(Parser, Debug)]
pub struct ListPrpsArgs {
    /// Output as JSON
    #[arg(long)]
    pub json: bool,
}

#[derive(Parser, Debug)]
pub struct ShowPrpsArgs {
    /// PRP ID (e.g., PRP-001)
    pub prp_id: String,
}

#[derive(Parser, Debug)]
pub struct ResumePrpsArgs {
    /// PRP ID (e.g., PRP-001)
    pub prp_id: String,
}

#[derive(Parser, Debug)]
pub struct RetryPrpsArgs {
    /// PRP ID (e.g., PRP-001)
    pub prp_id: String,
}

#[derive(Parser, Debug)]
pub struct DiscardPrpsArgs {
    /// PRP ID (e.g., PRP-001)
    pub prp_id: String,

    /// Reason for discarding
    #[arg(short, long)]
    pub reason: Option<String>,
}

/// Execute a prps command.
pub fn execute(args: PrpsArgs) -> Result<()> {
    let cwd = std::env::current_dir()?;

    match args.command {
        None | Some(PrpsCommands::List(_)) => {
            list_prps(&cwd, args.command.map(|c| match c {
                PrpsCommands::List(a) => a,
                _ => unreachable!(),
            }))
        }
        Some(PrpsCommands::Import) => import_prps(&cwd),
        Some(PrpsCommands::Show(show_args)) => show_prp(&cwd, &show_args.prp_id),
        Some(PrpsCommands::Process) => process_prps(&cwd),
        Some(PrpsCommands::Resume(resume_args)) => {
            resume_prp(&cwd, &resume_args.prp_id)
        }
        Some(PrpsCommands::Retry(retry_args)) => retry_prp(&cwd, &retry_args.prp_id),
        Some(PrpsCommands::Discard(discard_args)) => {
            discard_prp(&cwd, &discard_args.prp_id, discard_args.reason.as_deref())
        }
    }
}

/// Load PRP config from ralph.yml, with sensible defaults.
fn load_prps_config(cwd: &std::path::Path) -> Result<PrpsConfig> {
    use ralph_core::RalphConfig;

    // Try to load the config
    let config_path = cwd.join("ralph.yml");
    if config_path.exists() {
        let content = std::fs::read_to_string(&config_path)?;
        let config: RalphConfig = serde_yaml::from_str(&content)
            .context("Failed to parse ralph.yml")?;
        if config.prps.enabled {
            return Ok(config.prps);
        }
    }

    // Return default config if not configured
    Ok(PrpsConfig::default())
}

/// Import new PRPs from the configured import directory.
fn import_prps(cwd: &std::path::Path) -> Result<()> {
    let config = load_prps_config(cwd)?;
    let import_dir = resolve_path(cwd, &config.import_dir);

    if !import_dir.exists() {
        eprintln!(
            "Import directory does not exist: {}",
            import_dir.display()
        );
        return Ok(());
    }

    let queue = PrpQueue::new(cwd);
    let discovered = discover_prps(&import_dir)
        .context("Failed to discover PRPs in import directory")?;

    if discovered.is_empty() {
        println!("No PRPs found in {}", import_dir.display());
        return Ok(());
    }

    let mut imported_count = 0;
    let mut already_present_count = 0;

    for prp in &discovered {
        match queue.import(&prp.prp_id, &prp.title, &prp.path.to_string_lossy()) {
            Ok(_) => {
                // Check if it was actually imported or was already present
                if let Ok(Some(entry)) = queue.get_entry(&prp.prp_id) {
                    if entry.created_at == entry.updated_at {
                        imported_count += 1;
                        println!("Imported: {}", prp.prp_id);
                    } else {
                        already_present_count += 1;
                    }
                }
            }
            Err(e) => {
                eprintln!("Failed to import {}: {}", prp.prp_id, e);
            }
        }
    }

    println!(
        "Imported {} new PRP(s), {} already in queue",
        imported_count, already_present_count
    );

    Ok(())
}

/// Resolve a potentially relative path against the cwd.
fn resolve_path(cwd: &std::path::Path, path: &PathBuf) -> PathBuf {
    if path.is_absolute() {
        path.clone()
    } else {
        cwd.join(path)
    }
}

/// List all PRPs in the queue.
fn list_prps(cwd: &std::path::Path, args: Option<ListPrpsArgs>) -> Result<()> {
    let args = args.unwrap_or(ListPrpsArgs { json: false });
    let queue = PrpQueue::new(cwd);

    let entries = queue.list().unwrap_or_default();

    if args.json {
        // JSON output
        let json = serde_json::to_string_pretty(&entries)
            .context("Failed to serialize PRP list to JSON")?;
        println!("{}", json);
        return Ok(());
    }

    if entries.is_empty() {
        println!("No PRPs in queue");
        return Ok(());
    }

    // Human-readable table output
    println!("{:>5} {:<20} {:<20} {:<15} {:<25} {}",
        "ORDER", "PRP ID", "STATE", "PHASE", "BRANCH", "REASON");
    println!("{}", "-".repeat(100));

    for (i, entry) in entries.iter().enumerate() {
        let state_str = format!("{:?}", entry.state);
        let phase_str = entry
            .last_phase
            .map(|p| p.to_string())
            .unwrap_or_else(|| "-".to_string());
        let branch_str = entry
            .implementation_branch
            .as_deref()
            .unwrap_or("-");
        let reason_str = entry.failure_reason.as_deref().unwrap_or("-");

        println!("{:>5} {:<20} {:<20} {:<15} {:<25} {}",
            i + 1,
            entry.prp_id,
            state_str,
            phase_str,
            branch_str,
            reason_str);
    }

    Ok(())
}

/// Show detailed information about a specific PRP.
fn show_prp(cwd: &std::path::Path, prp_id: &str) -> Result<()> {
    let queue = PrpQueue::new(cwd);

    let entry = queue
        .get_entry(prp_id)
        .context("Failed to get PRP entry")?
        .ok_or_else(|| anyhow::anyhow!("PRP {} not found in queue", prp_id))?;

    println!("PRP: {}", entry.prp_id);
    println!("Title: {}", entry.title);
    println!("State: {:?}", entry.state);
    if let Some(phase) = entry.last_phase {
        println!("Last Phase: {}", phase);
    }
    println!("Queue Position: {}", entry.queue_position);
    println!();
    println!("Source: {}", entry.source_path);
    if let Some(ref archive) = entry.archive_path {
        println!("Archive: {}", archive);
    }
    println!();

    if let Some(ref branch) = entry.implementation_branch {
        println!("Implementation Branch: {}", branch);
    }
    if let Some(ref worktree) = entry.implementation_worktree {
        println!("Implementation Worktree: {}", worktree);
    }
    if let Some(pid) = entry.implementation_pid {
        println!("Implementation PID: {}", pid);
    }

    println!();
    println!("Created: {}", entry.created_at);
    println!("Updated: {}", entry.updated_at);

    if let Some(ref reason) = entry.failure_reason {
        println!();
        println!("Failure Reason: {}", reason);
    }

    Ok(())
}

/// Resume a specific PRP.
fn resume_prp(cwd: &std::path::Path, prp_id: &str) -> Result<()> {
    let queue = PrpQueue::new(cwd);

    let entry = queue
        .get_entry(prp_id)
        .context("Failed to get PRP entry")?
        .ok_or_else(|| anyhow::anyhow!("PRP {} not found in queue", prp_id))?;

    match entry.state {
        PrpState::Implementing | PrpState::Integrating => {
            println!(
                "PRP {} is already {} - use 'ralph prps process' to continue",
                prp_id,
                format!("{:?}", entry.state).to_lowercase()
            );
        }
        PrpState::NeedsReview => {
            println!(
                "PRP {} is in needs_review state - use 'ralph prps retry {}' to retry",
                prp_id, prp_id
            );
        }
        PrpState::Queued => {
            println!("PRP {} is queued - use 'ralph prps process' to start", prp_id);
        }
        PrpState::ReadyForIntegration => {
            println!(
                "PRP {} is ready for integration - use 'ralph prps process' to continue",
                prp_id
            );
        }
        PrpState::Integrated | PrpState::Discarded => {
            bail!(
                "PRP {} is already {:?} - cannot resume",
                prp_id,
                entry.state
            );
        }
    }

    Ok(())
}

/// Retry a PRP in needs_review state.
fn retry_prp(cwd: &std::path::Path, prp_id: &str) -> Result<()> {
    let queue = PrpQueue::new(cwd);

    let entry = queue
        .get_entry(prp_id)
        .context("Failed to get PRP entry")?
        .ok_or_else(|| anyhow::anyhow!("PRP {} not found in queue", prp_id))?;

    if entry.state != PrpState::NeedsReview {
        bail!(
            "PRP {} is in {:?} state, not needs_review - cannot retry",
            prp_id,
            entry.state
        );
    }

    queue
        .retry(prp_id)
        .context("Failed to retry PRP")?;

    println!("PRP {} has been retried", prp_id);
    Ok(())
}

/// Discard a PRP from the queue.
fn discard_prp(cwd: &std::path::Path, prp_id: &str, reason: Option<&str>) -> Result<()> {
    let queue = PrpQueue::new(cwd);

    let entry = queue
        .get_entry(prp_id)
        .context("Failed to get PRP entry")?
        .ok_or_else(|| anyhow::anyhow!("PRP {} not found in queue", prp_id))?;

    match entry.state {
        PrpState::Implementing | PrpState::Integrating => {
            bail!(
                "PRP {} is currently {} - cannot discard a running PRP",
                prp_id,
                format!("{:?}", entry.state).to_lowercase()
            );
        }
        _ => {}
    }

    queue
        .discard(prp_id, reason)
        .context("Failed to discard PRP")?;

    println!("PRP {} has been discarded", prp_id);
    Ok(())
}

// ─────────────────────────────────────────────────────────────────────────────
// PRP Processing Implementation
// ─────────────────────────────────────────────────────────────────────────────

/// Process PRPs in the queue, blocking until the head PRP is integrated.
fn process_prps(cwd: &Path) -> Result<()> {
    let config = load_prps_config(cwd)?;
    let queue = PrpQueue::new(cwd);

    // Check if queue is blocked by an earlier needs_review PRP
    if let Some(blocker) = queue.head_blocking_entry()? {
        if blocker.state == PrpState::NeedsReview {
            bail!(
                "Queue is blocked by {} which is in needs_review state. \
                 Use 'ralph prps retry {}' to retry or 'ralph prps discard {}' to discard.",
                blocker.prp_id,
                blocker.prp_id,
                blocker.prp_id
            );
        }
    }

    // Determine the next action based on queue head state
    let entries = queue.list()?;
    if entries.is_empty() {
        println!("No PRPs in queue. Run 'ralph prps import' first.");
        return Ok(());
    }

    // Find the first non-terminal entry
    let head = entries
        .iter()
        .find(|e| !e.state.is_terminal())
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("All PRPs are terminal"))?;

    println!("Processing PRP: {} ({:?})", head.prp_id, head.state);

    match head.state {
        PrpState::Queued => {
            // Start implementation phase
            run_implementation_phase(cwd, &config, &queue, &head)
        }
        PrpState::Implementing => {
            // Reconcile implementation - check if it's actually complete
            if check_implementation_ready(cwd, &config, &head)? {
                // Move to ready_for_integration
                let worktree_path = cwd.join(head.implementation_worktree.as_deref().unwrap_or(".worktrees/dummy"));
                let handoff_path = worktree_path.join(".ralph/agent/handoff.md");
                let events_path = worktree_path.join(".ralph/current-events");

                queue.mark_ready_for_integration(
                    &head.prp_id,
                    &handoff_path.to_string_lossy(),
                    &events_path.to_string_lossy(),
                )?;
                println!("PRP {} is ready for integration", head.prp_id);

                // Continue to integration phase
                run_integration_phase(cwd, &config, &queue, &head)
            } else {
                // Still implementing - try to resume
                reconcile_implementation(cwd, &config, &queue, &head)
            }
        }
        PrpState::ReadyForIntegration => {
            // Run integration phase
            run_integration_phase(cwd, &config, &queue, &head)
        }
        PrpState::Integrating => {
            // Reconcile integration
            reconcile_integration(cwd, &config, &queue, &head)
        }
        PrpState::NeedsReview => {
            bail!(
                "PRP {} is in needs_review state. Use 'ralph prps retry {}' to retry.",
                head.prp_id,
                head.prp_id
            );
        }
        PrpState::Integrated | PrpState::Discarded => {
            // This shouldn't happen since we filtered terminal states above
            bail!("PRP {} is already terminal ({:?})", head.prp_id, head.state);
        }
    }
}

/// Run the implementation phase for a PRP.
fn run_implementation_phase(
    cwd: &Path,
    config: &PrpsConfig,
    queue: &PrpQueue,
    entry: &ralph_core::prp_queue::PrpEntry,
) -> Result<()> {
    let prp_id = &entry.prp_id;

    // Derive deterministic worktree path and branch
    let branch_name = format!("{}{}", config.implementation_branch_prefix, prp_id);
    let worktree_rel_path = format!("prp-{}", prp_id);
    let worktree_path = cwd.join(&config.implementation_worktree_dir).join(&worktree_rel_path);
    let worktree_path_str = worktree_path.to_string_lossy().to_string();

    // Create or reuse the implementation worktree
    let worktree_config = WorktreeConfig::default();
    if !worktree_exists(cwd, &worktree_path_str, &worktree_config) {
        let spec = WorktreeSpec {
            worktree_path: worktree_path.clone(),
            branch_name: branch_name.clone(),
            base_ref: "HEAD".to_string(),
        };

        match create_worktree_with_spec(cwd, &spec) {
            Ok(_) => {
                println!("Created worktree at {}", worktree_path.display());
            }
            Err(WorktreeError::AlreadyExists(_)) => {
                println!("Reusing existing worktree at {}", worktree_path.display());
            }
            Err(e) => {
                bail!("Failed to create worktree: {}", e);
            }
        }

        // Copy the source PRP file to the worktree
        let source_path = cwd.join(&entry.source_path);
        let dest_prp_path = worktree_path.join(format!("{}.md", prp_id));
        if source_path.exists() && !dest_prp_path.exists() {
            fs::copy(&source_path, &dest_prp_path)?;
        }
    }

    // Mark as implementing
    let pid = std::process::id();
    queue.mark_implementing(prp_id, &branch_name, &worktree_path_str, pid, None)?;

    // Spawn ralph run in the implementation worktree
    println!("Starting implementation for {}...", prp_id);

    let ralph_cmd = std::env::current_exe()
        .context("Failed to get current executable")?;

    let impl_config_path = resolve_path(cwd, &config.implementation_config);

    // Build the prompt for implementation
    let prp_source = cwd.join(&entry.source_path);
    let prompt = format!(
        "Implement the PRP described in {}. Use the handoff file to capture completion criteria.",
        prp_source.display()
    );

    // Run ralph in the worktree
    let status = Command::new(&ralph_cmd)
        .current_dir(&worktree_path)
        .args(["run", "-c", impl_config_path.to_str().unwrap(), "-p", &prompt])
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()?;

    // Check if implementation is ready
    if check_implementation_ready(cwd, config, entry)? {
        // Mark as ready for integration
        let handoff_path = worktree_path.join(".ralph/agent/handoff.md");
        let events_path = worktree_path.join(".ralph/current-events");

        queue.mark_ready_for_integration(
            prp_id,
            &handoff_path.to_string_lossy(),
            &events_path.to_string_lossy(),
        )?;

        println!("PRP {} ready for integration", prp_id);

        // Continue to integration phase
        // We need to get the entry again since it was updated
        let updated_entry = queue.get_entry(prp_id)?
            .ok_or_else(|| anyhow::anyhow!("PRP {} not found", prp_id))?;
        run_integration_phase(cwd, config, queue, &updated_entry)
    } else if status.success() {
        // Process exited successfully but not ready - try corrective pass
        run_corrective_dod_pass(cwd, config, queue, entry)
    } else {
        // Process failed
        queue.mark_needs_review(prp_id, PrpPhase::Implementation, "Implementation process failed")?;
        bail!("Implementation failed for {}", prp_id);
    }
}

/// Check if implementation is ready for integration.
fn check_implementation_ready(
    cwd: &Path,
    _config: &PrpsConfig,
    entry: &ralph_core::prp_queue::PrpEntry,
) -> Result<bool> {
    let worktree_path = cwd.join(entry.implementation_worktree.as_deref().unwrap_or(".worktrees/dummy"));

    // Check for task.complete event in current-events
    let events_path = worktree_path.join(".ralph/current-events");
    let has_task_complete = if events_path.exists() {
        let content = fs::read_to_string(&events_path)?;
        content.contains("task.complete")
    } else {
        false
    };

    // Check for handoff.md
    let handoff_path = worktree_path.join(".ralph/agent/handoff.md");
    let has_handoff = handoff_path.exists();

    // Check DoD in the worktree copy of the PRP
    let worktree_prp_path = worktree_path.join(format!("{}.md", entry.prp_id));
    let dod_complete = if worktree_prp_path.exists() {
        definition_of_done_complete(&worktree_prp_path)?
    } else {
        false
    };

    Ok(has_task_complete && has_handoff && dod_complete)
}

/// Run a corrective pass to complete DoD checkboxes.
fn run_corrective_dod_pass(
    cwd: &Path,
    config: &PrpsConfig,
    queue: &PrpQueue,
    entry: &ralph_core::prp_queue::PrpEntry,
) -> Result<()> {
    let prp_id = &entry.prp_id;
    let worktree_path = cwd.join(entry.implementation_worktree.as_deref().unwrap_or(".worktrees/dummy"));

    println!("Running corrective DoD pass for {}...", prp_id);

    let ralph_cmd = std::env::current_exe()
        .context("Failed to get current executable")?;

    let impl_config_path = resolve_path(cwd, &config.implementation_config);

    let prompt = format!(
        "Review the PRP markdown and update the Definition of Done section to accurately reflect what was completed. \
         Only check off items that are genuinely done. Do not add new items.",
    );

    let _status = Command::new(&ralph_cmd)
        .current_dir(&worktree_path)
        .args(["run", "-c", impl_config_path.to_str().unwrap(), "-p", &prompt])
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()?;

    // Re-check readiness
    if check_implementation_ready(cwd, config, entry)? {
        let handoff_path = worktree_path.join(".ralph/agent/handoff.md");
        let events_path = worktree_path.join(".ralph/current-events");

        queue.mark_ready_for_integration(
            prp_id,
            &handoff_path.to_string_lossy(),
            &events_path.to_string_lossy(),
        )?;

        // Continue to integration
        let updated_entry = queue.get_entry(prp_id)?
            .ok_or_else(|| anyhow::anyhow!("PRP {} not found", prp_id))?;
        run_integration_phase(cwd, config, queue, &updated_entry)
    } else {
        queue.mark_needs_review(prp_id, PrpPhase::Implementation, "DoD incomplete after corrective pass")?;
        bail!("PRP {} still not ready after corrective pass", prp_id);
    }
}

/// Reconcile an implementation that was interrupted.
fn reconcile_implementation(
    cwd: &Path,
    config: &PrpsConfig,
    queue: &PrpQueue,
    entry: &ralph_core::prp_queue::PrpEntry,
) -> Result<()> {
    let prp_id = &entry.prp_id;
    println!("Reconciling implementation for {}...", prp_id);

    // Check if the process is still running
    if let Some(pid) = entry.implementation_pid {
        // Try to check if process is alive
        // On Unix, signal 0 doesn't actually send a signal but checks if process exists
        #[cfg(unix)]
        {
            match Command::new("kill")
                .arg("-0")
                .arg(pid.to_string())
                .output()
            {
                Ok(output) if output.status.success() => {
                    // Process is alive
                    println!("Implementation process {} is still running", pid);
                    return Ok(());
                }
                _ => {
                    // Process is dead, check readiness
                }
            }
        }
    }

    // Process is dead or pid not recorded - check readiness
    if check_implementation_ready(cwd, config, entry)? {
        let worktree_path = cwd.join(entry.implementation_worktree.as_deref().unwrap_or(".worktrees/dummy"));
        let handoff_path = worktree_path.join(".ralph/agent/handoff.md");
        let events_path = worktree_path.join(".ralph/current-events");

        queue.mark_ready_for_integration(
            prp_id,
            &handoff_path.to_string_lossy(),
            &events_path.to_string_lossy(),
        )?;

        let updated_entry = queue.get_entry(prp_id)?
            .ok_or_else(|| anyhow::anyhow!("PRP {} not found", prp_id))?;
        run_integration_phase(cwd, config, queue, &updated_entry)
    } else {
        // Needs intervention
        println!("Implementation for {} is incomplete and needs review", prp_id);
        // Try to restart implementation
        run_implementation_phase(cwd, config, queue, entry)
    }
}

/// Run the integration phase for a PRP.
fn run_integration_phase(
    cwd: &Path,
    config: &PrpsConfig,
    queue: &PrpQueue,
    entry: &ralph_core::prp_queue::PrpEntry,
) -> Result<()> {
    let prp_id = &entry.prp_id;
    println!("Starting integration for {}...", prp_id);

    // Create or reuse the integration worktree
    let integration_worktree_path = cwd.join(&config.integration_worktree);
    let integration_branch = &config.integration_branch;

    // Check if integration worktree exists
    let worktree_config = WorktreeConfig::default();
    if !worktree_exists(cwd, &config.integration_worktree.to_string_lossy(), &worktree_config) {
        // Create integration worktree
        let spec = WorktreeSpec {
            worktree_path: integration_worktree_path.clone(),
            branch_name: integration_branch.clone(),
            base_ref: "HEAD".to_string(),
        };

        match create_worktree_with_spec(cwd, &spec) {
            Ok(_) => {
                println!("Created integration worktree at {}", integration_worktree_path.display());
            }
            Err(WorktreeError::AlreadyExists(_)) => {
                println!("Reusing existing integration worktree at {}", integration_worktree_path.display());
            }
            Err(e) => {
                bail!("Failed to create integration worktree: {}", e);
            }
        }
    }

    // Mark as integrating
    let pid = std::process::id();
    queue.mark_integrating(
        prp_id,
        integration_branch,
        &config.integration_worktree.to_string_lossy(),
        pid,
        None,
    )?;

    let ralph_cmd = std::env::current_exe()
        .context("Failed to get current executable")?;

    let integration_config_path = resolve_path(cwd, &config.integration_config);

    // Build integration prompt
    let impl_branch = entry.implementation_branch.as_deref().unwrap_or("main");
    let impl_worktree = entry.implementation_worktree.as_deref().unwrap_or(".");

    let prompt = format!(
        "Integrate the changes from branch '{}' (worktree: {}) into the '{}' branch. \
         Archive the PRP markdown to {}. \
         Stop once this PRP is integrated. Do not process other PRPs.",
        impl_branch,
        impl_worktree,
        integration_branch,
        config.completed_dir.display()
    );

    // Run ralph in the integration worktree
    let status = Command::new(&ralph_cmd)
        .current_dir(&integration_worktree_path)
        .args(["run", "-c", integration_config_path.to_str().unwrap(), "-p", &prompt])
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()?;

    if status.success() && check_integration_success(cwd, config, entry)? {
        // Get the integration commit
        let commit = get_current_commit(&integration_worktree_path)?;

        // Archive the PRP markdown
        let source_path = cwd.join(&entry.source_path);
        let archive_path = archive_prp_to_completed(cwd, config, &source_path)?;

        queue.mark_integrated(prp_id, &commit, &archive_path.to_string_lossy())?;

        println!("PRP {} successfully integrated (commit: {})", prp_id, commit);
        Ok(())
    } else {
        queue.mark_needs_review(prp_id, PrpPhase::Integration, "Integration process failed or validation didn't pass")?;
        bail!("Integration failed for {}", prp_id);
    }
}

/// Check if integration was successful.
fn check_integration_success(
    cwd: &Path,
    config: &PrpsConfig,
    entry: &ralph_core::prp_queue::PrpEntry,
) -> Result<bool> {
    let integration_worktree_path = cwd.join(&config.integration_worktree);

    // Check that integration worktree is clean
    let status_output = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(&integration_worktree_path)
        .output()?;

    let is_clean = status_output.stdout.iter().all(|&b| b == b'\n' || b == b'\r' || b == b' ');

    // Check that archive exists
    let archive_path = cwd.join(&config.completed_dir).join(format!("{}.md", entry.prp_id));
    let archive_exists = archive_path.exists();

    // Check that there's a new commit
    let commit = get_current_commit(&integration_worktree_path)?;
    let has_commit = !commit.is_empty();

    Ok(is_clean && archive_exists && has_commit)
}

/// Get the current git commit SHA for a worktree.
fn get_current_commit(worktree_path: &Path) -> Result<String> {
    let output = Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(worktree_path)
        .output()?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        Ok(String::new())
    }
}

/// Archive a PRP to the completed directory.
fn archive_prp_to_completed(cwd: &Path, config: &PrpsConfig, source_path: &Path) -> Result<PathBuf> {
    use ralph_core::prp_files::archive_prp;

    let completed_dir = cwd.join(&config.completed_dir);
    let path = archive_prp(source_path, &completed_dir)?;
    Ok(path)
}

/// Reconcile an integration that was interrupted.
fn reconcile_integration(
    cwd: &Path,
    config: &PrpsConfig,
    queue: &PrpQueue,
    entry: &ralph_core::prp_queue::PrpEntry,
) -> Result<()> {
    let prp_id = &entry.prp_id;
    println!("Reconciling integration for {}...", prp_id);

    // Check if integration was actually successful
    if check_integration_success(cwd, config, entry)? {
        let commit = get_current_commit(&cwd.join(&config.integration_worktree))?;
        let archive_path = cwd.join(&config.completed_dir).join(format!("{}.md", prp_id));

        queue.mark_integrated(prp_id, &commit, &archive_path.to_string_lossy())?;
        println!("PRP {} integration confirmed", prp_id);
        Ok(())
    } else {
        // Check if process is still running
        if let Some(pid) = entry.integration_pid {
            #[cfg(unix)]
            {
                match Command::new("kill")
                    .arg("-0")
                    .arg(pid.to_string())
                    .output()
                {
                    Ok(output) if output.status.success() => {
                        println!("Integration process {} is still running", pid);
                        return Ok(());
                    }
                    _ => {}
                }
            }
        }

        // Process is dead - retry integration
        let updated_entry = queue.get_entry(prp_id)?
            .ok_or_else(|| anyhow::anyhow!("PRP {} not found", prp_id))?;
        run_integration_phase(cwd, config, queue, &updated_entry)
    }
}
