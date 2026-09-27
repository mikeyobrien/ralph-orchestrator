//! Landing handler for clean session boundaries.
//!
//! Orchestrates the "land the plane" sequence on loop completion:
//! 1. Verify task state (log warnings for open tasks)
//! 2. Auto-commit the loop's changes (never untracked files that predate it)
//! 3. Clean git state (stashes, prune refs)
//! 4. Generate handoff prompt
//!
//! This pattern ensures clean session boundaries and enables seamless
//! handoffs between Ralph loops.

use crate::git_ops::{
    AutoCommitResult, GitOpsError, UntrackedScope, auto_commit_changes, clean_stashes,
    is_working_tree_clean, list_untracked_files, prune_remote_refs,
};
use crate::handoff::{HandoffError, HandoffWriter};
use crate::loop_context::LoopContext;
use crate::task_store::TaskStore;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::path::PathBuf;
use tracing::{debug, warn};

/// Result of the landing sequence.
#[derive(Debug, Clone)]
pub struct LandingResult {
    /// Whether changes were auto-committed.
    pub committed: bool,

    /// The commit SHA if a commit was made.
    pub commit_sha: Option<String>,

    /// Path to the generated handoff file.
    pub handoff_path: PathBuf,

    /// IDs of tasks that remain open.
    pub open_tasks: Vec<String>,

    /// Number of stashes that were cleared.
    pub stashes_cleared: usize,

    /// Whether the working tree is clean after landing.
    pub working_tree_clean: bool,
}

/// Untracked files present when a loop started, stored at
/// [`LoopContext::landing_baseline_path`].
#[derive(Debug, Default, Serialize, Deserialize)]
struct UntrackedBaseline {
    untracked: BTreeSet<String>,
}

/// Records the untracked files present when a loop starts.
///
/// Landing later commits only untracked files absent from this baseline, so
/// operator files that already sat in the workspace (or were synced into a
/// new worktree) stay out of the loop's auto-commit. The baseline file lists
/// itself, since it lives under `.ralph/`, which a project may not ignore.
pub fn record_untracked_baseline(context: &LoopContext) -> std::io::Result<()> {
    let workspace = context.workspace();
    let baseline_path = context.landing_baseline_path();
    let mut untracked: BTreeSet<String> = list_untracked_files(workspace)
        .map_err(|e| std::io::Error::other(e.to_string()))?
        .into_iter()
        .collect();
    if let Ok(relative) = baseline_path.strip_prefix(workspace) {
        untracked.insert(relative.to_string_lossy().into_owned());
    }
    if let Some(parent) = baseline_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(&UntrackedBaseline { untracked })
        .map_err(std::io::Error::other)?;
    std::fs::write(baseline_path, json)
}

/// Reads the untracked baseline recorded at loop start, if any.
fn read_untracked_baseline(context: &LoopContext) -> Option<BTreeSet<String>> {
    let content = std::fs::read_to_string(context.landing_baseline_path()).ok()?;
    serde_json::from_str::<UntrackedBaseline>(&content)
        .ok()
        .map(|baseline| baseline.untracked)
}

/// Auto-commits the loop's changes: tracked edits plus untracked files the
/// loop created. Without a baseline, untracked files are left alone rather
/// than swept in, and a warning names how many were skipped.
pub fn commit_loop_changes(
    context: &LoopContext,
    loop_id: &str,
) -> Result<AutoCommitResult, GitOpsError> {
    let workspace = context.workspace();
    match read_untracked_baseline(context) {
        Some(baseline) => auto_commit_changes(
            workspace,
            loop_id,
            UntrackedScope::ExceptPreexisting(&baseline),
        ),
        None => {
            let skipped = list_untracked_files(workspace)
                .map(|f| f.len())
                .unwrap_or(0);
            if skipped > 0 {
                warn!(
                    loop_id = %loop_id,
                    skipped,
                    "No untracked-file baseline for this loop; committing tracked changes only"
                );
            }
            auto_commit_changes(workspace, loop_id, UntrackedScope::None)
        }
    }
}

