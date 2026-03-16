//! CLI commands for the `ralph worker` namespace.
//!
//! Subcommands:
//! - `list`: List all registered workers (default)
//! - `show <id>`: Show worker details
//! - `deregister <id>`: Remove a stale worker entry
//! - `reclaim`: Manually trigger reclaim_expired
//! - `summary`: Board summary (workers + task counts)

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use ralph_api::task_domain::{TaskDomain, TaskListParams};
use ralph_api::worker_domain::{WorkerDomain, WorkerReclaimExpiredInput, WorkerStatus};
use std::path::PathBuf;

/// Manage factory workers.
#[derive(Parser, Debug)]
pub struct WorkerArgs {
    #[command(subcommand)]
    pub command: Option<WorkerCommands>,

    /// Working directory (default: current directory)
    #[arg(long, global = true)]
    pub root: Option<PathBuf>,
}

#[derive(Subcommand, Debug)]
pub enum WorkerCommands {
    /// List all registered workers (default)
    List,

    /// Show worker details
    Show(ShowArgs),

    /// Remove a stale worker entry
    Deregister(DeregisterArgs),

    /// Manually trigger reclaim of expired worker leases
    Reclaim,

    /// Show board summary (workers + task counts)
    Summary,
}

#[derive(Parser, Debug)]
pub struct ShowArgs {
    /// Worker ID to show
    pub id: String,
}

#[derive(Parser, Debug)]
pub struct DeregisterArgs {
    /// Worker ID to remove
    pub id: String,
}

pub fn execute(args: WorkerArgs) -> Result<()> {
    let root = args
        .root
        .or_else(|| std::env::current_dir().ok())
        .context("Failed to determine workspace root")?;

    match args.command {
        None | Some(WorkerCommands::List) => list_workers(&root),
        Some(WorkerCommands::Show(show_args)) => show_worker(&root, &show_args.id),
        Some(WorkerCommands::Deregister(dereg_args)) => deregister_worker(&root, &dereg_args.id),
        Some(WorkerCommands::Reclaim) => reclaim_expired(&root),
        Some(WorkerCommands::Summary) => show_summary(&root),
    }
}

fn list_workers(root: &PathBuf) -> Result<()> {
    let domain = WorkerDomain::new(root).map_err(|e| anyhow::anyhow!("{}", e.message))?;
    let workers = domain
        .list()
        .map_err(|e| anyhow::anyhow!("{}", e.message))?;

    if workers.is_empty() {
        println!("No workers registered.");
        return Ok(());
    }

    println!(
        "{:<24} {:<20} {:<8} {:<8} {:<20}",
        "WORKER ID", "NAME", "STATUS", "BACKEND", "LAST HEARTBEAT"
    );
    for w in &workers {
        let status = match w.status {
            WorkerStatus::Idle => "idle",
            WorkerStatus::Busy => "busy",
            WorkerStatus::Blocked => "blocked",
            WorkerStatus::Dead => "dead",
        };
        let heartbeat = w
            .last_heartbeat_at
            .get(..19)
            .unwrap_or(&w.last_heartbeat_at);
        println!(
            "{:<24} {:<20} {:<8} {:<8} {:<20}",
            w.worker_id, w.worker_name, status, w.backend, heartbeat
        );
    }
    println!("\n{} worker(s) registered.", workers.len());
    Ok(())
}

fn show_worker(root: &PathBuf, id: &str) -> Result<()> {
    let domain = WorkerDomain::new(root).map_err(|e| anyhow::anyhow!("{}", e.message))?;
    let w = domain
        .get(id)
        .map_err(|e| anyhow::anyhow!("{}", e.message))?;

    println!("Worker ID:       {}", w.worker_id);
    println!("Name:            {}", w.worker_name);
    println!("Loop ID:         {}", w.loop_id);
    println!("Backend:         {}", w.backend);
    println!("Workspace:       {}", w.workspace_root);
    println!(
        "Status:          {}",
        match w.status {
            WorkerStatus::Idle => "idle",
            WorkerStatus::Busy => "busy",
            WorkerStatus::Blocked => "blocked",
            WorkerStatus::Dead => "dead",
        }
    );
    println!(
        "Current Task:    {}",
        w.current_task_id.as_deref().unwrap_or("-")
    );
    println!(
        "Current Hat:     {}",
        w.current_hat.as_deref().unwrap_or("-")
    );
    println!("Last Heartbeat:  {}", w.last_heartbeat_at);

    // Show tasks this worker has interacted with (via events)
    let task_domain = TaskDomain::new(root);
    let all_tasks = task_domain.list(TaskListParams {
        status: None,
        include_archived: Some(true),
    });
    let worker_tasks: Vec<_> = all_tasks
        .iter()
        .filter(|t| t.events.iter().any(|e| e.worker_id.as_deref() == Some(id)))
        .collect();

    if !worker_tasks.is_empty() {
        println!("\nTask History:");
        for task in &worker_tasks {
            println!("  {} [{}] {}", task.id, task.status, task.title);
            let worker_events: Vec<_> = task
                .events
                .iter()
                .filter(|e| e.worker_id.as_deref() == Some(id))
                .collect();
            for event in &worker_events {
                let ts = event.timestamp.get(..19).unwrap_or(&event.timestamp);
                let details = event.details.as_deref().unwrap_or("");
                println!("    {} {} {}", ts, event.event_type, details);
            }
        }
    }

    Ok(())
}

fn deregister_worker(root: &PathBuf, id: &str) -> Result<()> {
    let mut domain = WorkerDomain::new(root).map_err(|e| anyhow::anyhow!("{}", e.message))?;
    domain
        .deregister(id)
        .map_err(|e| anyhow::anyhow!("{}", e.message))?;
    println!("Deregistered worker: {}", id);
    Ok(())
}

fn reclaim_expired(root: &PathBuf) -> Result<()> {
    let mut domain = WorkerDomain::new(root).map_err(|e| anyhow::anyhow!("{}", e.message))?;
    let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    let result = domain
        .reclaim_expired(WorkerReclaimExpiredInput { as_of: now })
        .map_err(|e| anyhow::anyhow!("{}", e.message))?;

    if result.tasks.is_empty() && result.workers.is_empty() {
        println!("No expired leases to reclaim.");
    } else {
        println!(
            "Reclaimed {} task(s) and {} worker(s).",
            result.tasks.len(),
            result.workers.len()
        );
        for task in &result.tasks {
            println!("  Task: {} — {}", task.id, task.title);
        }
        for worker in &result.workers {
            println!("  Worker: {} ({})", worker.worker_id, worker.worker_name);
        }
    }
    Ok(())
}

fn show_summary(root: &PathBuf) -> Result<()> {
    let domain = WorkerDomain::new(root).map_err(|e| anyhow::anyhow!("{}", e.message))?;
    let workers = domain
        .list()
        .map_err(|e| anyhow::anyhow!("{}", e.message))?;

    let idle = workers
        .iter()
        .filter(|w| w.status == WorkerStatus::Idle)
        .count();
    let busy = workers
        .iter()
        .filter(|w| w.status == WorkerStatus::Busy)
        .count();
    let blocked = workers
        .iter()
        .filter(|w| w.status == WorkerStatus::Blocked)
        .count();
    let dead = workers
        .iter()
        .filter(|w| w.status == WorkerStatus::Dead)
        .count();

    println!("Worker Summary");
    println!("  Total:   {}", workers.len());
    println!("  Idle:    {}", idle);
    println!("  Busy:    {}", busy);
    println!("  Blocked: {}", blocked);
    println!("  Dead:    {}", dead);
    Ok(())
}
