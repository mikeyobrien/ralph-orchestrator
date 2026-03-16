//! Software factory: spawns N workers that claim tasks and run Ralph loops.
//!
//! Each worker:
//! 1. Registers with the worker domain
//! 2. Claims the next ready task (polls with backoff)
//! 3. Runs `run_loop_impl` with the task title as the prompt
//! 4. Completes the task (success/failure)
//! 5. Loops back to step 2

use anyhow::Result;
use ralph_adapters::{StreamLine, StreamLineSource};
use ralph_api::task_domain::{TaskCreateParams, TaskDomain};
use ralph_api::worker_domain::{WorkerDomain, WorkerHeartbeatInput, WorkerRecord, WorkerStatus};
use ralph_core::{
    FileLock, LoopContext, LoopNameGenerator, RalphConfig, TerminationReason, WorktreeConfig,
    create_worktree, remove_worktree,
};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Instant;
use tokio::sync::watch;
use tracing::{info, warn};

use crate::loop_runner::run_loop_impl;
use crate::{ColorMode, Verbosity};

/// Base sleep duration between idle polls (grows with exponential backoff).
const IDLE_POLL_BASE: std::time::Duration = std::time::Duration::from_secs(2);
/// Maximum sleep duration between idle polls.
const IDLE_POLL_MAX: std::time::Duration = std::time::Duration::from_secs(30);

/// RAII guard that deregisters a worker on drop.
struct WorkerGuard {
    workspace: PathBuf,
    worker_id: String,
}

impl Drop for WorkerGuard {
    fn drop(&mut self) {
        match WorkerDomain::new(&self.workspace) {
            Ok(mut domain) => {
                if let Err(e) = domain.deregister(&self.worker_id) {
                    warn!(worker_id = %self.worker_id, error = %e.message, "Failed to deregister worker on shutdown");
                } else {
                    info!(worker_id = %self.worker_id, "Deregistered factory worker");
                }
            }
            Err(e) => {
                warn!(worker_id = %self.worker_id, error = %e.message, "Failed to open worker domain for deregistration");
            }
        }
    }
}

/// Per-worker result after it finishes all its work.
struct WorkerResult {
    worker_id: String,
    tasks_completed: u32,
    tasks_failed: u32,
}

fn jittered_duration(base: std::time::Duration) -> std::time::Duration {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_nanos();
    let jitter_fraction = f64::from(nanos % 250) / 1000.0;
    base + base.mul_f64(jitter_fraction)
}

fn send_idle_heartbeat(workspace: &std::path::Path, worker_id: &str) {
    if let Ok(mut domain) = WorkerDomain::new(workspace) {
        let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        let input = WorkerHeartbeatInput {
            worker_id: worker_id.to_string(),
            status: WorkerStatus::Idle,
            current_task_id: None,
            current_hat: None,
            last_heartbeat_at: now,
            iteration: None,
            max_iterations: None,
        };
        if let Err(e) = domain.heartbeat(input) {
            warn!(worker_id = %worker_id, error = %e.message, "Failed to send idle heartbeat");
        }
    }
}

fn send_busy_heartbeat(
    workspace: &std::path::Path,
    worker_id: &str,
    task_id: &str,
    iteration: Option<u32>,
    max_iterations: Option<u32>,
) {
    if let Ok(mut domain) = WorkerDomain::new(workspace) {
        let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        let input = WorkerHeartbeatInput {
            worker_id: worker_id.to_string(),
            status: WorkerStatus::Busy,
            current_task_id: Some(task_id.to_string()),
            current_hat: None,
            last_heartbeat_at: now,
            iteration,
            max_iterations,
        };
        if let Err(e) = domain.heartbeat(input) {
            warn!(worker_id = %worker_id, error = %e.message, "Failed to send busy heartbeat");
        }
    }
}

