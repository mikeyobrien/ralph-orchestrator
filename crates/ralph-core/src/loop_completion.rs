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

use crate::git_ops::auto_commit_changes;
use crate::landing::{LandingHandler, LandingResult};
use crate::loop_context::LoopContext;
use crate::merge_queue::{MergeQueue, MergeQueueError};
use tracing::{debug, info, warn};

/// Action taken upon loop completion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompletionAction {
    /// No action needed (primary loop or non-worktree context).
    None,

    /// Loop was enqueued to the merge queue.
    Enqueued {
        /// The loop ID that was enqueued.
        loop_id: String,
        /// Landing result details (optional for backwards compatibility).
        landing: Option<CompletionLanding>,
    },

    /// Auto-merge is disabled; worktree left for manual handling.
    ManualMerge {
        /// The loop ID.
        loop_id: String,
        /// Path to the worktree directory.
        worktree_path: String,
        /// Landing result details (optional for backwards compatibility).
        landing: Option<CompletionLanding>,
    },

    /// Primary loop completed with landing.
    Landed {
        /// Landing result details.
        landing: CompletionLanding,
    },
}

/// Landing details included in completion actions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompletionLanding {
    /// Whether changes were auto-committed.
    pub committed: bool,
    /// The commit SHA if a commit was made.
    pub commit_sha: Option<String>,
    /// Path to the handoff file.
    pub handoff_path: String,
    /// Number of open tasks remaining.
    pub open_task_count: usize,
}

impl From<&LandingResult> for CompletionLanding {
    fn from(result: &LandingResult) -> Self {
        Self {
            committed: result.committed,
            commit_sha: result.commit_sha.clone(),
            handoff_path: result.handoff_path.to_string_lossy().to_string(),
            open_task_count: result.open_tasks.len(),
        }
    }
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
        // Execute landing sequence first (for all loops)
        let landing_result = self.execute_landing(context, prompt);

        // Primary loops complete with landing only
        if context.is_primary() {
            debug!("Primary loop completed with landing");
            return Ok(match landing_result {
                Some(result) => CompletionAction::Landed {
                    landing: CompletionLanding::from(&result),
                },
                None => CompletionAction::None,
            });
        }

        // Get loop ID from context (worktree loops always have one)
        let loop_id = match context.loop_id() {
            Some(id) => id.to_string(),
            None => {
                // Shouldn't happen for worktree contexts, but handle gracefully
                debug!("Loop completed without loop ID - treating as primary");
                return Ok(match landing_result {
                    Some(result) => CompletionAction::Landed {
                        landing: CompletionLanding::from(&result),
                    },
                    None => CompletionAction::None,
                });
            }
        };

        let worktree_path = context.workspace().to_string_lossy().to_string();
        let landing = landing_result.as_ref().map(CompletionLanding::from);

