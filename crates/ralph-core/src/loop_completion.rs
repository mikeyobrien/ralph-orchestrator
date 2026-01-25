//! Loop completion handler for worktree-based loops.
//!
//! Handles post-completion actions for loops running in git worktrees,
//! including auto-merge queue integration.
//!
//! # Design
//!
//! When a loop completes successfully (CompletionPromise):
//! - **Primary loop**: No special handling (runs in main workspace)
//! - **Worktree loop with auto-merge**: Enqueue to merge queue for merge-ralph
//! - **Worktree loop without auto-merge**: Log completion, leave worktree for manual merge
//!
//! # Example
//!
//! ```no_run
//! use ralph_core::loop_completion::{LoopCompletionHandler, CompletionAction};
//! use ralph_core::loop_context::LoopContext;
//! use std::path::PathBuf;
//!
//! // Primary loop - no special action
//! let primary = LoopContext::primary(PathBuf::from("/project"));
//! let handler = LoopCompletionHandler::new(true); // auto_merge enabled
//! let action = handler.handle_completion(&primary, "implement auth").unwrap();
//! assert!(matches!(action, CompletionAction::None));
//!
//! // Worktree loop with auto-merge - enqueues to merge queue
//! let worktree = LoopContext::worktree(
//!     "ralph-20250124-a3f2",
//!     PathBuf::from("/project/.worktrees/ralph-20250124-a3f2"),
//!     PathBuf::from("/project"),
//! );
//! let action = handler.handle_completion(&worktree, "implement auth").unwrap();
//! assert!(matches!(action, CompletionAction::Enqueued { .. }));
//! ```

use crate::loop_context::LoopContext;
use crate::merge_queue::{MergeQueue, MergeQueueError};
use tracing::{debug, info};

/// Action taken upon loop completion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompletionAction {
    /// No action needed (primary loop or non-worktree context).
    None,

    /// Loop was enqueued to the merge queue.
    Enqueued {
        /// The loop ID that was enqueued.
        loop_id: String,
    },

    /// Auto-merge is disabled; worktree left for manual handling.
    ManualMerge {
        /// The loop ID.
        loop_id: String,
        /// Path to the worktree directory.
        worktree_path: String,
    },
}

/// Errors that can occur during completion handling.
#[derive(Debug, thiserror::Error)]
pub enum CompletionError {
    /// Failed to enqueue to merge queue.
    #[error("Failed to enqueue to merge queue: {0}")]
    EnqueueFailed(#[from] MergeQueueError),
}

/// Handler for loop completion events.
///
/// Determines the appropriate action when a loop completes based on
/// whether it's a worktree loop and the auto-merge configuration.
pub struct LoopCompletionHandler {
    /// Whether auto-merge is enabled (default: true).
    auto_merge: bool,
}

impl Default for LoopCompletionHandler {
    fn default() -> Self {
        Self::new(true)
    }
}

impl LoopCompletionHandler {
    /// Creates a new completion handler.
    ///
    /// # Arguments
    ///
    /// * `auto_merge` - If true, completed worktree loops are enqueued for merge-ralph.
    ///   If false, worktrees are left for manual merge.
    pub fn new(auto_merge: bool) -> Self {
        Self { auto_merge }
    }

    /// Handles loop completion, taking appropriate action based on context.
    ///
    /// # Arguments
    ///
    /// * `context` - The loop context (primary or worktree)
    /// * `prompt` - The prompt that was executed (for merge queue metadata)
    ///
    /// # Returns
    ///
    /// The action that was taken, or an error if the action failed.
    pub fn handle_completion(
        &self,
        context: &LoopContext,
        prompt: &str,
    ) -> Result<CompletionAction, CompletionError> {
        // Primary loops don't need special handling
        if context.is_primary() {
            debug!("Primary loop completed - no special action needed");
            return Ok(CompletionAction::None);
        }

        // Get loop ID from context (worktree loops always have one)
        let loop_id = match context.loop_id() {
            Some(id) => id.to_string(),
            None => {
                // Shouldn't happen for worktree contexts, but handle gracefully
                debug!("Loop completed without loop ID - treating as primary");
                return Ok(CompletionAction::None);
            }
        };

        let worktree_path = context.workspace().to_string_lossy().to_string();

        if self.auto_merge {
            // Enqueue to merge queue for automatic merge-ralph processing
            let queue = MergeQueue::new(context.repo_root());
            queue.enqueue(&loop_id, prompt)?;

            info!(
                loop_id = %loop_id,
                worktree = %worktree_path,
                "Loop completed and enqueued for auto-merge"
            );

            Ok(CompletionAction::Enqueued { loop_id })
        } else {
            // Leave worktree for manual handling
            info!(
                loop_id = %loop_id,
                worktree = %worktree_path,
                "Loop completed - worktree preserved for manual merge (--no-auto-merge)"
            );

            Ok(CompletionAction::ManualMerge {
                loop_id,
                worktree_path,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_primary_loop_no_action() {
        let temp = TempDir::new().unwrap();
        let context = LoopContext::primary(temp.path().to_path_buf());
        let handler = LoopCompletionHandler::new(true);

        let action = handler.handle_completion(&context, "test prompt").unwrap();
        assert_eq!(action, CompletionAction::None);
    }

    #[test]
    fn test_worktree_loop_auto_merge_enqueues() {
        let temp = TempDir::new().unwrap();
        let repo_root = temp.path().to_path_buf();
        let worktree_path = repo_root.join(".worktrees/ralph-test-1234");

        // Create necessary directories
        std::fs::create_dir_all(&worktree_path).unwrap();
        std::fs::create_dir_all(repo_root.join(".ralph")).unwrap();

        let context = LoopContext::worktree("ralph-test-1234", worktree_path, repo_root.clone());

        let handler = LoopCompletionHandler::new(true); // auto_merge enabled

        let action = handler
            .handle_completion(&context, "implement feature X")
            .unwrap();

        match action {
            CompletionAction::Enqueued { loop_id } => {
                assert_eq!(loop_id, "ralph-test-1234");

                // Verify it was actually enqueued
                let queue = MergeQueue::new(&repo_root);
                let entry = queue.get_entry("ralph-test-1234").unwrap().unwrap();
                assert_eq!(entry.prompt, "implement feature X");
            }
            _ => panic!("Expected Enqueued action, got {:?}", action),
        }
    }

    #[test]
    fn test_worktree_loop_no_auto_merge_manual() {
        let temp = TempDir::new().unwrap();
        let repo_root = temp.path().to_path_buf();
        let worktree_path = repo_root.join(".worktrees/ralph-test-5678");

        std::fs::create_dir_all(&worktree_path).unwrap();

        let context =
            LoopContext::worktree("ralph-test-5678", worktree_path.clone(), repo_root.clone());

        let handler = LoopCompletionHandler::new(false); // auto_merge disabled

        let action = handler.handle_completion(&context, "test prompt").unwrap();

        match action {
            CompletionAction::ManualMerge {
                loop_id,
                worktree_path: path,
            } => {
                assert_eq!(loop_id, "ralph-test-5678");
                assert_eq!(path, worktree_path.to_string_lossy());
            }
            _ => panic!("Expected ManualMerge action, got {:?}", action),
        }

        // Verify nothing was enqueued
        let queue = MergeQueue::new(&repo_root);
        let entry = queue.get_entry("ralph-test-5678").unwrap();
        assert!(entry.is_none());
    }

    #[test]
    fn test_default_handler_has_auto_merge_enabled() {
        let handler = LoopCompletionHandler::default();
        assert!(handler.auto_merge);
    }
}
