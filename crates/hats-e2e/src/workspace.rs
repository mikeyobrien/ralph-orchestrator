//! Workspace management for E2E tests.
//!
//! This module provides functionality to create and manage isolated test workspaces.
//! Each test scenario gets its own workspace directory under `.e2e-tests/` to ensure
//! test isolation and enable post-test inspection of artifacts.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Manages isolated test workspaces for E2E tests.
///
/// The WorkspaceManager creates and cleans up directories under a base path
/// (typically `.e2e-tests/`). Each scenario gets its own subdirectory containing
/// its hats.yml, prompt files, and .hats/agent/ directory.
///
/// # Example
///
/// ```no_run
/// use hats_e2e::workspace::WorkspaceManager;
///
/// let manager = WorkspaceManager::new(".e2e-tests");
/// let workspace = manager.create_workspace("claude-connect").unwrap();
/// // Run tests...
/// manager.cleanup("claude-connect").unwrap();
/// ```
#[derive(Debug, Clone)]
pub struct WorkspaceManager {
    /// Base path for all workspaces (e.g., `.e2e-tests/`)
    base_path: PathBuf,
}

impl WorkspaceManager {
    /// Creates a new WorkspaceManager with the given base path.
    ///
    /// The base path will be created if it doesn't exist when `create_workspace` is called.
    pub fn new<P: AsRef<Path>>(base_path: P) -> Self {
        Self {
            base_path: base_path.as_ref().to_path_buf(),
        }
    }

    /// Returns the base path for all workspaces.
    pub fn base_path(&self) -> &Path {
        &self.base_path
    }

    /// Creates an isolated workspace for a test scenario.
    ///
    /// Creates the directory structure:
    /// ```text
    /// {base_path}/{scenario_id}/
    /// └── .hats/agent/
    /// ```
    ///
    /// Returns the path to the created workspace directory.
    ///
    /// # Errors
    ///
    /// Returns an error if the directory cannot be created.
    pub fn create_workspace(&self, scenario_id: &str) -> io::Result<PathBuf> {
        let workspace_path = self.base_path.join(scenario_id);

        // Create the workspace directory and .agent subdirectory
        fs::create_dir_all(workspace_path.join(".agent"))?;

        Ok(workspace_path)
    }

    /// Cleans up a specific workspace.
    ///
    /// Removes the entire workspace directory for the given scenario.
    ///
    /// # Errors
    ///
    /// Returns an error if the directory cannot be removed. Returns Ok(())
    /// if the directory doesn't exist (idempotent cleanup).
    pub fn cleanup(&self, scenario_id: &str) -> io::Result<()> {
        let workspace_path = self.base_path.join(scenario_id);

        if workspace_path.exists() {
            fs::remove_dir_all(workspace_path)?;
        }

        Ok(())
    }

    /// Cleans up all workspaces.
    ///
    /// Removes the entire base directory and all workspaces within it.
    ///
    /// # Errors
    ///
    /// Returns an error if the directory cannot be removed. Returns Ok(())
    /// if the directory doesn't exist (idempotent cleanup).
    pub fn cleanup_all(&self) -> io::Result<()> {
        if self.base_path.exists() {
            fs::remove_dir_all(&self.base_path)?;
        }

        Ok(())
    }

    /// Returns the path for a specific workspace (without creating it).
    pub fn workspace_path(&self, scenario_id: &str) -> PathBuf {
        self.base_path.join(scenario_id)
    }

    /// Checks if a workspace exists.
    pub fn workspace_exists(&self, scenario_id: &str) -> bool {
        self.workspace_path(scenario_id).exists()
    }

