//! Loop registry for tracking active Ralph loops across workspaces.
//!
//! The registry maintains a list of running loops with their metadata,
//! providing discovery and coordination capabilities for multi-loop scenarios.
//!
//! # Design
//!
//! - **JSON persistence**: Single JSON file at `.ralph/loops.json`
//! - **File locking**: Uses `flock()` for concurrent access safety
//! - **PID-based stale detection**: Automatically cleans up entries for dead processes
//!
//! # Example
//!
//! ```no_run
//! use ralph_core::loop_registry::{LoopRegistry, LoopEntry};
//!
//! fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let registry = LoopRegistry::new(".");
//!
//!     // Register this loop
//!     let entry = LoopEntry::new("implement auth", Some("/path/to/worktree"));
//!     let id = registry.register(entry)?;
//!
//!     // List all active loops
//!     for loop_entry in registry.list()? {
//!         println!("Loop {}: {}", loop_entry.id, loop_entry.prompt);
//!     }
//!
//!     // Deregister when done
//!     registry.deregister(&id)?;
//!     Ok(())
//! }
//! ```

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::process;

/// Metadata for a registered loop.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct LoopEntry {
    /// Unique loop ID: loop-{unix_timestamp}-{4_hex_chars}
    pub id: String,

    /// Process ID of the loop.
    pub pid: u32,

    /// When the loop was started.
    pub started: DateTime<Utc>,

    /// The prompt/task being executed.
    pub prompt: String,

    /// Path to the worktree (None if running in main workspace).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub worktree_path: Option<String>,

    /// The workspace root where the loop is running.
    pub workspace: String,
}

impl LoopEntry {
    /// Creates a new loop entry for the current process.
    pub fn new(prompt: impl Into<String>, worktree_path: Option<impl Into<String>>) -> Self {
        let workspace = std::env::current_dir()
            .map(|p| p.display().to_string())
            .unwrap_or_default();
        Self::with_id(Self::generate_id(), prompt, worktree_path, workspace)
    }

    /// Creates a new loop entry with a specific ID.
    ///
    /// Use this when you need the loop ID to match other identifiers
    /// (e.g., worktree name, branch name).
    pub fn with_id(
        id: impl Into<String>,
        prompt: impl Into<String>,
        worktree_path: Option<impl Into<String>>,
        workspace: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            pid: process::id(),
            started: Utc::now(),
            prompt: prompt.into(),
            worktree_path: worktree_path.map(Into::into),
            workspace: workspace.into(),
        }
    }

    /// Generates a unique loop ID: loop-{timestamp}-{hex_suffix}
    fn generate_id() -> String {
        crate::utils::generate_prefixed_id("loop")
    }

    /// Checks if the process for this loop is still running.
    ///
    /// For worktree loops, also verifies the worktree directory still exists.
    /// A process whose worktree has been removed externally is considered dead
    /// (zombie) even if the PID is still alive.
    pub fn is_alive(&self) -> bool {
        if !crate::utils::is_process_alive(self.pid) {
            return false;
        }

        if let Some(ref wt_path) = self.worktree_path {
            return std::path::Path::new(wt_path).is_dir();
        }

        true
    }

    /// Checks if the PID is alive (regardless of worktree state).
    pub fn is_pid_alive(&self) -> bool {
        crate::utils::is_process_alive(self.pid)
    }
}

/// The persisted registry data.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
struct RegistryData {
    loops: Vec<LoopEntry>,
}

/// Errors that can occur during registry operations.
#[derive(Debug, thiserror::Error)]
pub enum RegistryError {
    /// IO error during registry operations.
    #[error("IO error: {0}")]
    Io(#[from] io::Error),

    /// Failed to parse registry data.
    #[error("Failed to parse registry: {0}")]
    ParseError(String),

    /// Loop entry not found.
    #[error("Loop not found: {0}")]
    NotFound(String),

    /// Platform not supported.
    #[error("File locking not supported on this platform")]
    UnsupportedPlatform,
}

/// Registry for tracking active Ralph loops.
///
/// Provides thread-safe registration and discovery of running loops.
pub struct LoopRegistry {
    /// Path to the registry file.
    registry_path: PathBuf,
}

impl LoopRegistry {
    /// The relative path to the registry file within the workspace.
    pub const REGISTRY_FILE: &'static str = ".ralph/loops.json";