/// Spawns N workers that run in parallel on the shared workspace.
pub async fn run_factory(
    config: RalphConfig,
    num_workers: u32,
    color_mode: ColorMode,
    verbosity: Verbosity,
    api_url: Option<String>,
) -> Result<()> {
    let start = Instant::now();
    let (cancel_tx, _cancel_rx) = watch::channel(false);
    let workspace = config.core.workspace_root.clone();

    // Install Ctrl+C handler
    let cancel_tx_clone = cancel_tx.clone();
    tokio::spawn(async move {
        let _ = tokio::signal::ctrl_c().await;
        info!("Ctrl+C received, shutting down factory workers...");
        let _ = cancel_tx_clone.send(true);
    });

    if let Some(url) = &api_url {
        eprintln!("Log streaming enabled → {url}/rpc/v1");
    }

    let name_generator = LoopNameGenerator::from_config(&config.features.loop_naming);

    let mut handles = Vec::new();
    for i in 0..num_workers {
        let worker_loop_id = name_generator.generate_memorable_unique(|_| false);
        let worker_config = config.clone();
        let worker_cancel = cancel_tx.subscribe();
        let worker_workspace = workspace.clone();
        let worker_api_url = api_url.clone();

        let handle = tokio::spawn(async move {
            Box::pin(run_worker_loop(
                worker_config,
                worker_loop_id,
                i,
                color_mode,
                verbosity,
                worker_cancel,
                worker_workspace,
                worker_api_url,
            ))
            .await
        });
        handles.push(handle);
    }

    // Wait for all workers
    let mut total_completed = 0u32;
    let mut total_failed = 0u32;
    let mut worker_summaries = Vec::new();

    for handle in handles {
        match handle.await {
            Ok(Ok(result)) => {
                total_completed += result.tasks_completed;
                total_failed += result.tasks_failed;
                worker_summaries.push(result);
            }
            Ok(Err(e)) => {
                warn!(error = %e, "Worker exited with error");
            }
            Err(e) => {
                warn!(error = %e, "Worker task panicked");
            }
        }
    }

    let elapsed = start.elapsed();
    println!("\nFactory shutdown ({:.1}s)", elapsed.as_secs_f64());
    println!(
        "  Tasks completed: {}  failed: {}",
        total_completed, total_failed
    );
    for summary in &worker_summaries {
        println!(
            "  {} — completed: {} failed: {}",
            summary.worker_id, summary.tasks_completed, summary.tasks_failed
        );
    }

    Ok(())
}

/// Spawns a tokio task that reads `StreamLine` from the receiver and publishes
/// them as `_internal.publish` RPCs to the API server. Returns a join handle
/// and an abort handle for cleanup.
fn spawn_log_publisher(
    rx: std::sync::mpsc::Receiver<StreamLine>,
    api_url: String,
    task_id: String,
) -> tokio::task::JoinHandle<()> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .unwrap_or_default();

    let rpc_url = format!("{api_url}/rpc/v1");

    tokio::spawn(async move {
        // Read from the sync channel using spawn_blocking to avoid blocking
        // the tokio runtime.
        let (line_tx, mut line_rx) = tokio::sync::mpsc::channel::<StreamLine>(256);
        let reader_handle = tokio::task::spawn_blocking(move || {
            while let Ok(line) = rx.recv() {
                if line_tx.blocking_send(line).is_err() {
                    break;
                }
            }
        });

        let mut batch: Vec<(String, String)> = Vec::new();
        let flush_interval = tokio::time::Duration::from_millis(100);

        loop {
            // Collect lines with a timeout for batching
            let deadline = tokio::time::sleep(flush_interval);
            tokio::pin!(deadline);

            loop {
                tokio::select! {
                    line = line_rx.recv() => {
                        match line {
                            Some(stream_line) => {
                                let source = match stream_line.source {
                                    StreamLineSource::Stdout => "stdout",
                                    StreamLineSource::Stderr => "stderr",
                                };
                                batch.push((stream_line.text, source.to_string()));
                                if batch.len() >= 50 {
                                    break;
                                }
                            }
                            None => {
                                // Channel closed — flush remaining and exit
                                flush_batch(&client, &rpc_url, &task_id, &mut batch).await;
                                reader_handle.abort();
                                return;
                            }
                        }
                    }
                    () = &mut deadline => {
                        break;
                    }
                }
            }

            if !batch.is_empty() {
                flush_batch(&client, &rpc_url, &task_id, &mut batch).await;
            }
        }
    })
}

