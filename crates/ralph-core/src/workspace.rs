//! Workspace isolation for benchmark tasks.
//!
//! Provides isolated temporary directories for each benchmark task run.
//! Each workspace has its own `.git` directory to prevent polluting the main
//! repository with agent commits.
//!
//! # Example
//!
//! ```no_run
//! use ralph_core::workspace::{TaskWorkspace, CleanupPolicy};
//! use ralph_core::task_definition::TaskDefinition;
//! use std::path::Path;
//!
//! let task = TaskDefinition::builder("hello-world", "tasks/hello/PROMPT.md", "DONE")
//!     .verification_command("python hello.py")
//!     .build();
//!
//! // Create isolated workspace
//! let mut workspace = TaskWorkspace::create(&task, Path::new("/tmp/ralph-bench"))?;
//!
//! // Setup files from task definition
//! workspace.setup(&task, Path::new("./bench/tasks"))?;
//!
//! // Run benchmark in workspace.path()...
//!
//! // Cleanup based on policy
//! workspace.cleanup()?;
//! # Ok::<(), ralph_core::workspace::WorkspaceError>(())
//! ```

use crate::task_definition::{TaskDefinition, Verification};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

/// Cleanup policy for workspace directories.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum CleanupPolicy {
    /// Keep last N workspaces, delete older ones.
    Rotate(usize),

    /// Delete on success, keep failures for debugging.
    #[default]
    OnSuccess,

    /// Delete immediately after verification.
    Always,

    /// Keep all workspaces (manual cleanup).
    Never,
}

impl CleanupPolicy {
    /// Parse from string representation.
    #[allow(clippy::match_same_arms)] // Explicit "on_success" arm for clarity
    pub fn from_str(s: &str, keep_last_n: Option<usize>) -> Self {
        match s.to_lowercase().as_str() {
            "rotate" => CleanupPolicy::Rotate(keep_last_n.unwrap_or(5)),
            "on_success" => CleanupPolicy::OnSuccess,
            "always" => CleanupPolicy::Always,
            "never" => CleanupPolicy::Never,
            _ => CleanupPolicy::OnSuccess,
        }
    }
}

/// An isolated workspace for running a benchmark task.
///
/// The workspace is created in a temporary directory with:
/// - Its own `.git` directory (isolated from main repo)
/// - A fresh `.agent/scratchpad.md`
/// - Copied setup files from the task definition
#[derive(Debug)]
pub struct TaskWorkspace {
    /// Path to the workspace root directory.
    path: PathBuf,

    /// Task name for identification.
    task_name: String,

    /// Timestamp when workspace was created.
    created_at: u64,

    /// Whether this workspace has been cleaned up.
    cleaned_up: bool,
}

impl TaskWorkspace {
    /// Creates a new isolated workspace for the given task.
    ///
    /// The workspace is created at:
    /// `{base_dir}/ralph-bench-{task_name}-{timestamp}/`
    ///
    /// # Arguments
    ///
    /// * `task` - The task definition to create a workspace for
    /// * `base_dir` - Base directory for workspaces (e.g., `/tmp`)
    ///
    /// # Errors
    ///
    /// Returns `WorkspaceError` if directory creation or git init fails.
    pub fn create(task: &TaskDefinition, base_dir: &Path) -> Result<Self, WorkspaceError> {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);

        let dir_name = format!("ralph-bench-{}-{}", task.name, timestamp);
        let path = base_dir.join(&dir_name);

        // Create workspace directory
        fs::create_dir_all(&path)?;

        // Create .agent directory with empty scratchpad
        let agent_dir = path.join(".agent");
        fs::create_dir_all(&agent_dir)?;
        fs::write(agent_dir.join("scratchpad.md"), "")?;

        // Initialize isolated git repository
        let git_output = Command::new("git")
            .args(["init", "--initial-branch=main"])
            .current_dir(&path)
            .output()?;

        if !git_output.status.success() {
            let stderr = String::from_utf8_lossy(&git_output.stderr);
            return Err(WorkspaceError::GitInit(stderr.to_string()));
        }

        // Configure git user for commits (required for commits to work)
        Command::new("git")
            .args(["config", "user.email", "benchmark@ralph.local"])
            .current_dir(&path)
            .output()?;