    /// Creates a new registry instance for the given workspace.
    pub fn new(workspace_root: impl AsRef<Path>) -> Self {
        Self {
            registry_path: workspace_root.as_ref().join(Self::REGISTRY_FILE),
        }
    }

    /// Registers a new loop entry.
    ///
    /// Returns the entry's ID for later deregistration.
    pub fn register(&self, entry: LoopEntry) -> Result<String, RegistryError> {
        let id = entry.id.clone();
        self.with_lock(|data| {
            data.loops.retain(|e| e.pid != entry.pid);
            data.loops.push(entry);
        })?;
        Ok(id)
    }

    /// Deregisters a loop by ID.
    pub fn deregister(&self, id: &str) -> Result<(), RegistryError> {
        let found = self.with_lock(|data| {
            let original_len = data.loops.len();
            data.loops.retain(|e| e.id != id);
            data.loops.len() != original_len
        })?;
        if !found {
            return Err(RegistryError::NotFound(id.to_string()));
        }
        Ok(())
    }

    /// Gets a loop entry by ID.
    pub fn get(&self, id: &str) -> Result<Option<LoopEntry>, RegistryError> {
        self.with_lock(|data| data.loops.iter().find(|e| e.id == id).cloned())
    }

    /// Lists all active loops (after cleaning stale entries).
    pub fn list(&self) -> Result<Vec<LoopEntry>, RegistryError> {
        self.with_lock(|data| data.loops.clone())
    }

    /// Cleans stale entries (dead PIDs) and returns the number removed.
    pub fn clean_stale(&self) -> Result<usize, RegistryError> {
        self.with_lock(|data| {
            let original_len = data.loops.len();
            data.loops.retain(|e| e.is_alive());
            original_len - data.loops.len()
        })
    }

    /// Deregisters all entries for the current process.
    ///
    /// This is useful for cleanup on termination, since each process
    /// can only have one active loop entry.
    pub fn deregister_current_process(&self) -> Result<bool, RegistryError> {
        let pid = std::process::id();
        self.with_lock(|data| {
            let original_len = data.loops.len();
            data.loops.retain(|e| e.pid != pid);
            data.loops.len() != original_len
        })
    }

    /// Executes an operation with the registry file locked, returning its result.
    #[cfg(unix)]
    fn with_lock<F, R>(&self, f: F) -> Result<R, RegistryError>
    where
        F: FnOnce(&mut RegistryData) -> R,
    {
        use nix::fcntl::{Flock, FlockArg};

        crate::utils::ensure_parent_dir(&self.registry_path)?;

        let file = crate::utils::open_read_write(&self.registry_path)?;

        let flock = Flock::lock(file, FlockArg::LockExclusive)
            .map_err(|(_, errno)| RegistryError::Io(crate::utils::flock_io_error(errno)))?;

        let mut data = self.read_data_from_file(&flock)?;

        // Clean stale entries before any operation (dead PIDs only).
        // Keep zombie worktree entries (PID alive, worktree gone) so callers can
        // still discover and explicitly stop/clean them.
        data.loops.retain(|e| e.is_pid_alive());

        let result = f(&mut data);

        self.write_data_to_file(&flock, &data)?;

        Ok(result)
    }

    #[cfg(not(unix))]
    fn with_lock<F, R>(&self, _f: F) -> Result<R, RegistryError>
    where
        F: FnOnce(&mut RegistryData) -> R,
    {
        Err(RegistryError::UnsupportedPlatform)
    }

    /// Reads registry data from a locked file.
    #[cfg(unix)]
    fn read_data_from_file(
        &self,
        flock: &nix::fcntl::Flock<File>,
    ) -> Result<RegistryData, RegistryError> {
        let mut file = crate::utils::clone_file_from_flock(flock)?;
        file.seek(SeekFrom::Start(0))?;

        let mut contents = String::new();
        file.read_to_string(&mut contents)?;

        if contents.trim().is_empty() {
            return Ok(RegistryData::default());
        }

        serde_json::from_str(&contents).map_err(|e| RegistryError::ParseError(e.to_string()))
    }

