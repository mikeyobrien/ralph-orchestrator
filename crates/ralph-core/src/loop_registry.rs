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
use std::fs::{self, File, OpenOptions};
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
        Self {
            id: Self::generate_id(),
            pid: process::id(),
            started: Utc::now(),
            prompt: prompt.into(),
            worktree_path: worktree_path.map(Into::into),
            workspace: std::env::current_dir()
                .map(|p| p.display().to_string())
                .unwrap_or_default(),
        }
    }

    /// Creates a new loop entry with a specific workspace.
    pub fn with_workspace(
        prompt: impl Into<String>,
        worktree_path: Option<impl Into<String>>,
        workspace: impl Into<String>,
    ) -> Self {
        Self {
            id: Self::generate_id(),
            pid: process::id(),
            started: Utc::now(),
            prompt: prompt.into(),
            worktree_path: worktree_path.map(Into::into),
            workspace: workspace.into(),
        }
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
        use std::time::{SystemTime, UNIX_EPOCH};
        let duration = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards");
        let timestamp = duration.as_secs();
        let hex_suffix = format!("{:04x}", duration.subsec_micros() % 0x10000);
        format!("loop-{}-{}", timestamp, hex_suffix)
    }

    /// Checks if the process for this loop is still running.
    #[cfg(unix)]
    pub fn is_alive(&self) -> bool {
        use nix::sys::signal::kill;
        use nix::unistd::Pid;

        // Signal 0 (None) checks if process exists without sending a signal
        kill(Pid::from_raw(self.pid as i32), None)
            .map(|_| true)
            .unwrap_or(false)
    }

    #[cfg(not(unix))]
    pub fn is_alive(&self) -> bool {
        // On non-Unix platforms, assume alive (conservative)
        true
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
            // Remove any existing entry with the same PID (stale from crash)
            data.loops.retain(|e| e.pid != entry.pid);
            data.loops.push(entry);
        })?;
        Ok(id)
    }

    /// Deregisters a loop by ID.
    pub fn deregister(&self, id: &str) -> Result<(), RegistryError> {
        let mut found = false;
        self.with_lock(|data| {
            let original_len = data.loops.len();
            data.loops.retain(|e| e.id != id);
            found = data.loops.len() != original_len;
        })?;
        if !found {
            return Err(RegistryError::NotFound(id.to_string()));
        }
        Ok(())
    }

    /// Gets a loop entry by ID.
    pub fn get(&self, id: &str) -> Result<Option<LoopEntry>, RegistryError> {
        let mut result = None;
        self.with_lock(|data| {
            result = data.loops.iter().find(|e| e.id == id).cloned();
        })?;
        Ok(result)
    }

    /// Lists all active loops (after cleaning stale entries).
    pub fn list(&self) -> Result<Vec<LoopEntry>, RegistryError> {
        let mut result = Vec::new();
        self.with_lock(|data| {
            result = data.loops.clone();
        })?;
        Ok(result)
    }

    /// Cleans stale entries (dead PIDs) and returns the number removed.
    pub fn clean_stale(&self) -> Result<usize, RegistryError> {
        let mut removed = 0;
        self.with_lock(|data| {
            let original_len = data.loops.len();
            data.loops.retain(|e| e.is_alive());
            removed = original_len - data.loops.len();
        })?;
        Ok(removed)
    }

    /// Deregisters all entries for the current process.
    ///
    /// This is useful for cleanup on termination, since each process
    /// can only have one active loop entry.
    pub fn deregister_current_process(&self) -> Result<bool, RegistryError> {
        let pid = std::process::id();
        let mut found = false;
        self.with_lock(|data| {
            let original_len = data.loops.len();
            data.loops.retain(|e| e.pid != pid);
            found = data.loops.len() != original_len;
        })?;
        Ok(found)
    }

    /// Executes an operation with the registry file locked.
    #[cfg(unix)]
    fn with_lock<F>(&self, f: F) -> Result<(), RegistryError>
    where
        F: FnOnce(&mut RegistryData),
    {
        use nix::fcntl::{Flock, FlockArg};

        // Ensure .ralph directory exists
        if let Some(parent) = self.registry_path.parent() {
            fs::create_dir_all(parent)?;
        }

        // Open or create the file
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&self.registry_path)?;

        // Acquire exclusive lock (blocking)
        let flock = Flock::lock(file, FlockArg::LockExclusive).map_err(|(_, errno)| {
            RegistryError::Io(io::Error::new(
                io::ErrorKind::Other,
                format!("flock failed: {}", errno),
            ))
        })?;

        // Read existing data using the locked file
        let mut data = self.read_data_from_file(&flock)?;

        // Clean stale entries before any operation
        data.loops.retain(|e| e.is_alive());

        // Execute the user function
        f(&mut data);

        // Write back the data
        self.write_data_to_file(&flock, &data)?;

        Ok(())
    }

    #[cfg(not(unix))]
    fn with_lock<F>(&self, _f: F) -> Result<(), RegistryError>
    where
        F: FnOnce(&mut RegistryData),
    {
        Err(RegistryError::UnsupportedPlatform)
    }

    /// Reads registry data from a locked file.
    #[cfg(unix)]
    fn read_data_from_file(
        &self,
        flock: &nix::fcntl::Flock<File>,
    ) -> Result<RegistryData, RegistryError> {
        use std::os::fd::AsFd;

        // Get a clone of the underlying file via BorrowedFd
        let borrowed_fd = flock.as_fd();
        let owned_fd = borrowed_fd.try_clone_to_owned()?;
        let mut file: File = owned_fd.into();

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
        use std::os::fd::AsFd;

        // Get a clone of the underlying file via BorrowedFd
        let borrowed_fd = flock.as_fd();
        let owned_fd = borrowed_fd.try_clone_to_owned()?;
        let mut file: File = owned_fd.into();

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

        let entry1 = LoopEntry::new("prompt 1", None::<String>);
        let entry2 = LoopEntry::new("prompt 2", Some("/worktree"));

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
}