        Command::new("git")
            .args(["config", "user.name", "Ralph Benchmark"])
            .current_dir(&path)
            .output()?;

        Ok(Self {
            path,
            task_name: task.name.clone(),
            created_at: timestamp,
            cleaned_up: false,
        })
    }

    /// Returns the path to the workspace root directory.
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Returns the task name.
    pub fn task_name(&self) -> &str {
        &self.task_name
    }

    /// Returns the creation timestamp.
    pub fn created_at(&self) -> u64 {
        self.created_at
    }

    /// Sets up the workspace with files from the task definition.
    ///
    /// This copies:
    /// 1. The prompt file as `PROMPT.md`
    /// 2. Any setup files specified in the task definition
    ///
    /// # Arguments
    ///
    /// * `task` - The task definition containing setup configuration
    /// * `tasks_dir` - Base directory where task files are located
    ///
    /// # Errors
    ///
    /// Returns `WorkspaceError` if file copying fails.
    pub fn setup(&self, task: &TaskDefinition, tasks_dir: &Path) -> Result<(), WorkspaceError> {
        // Copy prompt file
        let prompt_src = tasks_dir.join(&task.prompt_file);
        let prompt_dst = self.path.join("PROMPT.md");

        if prompt_src.exists() {
            fs::copy(&prompt_src, &prompt_dst)?;
        } else {
            return Err(WorkspaceError::MissingFile(
                prompt_src.to_string_lossy().to_string(),
            ));
        }

        // Copy setup files
        for file in &task.setup.files {
            let src = tasks_dir.join(file);
            let dst = self.path.join(file);

            // Ensure parent directory exists
            if let Some(parent) = dst.parent() {
                fs::create_dir_all(parent)?;
            }

            if src.exists() {
                if src.is_dir() {
                    copy_dir_recursive(&src, &dst)?;
                } else {
                    fs::copy(&src, &dst)?;
                }
            } else {
                return Err(WorkspaceError::MissingFile(
                    src.to_string_lossy().to_string(),
                ));
            }
        }

        // Run setup script if specified
        if let Some(script) = &task.setup.script {
            let script_path = tasks_dir.join(script);
            if script_path.exists() {
                // Copy script first
                let script_dst = self.path.join(script);
                if let Some(parent) = script_dst.parent() {
                    fs::create_dir_all(parent)?;
                }
                fs::copy(&script_path, &script_dst)?;

                // Execute it
                let output = Command::new("bash")
                    .arg(&script_dst)
                    .current_dir(&self.path)
                    .output()?;

                if !output.status.success() {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    return Err(WorkspaceError::SetupScript(stderr.to_string()));
                }
            }
        }

        // Create initial git commit
        Command::new("git")
            .args(["add", "-A"])
            .current_dir(&self.path)
            .output()?;

        let commit_output = Command::new("git")
            .args(["commit", "-m", "Initial benchmark setup", "--allow-empty"])
            .current_dir(&self.path)
            .output()?;

        if !commit_output.status.success() {
            // Non-fatal: might have no files to commit
            tracing::debug!(
                "Initial commit warning: {}",
                String::from_utf8_lossy(&commit_output.stderr)
            );
        }

        Ok(())
    }

    /// Cleans up (removes) the workspace directory.
    ///
    /// # Errors
    ///
    /// Returns `WorkspaceError` if removal fails.
    pub fn cleanup(&mut self) -> Result<(), WorkspaceError> {
        if self.cleaned_up {
            return Ok(());
        }

        if self.path.exists() {
            fs::remove_dir_all(&self.path)?;
        }

        self.cleaned_up = true;
        Ok(())
    }

    /// Returns true if the workspace has been cleaned up.
    pub fn is_cleaned_up(&self) -> bool {
        self.cleaned_up
    }
}

impl Drop for TaskWorkspace {
    fn drop(&mut self) {
        // Don't automatically clean up on drop—let the caller decide based on policy
        if !self.cleaned_up && self.path.exists() {
            tracing::debug!(
                "Workspace {} not cleaned up, path retained: {}",
                self.task_name,
                self.path.display()
            );
        }
    }
}

/// Result of running a verification command.
#[derive(Debug, Clone)]
pub struct VerificationResult {
    /// Whether verification passed (exit code matched expected).
    pub passed: bool,

