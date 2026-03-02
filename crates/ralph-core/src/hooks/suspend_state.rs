//! Durable suspend-state artifacts for hook-driven suspension.
//!
//! Step 7.1 groundwork:
//! - `.ralph/suspend-state.json` stores structured suspend context.
//! - `.ralph/resume-requested` is a single-use operator resume signal.
//!
//! Writes use a temp-file + rename strategy so readers never observe partial
//! content for `suspend-state.json`.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::config::{HookPhaseEvent, HookSuspendMode};

/// Current suspend-state schema version.
pub const SUSPEND_STATE_SCHEMA_VERSION: u32 = 1;

/// Runtime lifecycle state persisted in `.ralph/suspend-state.json`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SuspendLifecycleState {
    Suspended,
}

/// Durable suspend-state payload written when a hook yields `on_error: suspend`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SuspendStateRecord {
    pub schema_version: u32,
    pub state: SuspendLifecycleState,
    pub loop_id: String,
    pub phase_event: HookPhaseEvent,
    pub hook_name: String,
    pub reason: String,
    pub suspend_mode: HookSuspendMode,
    pub suspended_at: DateTime<Utc>,
}

impl SuspendStateRecord {
    /// Construct a v1 suspend-state payload.
    #[must_use]
    pub fn new(
        loop_id: impl Into<String>,
        phase_event: HookPhaseEvent,
        hook_name: impl Into<String>,
        reason: impl Into<String>,
        suspend_mode: HookSuspendMode,
        suspended_at: DateTime<Utc>,
    ) -> Self {
        Self {
            schema_version: SUSPEND_STATE_SCHEMA_VERSION,
            state: SuspendLifecycleState::Suspended,
            loop_id: loop_id.into(),
            phase_event,
            hook_name: hook_name.into(),
            reason: reason.into(),
            suspend_mode,
            suspended_at,
        }
    }
}

/// File-store for suspend/resume artifacts under a loop workspace.
#[derive(Debug, Clone)]
pub struct SuspendStateStore {
    workspace_root: PathBuf,
}

impl SuspendStateStore {
    const RALPH_DIR: &'static str = ".ralph";
    const SUSPEND_STATE_FILE: &'static str = "suspend-state.json";
    const RESUME_REQUESTED_FILE: &'static str = "resume-requested";

    #[must_use]
    pub fn new(workspace_root: impl AsRef<Path>) -> Self {
        Self {
            workspace_root: workspace_root.as_ref().to_path_buf(),
        }
    }

    #[must_use]
    pub fn workspace_root(&self) -> &Path {
        &self.workspace_root
    }

    #[must_use]
    pub fn ralph_dir(&self) -> PathBuf {
        self.workspace_root.join(Self::RALPH_DIR)
    }

    #[must_use]
    pub fn suspend_state_path(&self) -> PathBuf {
        self.ralph_dir().join(Self::SUSPEND_STATE_FILE)
    }

    #[must_use]
    pub fn resume_requested_path(&self) -> PathBuf {
        self.ralph_dir().join(Self::RESUME_REQUESTED_FILE)
    }

    /// Atomically write suspend-state JSON.
    pub fn write_suspend_state(
        &self,
        state: &SuspendStateRecord,
    ) -> Result<(), SuspendStateStoreError> {
        let bytes = serde_json::to_vec_pretty(state)
            .map_err(|source| SuspendStateStoreError::SerializeState { source })?;
        self.write_atomic_file(&self.suspend_state_path(), &bytes)
    }

    /// Read suspend-state JSON if present.
    pub fn read_suspend_state(&self) -> Result<Option<SuspendStateRecord>, SuspendStateStoreError> {
        let path = self.suspend_state_path();
        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(source) if source.kind() == io::ErrorKind::NotFound => return Ok(None),
            Err(source) => {
                return Err(SuspendStateStoreError::Io {
                    action: "read suspend-state",
                    path,
                    source,
                });
            }
        };

        serde_json::from_slice(&bytes)
            .map(Some)
            .map_err(|source| SuspendStateStoreError::DeserializeState { path, source })
    }

    /// Remove suspend-state if present.
    pub fn clear_suspend_state(&self) -> Result<bool, SuspendStateStoreError> {
        remove_if_exists(&self.suspend_state_path(), "clear suspend-state")
    }

    /// Atomically write a resume signal file.
    pub fn write_resume_requested(&self) -> Result<(), SuspendStateStoreError> {
        self.write_atomic_file(&self.resume_requested_path(), b"")
    }

    /// True when a resume signal file exists.
    #[must_use]
    pub fn is_resume_requested(&self) -> bool {
        self.resume_requested_path().exists()
    }

    /// Consume a single-use resume signal file.
    pub fn consume_resume_requested(&self) -> Result<bool, SuspendStateStoreError> {
        remove_if_exists(&self.resume_requested_path(), "consume resume signal")
    }

    fn write_atomic_file(
        &self,
        destination: &Path,
        bytes: &[u8],
    ) -> Result<(), SuspendStateStoreError> {
        let parent = destination
            .parent()
            .ok_or_else(|| SuspendStateStoreError::Io {
                action: "resolve atomic write parent",
                path: destination.to_path_buf(),
                source: io::Error::new(io::ErrorKind::InvalidInput, "destination has no parent"),
            })?;

        fs::create_dir_all(parent).map_err(|source| SuspendStateStoreError::Io {
            action: "create suspend-state directory",
            path: parent.to_path_buf(),
            source,
        })?;

        let temp_path = parent.join(temp_file_name(destination));

        fs::write(&temp_path, bytes).map_err(|source| SuspendStateStoreError::Io {
            action: "write temporary suspend-state artifact",
            path: temp_path.clone(),
            source,
        })?;

        if let Err(source) = fs::rename(&temp_path, destination) {
            let _ = fs::remove_file(&temp_path);
            return Err(SuspendStateStoreError::Io {
                action: "atomically replace suspend-state artifact",
                path: destination.to_path_buf(),
                source,
            });
        }

        Ok(())
    }
}

