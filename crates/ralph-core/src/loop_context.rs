//! Loop context for path resolution in multi-loop scenarios.
//!
//! When running multiple Ralph loops concurrently, each loop needs its own
//! isolated paths for state files (events, tasks, scratchpad) while sharing
//! memories across loops for cross-loop learning.
//!
//! # Design
//!
//! - **Primary loop**: Runs in the main workspace, paths resolve to standard locations
//! - **Worktree loop**: Runs in a git worktree, paths resolve to worktree-local locations
//! - **Shared memories**: Memories are symlinked in worktrees, pointing to main workspace
//!
//! # Example
//!
//! ```
//! use ralph_core::loop_context::LoopContext;
//! use std::path::PathBuf;
//!
//! // Primary loop runs in current directory
//! let primary = LoopContext::primary(PathBuf::from("/project"));
//! assert_eq!(primary.events_path().to_string_lossy(), "/project/.ralph/events.jsonl");
//! assert_eq!(primary.tasks_path().to_string_lossy(), "/project/.agent/tasks.jsonl");
//!
//! // Worktree loop runs in isolated directory
//! let worktree = LoopContext::worktree(
//!     "loop-1234-abcd",
//!     PathBuf::from("/project/.worktrees/loop-1234-abcd"),
//!     PathBuf::from("/project"),
//! );
//! assert_eq!(worktree.events_path().to_string_lossy(),
//!            "/project/.worktrees/loop-1234-abcd/.ralph/events.jsonl");
//! ```

use std::path::{Path, PathBuf};

/// Context for resolving paths within a Ralph loop.
///
/// Encapsulates the working directory and loop identity, providing
/// consistent path resolution for all loop-local state files.
#[derive(Debug, Clone)]
pub struct LoopContext {
    /// The loop identifier (None for primary loop).
    loop_id: Option<String>,

    /// Working directory for this loop.
    /// For primary: the repo root.
    /// For worktree: the worktree directory.
    workspace: PathBuf,

    /// The main repo root (for memory symlink target).
    /// Same as workspace for primary loops.
    repo_root: PathBuf,

    /// Whether this is the primary loop (holds loop.lock).
    is_primary: bool,
}

impl LoopContext {
    /// Creates context for the primary loop running in the main workspace.
    ///
    /// The primary loop holds the loop lock and runs directly in the
    /// repository root without filesystem isolation.
    pub fn primary(workspace: PathBuf) -> Self {
        Self {
            loop_id: None,
            repo_root: workspace.clone(),
            workspace,
            is_primary: true,
        }
    }

    /// Creates context for a worktree-based loop.
    ///
    /// Worktree loops run in isolated git worktrees with their own
    /// `.ralph/` and `.agent/` directories, but share memories via symlink.
    ///
    /// # Arguments
    ///
    /// * `loop_id` - Unique identifier for this loop (e.g., "loop-1234-abcd")
    /// * `worktree_path` - Path to the worktree directory
    /// * `repo_root` - Path to the main repository root (for memory symlink)
    pub fn worktree(
        loop_id: impl Into<String>,
        worktree_path: PathBuf,
        repo_root: PathBuf,
    ) -> Self {
        Self {
            loop_id: Some(loop_id.into()),
            workspace: worktree_path,
            repo_root,
            is_primary: false,
        }
    }

    /// Returns the loop identifier, if any.
    ///
    /// Primary loops return None; worktree loops return their unique ID.
    pub fn loop_id(&self) -> Option<&str> {
        self.loop_id.as_deref()
    }

    /// Returns true if this is the primary loop.
    pub fn is_primary(&self) -> bool {
        self.is_primary
    }

    /// Returns the workspace root for this loop.
    ///
    /// This is the directory where the loop executes:
    /// - Primary: the repo root
    /// - Worktree: the worktree directory
    pub fn workspace(&self) -> &Path {
        &self.workspace
    }

    /// Returns the main repository root.
    ///
    /// For worktree loops, this is different from `workspace()` and
    /// is used to locate shared resources like the main memories file.
    pub fn repo_root(&self) -> &Path {
        &self.repo_root
    }

    // -------------------------------------------------------------------------
    // Path resolution methods
    // -------------------------------------------------------------------------

    /// Path to the `.ralph/` directory for this loop.
    pub fn ralph_dir(&self) -> PathBuf {
        self.workspace.join(".ralph")
    }

    /// Path to the `.agent/` directory for this loop.
    pub fn agent_dir(&self) -> PathBuf {
        self.workspace.join(".agent")
    }

    /// Path to the events JSONL file.
    ///
    /// Each loop has its own isolated events file.
    pub fn events_path(&self) -> PathBuf {
        self.ralph_dir().join("events.jsonl")
    }

    /// Path to the current-events marker file.
    ///
    /// This file contains the path to the active events file.
    pub fn current_events_marker(&self) -> PathBuf {
        self.ralph_dir().join("current-events")
    }

    /// Path to the tasks JSONL file.
    ///
    /// Each loop has its own isolated tasks file.
    pub fn tasks_path(&self) -> PathBuf {
        self.agent_dir().join("tasks.jsonl")
    }

