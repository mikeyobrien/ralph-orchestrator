//! Engine-agnostic loop-completion coordination.
//!
//! When ralph drives the autoloop runtime (`core.engine = "autoloop"`), the
//! in-house event loop — and the completion bookkeeping baked into it — is
//! bypassed. This module reproduces that bookkeeping so parallel-loop
//! coordination keeps working under v3:
//!
//! 1. write the run summary file,
//! 2. record loop history (completed / terminated),
//! 3. apply merge-queue state transitions for merge loops,
//! 4. land + enqueue completed worktree loops via [`LoopCompletionHandler`],
//! 5. drain the merge queue on primary-loop completion,
//! 6. deregister the process from the loop registry,
//! 7. print the termination banner.
//!
//! It depends only on the terminal [`TerminationReason`] and the
//! [`LoopContext`], never on engine internals, so it survives the deletion of
//! the in-house engine.

use std::path::{Path, PathBuf};
use std::process::Command;

use ralph_core::{
    CompletionAction, LoopCompletionHandler, LoopContext, LoopHistory, LoopRegistry, LoopState,
    MergeQueue, SummaryWriter, TerminationReason,
};
use tracing::{debug, info, warn};

use crate::display::print_termination;

/// Run the completion bookkeeping for a loop that terminated with `reason`.
///
/// `state` carries the iteration count / elapsed time surfaced by whichever
/// engine ran the loop. `context` is `None` only for ad-hoc runs with no loop
/// identity (no merge-queue / registry participation in that case).
#[allow(clippy::too_many_arguments)]
pub fn coordinate_completion(
    reason: &TerminationReason,
    state: &LoopState,
    context: Option<&LoopContext>,
    scratchpad: &str,
    prompt: &str,
    auto_merge: bool,
    loop_id: &str,
    use_colors: bool,
) {
    // 1. Summary file.
    let summary_writer = SummaryWriter::default();
    let scratchpad_path = Path::new(scratchpad);
    let scratchpad_opt = if scratchpad_path.exists() {
        Some(scratchpad_path)
    } else {
        None
    };
    let final_commit = last_commit_info();
    if let Err(e) = summary_writer.write(reason, state, scratchpad_opt, final_commit.as_deref()) {
        warn!("Failed to write summary file: {}", e);
    }

    // 2. Loop history.
    if let Some(ctx) = context {
        let history = LoopHistory::from_context(ctx);
        if matches!(reason, TerminationReason::Interrupted) {
            if let Err(e) = history.record_terminated("SIGTERM") {
                warn!("Failed to record termination in history: {}", e);
            }
        } else if let Err(e) = history.record_completed(history_reason(reason)) {
            warn!("Failed to record completion in history: {}", e);
        }
    }

    // 3. Merge-queue transitions for merge loops (RALPH_MERGE_LOOP_ID set).
    let merge_loop_id = std::env::var("RALPH_MERGE_LOOP_ID").ok();
    if let Some(ref mlid) = merge_loop_id {
        let repo_root = context
            .map(|c| c.repo_root().to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."));
        let queue = MergeQueue::new(&repo_root);

        if matches!(reason, TerminationReason::CompletionPromise) {
            match head_commit() {
                Some(sha) => {
                    if let Err(e) = queue.mark_merged(mlid, &sha) {
                        warn!(loop_id = %mlid, error = %e, "Failed to mark merge as completed");
                    } else {
                        info!(loop_id = %mlid, commit = %sha, "Merge completed successfully");
                    }
                }
                None => {
                    if let Err(e) =
                        queue.mark_needs_review(mlid, "merge complete but commit not found")
                    {
                        warn!(loop_id = %mlid, error = %e, "Failed to mark merge as needs-review");
                    } else {
                        warn!(loop_id = %mlid, "Merge completed but could not resolve commit SHA");
                    }
                }
            }
        } else if let Err(e) = queue.mark_needs_review(mlid, needs_review_reason(reason)) {
            warn!(loop_id = %mlid, error = %e, "Failed to mark merge as needs-review");
        } else {
            info!(loop_id = %mlid, reason = needs_review_reason(reason), "Merge marked as needs-review");
        }
    }

    // 4-6. Landing / enqueue, primary drain, registry deregister.
    if let Some(ctx) = context {
        if merge_loop_id.is_none() && matches!(reason, TerminationReason::CompletionPromise) {
            let handler = LoopCompletionHandler::new(auto_merge);
            match handler.handle_completion(ctx, prompt) {
                Ok(CompletionAction::None) => debug!("Loop completed, no action needed"),
                Ok(CompletionAction::Landed { .. }) => {
                    info!("Primary loop landed successfully")
                }
                Ok(CompletionAction::Enqueued { loop_id: lid, .. }) => {
                    info!(loop_id = %lid, "Loop queued for auto-merge");
                    let _ = LoopHistory::from_context(ctx).record_merge_queued();
                }
                Ok(CompletionAction::ManualMerge {
                    loop_id: lid,
                    worktree_path,
                    ..
                }) => {
                    info!(loop_id = %lid, "Loop completed. To merge manually: cd {} && git merge", worktree_path);
                }
                Err(e) => warn!("Completion handler failed: {}", e),
            }
        }

        // Primary loop drains queued worktree merges on completion.
        if ctx.is_primary() && matches!(reason, TerminationReason::CompletionPromise) {
            crate::loop_runner::process_pending_merges_cli(ctx.repo_root());
        }

        // Always deregister — the process is exiting regardless of reason.
        let registry = LoopRegistry::new(ctx.repo_root());
        if let Err(e) = registry.deregister_current_process() {
            warn!("Failed to deregister loop from registry: {}", e);
        }
    }

    // 7. Console termination banner.
    print_termination(reason, state, use_colors, Some(loop_id));
}

/// History event label (snake_case) for a termination reason.
fn history_reason(reason: &TerminationReason) -> &'static str {
    match reason {
        TerminationReason::CompletionPromise => "completion_promise",
        TerminationReason::MaxIterations => "max_iterations",
        TerminationReason::MaxRuntime => "max_runtime",
        TerminationReason::MaxCost => "max_cost",
        TerminationReason::ConsecutiveFailures => "consecutive_failures",
        TerminationReason::LoopThrashing => "loop_thrashing",
        TerminationReason::LoopStale => "loop_stale",
        TerminationReason::ValidationFailure => "validation_failure",
        TerminationReason::Stopped => "stopped",
        TerminationReason::Interrupted => "interrupted",
        TerminationReason::RestartRequested => "restart_requested",
        TerminationReason::WorkspaceGone => "workspace_gone",
        TerminationReason::Cancelled => "cancelled",
    }
}

