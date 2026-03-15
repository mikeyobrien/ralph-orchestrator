use std::fs;
use std::path::{Path, PathBuf};

use chrono::{SecondsFormat, Utc};
use ralph_core::FileLock;
use serde::{Deserialize, Serialize};

use crate::errors::ApiError;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileClaim {
    pub worker_id: String,
    pub task_id: String,
    pub files: Vec<String>,
    pub claimed_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct OwnershipSnapshot {
    claims: Vec<FileClaim>,
}

/// Tracks which files each worker/task owns in a shared workspace.
///
/// Stored in `.ralph/file-ownership.json`, guarded by file locks.
pub struct FileOwnershipRegistry {
    store_path: PathBuf,
}

impl FileOwnershipRegistry {
    pub fn new(workspace_root: impl AsRef<Path>) -> Self {
        Self {
            store_path: workspace_root.as_ref().join(".ralph/file-ownership.json"),
        }
    }

    /// Register file ownership when a task is claimed by a worker.
    pub fn claim(
        &self,
        worker_id: &str,
        task_id: &str,
        files: Vec<String>,
    ) -> Result<(), ApiError> {
        if files.is_empty() {
            return Ok(());
        }

        let lock = self.file_lock()?;
        let _guard = lock.exclusive().map_err(|error| {
            ApiError::internal(format!(
                "failed locking file-ownership registry '{}': {error}",
                self.store_path.display()
            ))
        })?;

        let mut snapshot = self.read_from_disk()?;

        // Remove any stale claim for the same worker+task
        snapshot
            .claims
            .retain(|c| !(c.worker_id == worker_id && c.task_id == task_id));

        snapshot.claims.push(FileClaim {
            worker_id: worker_id.to_string(),
            task_id: task_id.to_string(),
            files,
            claimed_at: Utc::now().to_rfc3339_opts(SecondsFormat::Secs, true),
        });

        self.persist_to_disk(&snapshot)
    }

    /// Release file ownership for a worker+task pair.
    pub fn release(&self, worker_id: &str, task_id: &str) -> Result<(), ApiError> {
        let lock = self.file_lock()?;
        let _guard = lock.exclusive().map_err(|error| {
            ApiError::internal(format!(
                "failed locking file-ownership registry '{}': {error}",
                self.store_path.display()
            ))
        })?;

        let mut snapshot = self.read_from_disk()?;
        let before = snapshot.claims.len();
        snapshot
            .claims
            .retain(|c| !(c.worker_id == worker_id && c.task_id == task_id));

        if snapshot.claims.len() != before {
            self.persist_to_disk(&snapshot)?;
        }

        Ok(())
    }

    /// Release all file claims held by a specific worker.
    pub fn release_all_for_worker(&self, worker_id: &str) -> Result<(), ApiError> {
        let lock = self.file_lock()?;
        let _guard = lock.exclusive().map_err(|error| {
            ApiError::internal(format!(
                "failed locking file-ownership registry '{}': {error}",
                self.store_path.display()
            ))
        })?;

        let mut snapshot = self.read_from_disk()?;
        let before = snapshot.claims.len();
        snapshot.claims.retain(|c| c.worker_id != worker_id);

        if snapshot.claims.len() != before {
            self.persist_to_disk(&snapshot)?;
        }

        Ok(())
    }

    /// List all current file claims.
    pub fn list_claims(&self) -> Result<Vec<FileClaim>, ApiError> {
        if !self.store_path.exists() {
            return Ok(Vec::new());
        }

        let lock = self.file_lock()?;
        let _guard = lock.shared().map_err(|error| {
            ApiError::internal(format!(
                "failed locking file-ownership registry '{}': {error}",
                self.store_path.display()
            ))
        })?;

        Ok(self.read_from_disk()?.claims)
    }

    /// Get files owned by other workers (not the given worker_id).
    pub fn other_workers_files(
        &self,
        worker_id: &str,
    ) -> Result<Vec<(String, String, String)>, ApiError> {
        let claims = self.list_claims()?;
        let mut result = Vec::new();
        for claim in claims {
            if claim.worker_id != worker_id {
                for file in &claim.files {
                    result.push((file.clone(), claim.worker_id.clone(), claim.task_id.clone()));
                }
            }
        }
        Ok(result)
    }

    fn read_from_disk(&self) -> Result<OwnershipSnapshot, ApiError> {
        if !self.store_path.exists() {
            return Ok(OwnershipSnapshot::default());
        }

        let content = fs::read_to_string(&self.store_path).map_err(|error| {
            ApiError::internal(format!(
                "failed reading file-ownership registry '{}': {error}",
                self.store_path.display()
            ))
        })?;

        if content.trim().is_empty() {
            return Ok(OwnershipSnapshot::default());
        }

        serde_json::from_str(&content).map_err(|error| {
            ApiError::internal(format!(
                "failed parsing file-ownership registry '{}': {error}",
                self.store_path.display()
            ))
        })
    }

    fn persist_to_disk(&self, snapshot: &OwnershipSnapshot) -> Result<(), ApiError> {
        if let Some(parent) = self.store_path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                ApiError::internal(format!(
                    "failed creating file-ownership directory '{}': {error}",
                    parent.display()
                ))
            })?;
        }

        let payload = serde_json::to_string_pretty(snapshot).map_err(|error| {
            ApiError::internal(format!(
                "failed serializing file-ownership registry: {error}"
            ))
        })?;

        fs::write(&self.store_path, payload).map_err(|error| {
            ApiError::internal(format!(
                "failed writing file-ownership registry '{}': {error}",
                self.store_path.display()
            ))
        })
    }

    fn file_lock(&self) -> Result<FileLock, ApiError> {
        FileLock::new(&self.store_path).map_err(|error| {
            ApiError::internal(format!(
                "failed preparing file-ownership lock '{}': {error}",
                self.store_path.display()
            ))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn setup() -> (TempDir, FileOwnershipRegistry) {
        let dir = TempDir::new().unwrap();
        let registry = FileOwnershipRegistry::new(dir.path());
        (dir, registry)
    }

    #[test]
    fn test_claim_and_list() {
        let (_dir, registry) = setup();

        registry
            .claim(
                "worker-1",
                "task-1",
                vec!["src/a.rs".to_string(), "src/b.rs".to_string()],
            )
            .unwrap();

        let claims = registry.list_claims().unwrap();
        assert_eq!(claims.len(), 1);
        assert_eq!(claims[0].worker_id, "worker-1");
        assert_eq!(claims[0].task_id, "task-1");
        assert_eq!(claims[0].files, vec!["src/a.rs", "src/b.rs"]);
    }

    #[test]
    fn test_release() {
        let (_dir, registry) = setup();

        registry
            .claim("worker-1", "task-1", vec!["src/a.rs".to_string()])
            .unwrap();
        registry
            .claim("worker-2", "task-2", vec!["src/b.rs".to_string()])
            .unwrap();

        registry.release("worker-1", "task-1").unwrap();

        let claims = registry.list_claims().unwrap();
        assert_eq!(claims.len(), 1);
        assert_eq!(claims[0].worker_id, "worker-2");
    }

    #[test]
    fn test_release_all_for_worker() {
        let (_dir, registry) = setup();

        registry
            .claim("worker-1", "task-1", vec!["src/a.rs".to_string()])
            .unwrap();
        registry
            .claim("worker-1", "task-2", vec!["src/b.rs".to_string()])
            .unwrap();
        registry
            .claim("worker-2", "task-3", vec!["src/c.rs".to_string()])
            .unwrap();

        registry.release_all_for_worker("worker-1").unwrap();

        let claims = registry.list_claims().unwrap();
        assert_eq!(claims.len(), 1);
        assert_eq!(claims[0].worker_id, "worker-2");
    }

    #[test]
    fn test_other_workers_files() {
        let (_dir, registry) = setup();

        registry
            .claim("worker-1", "task-1", vec!["src/a.rs".to_string()])
            .unwrap();
        registry
            .claim("worker-2", "task-2", vec!["src/b.rs".to_string()])
            .unwrap();

        let others = registry.other_workers_files("worker-1").unwrap();
        assert_eq!(others.len(), 1);
        assert_eq!(others[0].0, "src/b.rs");
        assert_eq!(others[0].1, "worker-2");
    }

    #[test]
    fn test_empty_files_noop() {
        let (_dir, registry) = setup();

        registry.claim("worker-1", "task-1", vec![]).unwrap();

        let claims = registry.list_claims().unwrap();
        assert!(claims.is_empty());
    }

    #[test]
    fn test_list_claims_no_file() {
        let (_dir, registry) = setup();
        let claims = registry.list_claims().unwrap();
        assert!(claims.is_empty());
    }

    #[test]
    fn test_claim_replaces_stale_entry_for_same_worker_and_task() {
        let (_dir, registry) = setup();

        registry
            .claim("worker-1", "task-1", vec!["src/old.rs".to_string()])
            .unwrap();
        registry
            .claim(
                "worker-1",
                "task-1",
                vec!["src/new.rs".to_string(), "src/extra.rs".to_string()],
            )
            .unwrap();

        let claims = registry.list_claims().unwrap();
        assert_eq!(
            claims.len(),
            1,
            "stale entry should be replaced, not duplicated"
        );
        assert_eq!(claims[0].files, vec!["src/new.rs", "src/extra.rs"]);
    }

    #[test]
    fn test_multiple_workers_same_files_both_tracked() {
        let (_dir, registry) = setup();

        registry
            .claim("worker-1", "task-1", vec!["src/shared.rs".to_string()])
            .unwrap();
        registry
            .claim("worker-2", "task-2", vec!["src/shared.rs".to_string()])
            .unwrap();

        let claims = registry.list_claims().unwrap();
        assert_eq!(claims.len(), 2, "different workers can claim same files");
    }

    #[test]
    fn test_release_nonexistent_is_noop() {
        let (_dir, registry) = setup();

        // Release for a worker+task that was never claimed
        registry.release("worker-99", "task-99").unwrap();

        let claims = registry.list_claims().unwrap();
        assert!(claims.is_empty());
    }

    #[test]
    fn test_other_workers_files_empty_when_sole_worker() {
        let (_dir, registry) = setup();

        registry
            .claim("worker-1", "task-1", vec!["src/a.rs".to_string()])
            .unwrap();

        let others = registry.other_workers_files("worker-1").unwrap();
        assert!(others.is_empty());
    }

    #[test]
    fn test_release_all_for_nonexistent_worker_is_noop() {
        let (_dir, registry) = setup();

        registry
            .claim("worker-1", "task-1", vec!["src/a.rs".to_string()])
            .unwrap();

        registry.release_all_for_worker("worker-99").unwrap();

        let claims = registry.list_claims().unwrap();
        assert_eq!(claims.len(), 1, "existing claims should be untouched");
    }
}
