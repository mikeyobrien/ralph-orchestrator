//! Loop context for path resolution in multi-loop scenarios.
//!
//! When running multiple Hats loops concurrently, each loop needs its own
//! isolated paths for state files (events, tasks, scratchpad) while sharing
//! memories across loops for cross-loop learning.
//!
//! # Design
//!
//! - **Primary loop**: Runs in the main workspace, paths resolve to standard locations
//! - **Worktree loop**: Runs in a git worktree, paths resolve to worktree-local locations
//! - **Shared memories**: Memories are symlinked in worktrees, pointing to main workspace
//! - **Shared specs/tasks**: Specs and code tasks are symlinked in worktrees
//!
//! # Directory Structure
//!
//! All Hats state is consolidated under `.hats/`:
//! ```text
//! .hats/
//! ├── agent/                    # Agent state (memories, tasks, scratchpad)
//! │   ├── memories.md           # Symlinked in worktrees
//! │   ├── tasks.jsonl           # Isolated per worktree
//! │   ├── scratchpad.md         # Isolated per worktree
//! │   └── context.md            # Worktree metadata (worktrees only)
//! ├── specs/                    # Specification files (symlinked in worktrees)
//! ├── tasks/                    # Code task files (symlinked in worktrees)
//! ├── loop.lock
//! ├── loops.json
//! ├── merge-queue.jsonl
//! ├── events.jsonl
//! ├── current-events
//! ├── history.jsonl
//! ├── diagnostics/
//! └── planning-sessions/
//! ```
//!
//! # Example
//!
//! ```
//! use hats_core::loop_context::LoopContext;
//! use std::path::PathBuf;
//!
//! // Primary loop runs in current directory
//! let primary = LoopContext::primary(PathBuf::from("/project"));
//! assert_eq!(primary.events_path().to_string_lossy(), "/project/.hats/events.jsonl");
//! assert_eq!(primary.tasks_path().to_string_lossy(), "/project/.hats/agent/tasks.jsonl");
//!
//! // Worktree loop runs in isolated directory
//! let worktree = LoopContext::worktree(
//!     "loop-1234-abcd",
//!     PathBuf::from("/project/.worktrees/loop-1234-abcd"),
//!     PathBuf::from("/project"),
//! );
//! assert_eq!(worktree.events_path().to_string_lossy(),
//!            "/project/.worktrees/loop-1234-abcd/.hats/events.jsonl");
//! ```

use std::path::{Path, PathBuf};

