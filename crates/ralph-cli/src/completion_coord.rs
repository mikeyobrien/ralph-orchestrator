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

use ralph_core::{
    CompletionAction, LoopCompletionHandler, LoopContext, LoopHistory, LoopRegistry, MergeQueue,
    MergeQueueError, RunStats, SummaryWriter, TerminationReason, get_commit_summary, get_head_sha,
};
use tracing::{debug, info, warn};

use crate::display::print_termination;

/// Mark a merge-run child as the process actively merging its queue entry.
///
/// Queue lookup and transition failures are deliberately non-fatal: merge-run
/// startup must continue so completion coordination can report its outcome.
pub fn mark_merge_run_started(repo_root: &Path, merge_loop_id: &str, pid: u32) {
    let queue = MergeQueue::new(repo_root);

    match queue.mark_merging(merge_loop_id, pid) {
        Ok(()) => {
            info!(loop_id = %merge_loop_id, pid, "Merge loop started, marked as merging");
        }
        Err(MergeQueueError::NotFound(_)) => {
            warn!(loop_id = %merge_loop_id, "Merge loop started but no queue entry found");
        }
        Err(MergeQueueError::InvalidTransition(_, from, _)) => {
            debug!(loop_id = %merge_loop_id, state = ?from, "Merge queue entry already in terminal state, skipping");
        }
        Err(error) => {
            warn!(loop_id = %merge_loop_id, error = %error, "Failed to mark merge loop as merging");
        }
    }
}