    /// Writes registry data to a locked file.
    #[cfg(unix)]
    fn write_data_to_file(
        &self,
        flock: &nix::fcntl::Flock<File>,
        data: &RegistryData,
    ) -> Result<(), RegistryError> {
        let mut file = crate::utils::clone_file_from_flock(flock)?;
        file.set_len(0)?;
        file.seek(SeekFrom::Start(0))?;

        let json = serde_json::to_string_pretty(data)
            .map_err(|e| RegistryError::ParseError(e.to_string()))?;

        file.write_all(json.as_bytes())?;
        file.sync_all()?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_loop_entry_creation() {
        let entry = LoopEntry::new("test prompt", None::<String>);
        assert!(entry.id.starts_with("loop-"));
        assert_eq!(entry.pid, process::id());
        assert_eq!(entry.prompt, "test prompt");
        assert!(entry.worktree_path.is_none());
    }

    #[test]
    fn test_loop_entry_with_worktree() {
        let entry = LoopEntry::new("test prompt", Some("/path/to/worktree"));
        assert_eq!(entry.worktree_path, Some("/path/to/worktree".to_string()));
    }

    #[test]
    fn test_loop_entry_id_format() {
        let entry = LoopEntry::new("test", None::<String>);
        let parts: Vec<&str> = entry.id.split('-').collect();
        assert_eq!(parts.len(), 3);
        assert_eq!(parts[0], "loop");
    }

    #[test]
    fn test_loop_entry_is_alive() {
        let entry = LoopEntry::new("test", None::<String>);
        // Current process should be alive
        assert!(entry.is_alive());
    }

    #[test]
    fn test_loop_entry_with_id() {
        let entry = LoopEntry::with_id(
            "bright-maple",
            "test prompt",
            Some("/path/to/worktree"),
            "/workspace",
        );
        assert_eq!(entry.id, "bright-maple");
        assert_eq!(entry.pid, process::id());
        assert_eq!(entry.prompt, "test prompt");
        assert_eq!(entry.worktree_path, Some("/path/to/worktree".to_string()));
        assert_eq!(entry.workspace, "/workspace");
    }

    #[test]
    fn test_registry_creates_file() {
        let temp_dir = TempDir::new().unwrap();
        let registry_path = temp_dir.path().join(".ralph/loops.json");

        assert!(!registry_path.exists());

        let registry = LoopRegistry::new(temp_dir.path());
        let entry = LoopEntry::new("test prompt", None::<String>);
        registry.register(entry).unwrap();

        assert!(registry_path.exists());
    }

    #[test]
    fn test_registry_register_and_list() {
        let temp_dir = TempDir::new().unwrap();
        let registry = LoopRegistry::new(temp_dir.path());

        let entry = LoopEntry::new("test prompt", None::<String>);
        let id = entry.id.clone();

        registry.register(entry).unwrap();

        let loops = registry.list().unwrap();
        assert_eq!(loops.len(), 1);
        assert_eq!(loops[0].id, id);
        assert_eq!(loops[0].prompt, "test prompt");
    }

    #[test]
    fn test_registry_get() {
        let temp_dir = TempDir::new().unwrap();
        let registry = LoopRegistry::new(temp_dir.path());

        let entry = LoopEntry::new("test prompt", None::<String>);
        let id = entry.id.clone();

        registry.register(entry).unwrap();

        let retrieved = registry.get(&id).unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().prompt, "test prompt");
    }

    #[test]
    fn test_registry_get_nonexistent() {
        let temp_dir = TempDir::new().unwrap();
        let registry = LoopRegistry::new(temp_dir.path());

        // Need to create the file first
        let entry = LoopEntry::new("dummy", None::<String>);
        let id = entry.id.clone();
        registry.register(entry).unwrap();
        registry.deregister(&id).unwrap();

        let retrieved = registry.get("nonexistent").unwrap();
        assert!(retrieved.is_none());
    }