    /// Path to the scratchpad markdown file.
    ///
    /// Each loop has its own isolated scratchpad.
    pub fn scratchpad_path(&self) -> PathBuf {
        self.agent_dir().join("scratchpad.md")
    }

    /// Path to the memories markdown file.
    ///
    /// For primary loops, this is the actual memories file.
    /// For worktree loops, this is a symlink to the main repo's memories.
    pub fn memories_path(&self) -> PathBuf {
        self.agent_dir().join("memories.md")
    }

    /// Path to the main repository's memories file.
    ///
    /// Used to create symlinks in worktree loops.
    pub fn main_memories_path(&self) -> PathBuf {
        self.repo_root.join(".agent").join("memories.md")
    }

    /// Path to the summary markdown file.
    ///
    /// Each loop has its own isolated summary.
    pub fn summary_path(&self) -> PathBuf {
        self.agent_dir().join("summary.md")
    }

    /// Path to the diagnostics directory.
    ///
    /// Each loop has its own diagnostics output.
    pub fn diagnostics_dir(&self) -> PathBuf {
        self.ralph_dir().join("diagnostics")
    }

    /// Path to the loop history JSONL file.
    ///
    /// Event-sourced history for crash recovery and debugging.
    pub fn history_path(&self) -> PathBuf {
        self.ralph_dir().join("history.jsonl")
    }

    /// Path to the loop lock file (only meaningful for primary loop detection).
    pub fn loop_lock_path(&self) -> PathBuf {
        // Lock is always in the main repo root
        self.repo_root.join(".ralph").join("loop.lock")
    }

    /// Path to the merge queue JSONL file.
    ///
    /// The merge queue is shared across all loops (in main repo).
    pub fn merge_queue_path(&self) -> PathBuf {
        self.repo_root.join(".ralph").join("merge-queue.jsonl")
    }

    /// Path to the loop registry JSON file.
    ///
    /// The registry is shared across all loops (in main repo).
    pub fn loop_registry_path(&self) -> PathBuf {
        self.repo_root.join(".ralph").join("loops.json")
    }

    // -------------------------------------------------------------------------
    // Directory management
    // -------------------------------------------------------------------------

    /// Ensures the `.ralph/` directory exists.
    pub fn ensure_ralph_dir(&self) -> std::io::Result<()> {
        std::fs::create_dir_all(self.ralph_dir())
    }

    /// Ensures the `.agent/` directory exists.
    pub fn ensure_agent_dir(&self) -> std::io::Result<()> {
        std::fs::create_dir_all(self.agent_dir())
    }

    /// Ensures both `.ralph/` and `.agent/` directories exist.
    pub fn ensure_directories(&self) -> std::io::Result<()> {
        self.ensure_ralph_dir()?;
        self.ensure_agent_dir()
    }

    /// Creates the memory symlink in a worktree pointing to main repo.
    ///
    /// This is only relevant for worktree loops. For primary loops,
    /// this is a no-op.
    ///
    /// # Returns
    ///
    /// - `Ok(true)` - Symlink was created
    /// - `Ok(false)` - Already exists or is primary loop
    /// - `Err(_)` - Symlink creation failed
    #[cfg(unix)]
    pub fn setup_memory_symlink(&self) -> std::io::Result<bool> {
        if self.is_primary {
            return Ok(false);
        }

        let memories_path = self.memories_path();
        let main_memories = self.main_memories_path();

        // Skip if already exists (symlink or file)
        if memories_path.exists() || memories_path.is_symlink() {
            return Ok(false);
        }

        // Ensure parent directory exists
        self.ensure_agent_dir()?;

        // Create symlink
        std::os::unix::fs::symlink(&main_memories, &memories_path)?;
        Ok(true)
    }