        if self.auto_merge {
            // Auto-commit any uncommitted changes before enqueueing
            match auto_commit_changes(context.workspace(), &loop_id) {
                Ok(result) => {
                    if result.committed {
                        info!(
                            loop_id = %loop_id,
                            commit = ?result.commit_sha,
                            files = result.files_staged,
                            "Auto-committed changes before merge queue"
                        );
                    }
                }
                Err(e) => {
                    warn!(
                        loop_id = %loop_id,
                        error = %e,
                        "Auto-commit failed, proceeding with enqueue"
                    );
                }
            }

            // Enqueue to merge queue for automatic merge-ralph processing
            let queue = MergeQueue::new(context.repo_root());
            queue.enqueue(&loop_id, prompt)?;

            info!(
                loop_id = %loop_id,
                worktree = %worktree_path,
                committed = ?landing.as_ref().map(|l| l.committed),
                "Loop completed and enqueued for auto-merge"
            );

            Ok(CompletionAction::Enqueued { loop_id, landing })
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
                landing,
            })
        }
    }

    /// Executes the landing sequence.
    ///
    /// Returns the landing result if successful, or None if landing failed.
    fn execute_landing(&self, context: &LoopContext, prompt: &str) -> Option<LandingResult> {
        let handler = LandingHandler::new(context.clone());

        match handler.land(prompt) {
            Ok(result) => {
                if result.committed {
                    info!(
                        commit = ?result.commit_sha,
                        handoff = %result.handoff_path.display(),
                        "Landing completed with auto-commit"
                    );
                } else {
                    debug!(
                        handoff = %result.handoff_path.display(),
                        "Landing completed (no changes to commit)"
                    );
                }
                Some(result)
            }
            Err(e) => {
                warn!(error = %e, "Landing sequence failed, proceeding without landing");
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::Command;
    use tempfile::TempDir;

    fn init_git_repo(dir: &std::path::Path) {
        Command::new("git")
            .args(["init", "--initial-branch=main"])
            .current_dir(dir)
            .output()
            .unwrap();

        Command::new("git")
            .args(["config", "user.email", "test@test.local"])
            .current_dir(dir)
            .output()
            .unwrap();

        Command::new("git")
            .args(["config", "user.name", "Test User"])
            .current_dir(dir)
            .output()
            .unwrap();

        std::fs::write(dir.join("README.md"), "# Test").unwrap();

        // Add .ralph/ to .gitignore so landing doesn't create uncommitted changes
        std::fs::write(dir.join(".gitignore"), ".ralph/\n").unwrap();

        Command::new("git")
            .args(["add", "README.md", ".gitignore"])
            .current_dir(dir)
            .output()
            .unwrap();
        Command::new("git")
            .args(["commit", "-m", "Initial commit"])
            .current_dir(dir)
            .output()
            .unwrap();
    }

    #[test]
    fn test_primary_loop_with_landing() {
        let temp = TempDir::new().unwrap();
        init_git_repo(temp.path());
        let context = LoopContext::primary(temp.path().to_path_buf());
        context.ensure_directories().unwrap();
        let handler = LoopCompletionHandler::new(true);

        let action = handler.handle_completion(&context, "test prompt").unwrap();
        // Primary loops now return Landed instead of None
        assert!(
            matches!(action, CompletionAction::Landed { .. }),
            "Expected Landed, got {:?}",
            action
        );
    }

    #[test]
    fn test_worktree_loop_auto_merge_enqueues() {
        let temp = TempDir::new().unwrap();
        let repo_root = temp.path().to_path_buf();
        init_git_repo(&repo_root);
        let worktree_path = repo_root.join(".worktrees/ralph-test-1234");

        // Create necessary directories
        std::fs::create_dir_all(&worktree_path).unwrap();
        std::fs::create_dir_all(repo_root.join(".ralph")).unwrap();

        let context =
            LoopContext::worktree("ralph-test-1234", worktree_path.clone(), repo_root.clone());
        context.ensure_directories().unwrap();

        let handler = LoopCompletionHandler::new(true); // auto_merge enabled

        let action = handler
            .handle_completion(&context, "implement feature X")
            .unwrap();

        match action {
            CompletionAction::Enqueued { loop_id, landing } => {
                assert_eq!(loop_id, "ralph-test-1234");
                // Landing should have been executed
                assert!(landing.is_some());

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
        init_git_repo(&repo_root);
        let worktree_path = repo_root.join(".worktrees/ralph-test-5678");

        std::fs::create_dir_all(&worktree_path).unwrap();

        let context =
            LoopContext::worktree("ralph-test-5678", worktree_path.clone(), repo_root.clone());
        context.ensure_directories().unwrap();

        let handler = LoopCompletionHandler::new(false); // auto_merge disabled

        let action = handler.handle_completion(&context, "test prompt").unwrap();

        match action {
            CompletionAction::ManualMerge {
                loop_id,
                worktree_path: path,
                landing,
            } => {
                assert_eq!(loop_id, "ralph-test-5678");
                assert_eq!(path, worktree_path.to_string_lossy());
                // Landing should have been executed
                assert!(landing.is_some());
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

    #[test]
    fn test_worktree_loop_auto_commits_uncommitted_changes() {
        let temp = TempDir::new().unwrap();
        let repo_root = temp.path().to_path_buf();
        init_git_repo(&repo_root);

        // Create worktree directory and set up as a git worktree
        let worktree_path = repo_root.join(".worktrees/ralph-autocommit");
        let branch_name = "ralph/ralph-autocommit";

        // Create the worktree
        std::fs::create_dir_all(repo_root.join(".worktrees")).unwrap();
        Command::new("git")
            .args(["worktree", "add", "-b", branch_name])
            .arg(&worktree_path)
            .current_dir(&repo_root)
            .output()
            .unwrap();

        // Create uncommitted changes in the worktree
        std::fs::write(worktree_path.join("feature.txt"), "new feature").unwrap();

        // Create .ralph directory for merge queue
        std::fs::create_dir_all(repo_root.join(".ralph")).unwrap();

        let context =
            LoopContext::worktree("ralph-autocommit", worktree_path.clone(), repo_root.clone());

        let handler = LoopCompletionHandler::new(true);

        let action = handler.handle_completion(&context, "add feature").unwrap();

        // Should enqueue successfully
        assert!(matches!(action, CompletionAction::Enqueued { .. }));

        // Verify the changes were committed
        let output = Command::new("git")
            .args(["log", "-1", "--pretty=%s"])
            .current_dir(&worktree_path)
            .output()
            .unwrap();
        let message = String::from_utf8_lossy(&output.stdout);
        assert!(
            message.contains("auto-commit before merge"),
            "Expected auto-commit message, got: {}",
            message
        );

        // Verify working tree is clean
        let output = Command::new("git")
            .args(["status", "--porcelain"])
            .current_dir(&worktree_path)
            .output()
            .unwrap();
        let status = String::from_utf8_lossy(&output.stdout);
        assert!(status.trim().is_empty(), "Working tree should be clean");
    }

    #[test]
    fn test_worktree_loop_no_auto_commit_when_clean() {
        let temp = TempDir::new().unwrap();
        let repo_root = temp.path().to_path_buf();
        init_git_repo(&repo_root);

        // Create worktree
        let worktree_path = repo_root.join(".worktrees/ralph-clean");
        let branch_name = "ralph/ralph-clean";

        std::fs::create_dir_all(repo_root.join(".worktrees")).unwrap();
        Command::new("git")
            .args(["worktree", "add", "-b", branch_name])
            .arg(&worktree_path)
            .current_dir(&repo_root)
            .output()
            .unwrap();

        // Get the initial commit count
        let output = Command::new("git")
            .args(["rev-list", "--count", "HEAD"])
            .current_dir(&worktree_path)
            .output()
            .unwrap();
        let initial_count: i32 = String::from_utf8_lossy(&output.stdout)
            .trim()
            .parse()
            .unwrap();

        // Create .ralph directory for merge queue
        std::fs::create_dir_all(repo_root.join(".ralph")).unwrap();

        let context =
            LoopContext::worktree("ralph-clean", worktree_path.clone(), repo_root.clone());

        let handler = LoopCompletionHandler::new(true);

        let action = handler.handle_completion(&context, "no changes").unwrap();

        assert!(matches!(action, CompletionAction::Enqueued { .. }));

        // Verify no new commit was made
        let output = Command::new("git")
            .args(["rev-list", "--count", "HEAD"])
            .current_dir(&worktree_path)
            .output()
            .unwrap();
        let final_count: i32 = String::from_utf8_lossy(&output.stdout)
            .trim()
            .parse()
            .unwrap();

        assert_eq!(
            initial_count, final_count,
            "No new commit should be made when working tree is clean"
        );
    }
}