    #[test]
    fn test_registry_deregister() {
        let temp_dir = TempDir::new().unwrap();
        let registry = LoopRegistry::new(temp_dir.path());

        let entry = LoopEntry::new("test prompt", None::<String>);
        let id = entry.id.clone();

        registry.register(entry).unwrap();
        assert_eq!(registry.list().unwrap().len(), 1);

        registry.deregister(&id).unwrap();
        assert_eq!(registry.list().unwrap().len(), 0);
    }

    #[test]
    fn test_registry_deregister_nonexistent() {
        let temp_dir = TempDir::new().unwrap();
        let registry = LoopRegistry::new(temp_dir.path());

        // Register and deregister to create the file
        let entry = LoopEntry::new("dummy", None::<String>);
        let id = entry.id.clone();
        registry.register(entry).unwrap();
        registry.deregister(&id).unwrap();

        let result = registry.deregister("nonexistent");
        assert!(matches!(result, Err(RegistryError::NotFound(_))));
    }

    #[test]
    fn test_registry_same_pid_replaces() {
        // Same PID entries replace each other (prevents stale entries from crashes)
        let temp_dir = TempDir::new().unwrap();
        let registry = LoopRegistry::new(temp_dir.path());

        // Create a real worktree directory so is_alive() doesn't treat it as zombie
        let wt_dir = temp_dir.path().join("worktree");
        fs::create_dir_all(&wt_dir).unwrap();

        let entry1 = LoopEntry::new("prompt 1", None::<String>);
        let entry2 = LoopEntry::new("prompt 2", Some(wt_dir.display().to_string()));

        // Both entries have the same PID (current process)
        assert_eq!(entry1.pid, entry2.pid);

        registry.register(entry1).unwrap();
        registry.register(entry2).unwrap();

        // Second entry should replace first (same PID)
        let loops = registry.list().unwrap();
        assert_eq!(loops.len(), 1);
        assert_eq!(loops[0].prompt, "prompt 2");
    }

    #[test]
    fn test_registry_different_pids_coexist() {
        // Entries with different PIDs should coexist
        let temp_dir = TempDir::new().unwrap();
        let registry = LoopRegistry::new(temp_dir.path());

        // Create entry with current PID
        let entry1 = LoopEntry::new("prompt 1", None::<String>);
        let id1 = entry1.id.clone();
        registry.register(entry1).unwrap();

        // Manually create entry with different PID (simulating another process)
        let mut entry2 = LoopEntry::new("prompt 2", Some("/worktree"));
        entry2.pid = 99999; // Fake PID - won't exist so will be cleaned as stale
        let id2 = entry2.id.clone();

        // Write entry2 directly to file to bypass PID check
        let registry_path = temp_dir.path().join(".ralph/loops.json");
        let content = fs::read_to_string(&registry_path).unwrap();
        let mut data: serde_json::Value = serde_json::from_str(&content).unwrap();
        let loops = data["loops"].as_array_mut().unwrap();
        loops.push(serde_json::json!({
            "id": id2,
            "pid": 99999,
            "started": entry2.started,
            "prompt": "prompt 2",
            "worktree_path": "/worktree",
            "workspace": entry2.workspace
        }));
        fs::write(&registry_path, serde_json::to_string_pretty(&data).unwrap()).unwrap();

        // List should clean the stale entry (PID 99999 doesn't exist)
        // But our current process entry should remain
        let loops = registry.list().unwrap();
        assert_eq!(loops.len(), 1);
        assert_eq!(loops[0].id, id1);
    }

    #[test]
    fn test_registry_replaces_same_pid() {
        let temp_dir = TempDir::new().unwrap();
        let registry = LoopRegistry::new(temp_dir.path());

        // Register first entry
        let entry1 = LoopEntry::new("prompt 1", None::<String>);
        registry.register(entry1).unwrap();

        // Register second entry with same PID (simulates restart)
        let entry2 = LoopEntry::new("prompt 2", None::<String>);
        registry.register(entry2).unwrap();

        // Should only have one entry (the new one replaced the old)
        let loops = registry.list().unwrap();
        assert_eq!(loops.len(), 1);
        assert_eq!(loops[0].prompt, "prompt 2");
    }