    /// Actual exit code from the command.
    pub exit_code: i32,

    /// Expected exit code for success.
    pub expected_exit_code: i32,

    /// Stdout output from the command.
    pub stdout: String,

    /// Stderr output from the command.
    pub stderr: String,
}

impl VerificationResult {
    /// Returns a human-readable summary of the result.
    pub fn summary(&self) -> String {
        if self.passed {
            format!("PASSED (exit code {})", self.exit_code)
        } else {
            format!(
                "FAILED (exit code {}, expected {})",
                self.exit_code, self.expected_exit_code
            )
        }
    }
}

impl TaskWorkspace {
    /// Runs a verification command in the workspace directory.
    ///
    /// The command is executed via `bash -c` in the workspace's root directory.
    ///
    /// # Arguments
    ///
    /// * `verification` - The verification configuration with command and expected exit code
    ///
    /// # Returns
    ///
    /// A `VerificationResult` indicating whether the command passed and capturing output.
    ///
    /// # Errors
    ///
    /// Returns `WorkspaceError::Verification` if the command fails to execute
    /// (not the same as the command returning a non-zero exit code).
    pub fn run_verification(
        &self,
        verification: &Verification,
    ) -> Result<VerificationResult, WorkspaceError> {
        if verification.command.is_empty() {
            // No verification command - consider it passed
            return Ok(VerificationResult {
                passed: true,
                exit_code: 0,
                expected_exit_code: 0,
                stdout: String::new(),
                stderr: String::new(),
            });
        }

        tracing::debug!(
            "Running verification in {}: {}",
            self.path.display(),
            verification.command
        );

        let output = Command::new("bash")
            .args(["-c", &verification.command])
            .current_dir(&self.path)
            .output()
            .map_err(|e| WorkspaceError::Verification(format!("Failed to execute: {}", e)))?;

        let exit_code = output.status.code().unwrap_or(-1);
        let passed = exit_code == verification.success_exit_code;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();

        tracing::debug!(
            "Verification result: {} (exit code {}, expected {})",
            if passed { "PASSED" } else { "FAILED" },
            exit_code,
            verification.success_exit_code
        );

        Ok(VerificationResult {
            passed,
            exit_code,
            expected_exit_code: verification.success_exit_code,
            stdout,
            stderr,
        })
    }
}

/// Manages workspace cleanup according to a policy.
#[derive(Debug)]
pub struct WorkspaceManager {
    /// Base directory for workspaces.
    base_dir: PathBuf,

    /// Cleanup policy to apply.
    policy: CleanupPolicy,
}

impl WorkspaceManager {
    /// Creates a new workspace manager.
    pub fn new(base_dir: impl Into<PathBuf>, policy: CleanupPolicy) -> Self {
        Self {
            base_dir: base_dir.into(),
            policy,
        }
    }

    /// Returns the base directory.
    pub fn base_dir(&self) -> &Path {
        &self.base_dir
    }

    /// Returns the cleanup policy.
    pub fn policy(&self) -> CleanupPolicy {
        self.policy
    }

    /// Creates a workspace for the given task.
    pub fn create_workspace(&self, task: &TaskDefinition) -> Result<TaskWorkspace, WorkspaceError> {
        TaskWorkspace::create(task, &self.base_dir)
    }

    /// Applies cleanup policy after a task run.
    ///
    /// # Arguments
    ///
    /// * `workspace` - The workspace to potentially clean up
    /// * `success` - Whether the task verification passed
    ///
    /// # Returns
    ///
    /// `true` if the workspace was cleaned up, `false` if retained.
    pub fn apply_cleanup(
        &self,
        workspace: &mut TaskWorkspace,
        success: bool,
    ) -> Result<bool, WorkspaceError> {
        match self.policy {
            CleanupPolicy::Always => {
                workspace.cleanup()?;
                Ok(true)
            }
            CleanupPolicy::OnSuccess => {
                if success {
                    workspace.cleanup()?;
                    Ok(true)
                } else {
                    Ok(false)
                }
            }
            CleanupPolicy::Never => Ok(false),
            CleanupPolicy::Rotate(keep_last_n) => {
                // Don't clean up this workspace yet, but rotate old ones
                self.rotate_workspaces(keep_last_n)?;
                Ok(false)
            }
        }
    }