    /// Creates the memory symlink in a worktree (non-Unix stub).
    #[cfg(not(unix))]
    pub fn setup_memory_symlink(&self) -> std::io::Result<bool> {
        // On non-Unix platforms, we don't create symlinks
        // (worktree mode not supported)
        Ok(false)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_primary_context() {
        let ctx = LoopContext::primary(PathBuf::from("/project"));

        assert!(ctx.is_primary());
        assert!(ctx.loop_id().is_none());
        assert_eq!(ctx.workspace(), Path::new("/project"));
        assert_eq!(ctx.repo_root(), Path::new("/project"));
    }

    #[test]
    fn test_worktree_context() {
        let ctx = LoopContext::worktree(
            "loop-1234-abcd",
            PathBuf::from("/project/.worktrees/loop-1234-abcd"),
            PathBuf::from("/project"),
        );

        assert!(!ctx.is_primary());
        assert_eq!(ctx.loop_id(), Some("loop-1234-abcd"));
        assert_eq!(
            ctx.workspace(),
            Path::new("/project/.worktrees/loop-1234-abcd")
        );
        assert_eq!(ctx.repo_root(), Path::new("/project"));
    }

    #[test]
    fn test_primary_path_resolution() {
        let ctx = LoopContext::primary(PathBuf::from("/project"));

        assert_eq!(ctx.ralph_dir(), PathBuf::from("/project/.ralph"));
        assert_eq!(ctx.agent_dir(), PathBuf::from("/project/.agent"));
        assert_eq!(
            ctx.events_path(),
            PathBuf::from("/project/.ralph/events.jsonl")
        );
        assert_eq!(
            ctx.tasks_path(),
            PathBuf::from("/project/.agent/tasks.jsonl")
        );
        assert_eq!(
            ctx.scratchpad_path(),
            PathBuf::from("/project/.agent/scratchpad.md")
        );
        assert_eq!(
            ctx.memories_path(),
            PathBuf::from("/project/.agent/memories.md")
        );
        assert_eq!(
            ctx.summary_path(),
            PathBuf::from("/project/.agent/summary.md")
        );
        assert_eq!(
            ctx.diagnostics_dir(),
            PathBuf::from("/project/.ralph/diagnostics")
        );
        assert_eq!(
            ctx.history_path(),
            PathBuf::from("/project/.ralph/history.jsonl")
        );
    }

    #[test]
    fn test_worktree_path_resolution() {
        let ctx = LoopContext::worktree(
            "loop-1234-abcd",
            PathBuf::from("/project/.worktrees/loop-1234-abcd"),
            PathBuf::from("/project"),
        );

        // Loop-local paths resolve to worktree
        assert_eq!(
            ctx.ralph_dir(),
            PathBuf::from("/project/.worktrees/loop-1234-abcd/.ralph")
        );
        assert_eq!(
            ctx.events_path(),
            PathBuf::from("/project/.worktrees/loop-1234-abcd/.ralph/events.jsonl")
        );
        assert_eq!(
            ctx.tasks_path(),
            PathBuf::from("/project/.worktrees/loop-1234-abcd/.agent/tasks.jsonl")
        );
        assert_eq!(
            ctx.scratchpad_path(),
            PathBuf::from("/project/.worktrees/loop-1234-abcd/.agent/scratchpad.md")
        );

        // Memories path is in worktree (symlink to main repo)
        assert_eq!(
            ctx.memories_path(),
            PathBuf::from("/project/.worktrees/loop-1234-abcd/.agent/memories.md")
        );

        // Main memories path is in repo root
        assert_eq!(
            ctx.main_memories_path(),
            PathBuf::from("/project/.agent/memories.md")
        );

        // Shared resources resolve to main repo
        assert_eq!(
            ctx.loop_lock_path(),
            PathBuf::from("/project/.ralph/loop.lock")
        );
        assert_eq!(
            ctx.merge_queue_path(),
            PathBuf::from("/project/.ralph/merge-queue.jsonl")
        );
        assert_eq!(
            ctx.loop_registry_path(),
            PathBuf::from("/project/.ralph/loops.json")
        );
    }

    #[test]
    fn test_ensure_directories() {
        let temp = TempDir::new().unwrap();
        let ctx = LoopContext::primary(temp.path().to_path_buf());

        // Directories don't exist initially
        assert!(!ctx.ralph_dir().exists());
        assert!(!ctx.agent_dir().exists());

        // Create them
        ctx.ensure_directories().unwrap();

        // Now they exist
        assert!(ctx.ralph_dir().exists());
        assert!(ctx.agent_dir().exists());
    }

    #[cfg(unix)]
    #[test]
    fn test_memory_symlink_primary_noop() {
        let temp = TempDir::new().unwrap();
        let ctx = LoopContext::primary(temp.path().to_path_buf());

        // Primary loop doesn't create symlinks
        let created = ctx.setup_memory_symlink().unwrap();
        assert!(!created);
    }

    #[cfg(unix)]
    #[test]
    fn test_memory_symlink_worktree() {
        let temp = TempDir::new().unwrap();
        let repo_root = temp.path().to_path_buf();
        let worktree_path = repo_root.join(".worktrees/loop-1234");

        // Create the main memories file
        std::fs::create_dir_all(repo_root.join(".agent")).unwrap();
        std::fs::write(repo_root.join(".agent/memories.md"), "# Memories\n").unwrap();

        let ctx = LoopContext::worktree("loop-1234", worktree_path.clone(), repo_root.clone());

        // Create symlink
        ctx.ensure_agent_dir().unwrap();
        let created = ctx.setup_memory_symlink().unwrap();
        assert!(created);

        // Verify symlink exists and points to main memories
        let memories = ctx.memories_path();
        assert!(memories.is_symlink());
        assert_eq!(
            std::fs::read_link(&memories).unwrap(),
            ctx.main_memories_path()
        );

        // Second call is a no-op
        let created_again = ctx.setup_memory_symlink().unwrap();
        assert!(!created_again);
    }

    #[test]
    fn test_current_events_marker() {
        let ctx = LoopContext::primary(PathBuf::from("/project"));
        assert_eq!(
            ctx.current_events_marker(),
            PathBuf::from("/project/.ralph/current-events")
        );
    }
}