    /// Lists all existing workspaces.
    ///
    /// Returns the scenario IDs of all workspaces that currently exist.
    pub fn list_workspaces(&self) -> io::Result<Vec<String>> {
        if !self.base_path.exists() {
            return Ok(Vec::new());
        }

        let mut workspaces = Vec::new();
        for entry in fs::read_dir(&self.base_path)? {
            let entry = entry?;
            if entry.file_type()?.is_dir()
                && let Some(name) = entry.file_name().to_str()
                && !name.starts_with('.')
            {
                workspaces.push(name.to_string());
            }
        }
        workspaces.sort();
        Ok(workspaces)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    /// Creates a unique test base path to avoid conflicts between parallel tests.
    fn test_base_path(test_name: &str) -> PathBuf {
        env::temp_dir().join(format!(
            "hats-e2e-test-{}-{}",
            test_name,
            std::process::id()
        ))
    }

    #[test]
    fn test_workspace_creation() {
        let base = test_base_path("creation");
        let ws = WorkspaceManager::new(&base);

        // Create workspace
        let path = ws.create_workspace("test-scenario").unwrap();
        assert!(path.exists(), "Workspace directory should exist");
        assert!(
            path.join(".agent").exists(),
            ".agent directory should exist"
        );

        // Verify workspace path matches expected
        assert_eq!(path, base.join("test-scenario"));

        // Cleanup
        ws.cleanup("test-scenario").unwrap();
        assert!(!path.exists(), "Workspace should be removed after cleanup");

        // Cleanup base
        ws.cleanup_all().unwrap();
    }

    #[test]
    fn test_cleanup_idempotent() {
        let base = test_base_path("idempotent");
        let ws = WorkspaceManager::new(&base);

        // Cleanup non-existent workspace should succeed
        ws.cleanup("nonexistent").unwrap();

        // Cleanup all when base doesn't exist should succeed
        ws.cleanup_all().unwrap();
    }

    #[test]
    fn test_multiple_workspaces() {
        let base = test_base_path("multiple");
        let ws = WorkspaceManager::new(&base);

        // Create multiple workspaces
        let path1 = ws.create_workspace("scenario-a").unwrap();
        let path2 = ws.create_workspace("scenario-b").unwrap();
        let path3 = ws.create_workspace("scenario-c").unwrap();

        assert!(path1.exists());
        assert!(path2.exists());
        assert!(path3.exists());

        // List workspaces
        let workspaces = ws.list_workspaces().unwrap();
        assert_eq!(workspaces, vec!["scenario-a", "scenario-b", "scenario-c"]);

        // Cleanup one
        ws.cleanup("scenario-b").unwrap();
        assert!(path1.exists());
        assert!(!path2.exists());
        assert!(path3.exists());

        let workspaces = ws.list_workspaces().unwrap();
        assert_eq!(workspaces, vec!["scenario-a", "scenario-c"]);

        // Cleanup all
        ws.cleanup_all().unwrap();
        assert!(!base.exists());
    }

    #[test]
    fn test_workspace_exists() {
        let base = test_base_path("exists");
        let ws = WorkspaceManager::new(&base);

        assert!(!ws.workspace_exists("test"));

        ws.create_workspace("test").unwrap();
        assert!(ws.workspace_exists("test"));

        ws.cleanup_all().unwrap();
    }

    #[test]
    fn test_workspace_with_files() {
        let base = test_base_path("files");
        let ws = WorkspaceManager::new(&base);

        // Create workspace
        let path = ws.create_workspace("with-files").unwrap();

        // Create some files in the workspace (simulating test artifacts)
        fs::write(path.join("hats.yml"), "cli:\n  backend: claude\n").unwrap();
        fs::write(path.join("prompt.md"), "Say hello").unwrap();
        fs::write(path.join(".agent").join("scratchpad.md"), "# Scratchpad\n").unwrap();

        // Verify files exist
        assert!(path.join("hats.yml").exists());
        assert!(path.join("prompt.md").exists());
        assert!(path.join(".agent").join("scratchpad.md").exists());

        // Cleanup should remove everything
        ws.cleanup("with-files").unwrap();
        assert!(!path.exists());

        ws.cleanup_all().unwrap();
    }

    #[test]
    fn test_base_path_accessor() {
        let base = PathBuf::from("/tmp/test-base");
        let ws = WorkspaceManager::new(&base);
        assert_eq!(ws.base_path(), &base);
    }

    #[test]
    fn test_workspace_path_without_creation() {
        let base = PathBuf::from("/tmp/test-base");
        let ws = WorkspaceManager::new(&base);

        let path = ws.workspace_path("my-scenario");
        assert_eq!(path, PathBuf::from("/tmp/test-base/my-scenario"));
        // Note: path should NOT exist since we didn't call create_workspace
    }
}