    #[test]
    fn test_registry_persistence() {
        let temp_dir = TempDir::new().unwrap();

        let id = {
            let registry = LoopRegistry::new(temp_dir.path());
            let entry = LoopEntry::new("persistent prompt", None::<String>);
            let id = entry.id.clone();
            registry.register(entry).unwrap();
            id
        };

        // Load again and verify data persisted
        let registry = LoopRegistry::new(temp_dir.path());
        let loops = registry.list().unwrap();
        assert_eq!(loops.len(), 1);
        assert_eq!(loops[0].id, id);
        assert_eq!(loops[0].prompt, "persistent prompt");
    }

    #[test]
    fn test_entry_serialization() {
        let entry = LoopEntry::new("test prompt", Some("/worktree/path"));
        let json = serde_json::to_string(&entry).unwrap();
        let deserialized: LoopEntry = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.id, entry.id);
        assert_eq!(deserialized.pid, entry.pid);
        assert_eq!(deserialized.prompt, "test prompt");
        assert_eq!(
            deserialized.worktree_path,
            Some("/worktree/path".to_string())
        );
    }

    #[test]
    fn test_entry_serialization_no_worktree() {
        let entry = LoopEntry::new("test prompt", None::<String>);
        let json = serde_json::to_string(&entry).unwrap();

        // Verify worktree_path is not in JSON when None
        assert!(!json.contains("worktree_path"));

        let deserialized: LoopEntry = serde_json::from_str(&json).unwrap();
        assert!(deserialized.worktree_path.is_none());
    }

    #[test]
    fn test_deregister_current_process() {
        let temp_dir = TempDir::new().unwrap();
        let registry = LoopRegistry::new(temp_dir.path());

        // Register an entry (uses current PID)
        let entry = LoopEntry::new("test prompt", None::<String>);
        registry.register(entry).unwrap();
        assert_eq!(registry.list().unwrap().len(), 1);

        // Deregister by current process
        let found = registry.deregister_current_process().unwrap();
        assert!(found);
        assert_eq!(registry.list().unwrap().len(), 0);

        // Second deregister should return false (nothing to remove)
        let found = registry.deregister_current_process().unwrap();
        assert!(!found);
    }

    #[test]
    fn test_zombie_worktree_detected_as_dead() {
        let temp_dir = TempDir::new().unwrap();

        // Create a worktree directory, then remove it
        let wt_dir = temp_dir.path().join("fake-worktree");
        fs::create_dir_all(&wt_dir).unwrap();

        let mut entry = LoopEntry::new("zombie test", Some(wt_dir.display().to_string()));
        // Use current PID so is_pid_alive() returns true
        entry.pid = process::id();

        // Worktree exists → alive
        assert!(entry.is_alive());
        assert!(entry.is_pid_alive());

        // Remove the worktree directory
        fs::remove_dir_all(&wt_dir).unwrap();

        // PID still alive, but worktree gone → zombie → is_alive() returns false
        assert!(!entry.is_alive());
        assert!(entry.is_pid_alive());
    }

    #[test]
    fn test_no_worktree_entry_unaffected() {
        // Entries without worktree_path should not be affected by the new check
        let entry = LoopEntry::new("primary loop", None::<String>);
        assert!(entry.is_alive());
        assert!(entry.is_pid_alive());
    }

    #[test]
    fn test_with_lock_keeps_zombie_until_explicit_cleanup() {
        let temp_dir = TempDir::new().unwrap();
        let registry = LoopRegistry::new(temp_dir.path());

        // Create and register a live worktree loop entry.
        let wt_dir = temp_dir.path().join("zombie-worktree");
        fs::create_dir_all(&wt_dir).unwrap();

        let entry = LoopEntry::new("zombie keep test", Some(wt_dir.display().to_string()));
        let id = entry.id.clone();
        registry.register(entry).unwrap();

        // Remove worktree: entry becomes zombie (PID alive, worktree missing).
        fs::remove_dir_all(&wt_dir).unwrap();

        // Regular registry reads should keep the zombie entry available so CLI/API
        // can report and clean it up.
        let got = registry.get(&id).unwrap();
        assert!(got.is_some());

        // Explicit stale cleanup should remove zombie entries.
        let removed = registry.clean_stale().unwrap();
        assert_eq!(removed, 1);
        assert!(registry.get(&id).unwrap().is_none());
    }
}