/// Context for resolving paths within a Hats loop.
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
    /// `.hats/` directory, but share memories, specs, and code tasks via symlink.
    ///
    /// # Arguments
    ///
    /// * `loop_id` - Unique identifier for this loop (e.g., "loop-1234-abcd")
    /// * `worktree_path` - Path to the worktree directory
    /// * `repo_root` - Path to the main repository root (for symlinks)
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

    /// Path to the `.hats/` directory for this loop.
    pub fn hats_dir(&self) -> PathBuf {
        self.workspace.join(".hats")
    }

    /// Path to the `.hats/agent/` directory for this loop.
    ///
    /// This directory contains agent state: memories, tasks, scratchpad, etc.
    pub fn agent_dir(&self) -> PathBuf {
        self.hats_dir().join("agent")
    }

    /// Path to the events JSONL file.
    ///
    /// Each loop has its own isolated events file.
    pub fn events_path(&self) -> PathBuf {
        self.hats_dir().join("events.jsonl")
    }

    /// Path to the current-events marker file.
    ///
    /// This file contains the path to the active events file.
    pub fn current_events_marker(&self) -> PathBuf {
        self.hats_dir().join("current-events")
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
        self.repo_root
            .join(".hats")
            .join("agent")
            .join("memories.md")
    }

    /// Path to the context markdown file.
    ///
    /// This file contains worktree metadata (loop ID, workspace, branch, etc.)
    /// and is only created in worktree loops.
    pub fn context_path(&self) -> PathBuf {
        self.agent_dir().join("context.md")
    }

    /// Path to the specs directory for this loop.
    ///
    /// For primary loops, this is the actual specs directory.
    /// For worktree loops, this is a symlink to the main repo's specs.
    pub fn specs_dir(&self) -> PathBuf {
        self.hats_dir().join("specs")
    }

    /// Path to the code tasks directory for this loop.
    ///
    /// For primary loops, this is the actual code tasks directory.
    /// For worktree loops, this is a symlink to the main repo's code tasks.
    /// Note: This is different from tasks_path() which is for runtime task tracking.
    pub fn code_tasks_dir(&self) -> PathBuf {
        self.hats_dir().join("tasks")
    }

    /// Path to the main repository's specs directory.
    ///
    /// Used to create symlinks in worktree loops.
    pub fn main_specs_dir(&self) -> PathBuf {
        self.repo_root.join(".hats").join("specs")
    }

    /// Path to the main repository's code tasks directory.
    ///
    /// Used to create symlinks in worktree loops.
    pub fn main_code_tasks_dir(&self) -> PathBuf {
        self.repo_root.join(".hats").join("tasks")
    }

    /// Path to the summary markdown file.
    ///
    /// Each loop has its own isolated summary.
    pub fn summary_path(&self) -> PathBuf {
        self.agent_dir().join("summary.md")
    }

    /// Path to the handoff markdown file.
    ///
    /// Generated on loop completion to provide context for the next session.
    /// Contains completed tasks, remaining work, and a ready-to-paste prompt.
    pub fn handoff_path(&self) -> PathBuf {
        self.agent_dir().join("handoff.md")
    }

    /// Path to the diagnostics directory.
    ///
    /// Each loop has its own diagnostics output.
    pub fn diagnostics_dir(&self) -> PathBuf {
        self.hats_dir().join("diagnostics")
    }

    /// Path to the loop history JSONL file.
    ///
    /// Event-sourced history for crash recovery and debugging.
    pub fn history_path(&self) -> PathBuf {
        self.hats_dir().join("history.jsonl")
    }

    /// Path to the loop lock file (only meaningful for primary loop detection).
    pub fn loop_lock_path(&self) -> PathBuf {
        // Lock is always in the main repo root
        self.repo_root.join(".hats").join("loop.lock")
    }

    /// Path to the merge queue JSONL file.
    ///
    /// The merge queue is shared across all loops (in main repo).
    pub fn merge_queue_path(&self) -> PathBuf {
        self.repo_root.join(".hats").join("merge-queue.jsonl")
    }

    /// Path to the loop registry JSON file.
    ///
    /// The registry is shared across all loops (in main repo).
    pub fn loop_registry_path(&self) -> PathBuf {
        self.repo_root.join(".hats").join("loops.json")
    }

    /// Path to the planning sessions directory.
    ///
    /// Contains all planning session subdirectories.
    pub fn planning_sessions_dir(&self) -> PathBuf {
        self.hats_dir().join("planning-sessions")
    }

    /// Path to a specific planning session directory.
    ///
    /// # Arguments
    ///
    /// * `id` - The session ID (e.g., "20260127-143022-a7f2")
    pub fn planning_session_dir(&self, id: &str) -> PathBuf {
        self.planning_sessions_dir().join(id)
    }

    /// Path to the conversation file for a planning session.
    ///
    /// # Arguments
    ///
    /// * `id` - The session ID
    pub fn planning_conversation_path(&self, id: &str) -> PathBuf {
        self.planning_session_dir(id).join("conversation.jsonl")
    }

    /// Path to the session metadata file for a planning session.
    ///
    /// # Arguments
    ///
    /// * `id` - The session ID
    pub fn planning_session_metadata_path(&self, id: &str) -> PathBuf {
        self.planning_session_dir(id).join("session.json")
    }

    /// Path to the artifacts directory for a planning session.
    ///
    /// # Arguments
    ///
    /// * `id` - The session ID
    pub fn planning_artifacts_dir(&self, id: &str) -> PathBuf {
        self.planning_session_dir(id).join("artifacts")
    }

    // -------------------------------------------------------------------------
    // Directory management
    // -------------------------------------------------------------------------

    /// Ensures the `.hats/` directory exists.
    pub fn ensure_hats_dir(&self) -> std::io::Result<()> {
        std::fs::create_dir_all(self.hats_dir())
    }

    /// Ensures the `.hats/agent/` directory exists.
    pub fn ensure_agent_dir(&self) -> std::io::Result<()> {
        std::fs::create_dir_all(self.agent_dir())
    }

    /// Ensures the `.hats/specs/` directory exists.
    pub fn ensure_specs_dir(&self) -> std::io::Result<()> {
        std::fs::create_dir_all(self.specs_dir())
    }

    /// Ensures the `.hats/tasks/` directory exists.
    pub fn ensure_code_tasks_dir(&self) -> std::io::Result<()> {
        std::fs::create_dir_all(self.code_tasks_dir())
    }

    /// Ensures all required directories exist.
    pub fn ensure_directories(&self) -> std::io::Result<()> {
        self.ensure_hats_dir()?;
        self.ensure_agent_dir()?;
        // Note: specs_dir and code_tasks_dir are optional (may be symlinks in worktrees)
        Ok(())
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

    /// Creates the specs symlink in a worktree pointing to main repo.
    ///
    /// This allows worktree loops to access specs from the main repo,
    /// even when they are untracked (not committed to git).
    ///
    /// # Returns
    ///
    /// - `Ok(true)` - Symlink was created
    /// - `Ok(false)` - Already exists or is primary loop
    /// - `Err(_)` - Symlink creation failed
    #[cfg(unix)]
    pub fn setup_specs_symlink(&self) -> std::io::Result<bool> {
        if self.is_primary {
            return Ok(false);
        }

        let specs_path = self.specs_dir();
        let main_specs = self.main_specs_dir();

        // Skip if already exists (symlink or directory)
        if specs_path.exists() || specs_path.is_symlink() {
            return Ok(false);
        }

        // Ensure parent directory exists
        self.ensure_hats_dir()?;

        // Create symlink
        std::os::unix::fs::symlink(&main_specs, &specs_path)?;
        Ok(true)
    }

    /// Creates the specs symlink in a worktree (non-Unix stub).
    #[cfg(not(unix))]
    pub fn setup_specs_symlink(&self) -> std::io::Result<bool> {
        Ok(false)
    }

    /// Creates the code tasks symlink in a worktree pointing to main repo.
    ///
    /// This allows worktree loops to access code task files from the main repo,
    /// even when they are untracked (not committed to git).
    ///
    /// # Returns
    ///
    /// - `Ok(true)` - Symlink was created
    /// - `Ok(false)` - Already exists or is primary loop
    /// - `Err(_)` - Symlink creation failed
    #[cfg(unix)]
    pub fn setup_code_tasks_symlink(&self) -> std::io::Result<bool> {
        if self.is_primary {
            return Ok(false);
        }

        let tasks_path = self.code_tasks_dir();
        let main_tasks = self.main_code_tasks_dir();

        // Skip if already exists (symlink or directory)
        if tasks_path.exists() || tasks_path.is_symlink() {
            return Ok(false);
        }

        // Ensure parent directory exists
        self.ensure_hats_dir()?;

        // Create symlink
        std::os::unix::fs::symlink(&main_tasks, &tasks_path)?;
        Ok(true)
    }

    /// Creates the code tasks symlink in a worktree (non-Unix stub).
    #[cfg(not(unix))]
    pub fn setup_code_tasks_symlink(&self) -> std::io::Result<bool> {
        Ok(false)
    }

    /// Generates a context.md file in the worktree with metadata.
    ///
    /// This file tells the agent it's running in a worktree and provides
    /// information about the worktree context (loop ID, workspace, branch, etc.)
    ///
    /// # Arguments
    ///
    /// * `branch` - The git branch name for this worktree
    /// * `prompt` - The prompt that started this loop
    ///
    /// # Returns
    ///
    /// - `Ok(true)` - Context file was created
    /// - `Ok(false)` - Already exists or is primary loop
    /// - `Err(_)` - File creation failed
    pub fn generate_context_file(&self, branch: &str, prompt: &str) -> std::io::Result<bool> {
        if self.is_primary {
            return Ok(false);
        }

        let context_path = self.context_path();

        // Skip if already exists
        if context_path.exists() {
            return Ok(false);
        }

        // Ensure parent directory exists
        self.ensure_agent_dir()?;

        let loop_id = self.loop_id().unwrap_or("unknown");
        let created = chrono::Utc::now().to_rfc3339();

        // Truncate prompt for readability
        let prompt_preview = if prompt.len() > 200 {
            format!("{}...", &prompt[..200])
        } else {
            prompt.to_string()
        };

        let content = format!(
            r#"# Worktree Context

- **Loop ID**: {}
- **Workspace**: {}
- **Main Repo**: {}
- **Branch**: {}
- **Created**: {}
- **Prompt**: "{}"

## Notes

This is a worktree-based parallel loop. The following resources are symlinked
to the main repository:

- `.hats/agent/memories.md` → shared memories
- `.hats/specs/` → shared specifications
- `.hats/tasks/` → shared code task files

Local state (scratchpad, runtime tasks, events) is isolated to this worktree.
"#,
            loop_id,
            self.workspace.display(),
            self.repo_root.display(),
            branch,
            created,
            prompt_preview
        );

        std::fs::write(&context_path, content)?;
        Ok(true)
    }

    /// Sets up all worktree symlinks (memories, specs, code tasks).
    ///
    /// Convenience method that calls all setup_*_symlink methods.
    /// Only relevant for worktree loops - no-op for primary loops.
    #[cfg(unix)]
    pub fn setup_worktree_symlinks(&self) -> std::io::Result<()> {
        self.setup_memory_symlink()?;
        self.setup_specs_symlink()?;
        self.setup_code_tasks_symlink()?;
        Ok(())
    }

    /// Sets up all worktree symlinks (non-Unix stub).
    #[cfg(not(unix))]
    pub fn setup_worktree_symlinks(&self) -> std::io::Result<()> {
        Ok(())
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

        assert_eq!(ctx.hats_dir(), PathBuf::from("/project/.hats"));
        assert_eq!(ctx.agent_dir(), PathBuf::from("/project/.hats/agent"));
        assert_eq!(
            ctx.events_path(),
            PathBuf::from("/project/.hats/events.jsonl")
        );
        assert_eq!(
            ctx.tasks_path(),
            PathBuf::from("/project/.hats/agent/tasks.jsonl")
        );
        assert_eq!(
            ctx.scratchpad_path(),
            PathBuf::from("/project/.hats/agent/scratchpad.md")
        );
        assert_eq!(
            ctx.memories_path(),
            PathBuf::from("/project/.hats/agent/memories.md")
        );
        assert_eq!(
            ctx.summary_path(),
            PathBuf::from("/project/.hats/agent/summary.md")
        );
        assert_eq!(
            ctx.handoff_path(),
            PathBuf::from("/project/.hats/agent/handoff.md")
        );
        assert_eq!(ctx.specs_dir(), PathBuf::from("/project/.hats/specs"));
        assert_eq!(ctx.code_tasks_dir(), PathBuf::from("/project/.hats/tasks"));
        assert_eq!(
            ctx.diagnostics_dir(),
            PathBuf::from("/project/.hats/diagnostics")
        );
        assert_eq!(
            ctx.history_path(),
            PathBuf::from("/project/.hats/history.jsonl")
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
            ctx.hats_dir(),
            PathBuf::from("/project/.worktrees/loop-1234-abcd/.hats")
        );
        assert_eq!(
            ctx.agent_dir(),
            PathBuf::from("/project/.worktrees/loop-1234-abcd/.hats/agent")
        );
        assert_eq!(
            ctx.events_path(),
            PathBuf::from("/project/.worktrees/loop-1234-abcd/.hats/events.jsonl")
        );
        assert_eq!(
            ctx.tasks_path(),
            PathBuf::from("/project/.worktrees/loop-1234-abcd/.hats/agent/tasks.jsonl")
        );
        assert_eq!(
            ctx.scratchpad_path(),
            PathBuf::from("/project/.worktrees/loop-1234-abcd/.hats/agent/scratchpad.md")
        );

        // Memories path is in worktree (symlink to main repo)
        assert_eq!(
            ctx.memories_path(),
            PathBuf::from("/project/.worktrees/loop-1234-abcd/.hats/agent/memories.md")
        );

        // Main memories path is in repo root
        assert_eq!(
            ctx.main_memories_path(),
            PathBuf::from("/project/.hats/agent/memories.md")
        );

        // Specs and code tasks paths (symlinks to main repo)
        assert_eq!(
            ctx.specs_dir(),
            PathBuf::from("/project/.worktrees/loop-1234-abcd/.hats/specs")
        );
        assert_eq!(
            ctx.code_tasks_dir(),
            PathBuf::from("/project/.worktrees/loop-1234-abcd/.hats/tasks")
        );
        assert_eq!(ctx.main_specs_dir(), PathBuf::from("/project/.hats/specs"));
        assert_eq!(
            ctx.main_code_tasks_dir(),
            PathBuf::from("/project/.hats/tasks")
        );

        // Shared resources resolve to main repo
        assert_eq!(
            ctx.loop_lock_path(),
            PathBuf::from("/project/.hats/loop.lock")
        );
        assert_eq!(
            ctx.merge_queue_path(),
            PathBuf::from("/project/.hats/merge-queue.jsonl")
        );
        assert_eq!(
            ctx.loop_registry_path(),
            PathBuf::from("/project/.hats/loops.json")
        );
    }

    #[test]
    fn test_ensure_directories() {
        let temp = TempDir::new().unwrap();
        let ctx = LoopContext::primary(temp.path().to_path_buf());

        // Directories don't exist initially
        assert!(!ctx.hats_dir().exists());
        assert!(!ctx.agent_dir().exists());

        // Create them
        ctx.ensure_directories().unwrap();

        // Now they exist
        assert!(ctx.hats_dir().exists());
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

        // Create the main memories file under .hats/agent/
        std::fs::create_dir_all(repo_root.join(".hats/agent")).unwrap();
        std::fs::write(repo_root.join(".hats/agent/memories.md"), "# Memories\n").unwrap();

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
            PathBuf::from("/project/.hats/current-events")
        );
    }

    #[test]
    fn test_planning_sessions_paths() {
        let ctx = LoopContext::primary(PathBuf::from("/project"));

        assert_eq!(
            ctx.planning_sessions_dir(),
            PathBuf::from("/project/.hats/planning-sessions")
        );
    }

    #[test]
    fn test_planning_session_paths() {
        let ctx = LoopContext::primary(PathBuf::from("/project"));
        let session_id = "20260127-143022-a7f2";

        assert_eq!(
            ctx.planning_session_dir(session_id),
            PathBuf::from("/project/.hats/planning-sessions/20260127-143022-a7f2")
        );
        assert_eq!(
            ctx.planning_conversation_path(session_id),
            PathBuf::from(
                "/project/.hats/planning-sessions/20260127-143022-a7f2/conversation.jsonl"
            )
        );
        assert_eq!(
            ctx.planning_session_metadata_path(session_id),
            PathBuf::from("/project/.hats/planning-sessions/20260127-143022-a7f2/session.json")
        );
        assert_eq!(
            ctx.planning_artifacts_dir(session_id),
            PathBuf::from("/project/.hats/planning-sessions/20260127-143022-a7f2/artifacts")
        );
    }

    #[test]
    fn test_planning_session_directory_creation() {
        let temp = TempDir::new().unwrap();
        let ctx = LoopContext::primary(temp.path().to_path_buf());
        let session_id = "test-session";

        // Create session directory
        std::fs::create_dir_all(ctx.planning_session_dir(session_id)).unwrap();

        // Verify directory exists
        assert!(ctx.planning_session_dir(session_id).exists());
        assert!(ctx.planning_sessions_dir().exists());
    }

    #[test]
    fn test_context_path() {
        let ctx = LoopContext::primary(PathBuf::from("/project"));
        assert_eq!(
            ctx.context_path(),
            PathBuf::from("/project/.hats/agent/context.md")
        );
    }

    #[cfg(unix)]
    #[test]
    fn test_specs_symlink_worktree() {
        let temp = TempDir::new().unwrap();
        let repo_root = temp.path().to_path_buf();
        let worktree_path = repo_root.join(".worktrees/loop-1234");

        // Create the main specs directory
        std::fs::create_dir_all(repo_root.join(".hats/specs")).unwrap();
        std::fs::write(repo_root.join(".hats/specs/test.spec.md"), "# Test Spec\n").unwrap();

        let ctx = LoopContext::worktree("loop-1234", worktree_path.clone(), repo_root.clone());

        // Ensure .hats dir exists
        ctx.ensure_hats_dir().unwrap();

        // Create symlink
        let created = ctx.setup_specs_symlink().unwrap();
        assert!(created);

        // Verify symlink exists and points to main specs
        let specs = ctx.specs_dir();
        assert!(specs.is_symlink());
        assert_eq!(std::fs::read_link(&specs).unwrap(), ctx.main_specs_dir());

        // Second call is a no-op
        let created_again = ctx.setup_specs_symlink().unwrap();
        assert!(!created_again);
    }

    #[cfg(unix)]
    #[test]
    fn test_code_tasks_symlink_worktree() {
        let temp = TempDir::new().unwrap();
        let repo_root = temp.path().to_path_buf();
        let worktree_path = repo_root.join(".worktrees/loop-1234");

        // Create the main code tasks directory
        std::fs::create_dir_all(repo_root.join(".hats/tasks")).unwrap();
        std::fs::write(
            repo_root.join(".hats/tasks/test.code-task.md"),
            "# Test Task\n",
        )
        .unwrap();

        let ctx = LoopContext::worktree("loop-1234", worktree_path.clone(), repo_root.clone());

        // Ensure .hats dir exists
        ctx.ensure_hats_dir().unwrap();

        // Create symlink
        let created = ctx.setup_code_tasks_symlink().unwrap();
        assert!(created);

        // Verify symlink exists and points to main code tasks
        let tasks = ctx.code_tasks_dir();
        assert!(tasks.is_symlink());
        assert_eq!(
            std::fs::read_link(&tasks).unwrap(),
            ctx.main_code_tasks_dir()
        );

        // Second call is a no-op
        let created_again = ctx.setup_code_tasks_symlink().unwrap();
        assert!(!created_again);
    }

    #[test]
    fn test_generate_context_file_primary_noop() {
        let temp = TempDir::new().unwrap();
        let ctx = LoopContext::primary(temp.path().to_path_buf());

        // Primary loop doesn't create context file
        let created = ctx.generate_context_file("main", "test prompt").unwrap();
        assert!(!created);
    }

    #[test]
    fn test_generate_context_file_worktree() {
        let temp = TempDir::new().unwrap();
        let repo_root = temp.path().to_path_buf();
        let worktree_path = repo_root.join(".worktrees/loop-1234");

        let ctx = LoopContext::worktree("loop-1234", worktree_path.clone(), repo_root.clone());

        // Create context file
        ctx.ensure_agent_dir().unwrap();
        let created = ctx
            .generate_context_file("hats/loop-1234", "Add footer")
            .unwrap();
        assert!(created);

        // Verify file exists and contains expected content
        let context_path = ctx.context_path();
        assert!(context_path.exists());

        let content = std::fs::read_to_string(&context_path).unwrap();
        assert!(content.contains("# Worktree Context"));
        assert!(content.contains("loop-1234"));
        assert!(content.contains("Add footer"));
        assert!(content.contains("hats/loop-1234"));

        // Second call is a no-op
        let created_again = ctx
            .generate_context_file("hats/loop-1234", "Add footer")
            .unwrap();
        assert!(!created_again);
    }

    #[cfg(unix)]
    #[test]
    fn test_setup_worktree_symlinks() {
        let temp = TempDir::new().unwrap();
        let repo_root = temp.path().to_path_buf();
        let worktree_path = repo_root.join(".worktrees/loop-1234");

        // Create main repo directories
        std::fs::create_dir_all(repo_root.join(".hats/agent")).unwrap();
        std::fs::create_dir_all(repo_root.join(".hats/specs")).unwrap();
        std::fs::create_dir_all(repo_root.join(".hats/tasks")).unwrap();
        std::fs::write(repo_root.join(".hats/agent/memories.md"), "# Memories\n").unwrap();

        let ctx = LoopContext::worktree("loop-1234", worktree_path.clone(), repo_root.clone());

        // Setup all symlinks
        ctx.ensure_directories().unwrap();
        ctx.setup_worktree_symlinks().unwrap();

        // Verify all symlinks exist
        assert!(ctx.memories_path().is_symlink());
        assert!(ctx.specs_dir().is_symlink());
        assert!(ctx.code_tasks_dir().is_symlink());
    }
}