/// Human-readable reason recorded when a non-completing merge loop needs review.
fn needs_review_reason(reason: &TerminationReason) -> &'static str {
    match reason {
        TerminationReason::MaxIterations => "max iterations reached",
        TerminationReason::MaxRuntime => "max runtime exceeded",
        TerminationReason::MaxCost => "max cost exceeded",
        TerminationReason::ConsecutiveFailures => "consecutive failures",
        TerminationReason::LoopThrashing => "loop thrashing detected",
        TerminationReason::LoopStale => "stale loop detected",
        TerminationReason::ValidationFailure => "validation failure",
        TerminationReason::Stopped => "manually stopped",
        TerminationReason::Interrupted => "interrupted by signal",
        TerminationReason::RestartRequested => "restart requested",
        TerminationReason::WorkspaceGone => "workspace directory removed",
        TerminationReason::Cancelled => "cancelled by human",
        TerminationReason::CompletionPromise => "completed",
    }
}

/// `git log -1 --format="%H %s"` (commit + subject), or `None`.
fn last_commit_info() -> Option<String> {
    git_line(&["log", "-1", "--format=%H %s"])
}

/// `git rev-parse HEAD`, or `None`.
fn head_commit() -> Option<String> {
    git_line(&["rev-parse", "HEAD"])
}

fn git_line(args: &[&str]) -> Option<String> {
    let out = Command::new("git").args(args).output().ok()?;
    if out.status.success() {
        String::from_utf8(out.stdout)
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn history_and_review_labels_cover_all_reasons() {
        // Exhaustive match arms guarantee compile-time coverage; spot-check a few.
        assert_eq!(
            history_reason(&TerminationReason::CompletionPromise),
            "completion_promise"
        );
        assert_eq!(
            needs_review_reason(&TerminationReason::MaxIterations),
            "max iterations reached"
        );
    }

    #[test]
    fn coordinate_without_context_is_a_noop_for_merge_state() {
        // No loop context => no registry / merge-queue participation, just the
        // summary + banner. Should not panic.
        let state = LoopState::new();
        coordinate_completion(
            &TerminationReason::MaxIterations,
            &state,
            None,
            "/nonexistent/scratchpad.md",
            "prompt",
            false,
            "test-loop",
            false,
        );
    }
}