    /// Rotates old workspaces, keeping only the last N.
    pub fn rotate_workspaces(&self, keep_last_n: usize) -> Result<(), WorkspaceError> {
        if !self.base_dir.exists() {
            return Ok(());
        }

        // Find all ralph-bench-* directories
        let mut workspaces: Vec<(PathBuf, u64)> = Vec::new();

        for entry in fs::read_dir(&self.base_dir)? {
            let entry = entry?;
            let path = entry.path();

            if !path.is_dir() {
                continue;
            }
            let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            if !name.starts_with("ralph-bench-") {
                continue;
            }
            if let Some(ts) = extract_timestamp(name) {
                workspaces.push((path, ts));
            }
        }

        // Sort by timestamp (newest first)
        workspaces.sort_by(|a, b| b.1.cmp(&a.1));

        // Delete workspaces beyond keep_last_n
        for (path, _) in workspaces.into_iter().skip(keep_last_n) {
            tracing::debug!("Rotating old workspace: {}", path.display());
            fs::remove_dir_all(&path)?;
        }

        Ok(())
    }

    /// Lists all workspace directories in the base directory.
    pub fn list_workspaces(&self) -> Result<Vec<WorkspaceInfo>, WorkspaceError> {
        if !self.base_dir.exists() {
            return Ok(Vec::new());
        }

        let mut workspaces = Vec::new();

        for entry in fs::read_dir(&self.base_dir)? {
            let entry = entry?;
            let path = entry.path();

            if !path.is_dir() {
                continue;
            }
            let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
                continue;
            };
            if !name.starts_with("ralph-bench-") {
                continue;
            }
            let timestamp = extract_timestamp(name);
            let task_name = extract_task_name(name);
            workspaces.push(WorkspaceInfo {
                path,
                task_name,
                timestamp,
            });
        }

        // Sort by timestamp (newest first)
        workspaces.sort_by(|a, b| b.timestamp.cmp(&a.timestamp));

        Ok(workspaces)
    }
}

/// Information about an existing workspace.
#[derive(Debug, Clone)]
pub struct WorkspaceInfo {
    /// Path to the workspace directory.
    pub path: PathBuf,

    /// Task name extracted from directory name.
    pub task_name: Option<String>,

    /// Timestamp extracted from directory name.
    pub timestamp: Option<u64>,
}

/// Errors that can occur during workspace operations.
#[derive(Debug, thiserror::Error)]
pub enum WorkspaceError {
    /// IO error.
    #[error("IO error: {0}")]
    Io(#[from] io::Error),

    /// Git initialization failed.
    #[error("Git init failed: {0}")]
    GitInit(String),

    /// Required file not found.
    #[error("Missing required file: {0}")]
    MissingFile(String),

    /// Setup script failed.
    #[error("Setup script failed: {0}")]
    SetupScript(String),

    /// Verification command failed to execute.
    #[error("Verification failed: {0}")]
    Verification(String),
}

// ─────────────────────────────────────────────────────────────────────────────
// Helper functions
// ─────────────────────────────────────────────────────────────────────────────

/// Recursively copies a directory.
fn copy_dir_recursive(src: &Path, dst: &Path) -> io::Result<()> {
    fs::create_dir_all(dst)?;

    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path)?;
        }
    }

    Ok(())
}

/// Extracts timestamp from workspace directory name.
///
/// Format: `ralph-bench-{task_name}-{timestamp}`
fn extract_timestamp(dir_name: &str) -> Option<u64> {
    dir_name
        .rsplit('-')
        .next()
        .and_then(|s| s.parse::<u64>().ok())
}