/// Errors that can occur during landing.
#[derive(Debug, thiserror::Error)]
pub enum LandingError {
    /// IO error during file operations.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Git operation failed.
    #[error("Git error: {0}")]
    Git(#[from] crate::git_ops::GitOpsError),

    /// Handoff generation failed.
    #[error("Handoff error: {0}")]
    Handoff(#[from] HandoffError),
}

/// Configuration for the landing handler.
#[derive(Debug, Clone)]
pub struct LandingConfig {
    /// Whether to auto-commit uncommitted changes.
    pub auto_commit: bool,

    /// Whether to clear git stashes.
    pub clear_stashes: bool,

    /// Whether to prune remote refs.
    pub prune_refs: bool,

    /// Whether to generate the handoff file.
    pub generate_handoff: bool,
}

impl Default for LandingConfig {
    fn default() -> Self {
        Self {
            auto_commit: true,
            clear_stashes: true,
            prune_refs: true,
            generate_handoff: true,
        }
    }
}

/// Handler for the landing sequence.
///
/// Orchestrates clean session exit with commit, cleanup, and handoff.
pub struct LandingHandler {
    context: LoopContext,
    config: LandingConfig,
}

impl LandingHandler {
    /// Creates a new landing handler for the given loop context.
    pub fn new(context: LoopContext) -> Self {
        Self {
            context,
            config: LandingConfig::default(),
        }
    }

    /// Creates a landing handler with custom configuration.
    pub fn with_config(context: LoopContext, config: LandingConfig) -> Self {
        Self { context, config }
    }

    /// Executes the landing sequence.
    ///
    /// # Arguments
    ///
    /// * `prompt` - The original prompt that started this loop
    ///
    /// # Returns
    ///
    /// A `LandingResult` with details about what was done, or an error if
    /// a critical step failed.
    pub fn land(&self, prompt: &str) -> Result<LandingResult, LandingError> {
        let workspace = self.context.workspace();
        let loop_id = self.context.loop_id().unwrap_or("primary").to_string();

        debug!(loop_id = %loop_id, "Beginning landing sequence");

        // Step 1: Verify task state
        let open_tasks = self.verify_tasks();
        if !open_tasks.is_empty() {
            warn!(
                loop_id = %loop_id,
                open_tasks = ?open_tasks,
                "Landing with {} open tasks",
                open_tasks.len()
            );
        }

        // Step 2: Auto-commit uncommitted changes
        let commit_result = if self.config.auto_commit {
            match commit_loop_changes(&self.context, &loop_id) {
                Ok(result) => {
                    if result.committed {
                        debug!(
                            loop_id = %loop_id,
                            commit = ?result.commit_sha,
                            files = result.files_staged,
                            "Auto-committed changes during landing"
                        );
                    }
                    result
                }
                Err(e) => {
                    warn!(loop_id = %loop_id, error = %e, "Auto-commit failed during landing");
                    AutoCommitResult::no_commit()
                }
            }
        } else {
            AutoCommitResult::no_commit()
        };

        // Step 3: Clean git state
        let stashes_cleared = if self.config.clear_stashes {
            match clean_stashes(workspace) {
                Ok(count) => {
                    if count > 0 {
                        debug!(loop_id = %loop_id, count, "Cleared stashes during landing");
                    }
                    count
                }
                Err(e) => {
                    warn!(loop_id = %loop_id, error = %e, "Failed to clear stashes");
                    0
                }
            }
        } else {
            0
        };

        if self.config.prune_refs
            && let Err(e) = prune_remote_refs(workspace)
        {
            warn!(loop_id = %loop_id, error = %e, "Failed to prune remote refs");
        }

        // Step 4: Generate handoff prompt
        let handoff_path = if self.config.generate_handoff {
            let writer = HandoffWriter::new(self.context.clone());
            match writer.write(prompt) {
                Ok(result) => {
                    debug!(
                        loop_id = %loop_id,
                        completed = result.completed_tasks,
                        open = result.open_tasks,
                        "Generated handoff file in Ralph state"
                    );
                    result.path
                }
                Err(e) => {
                    warn!(loop_id = %loop_id, error = %e, "Failed to generate handoff");
                    self.context.handoff_path()
                }
            }
        } else {
            self.context.handoff_path()
        };

        // Check final working tree state
        let working_tree_clean = is_working_tree_clean(workspace).unwrap_or(false);

        Ok(LandingResult {
            committed: commit_result.committed,
            commit_sha: commit_result.commit_sha,
            handoff_path,
            open_tasks,
            stashes_cleared,
            working_tree_clean,
        })
    }

    /// Verifies task state and returns list of open task IDs.
    fn verify_tasks(&self) -> Vec<String> {
        let tasks_path = self.context.tasks_path();

        match TaskStore::load(&tasks_path) {
            Ok(store) => store.open().iter().map(|t| t.id.clone()).collect(),
            Err(e) => {
                debug!(error = %e, "Could not load tasks for verification");
                Vec::new()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::task::Task;
    use crate::testing::init_test_repo;
    use std::fs;
    use std::process::Command;
    use tempfile::TempDir;

    fn setup_test_context() -> (TempDir, LoopContext) {
        let temp = TempDir::new().unwrap();
        init_test_repo(temp.path(), &[]);

        // Add .ralph/ to .gitignore so handoff files don't create uncommitted changes
        fs::write(temp.path().join(".gitignore"), ".ralph/\n").unwrap();
        Command::new("git")
            .args(["add", ".gitignore"])
            .current_dir(temp.path())
            .output()
            .unwrap();
        Command::new("git")
            .args(["commit", "-m", "Add gitignore"])
            .current_dir(temp.path())
            .output()
            .unwrap();

        let ctx = LoopContext::primary(temp.path().to_path_buf());
        ctx.ensure_directories().unwrap();
        (temp, ctx)
    }

    #[test]
    fn test_landing_clean_repo() {
        let (_temp, ctx) = setup_test_context();
        let handler = LandingHandler::new(ctx.clone());

        let result = handler.land("Test prompt").unwrap();

        assert!(!result.committed); // No changes to commit (.ralph/ is gitignored)
        assert!(result.commit_sha.is_none());
        assert!(result.open_tasks.is_empty());
        assert!(result.working_tree_clean);
        assert!(result.handoff_path.exists());
    }

    #[test]
    fn test_landing_with_uncommitted_changes() {
        let (temp, ctx) = setup_test_context();
        record_untracked_baseline(&ctx).unwrap();

        // Create uncommitted changes (outside .ralph/ which is gitignored)
        fs::write(temp.path().join("new_file.txt"), "content").unwrap();

        let handler = LandingHandler::new(ctx.clone());
        let result = handler.land("Test prompt").unwrap();

        assert!(result.committed);
        assert!(result.commit_sha.is_some());
        assert!(result.working_tree_clean);
    }

    #[test]
    fn test_landing_without_baseline_commits_tracked_changes_only() {
        let (temp, ctx) = setup_test_context();
        fs::write(temp.path().join("README.md"), "# Test\nedited\n").unwrap();
        fs::write(temp.path().join("unknown_origin.txt"), "content").unwrap();

        let result = LandingHandler::new(ctx.clone())
            .land("Test prompt")
            .unwrap();

        assert!(result.committed);
        let committed = committed_paths(temp.path());
        assert!(
            !committed.contains(&"unknown_origin.txt".to_string()),
            "without a baseline, untracked files must not be swept in: {committed:?}"
        );
        assert!(!result.working_tree_clean);
    }

    #[test]
    fn test_baseline_lists_itself_when_ralph_dir_is_not_ignored() {
        let temp = TempDir::new().unwrap();
        init_test_repo(temp.path(), &[]);
        let ctx = LoopContext::primary(temp.path().to_path_buf());
        ctx.ensure_directories().unwrap();
        record_untracked_baseline(&ctx).unwrap();

        fs::write(temp.path().join("feature.rs"), "fn f() {}\n").unwrap();
        let result = commit_loop_changes(&ctx, "primary").unwrap();

        assert!(result.committed);
        let committed = committed_paths(temp.path());
        assert!(
            committed.contains(&"feature.rs".to_string()),
            "{committed:?}"
        );
        assert!(
            !committed.contains(&".ralph/landing-untracked-baseline.json".to_string()),
            "{committed:?}"
        );
    }

    fn committed_paths(repo: &std::path::Path) -> Vec<String> {
        let output = Command::new("git")
            .args(["ls-tree", "-r", "--name-only", "HEAD"])
            .current_dir(repo)
            .output()
            .unwrap();
        String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(str::to_owned)
            .collect()
    }

    #[test]
    fn test_landing_does_not_commit_untracked_files_that_predate_the_loop() {
        let (temp, ctx) = setup_test_context();
        // An operator-local file already sitting in the workspace at loop start.
        fs::write(temp.path().join("ralph.operator.yml"), "backend: local\n").unwrap();
        record_untracked_baseline(&ctx).unwrap();

        // The loop's own work: a new file and an edit to a tracked file.
        fs::write(temp.path().join("feature.rs"), "fn feature() {}\n").unwrap();
        fs::write(temp.path().join("README.md"), "# Test\nedited\n").unwrap();

        let result = LandingHandler::new(ctx.clone())
            .land("Test prompt")
            .unwrap();

        assert!(result.committed);
        let committed = committed_paths(temp.path());
        assert!(
            committed.contains(&"feature.rs".to_string()),
            "{committed:?}"
        );
        assert!(
            !committed.contains(&"ralph.operator.yml".to_string()),
            "a pre-existing untracked operator file was swept into the landing commit: {committed:?}"
        );
        assert!(temp.path().join("ralph.operator.yml").exists());
        let readme = Command::new("git")
            .args(["show", "HEAD:README.md"])
            .current_dir(temp.path())
            .output()
            .unwrap();
        assert!(String::from_utf8_lossy(&readme.stdout).contains("edited"));
    }

    #[test]
    fn test_landing_with_open_tasks() {
        let (_temp, ctx) = setup_test_context();

        // Create an open task
        let mut store = TaskStore::load(&ctx.tasks_path()).unwrap();
        let task = Task::new("Open task".to_string(), 1);
        store.add(task);
        store.save().unwrap();

        let handler = LandingHandler::new(ctx.clone());
        let result = handler.land("Test prompt").unwrap();

        assert_eq!(result.open_tasks.len(), 1);
    }

    #[test]
    fn test_landing_with_stashes() {
        let (temp, ctx) = setup_test_context();

        // Create a stash
        fs::write(temp.path().join("README.md"), "# Modified").unwrap();
        Command::new("git")
            .args(["stash", "push", "-m", "test stash"])
            .current_dir(temp.path())
            .output()
            .unwrap();

        let handler = LandingHandler::new(ctx.clone());
        let result = handler.land("Test prompt").unwrap();

        assert_eq!(result.stashes_cleared, 1);
    }

    #[test]
    fn test_landing_config_disables_features() {
        let (temp, ctx) = setup_test_context();

        // Create uncommitted changes
        fs::write(temp.path().join("new_file.txt"), "content").unwrap();

        let config = LandingConfig {
            auto_commit: false,
            clear_stashes: false,
            prune_refs: false,
            generate_handoff: false,
        };

        let handler = LandingHandler::with_config(ctx.clone(), config);
        let result = handler.land("Test prompt").unwrap();

        assert!(!result.committed); // Auto-commit disabled
        assert!(!result.working_tree_clean); // Changes still there
    }

    #[test]
    fn test_landing_generates_handoff_content() {
        let (_temp, ctx) = setup_test_context();

        // Create some tasks
        let mut store = TaskStore::load(&ctx.tasks_path()).unwrap();
        let task1 = Task::new("Completed task".to_string(), 1);
        let id1 = task1.id.clone();
        store.add(task1);
        store.close(&id1);

        let task2 = Task::new("Open task".to_string(), 2);
        store.add(task2);
        store.save().unwrap();

        let handler = LandingHandler::new(ctx.clone());
        let result = handler.land("Original prompt here").unwrap();

        let content = fs::read_to_string(&result.handoff_path).unwrap();

        assert!(content.contains("Session Handoff"));
        assert!(content.contains("[x] Completed task"));
        assert!(content.contains("[ ] Open task"));
        assert!(content.contains("Original prompt here"));
    }

    #[test]
    fn test_worktree_landing() {
        let temp = TempDir::new().unwrap();
        let repo_root = temp.path().to_path_buf();
        init_test_repo(&repo_root, &[]);

        // Create .ralph directories
        fs::create_dir_all(repo_root.join(".ralph/agent")).unwrap();

        let worktree_path = repo_root.join(".worktrees/ralph-test-1234");
        fs::create_dir_all(&worktree_path).unwrap();

        // Create a worktree context
        let ctx =
            LoopContext::worktree("ralph-test-1234", worktree_path.clone(), repo_root.clone());

        // Need to ensure directories exist for the worktree context
        ctx.ensure_directories().unwrap();

        let handler = LandingHandler::new(ctx.clone());
        let result = handler.land("Worktree prompt").unwrap();

        // Handoff should be in the worktree's agent dir
        assert!(result.handoff_path.to_string_lossy().contains(".worktrees"));
    }
}