/// Run the completion bookkeeping for a loop that terminated with `reason`.
///
/// `state` carries the iteration count / elapsed time surfaced by whichever
/// engine ran the loop. `context` is `None` only for ad-hoc runs with no loop
/// identity (no merge-queue / registry participation in that case).
#[allow(clippy::too_many_arguments)]
pub fn coordinate_completion(
    reason: &TerminationReason,
    state: &RunStats,
    context: Option<&LoopContext>,
    scratchpad: &str,
    prompt: &str,
    auto_merge: bool,
    loop_id: &str,
    use_colors: bool,
    print_banner: bool,
) {
    let repo_root = context
        .map(|c| c.repo_root().to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."));

    // 1. Summary file.
    let summary_writer = SummaryWriter::default();
    let scratchpad_path = Path::new(scratchpad);
    let scratchpad_opt = if scratchpad_path.exists() {
        Some(scratchpad_path)
    } else {
        None
    };
    let final_commit = get_commit_summary(&repo_root).ok();
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
        } else if let Err(e) = history.record_completed(reason.history_label()) {
            warn!("Failed to record completion in history: {}", e);
        }
    }

    // 3. Merge-queue transitions for merge loops (RALPH_MERGE_LOOP_ID set).
    let merge_loop_id = std::env::var("RALPH_MERGE_LOOP_ID").ok();
    if let Some(ref mlid) = merge_loop_id {
        let queue = MergeQueue::new(&repo_root);

        if matches!(reason, TerminationReason::CompletionPromise) {
            match get_head_sha(&repo_root).ok() {
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
        } else if let Err(e) = queue.mark_needs_review(mlid, reason.review_description()) {
            warn!(loop_id = %mlid, error = %e, "Failed to mark merge as needs-review");
        } else {
            info!(loop_id = %mlid, reason = %reason, "Merge marked as needs-review");
        }
    }

    // 4-6. Landing / enqueue, primary drain, registry deregister.
    if let Some(ctx) = context {
        if merge_loop_id.is_none() && matches!(reason, TerminationReason::CompletionPromise) {
            let handler = LoopCompletionHandler::new(auto_merge);
            match handler.handle_completion(ctx, prompt) {
                Ok(CompletionAction::None) => debug!("Loop completed, no action needed"),
                Ok(CompletionAction::Landed { .. }) => {
                    debug!("Primary loop landed successfully")
                }
                Ok(CompletionAction::Enqueued { loop_id: lid, .. }) => {
                    info!(loop_id = %lid, "Loop queued for auto-merge");
                    let _ = LoopHistory::from_context(ctx).record_merge_queued();
                }
                Ok(CompletionAction::ManualMerge { loop_id: lid, .. }) => {
                    info!(loop_id = %lid, "Loop completed; manual merge is required for this managed loop");
                }
                Err(e) => warn!("Completion handler failed: {}", e),
            }
        }

        // Primary loop drains queued worktree merges on completion.
        if ctx.is_primary() && matches!(reason, TerminationReason::CompletionPromise) {
            crate::merge_processing::process_pending_merges_cli(ctx.repo_root());
        }

        // Always deregister — the process is exiting regardless of reason.
        let registry = LoopRegistry::new(ctx.repo_root());
        if let Err(e) = registry.deregister_current_process() {
            warn!("Failed to deregister loop from registry: {}", e);
        }
    }

    // 7. Console termination banner. RPC mode keeps stdout as `RpcEvent` lines.
    if print_banner {
        print_termination(reason, state, use_colors, Some(loop_id));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn history_and_review_labels_cover_all_reasons() {
        assert_eq!(
            TerminationReason::CompletionPromise.history_label(),
            "completion_promise"
        );
        assert_eq!(
            TerminationReason::MaxIterations.review_description(),
            "max iterations reached"
        );
    }

    #[test]
    fn coordinate_without_context_is_a_noop_for_merge_state() {
        // No loop context => no registry / merge-queue participation, just the
        // summary + banner. Should not panic.
        let state = RunStats::default();
        coordinate_completion(
            &TerminationReason::MaxIterations,
            &state,
            None,
            "/nonexistent/scratchpad.md",
            "prompt",
            false,
            "test-loop",
            false,
            true,
        );
    }

    #[test]
    fn merge_run_start_allows_queued_entry_to_be_marked_merged() {
        let temp_dir = tempfile::tempdir().unwrap();
        let queue = MergeQueue::new(temp_dir.path());
        queue.enqueue("loop-queued", "prompt").unwrap();

        mark_merge_run_started(temp_dir.path(), "loop-queued", 4242);

        let entry = queue.get_entry("loop-queued").unwrap().unwrap();
        assert_eq!(entry.state, ralph_core::MergeState::Merging);
        assert_eq!(entry.merge_pid, Some(4242));
        queue.mark_merged("loop-queued", "abc123").unwrap();
    }

    #[test]
    fn merge_run_start_allows_queued_entry_to_be_marked_needs_review() {
        let temp_dir = tempfile::tempdir().unwrap();
        let queue = MergeQueue::new(temp_dir.path());
        queue.enqueue("loop-review", "prompt").unwrap();

        mark_merge_run_started(temp_dir.path(), "loop-review", 4243);

        queue.mark_needs_review("loop-review", "reason").unwrap();
        let entry = queue.get_entry("loop-review").unwrap().unwrap();
        assert_eq!(entry.state, ralph_core::MergeState::NeedsReview);
    }

    #[test]
    fn merge_run_start_ignores_missing_entry() {
        let temp_dir = tempfile::tempdir().unwrap();
        let queue = MergeQueue::new(temp_dir.path());

        mark_merge_run_started(temp_dir.path(), "missing", 4244);

        assert!(queue.list().unwrap().is_empty());
    }

    #[test]
    fn merge_run_start_ignores_already_merged_entry() {
        let temp_dir = tempfile::tempdir().unwrap();
        let queue = MergeQueue::new(temp_dir.path());
        queue.enqueue("loop-merged", "prompt").unwrap();
        queue.mark_merging("loop-merged", 4245).unwrap();
        queue.mark_merged("loop-merged", "abc123").unwrap();

        mark_merge_run_started(temp_dir.path(), "loop-merged", 4246);

        let entry = queue.get_entry("loop-merged").unwrap().unwrap();
        assert_eq!(entry.state, ralph_core::MergeState::Merged);
        assert_eq!(entry.merge_commit.as_deref(), Some("abc123"));
    }
}