/// Extracts task name from workspace directory name.
///
/// Format: `ralph-bench-{task_name}-{timestamp}`
fn extract_task_name(dir_name: &str) -> Option<String> {
    let stripped = dir_name.strip_prefix("ralph-bench-")?;
    // Find the last dash before the timestamp
    let parts: Vec<&str> = stripped.rsplitn(2, '-').collect();
    if parts.len() == 2 {
        Some(parts[1].to_string())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_test_task(name: &str) -> TaskDefinition {
        TaskDefinition::builder(name, "tasks/test/PROMPT.md", "DONE")
            .verification_command("echo ok")
            .build()
    }

    #[test]
    fn test_workspace_create() {
        let temp_dir = TempDir::new().unwrap();
        let task = make_test_task("hello-world");

        let workspace = TaskWorkspace::create(&task, temp_dir.path()).unwrap();

        assert!(workspace.path().exists());
        assert!(workspace.path().join(".git").exists());
        assert!(workspace.path().join(".agent").exists());
        assert!(workspace.path().join(".agent/scratchpad.md").exists());
        assert_eq!(workspace.task_name(), "hello-world");
    }

    #[test]
    fn test_workspace_cleanup() {
        let temp_dir = TempDir::new().unwrap();
        let task = make_test_task("cleanup-test");

        let mut workspace = TaskWorkspace::create(&task, temp_dir.path()).unwrap();
        let path = workspace.path().to_path_buf();

        assert!(path.exists());
        assert!(!workspace.is_cleaned_up());

        workspace.cleanup().unwrap();

        assert!(!path.exists());
        assert!(workspace.is_cleaned_up());

        // Cleanup is idempotent
        workspace.cleanup().unwrap();
    }

    #[test]
    fn test_workspace_setup_with_prompt() {
        let temp_dir = TempDir::new().unwrap();
        let tasks_dir = TempDir::new().unwrap();

        // Create prompt file
        let prompt_dir = tasks_dir.path().join("tasks/test");
        fs::create_dir_all(&prompt_dir).unwrap();
        fs::write(
            prompt_dir.join("PROMPT.md"),
            "# Test Prompt\n\nDo something.",
        )
        .unwrap();

        let task = make_test_task("setup-test");
        let workspace = TaskWorkspace::create(&task, temp_dir.path()).unwrap();

        workspace.setup(&task, tasks_dir.path()).unwrap();

        // Prompt should be copied
        let prompt_dst = workspace.path().join("PROMPT.md");
        assert!(prompt_dst.exists());
        assert!(
            fs::read_to_string(&prompt_dst)
                .unwrap()
                .contains("Test Prompt")
        );
    }

    #[test]
    fn test_workspace_setup_with_files() {
        let temp_dir = TempDir::new().unwrap();
        let tasks_dir = TempDir::new().unwrap();

        // Create prompt and setup files
        let prompt_dir = tasks_dir.path().join("tasks/test");
        fs::create_dir_all(&prompt_dir).unwrap();
        fs::write(prompt_dir.join("PROMPT.md"), "# Test").unwrap();
        fs::write(tasks_dir.path().join("helper.py"), "# helper").unwrap();

        let task = TaskDefinition::builder("setup-files-test", "tasks/test/PROMPT.md", "DONE")
            .verification_command("echo ok")
            .setup_files(vec!["helper.py".to_string()])
            .build();

        let workspace = TaskWorkspace::create(&task, temp_dir.path()).unwrap();
        workspace.setup(&task, tasks_dir.path()).unwrap();

        // Setup file should be copied
        assert!(workspace.path().join("helper.py").exists());
    }

    #[test]
    fn test_workspace_setup_missing_prompt() {
        let temp_dir = TempDir::new().unwrap();
        let tasks_dir = TempDir::new().unwrap();

        let task = make_test_task("missing-prompt");
        let workspace = TaskWorkspace::create(&task, temp_dir.path()).unwrap();

        let result = workspace.setup(&task, tasks_dir.path());
        assert!(matches!(result, Err(WorkspaceError::MissingFile(_))));
    }

    #[test]
    fn test_cleanup_policy_from_str() {
        assert_eq!(
            CleanupPolicy::from_str("rotate", Some(10)),
            CleanupPolicy::Rotate(10)
        );
        assert_eq!(
            CleanupPolicy::from_str("rotate", None),
            CleanupPolicy::Rotate(5)
        );
        assert_eq!(
            CleanupPolicy::from_str("on_success", None),
            CleanupPolicy::OnSuccess
        );
        assert_eq!(
            CleanupPolicy::from_str("always", None),
            CleanupPolicy::Always
        );
        assert_eq!(CleanupPolicy::from_str("never", None), CleanupPolicy::Never);
        assert_eq!(
            CleanupPolicy::from_str("ROTATE", Some(3)),
            CleanupPolicy::Rotate(3)
        );
        assert_eq!(
            CleanupPolicy::from_str("unknown", None),
            CleanupPolicy::OnSuccess
        );
    }

    #[test]
    fn test_extract_timestamp() {
        assert_eq!(
            extract_timestamp("ralph-bench-hello-world-1704067200000"),
            Some(1_704_067_200_000)
        );
        assert_eq!(
            extract_timestamp("ralph-bench-fizz-buzz-tdd-1704067300000"),
            Some(1_704_067_300_000)
        );
        assert_eq!(extract_timestamp("ralph-bench-invalid"), None);
        assert_eq!(extract_timestamp("other-dir"), None);
    }

    #[test]
    fn test_extract_task_name() {
        assert_eq!(
            extract_task_name("ralph-bench-hello-world-1704067200000"),
            Some("hello-world".to_string())
        );
        assert_eq!(
            extract_task_name("ralph-bench-simple-1704067200000"),
            Some("simple".to_string())
        );
    }

    #[test]
    fn test_workspace_manager_rotate() {
        let temp_dir = TempDir::new().unwrap();
        let manager = WorkspaceManager::new(temp_dir.path(), CleanupPolicy::Rotate(2));

        // Create multiple workspaces
        let task = make_test_task("rotate-test");
        let ws1 = manager.create_workspace(&task).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(10));
        let ws2 = manager.create_workspace(&task).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(10));
        let ws3 = manager.create_workspace(&task).unwrap();

        // All three exist
        assert!(ws1.path().exists());
        assert!(ws2.path().exists());
        assert!(ws3.path().exists());

        // Rotate should keep only 2
        manager.rotate_workspaces(2).unwrap();

        // ws1 should be deleted (oldest)
        assert!(!ws1.path().exists());
        // ws2 and ws3 should remain
        assert!(ws2.path().exists());
        assert!(ws3.path().exists());
    }

    #[test]
    fn test_workspace_manager_apply_cleanup_always() {
        let temp_dir = TempDir::new().unwrap();
        let manager = WorkspaceManager::new(temp_dir.path(), CleanupPolicy::Always);

        let task = make_test_task("always-cleanup");
        let mut workspace = manager.create_workspace(&task).unwrap();
        let path = workspace.path().to_path_buf();

        assert!(path.exists());

        let cleaned = manager.apply_cleanup(&mut workspace, true).unwrap();
        assert!(cleaned);
        assert!(!path.exists());
    }

    #[test]
    fn test_workspace_manager_apply_cleanup_on_success() {
        let temp_dir = TempDir::new().unwrap();
        let manager = WorkspaceManager::new(temp_dir.path(), CleanupPolicy::OnSuccess);

        let task = make_test_task("on-success-cleanup");

        // Success case: should cleanup
        let mut ws_success = manager.create_workspace(&task).unwrap();
        let path_success = ws_success.path().to_path_buf();
        let cleaned = manager.apply_cleanup(&mut ws_success, true).unwrap();
        assert!(cleaned);
        assert!(!path_success.exists());

        // Failure case: should keep
        let mut ws_failure = manager.create_workspace(&task).unwrap();
        let path_failure = ws_failure.path().to_path_buf();
        let cleaned = manager.apply_cleanup(&mut ws_failure, false).unwrap();
        assert!(!cleaned);
        assert!(path_failure.exists());
    }

    #[test]
    fn test_workspace_manager_list_workspaces() {
        let temp_dir = TempDir::new().unwrap();
        let manager = WorkspaceManager::new(temp_dir.path(), CleanupPolicy::Never);

        let task1 = make_test_task("list-test-a");
        let task2 = make_test_task("list-test-b");

        let _ws1 = manager.create_workspace(&task1).unwrap();
        std::thread::sleep(std::time::Duration::from_millis(10));
        let _ws2 = manager.create_workspace(&task2).unwrap();

        let list = manager.list_workspaces().unwrap();
        assert_eq!(list.len(), 2);

        // Should be sorted newest first
        assert!(list[0].timestamp > list[1].timestamp);
    }

    #[test]
    fn test_copy_dir_recursive() {
        let temp_dir = TempDir::new().unwrap();
        let src = temp_dir.path().join("src");
        let dst = temp_dir.path().join("dst");

        // Create source structure
        fs::create_dir_all(src.join("subdir")).unwrap();
        fs::write(src.join("file1.txt"), "content1").unwrap();
        fs::write(src.join("subdir/file2.txt"), "content2").unwrap();

        // Copy
        copy_dir_recursive(&src, &dst).unwrap();

        // Verify
        assert!(dst.join("file1.txt").exists());
        assert!(dst.join("subdir/file2.txt").exists());
        assert_eq!(
            fs::read_to_string(dst.join("file1.txt")).unwrap(),
            "content1"
        );
        assert_eq!(
            fs::read_to_string(dst.join("subdir/file2.txt")).unwrap(),
            "content2"
        );
    }

    #[test]
    fn test_run_verification_success() {
        let temp_dir = TempDir::new().unwrap();
        let task = make_test_task("verify-success");
        let workspace = TaskWorkspace::create(&task, temp_dir.path()).unwrap();

        // Create a file that verification will check
        fs::write(workspace.path().join("hello.txt"), "Hello, World!").unwrap();

        let verification = Verification {
            command: "cat hello.txt | grep -q 'Hello, World!'".to_string(),
            success_exit_code: 0,
        };

        let result = workspace.run_verification(&verification).unwrap();
        assert!(result.passed);
        assert_eq!(result.exit_code, 0);
        assert_eq!(result.expected_exit_code, 0);
    }

    #[test]
    fn test_run_verification_failure() {
        let temp_dir = TempDir::new().unwrap();
        let task = make_test_task("verify-failure");
        let workspace = TaskWorkspace::create(&task, temp_dir.path()).unwrap();

        // File doesn't exist, grep will fail
        let verification = Verification {
            command: "cat nonexistent.txt".to_string(),
            success_exit_code: 0,
        };

        let result = workspace.run_verification(&verification).unwrap();
        assert!(!result.passed);
        assert_ne!(result.exit_code, 0);
    }

    #[test]
    fn test_run_verification_custom_exit_code() {
        let temp_dir = TempDir::new().unwrap();
        let task = make_test_task("verify-custom-exit");
        let workspace = TaskWorkspace::create(&task, temp_dir.path()).unwrap();

        // Command exits with code 42
        let verification = Verification {
            command: "exit 42".to_string(),
            success_exit_code: 42,
        };

        let result = workspace.run_verification(&verification).unwrap();
        assert!(result.passed);
        assert_eq!(result.exit_code, 42);
        assert_eq!(result.expected_exit_code, 42);
    }

    #[test]
    fn test_run_verification_empty_command() {
        let temp_dir = TempDir::new().unwrap();
        let task = make_test_task("verify-empty");
        let workspace = TaskWorkspace::create(&task, temp_dir.path()).unwrap();

        let verification = Verification {
            command: String::new(),
            success_exit_code: 0,
        };

        let result = workspace.run_verification(&verification).unwrap();
        assert!(result.passed);
    }

    #[test]
    fn test_run_verification_captures_output() {
        let temp_dir = TempDir::new().unwrap();
        let task = make_test_task("verify-capture");
        let workspace = TaskWorkspace::create(&task, temp_dir.path()).unwrap();

        let verification = Verification {
            command: "echo 'stdout message' && echo 'stderr message' >&2".to_string(),
            success_exit_code: 0,
        };

        let result = workspace.run_verification(&verification).unwrap();
        assert!(result.passed);
        assert!(result.stdout.contains("stdout message"));
        assert!(result.stderr.contains("stderr message"));
    }

    #[test]
    fn test_verification_result_summary() {
        let passed_result = VerificationResult {
            passed: true,
            exit_code: 0,
            expected_exit_code: 0,
            stdout: String::new(),
            stderr: String::new(),
        };
        assert_eq!(passed_result.summary(), "PASSED (exit code 0)");

        let failed_result = VerificationResult {
            passed: false,
            exit_code: 1,
            expected_exit_code: 0,
            stdout: String::new(),
            stderr: String::new(),
        };
        assert_eq!(failed_result.summary(), "FAILED (exit code 1, expected 0)");
    }
}