async fn flush_batch(
    client: &reqwest::Client,
    rpc_url: &str,
    task_id: &str,
    batch: &mut Vec<(String, String)>,
) {
    for (line, source) in batch.drain(..) {
        let ts = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
        let body = serde_json::json!({
            "apiVersion": "v1",
            "id": format!("log-{}", uuid_v4_fast()),
            "method": "_internal.publish",
            "params": {
                "topic": "task.log.line",
                "resourceType": "task",
                "resourceId": task_id,
                "payload": {
                    "line": line,
                    "source": source,
                    "timestamp": ts,
                }
            },
            "meta": {
                "idempotencyKey": format!("log-{}", uuid_v4_fast()),
            }
        });

        if let Err(e) = client.post(rpc_url).json(&body).send().await {
            warn!(error = %e, "Failed to publish log line to API");
        }
    }
}

/// Fast pseudo-UUID v4 using timestamp + random bits.
fn uuid_v4_fast() -> String {
    use std::time::SystemTime;
    let ts = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let rand_bits: u64 = {
        // Simple cheap randomness from memory address + time
        let stack_var = 0u8;
        let addr = &raw const stack_var as u64;
        addr.wrapping_mul(ts as u64).wrapping_add(ts as u64)
    };
    format!("{:016x}-{:016x}", ts as u64, rand_bits)
}

/// Gets the current branch name for a git repository.
fn get_current_branch(repo_root: &Path) -> Result<String, String> {
    let output = Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(repo_root)
        .output()
        .map_err(|e| format!("Failed to run git: {e}"))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("git rev-parse failed: {stderr}"));
    }

    Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

/// Rebases the worktree branch onto the current branch, then fast-forward merges.
/// Returns Ok(()) on success, Err(reason) on conflict.
fn rebase_and_integrate(
    repo_root: &Path,
    worktree_path: &Path,
    branch: &str,
    current_branch: &str,
) -> Result<(), String> {
    // 1. Rebase worktree branch onto current branch
    let rebase_output = Command::new("git")
        .args(["rebase", current_branch])
        .current_dir(worktree_path)
        .output()
        .map_err(|e| format!("Failed to run git rebase: {e}"))?;

    if !rebase_output.status.success() {
        // Abort the failed rebase
        let _ = Command::new("git")
            .args(["rebase", "--abort"])
            .current_dir(worktree_path)
            .output();
        let stderr = String::from_utf8_lossy(&rebase_output.stderr);
        return Err(format!("Rebase conflict: {stderr}"));
    }

    // 2. Fast-forward merge into parent branch from repo root
    let merge_output = Command::new("git")
        .args(["merge", "--ff-only", branch])
        .current_dir(repo_root)
        .output()
        .map_err(|e| format!("Failed to run git merge: {e}"))?;

    if !merge_output.status.success() {
        let stderr = String::from_utf8_lossy(&merge_output.stderr);
        return Err(format!("Fast-forward merge failed: {stderr}"));
    }

    Ok(())
}

/// Acquires a file-based merge lock, runs the closure, and releases the lock.
fn with_merge_lock<F, T>(workspace: &Path, f: F) -> Result<T, anyhow::Error>
where
    F: FnOnce() -> Result<T, anyhow::Error>,
{
    let lock = FileLock::new(workspace.join(".ralph").join("merge"))?;
    let _guard = lock.exclusive()?;
    f()
}