/// Suspend-state store operations that can fail.
#[derive(Debug, thiserror::Error)]
pub enum SuspendStateStoreError {
    #[error("I/O error while {action} at {path}: {source}")]
    Io {
        action: &'static str,
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error("failed to serialize suspend-state JSON: {source}")]
    SerializeState {
        #[source]
        source: serde_json::Error,
    },

    #[error("failed to parse suspend-state JSON at {path}: {source}")]
    DeserializeState {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
}

fn remove_if_exists(path: &Path, action: &'static str) -> Result<bool, SuspendStateStoreError> {
    match fs::remove_file(path) {
        Ok(()) => Ok(true),
        Err(source) if source.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(source) => Err(SuspendStateStoreError::Io {
            action,
            path: path.to_path_buf(),
            source,
        }),
    }
}

fn temp_file_name(destination: &Path) -> String {
    let file_name = destination
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("suspend-artifact");

    format!(
        ".{file_name}.tmp-{}-{}",
        std::process::id(),
        unix_epoch_nanos()
    )
}

fn unix_epoch_nanos() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn fixed_time() -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 2, 28, 15, 31, 0)
            .single()
            .expect("valid timestamp")
    }

    fn sample_record() -> SuspendStateRecord {
        SuspendStateRecord::new(
            "loop-1234-abcd",
            HookPhaseEvent::PreIterationStart,
            "manual-gate",
            "operator approval required",
            HookSuspendMode::WaitForResume,
            fixed_time(),
        )
    }

    #[test]
    fn test_suspend_state_record_serializes_v1_schema_shape() {
        let value = serde_json::to_value(sample_record()).expect("serialize state");

        assert_eq!(value["schema_version"], 1);
        assert_eq!(value["state"], "suspended");
        assert_eq!(value["loop_id"], "loop-1234-abcd");
        assert_eq!(value["phase_event"], "pre.iteration.start");
        assert_eq!(value["hook_name"], "manual-gate");
        assert_eq!(value["reason"], "operator approval required");
        assert_eq!(value["suspend_mode"], "wait_for_resume");
        assert_eq!(value["suspended_at"], "2026-02-28T15:31:00Z");
    }

    #[test]
    fn test_paths_resolve_under_workspace_ralph_dir() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let store = SuspendStateStore::new(temp_dir.path());

        assert_eq!(
            store.suspend_state_path(),
            temp_dir.path().join(".ralph/suspend-state.json")
        );
        assert_eq!(
            store.resume_requested_path(),
            temp_dir.path().join(".ralph/resume-requested")
        );
    }

    #[test]
    fn test_write_and_read_suspend_state_round_trip() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let store = SuspendStateStore::new(temp_dir.path());
        let state = sample_record();

        store
            .write_suspend_state(&state)
            .expect("write suspend state");

        let read_back = store
            .read_suspend_state()
            .expect("read state")
            .expect("state present");

        assert_eq!(read_back, state);
    }

    #[test]
    fn test_write_suspend_state_replaces_file_without_leaking_temp_files() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let store = SuspendStateStore::new(temp_dir.path());

        let mut first = sample_record();
        first.hook_name = "first-hook".to_string();
        store
            .write_suspend_state(&first)
            .expect("write first state");

        let mut second = sample_record();
        second.hook_name = "second-hook".to_string();
        store
            .write_suspend_state(&second)
            .expect("write second state");

        let contents =
            fs::read_to_string(store.suspend_state_path()).expect("read suspend-state contents");
        assert!(contents.contains("\"hook_name\": \"second-hook\""));

        let temp_files: Vec<String> = fs::read_dir(store.ralph_dir())
            .expect("read .ralph dir")
            .filter_map(Result::ok)
            .map(|entry| entry.file_name().to_string_lossy().to_string())
            .filter(|name| name.contains(".tmp-"))
            .collect();
        assert!(temp_files.is_empty(), "temp files leaked: {temp_files:?}");
    }

    #[test]
    fn test_resume_signal_is_single_use() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let store = SuspendStateStore::new(temp_dir.path());

        assert!(!store.is_resume_requested());
        assert!(
            !store
                .consume_resume_requested()
                .expect("consume absent signal")
        );

        store.write_resume_requested().expect("write resume signal");
        assert!(store.is_resume_requested());

        assert!(
            store
                .consume_resume_requested()
                .expect("consume present signal")
        );
        assert!(!store.is_resume_requested());
        assert!(
            !store
                .consume_resume_requested()
                .expect("consume absent signal again")
        );
    }

    #[test]
    fn test_clear_suspend_state_is_idempotent() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let store = SuspendStateStore::new(temp_dir.path());

        assert!(
            !store
                .clear_suspend_state()
                .expect("clear absent suspend state")
        );

        store
            .write_suspend_state(&sample_record())
            .expect("write suspend state");
        assert!(
            store
                .clear_suspend_state()
                .expect("clear present suspend state")
        );

        assert!(store.read_suspend_state().expect("read state").is_none());
    }

    #[test]
    fn test_read_suspend_state_invalid_json_returns_deserialize_error() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let store = SuspendStateStore::new(temp_dir.path());

        fs::create_dir_all(store.ralph_dir()).expect("create .ralph dir");
        fs::write(store.suspend_state_path(), "not-json").expect("write invalid json");

        let err = store
            .read_suspend_state()
            .expect_err("invalid json should fail");

        assert!(matches!(
            err,
            SuspendStateStoreError::DeserializeState { .. }
        ));
    }
}