/// Per-worker loop: register, claim tasks, run loops, complete tasks.
async fn run_worker_loop(
    config: RalphConfig,
    loop_id: String,
    worker_index: u32,
    color_mode: ColorMode,
    verbosity: Verbosity,
    cancel: watch::Receiver<bool>,
    workspace: PathBuf,
    api_url: Option<String>,
) -> Result<WorkerResult> {
    let worker_id = format!("worker-{}", loop_id);
    let worker_name = config
        .worker
        .worker_name
        .clone()
        .map(|n| format!("{}-{}", n, worker_index))
        .unwrap_or_else(|| loop_id.clone());

    // Register worker
    let mut domain = WorkerDomain::new(&workspace)
        .map_err(|e| anyhow::anyhow!("Failed to init worker domain: {}", e.message))?;

    let now = chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true);
    let record = WorkerRecord {
        worker_id: worker_id.clone(),
        worker_name,
        loop_id: loop_id.clone(),
        backend: config.cli.backend.clone(),
        workspace_root: workspace.to_string_lossy().to_string(),
        current_task_id: None,
        current_hat: None,
        status: WorkerStatus::Idle,
        last_heartbeat_at: now.clone(),
        iteration: None,
        max_iterations: None,
        registered_at: Some(now),
    };
    domain
        .register(record)
        .map_err(|e| anyhow::anyhow!("Failed to register worker: {}", e.message))?;
    info!(worker_id = %worker_id, "Registered factory worker");

    // RAII deregistration on drop
    let _guard = WorkerGuard {
        workspace: workspace.clone(),
        worker_id: worker_id.clone(),
    };

    let mut tasks_completed = 0u32;
    let mut tasks_failed = 0u32;
    let mut idle_backoff = IDLE_POLL_BASE;

    loop {
        if *cancel.borrow() {
            info!(worker_id = %worker_id, "Cancelled, exiting worker loop");
            break;
        }

        // Re-open domain for each claim attempt (fresh file lock)
        let mut domain = match WorkerDomain::new(&workspace) {
            Ok(d) => d,
            Err(e) => {
                warn!(worker_id = %worker_id, error = %e.message, "Failed to open worker domain");
                break;
            }
        };

        // Claim next ready task
        let claim_result = match domain.claim_next(&worker_id) {
            Ok(result) => result,
            Err(e) => {
                warn!(worker_id = %worker_id, error = %e.message, "claim_next failed");
                send_idle_heartbeat(&workspace, &worker_id);
                tokio::time::sleep(jittered_duration(idle_backoff)).await;
                idle_backoff = (idle_backoff * 2).min(IDLE_POLL_MAX);
                continue;
            }
        };

        let task = match claim_result.task {
            Some(task) => {
                idle_backoff = IDLE_POLL_BASE;
                task
            }
            None => {
                send_idle_heartbeat(&workspace, &worker_id);
                tokio::time::sleep(jittered_duration(idle_backoff)).await;
                idle_backoff = (idle_backoff * 2).min(IDLE_POLL_MAX);
                continue;
            }
        };

        let task_id = task.id.clone();
        let task_title = task.title.clone();
        info!(worker_id = %worker_id, task_id = %task_id, title = %task_title, "Claimed task");

        // Create an isolated worktree for this task
        let worktree_id = task_id.clone();
        let worktree_result = create_worktree(&workspace, &worktree_id, &WorktreeConfig::default());
        let worktree = match worktree_result {
            Ok(wt) => wt,
            Err(e) => {
                warn!(worker_id = %worker_id, task_id = %task_id, error = %e, "Failed to create worktree");
                // Cancel the task (in_progress → cancelled is valid) instead of
                // using complete_task which requeues to "ready" and causes an
                // infinite claim loop when worktree creation persistently fails.
                let err_msg = format!("Worktree creation failed: {e}");
                let mut tasks = TaskDomain::new(&workspace);
                let _ = tasks.update(ralph_api::task_domain::TaskUpdateInput {
                    id: task_id.clone(),
                    status: Some("cancelled".to_string()),
                    title: None,
                    priority: None,
                    blocked_by: None,
                    assignee_worker_id: Some(None),
                    claimed_at: Some(None),
                    lease_expires_at: Some(None),
                });
                warn!(worker_id = %worker_id, task_id = %task_id, reason = %err_msg, "Task cancelled");
                // Clear worker's current_task_id
                send_idle_heartbeat(&workspace, &worker_id);
                tasks_failed += 1;
                continue;
            }
        };
        let worktree_path = worktree.path.clone();
        let worktree_branch = worktree.branch.clone();
        info!(worker_id = %worker_id, task_id = %task_id, worktree = %worktree_path.display(), "Created worktree");

        // Set up worktree context with symlinks
        let ctx = LoopContext::worktree(loop_id.clone(), worktree_path.clone(), workspace.clone());
        if let Err(e) = ctx.setup_worktree_symlinks() {
            warn!(worker_id = %worker_id, error = %e, "Failed to setup worktree symlinks");
        }
        if let Err(e) = ctx.generate_context_file(&worktree_branch, &task_title) {
            warn!(worker_id = %worker_id, error = %e, "Failed to generate context file");
        }

        // Configure the loop: set the task title as the prompt
        let mut task_config = config.clone();
        task_config.event_loop.prompt = Some(task_title.clone());
        task_config.event_loop.prompt_file = String::new();
        // Workers always run headless
        task_config.worker.enabled = true;
        // Run the loop in the worktree
        task_config.core.workspace_root = worktree_path.clone();

        // Set up log streaming channel if API URL is configured
        let (output_tx, publisher_handle) = if let Some(url) = &api_url {
            let (tx, rx) = std::sync::mpsc::channel::<StreamLine>();
            let handle = spawn_log_publisher(rx, url.clone(), task_id.clone());
            (Some(tx), Some(handle))
        } else {
            (None, None)
        };

        // Iteration counter shared between the heartbeat task and the loop runner
        let iteration_counter = Arc::new(AtomicU32::new(0));
        let max_iterations = task_config.event_loop.max_iterations;

        // Spawn background heartbeat task to keep the worker alive during execution
        let hb_interval = std::time::Duration::from_secs(config.worker.heartbeat_interval_seconds);
        let (hb_stop_tx, mut hb_stop_rx) = watch::channel(false);
        let hb_workspace = workspace.clone();
        let hb_worker_id = worker_id.clone();
        let hb_task_id = task_id.clone();
        let hb_counter = iteration_counter.clone();
        let heartbeat_handle = tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = tokio::time::sleep(hb_interval) => {
                        let iter = hb_counter.load(Ordering::Relaxed);
                        let iter_opt = if iter > 0 { Some(iter) } else { None };
                        send_busy_heartbeat(&hb_workspace, &hb_worker_id, &hb_task_id, iter_opt, Some(max_iterations));
                    }
                    _ = hb_stop_rx.changed() => break,
                }
            }
        });

        // Run the full Ralph loop for this single task
        let termination = run_loop_impl(
            task_config,
            color_mode,
            false, // not resume
            false, // no tui
            false, // no rpc
            verbosity,
            None, // no session recording
            Some(ctx),
            Vec::new(),  // no custom args
            Some(false), // no auto-merge for workers
            None,        // no resume loop id
            output_tx,
            Some(iteration_counter),
        )
        .await;

        // Stop the background heartbeat task
        let _ = hb_stop_tx.send(true);
        let _ = heartbeat_handle.await;

        // Drop the sender to signal the publisher to flush and exit
        // (output_tx was moved into run_loop_impl, so it's already dropped)
        if let Some(handle) = publisher_handle {
            // Give the publisher a moment to flush remaining lines
            let _ = tokio::time::timeout(std::time::Duration::from_secs(2), handle).await;
        }

        // Complete the task based on loop result
        let mut domain = match WorkerDomain::new(&workspace) {
            Ok(d) => d,
            Err(e) => {
                warn!(worker_id = %worker_id, error = %e.message, "Failed to open worker domain for completion");
                // Cleanup worktree on domain error
                if let Err(e) = remove_worktree(&workspace, &worktree_path) {
                    warn!(worker_id = %worker_id, error = %e, "Failed to remove worktree");
                }
                tasks_failed += 1;
                continue;
            }
        };

        // Track whether we should clean up the worktree (skip on merge conflict)
        let mut should_cleanup_worktree = true;
        let mut should_break = false;

        match termination {
            Ok(TerminationReason::CompletionPromise) => {
                info!(worker_id = %worker_id, task_id = %task_id, "Task completed, integrating changes");

                // Rebase + fast-forward merge under a merge lock
                let wt_branch = worktree_branch.clone();
                let wt_path = worktree_path.clone();
                let ws = workspace.clone();
                let integrate_result = with_merge_lock(&workspace, move || {
                    let current_branch =
                        get_current_branch(&ws).map_err(|e| anyhow::anyhow!("{e}"))?;
                    rebase_and_integrate(&ws, &wt_path, &wt_branch, &current_branch)
                        .map_err(|e| anyhow::anyhow!("{e}"))
                });

                match integrate_result {
                    Ok(()) => {
                        info!(worker_id = %worker_id, task_id = %task_id, "Changes integrated successfully");
                        if let Err(e) = domain.complete_task(&worker_id, &task_id, true, None) {
                            warn!(error = %e.message, "Failed to mark task as done");
                        }
                        tasks_completed += 1;
                    }
                    Err(e) => {
                        let reason = format!("{e}");
                        warn!(worker_id = %worker_id, task_id = %task_id, error = %reason, "Integration failed");
                        if let Err(e) =
                            domain.complete_task(&worker_id, &task_id, false, Some(reason.clone()))
                        {
                            warn!(error = %e.message, "Failed to mark task as failed");
                        }
                        // On rebase conflict: keep worktree and create a merge task
                        if reason.contains("Rebase conflict") {
                            should_cleanup_worktree = false;
                            let merge_task_id = format!("merge-{task_id}");
                            let merge_prompt = format!(
                                "Resolve merge conflicts and integrate branch {} from worktree at {}. \
                                 The original task was: {}",
                                worktree_branch,
                                worktree_path.display(),
                                task_title,
                            );
                            let mut tasks = TaskDomain::new(&workspace);
                            match tasks.create(TaskCreateParams {
                                id: merge_task_id.clone(),
                                title: format!("Merge: {}", task_title),
                                status: Some("ready".to_string()),
                                priority: Some(1),
                                blocked_by: None,
                                merge_loop_prompt: Some(merge_prompt),
                                assignee_worker_id: None,
                                claimed_at: None,
                                lease_expires_at: None,
                                scope_files: None,
                            }) {
                                Ok(_) => {
                                    info!(worker_id = %worker_id, merge_task = %merge_task_id, "Created merge task for conflict resolution")
                                }
                                Err(e) => {
                                    warn!(worker_id = %worker_id, error = %e.message, "Failed to create merge task")
                                }
                            }
                        }
                        tasks_failed += 1;
                    }
                }
            }
            Ok(TerminationReason::Interrupted) => {
                info!(worker_id = %worker_id, task_id = %task_id, "Task interrupted");
                if let Err(e) = domain.complete_task(
                    &worker_id,
                    &task_id,
                    false,
                    Some("Interrupted".to_string()),
                ) {
                    warn!(error = %e.message, "Failed to mark task as failed");
                }
                tasks_failed += 1;
                should_break = true;
            }
            Ok(reason) => {
                let reason_str = format!("{:?}", reason);
                info!(worker_id = %worker_id, task_id = %task_id, reason = %reason_str, "Task ended without completion");
                if let Err(e) = domain.complete_task(&worker_id, &task_id, false, Some(reason_str))
                {
                    warn!(error = %e.message, "Failed to mark task as failed");
                }
                tasks_failed += 1;
            }
            Err(e) => {
                warn!(worker_id = %worker_id, task_id = %task_id, error = %e, "Loop returned error");
                if let Err(comp_e) =
                    domain.complete_task(&worker_id, &task_id, false, Some(format!("Error: {}", e)))
                {
                    warn!(error = %comp_e.message, "Failed to mark task as failed");
                }
                tasks_failed += 1;
            }
        }

        // Cleanup worktree (skip if merge conflict — keep for manual resolution)
        if should_cleanup_worktree && let Err(e) = remove_worktree(&workspace, &worktree_path) {
            warn!(worker_id = %worker_id, error = %e, "Failed to remove worktree");
        }

        if should_break {
            break;
        }
    }

    Ok(WorkerResult {
        worker_id,
        tasks_completed,
        tasks_failed,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jittered_duration_is_within_expected_range() {
        let base = std::time::Duration::from_secs(2);
        // jitter fraction is (nanos % 250) / 1000 → range [0.0, 0.249]
        // so result ∈ [base, base * 1.25)
        for _ in 0..100 {
            let result = jittered_duration(base);
            assert!(
                result >= base,
                "jittered duration {result:?} should be >= base {base:?}"
            );
            assert!(
                result < base + base.mul_f64(0.25),
                "jittered duration {result:?} should be < base * 1.25"
            );
        }
    }

    #[test]
    fn jittered_duration_with_zero_base_returns_zero() {
        let base = std::time::Duration::ZERO;
        let result = jittered_duration(base);
        assert_eq!(result, std::time::Duration::ZERO);
    }

    #[test]
    fn jittered_duration_with_max_idle_poll() {
        let result = jittered_duration(IDLE_POLL_MAX);
        assert!(result >= IDLE_POLL_MAX);
        assert!(result < IDLE_POLL_MAX + IDLE_POLL_MAX.mul_f64(0.25));
    }

    #[test]
    fn uuid_v4_fast_has_expected_format() {
        let uuid = uuid_v4_fast();
        // Format: 16 hex chars - 16 hex chars
        let parts: Vec<&str> = uuid.split('-').collect();
        assert_eq!(
            parts.len(),
            2,
            "uuid should have exactly one hyphen: {uuid}"
        );
        assert_eq!(parts[0].len(), 16, "first part should be 16 chars: {uuid}");
        assert_eq!(parts[1].len(), 16, "second part should be 16 chars: {uuid}");
        assert!(
            parts[0].chars().all(|c| c.is_ascii_hexdigit()),
            "first part should be hex: {uuid}"
        );
        assert!(
            parts[1].chars().all(|c| c.is_ascii_hexdigit()),
            "second part should be hex: {uuid}"
        );
    }

    #[test]
    fn uuid_v4_fast_produces_different_values() {
        let a = uuid_v4_fast();
        // Small delay to ensure different timestamp nanos
        std::thread::sleep(std::time::Duration::from_millis(1));
        let b = uuid_v4_fast();
        assert_ne!(a, b, "consecutive UUIDs should differ");
    }

    #[test]
    fn idle_poll_constants_are_sane() {
        assert!(IDLE_POLL_BASE < IDLE_POLL_MAX);
        assert!(IDLE_POLL_BASE.as_secs() >= 1);
        assert!(IDLE_POLL_MAX.as_secs() <= 60);
    }

    #[test]
    fn exponential_backoff_growth_caps_at_max() {
        let mut backoff = IDLE_POLL_BASE;
        for _ in 0..20 {
            backoff = (backoff * 2).min(IDLE_POLL_MAX);
        }
        assert_eq!(backoff, IDLE_POLL_MAX);
    }

    #[test]
    fn exponential_backoff_doubles_each_step() {
        let mut backoff = IDLE_POLL_BASE;
        let first = backoff;
        backoff = (backoff * 2).min(IDLE_POLL_MAX);
        assert_eq!(backoff, first * 2);
        backoff = (backoff * 2).min(IDLE_POLL_MAX);
        assert_eq!(backoff, first * 4);
    }
}
