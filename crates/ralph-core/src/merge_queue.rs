//! Merge queue for tracking parallel loop merges.
//!
//! The merge queue maintains an append-only log of merge events, tracking
//! loops from completion through successful merge or failure. It uses JSONL
//! format for durability and easy debugging.
//!
//! # Design
//!
//! - **JSONL persistence**: Append-only log at `.ralph/merge-queue.jsonl`
//! - **File locking**: Uses `flock()` for concurrent access safety
//! - **Event sourcing**: State is derived from event history
//!
//! # Example
//!
//! ```no_run
//! use ralph_core::merge_queue::{MergeQueue, MergeQueueError};
//!
//! fn main() -> Result<(), MergeQueueError> {
//!     let queue = MergeQueue::new(".");
//!
//!     // Queue a completed loop for merge
//!     queue.enqueue("ralph-20250124-a3f2", "implement auth")?;
//!
//!     // Get next pending loop
//!     if let Some(entry) = queue.next_pending()? {
//!         // Mark as merging
//!         queue.mark_merging(&entry.loop_id, std::process::id())?;
//!
//!         // ... perform merge ...
//!
//!         // Mark result
//!         queue.mark_merged(&entry.loop_id, "abc123def")?;
//!     }
//!
//!     Ok(())
//! }
//! ```

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io::{self, BufRead, BufReader, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

/// A merge queue event recorded in the JSONL log.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MergeEvent {
    /// Timestamp of the event.
    pub ts: DateTime<Utc>,

    /// Loop ID this event relates to.
    pub loop_id: String,

    /// Type of event.
    pub event: MergeEventType,
}

/// Types of merge events.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum MergeEventType {
    /// Loop has been queued for merge.
    Queued {
        /// The prompt that was executed in this loop.
        prompt: String,
    },

    /// Merge operation has started.
    Merging {
        /// PID of the merge-ralph process.
        pid: u32,
    },

    /// Merge completed successfully.
    Merged {
        /// The commit SHA of the merge commit.
        commit: String,
    },

    /// Merge failed and needs manual review.
    NeedsReview {
        /// Reason for the failure.
        reason: String,
    },

    /// Loop was manually discarded.
    Discarded {
        /// Reason for discarding (optional).
        reason: Option<String>,
    },
}

/// Current state of a loop in the merge queue.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MergeState {
    /// Waiting to be merged.
    Queued,
    /// Currently being merged.
    Merging,
    /// Successfully merged.
    Merged,
    /// Needs manual review.
    NeedsReview,
    /// Discarded by user.
    Discarded,
}

/// Summary of a loop's merge status.
#[derive(Debug, Clone)]
pub struct MergeEntry {
    /// Loop ID.
    pub loop_id: String,

    /// Original prompt.
    pub prompt: String,

    /// Current state.
    pub state: MergeState,

    /// When the loop was queued.
    pub queued_at: DateTime<Utc>,

    /// PID of merge-ralph if merging.
    pub merge_pid: Option<u32>,

    /// Merge commit SHA if merged.
    pub merge_commit: Option<String>,

    /// Failure reason if needs_review.
    pub failure_reason: Option<String>,

    /// Discard reason if discarded.
    pub discard_reason: Option<String>,
}

/// Errors that can occur during merge queue operations.
#[derive(Debug, thiserror::Error)]
pub enum MergeQueueError {
    /// IO error during queue operations.
    #[error("IO error: {0}")]
    Io(#[from] io::Error),

    /// Failed to parse queue data.
    #[error("Failed to parse merge queue: {0}")]
    ParseError(String),

    /// Loop entry not found.
    #[error("Loop not found in queue: {0}")]
    NotFound(String),

    /// Invalid state transition.
    #[error("Invalid state transition for {0}: cannot transition from {1:?} to {2:?}")]
    InvalidTransition(String, MergeState, MergeState),

    /// Platform not supported.
    #[error("File locking not supported on this platform")]
    UnsupportedPlatform,
}

/// Merge queue for tracking parallel loop merges.
///
/// The queue maintains an append-only JSONL log of merge events.
/// State is derived by replaying events for each loop.
pub struct MergeQueue {
    /// Path to the merge queue file.
    queue_path: PathBuf,
}

impl MergeQueue {
    /// The relative path to the merge queue file within the workspace.
    pub const QUEUE_FILE: &'static str = ".ralph/merge-queue.jsonl";

    /// Creates a new merge queue instance for the given workspace.
    pub fn new(workspace_root: impl AsRef<Path>) -> Self {
        Self {
            queue_path: workspace_root.as_ref().join(Self::QUEUE_FILE),
        }
    }

    /// Enqueues a completed loop for merging.
    ///
    /// # Arguments
    ///
    /// * `loop_id` - The loop identifier
    /// * `prompt` - The prompt that was executed
    pub fn enqueue(&self, loop_id: &str, prompt: &str) -> Result<(), MergeQueueError> {
        let event = MergeEvent {
            ts: Utc::now(),
            loop_id: loop_id.to_string(),
            event: MergeEventType::Queued {
                prompt: prompt.to_string(),
            },
        };
        self.append_event(&event)
    }

    /// Marks a loop as being merged.
    ///
    /// # Arguments
    ///
    /// * `loop_id` - The loop identifier
    /// * `pid` - PID of the merge-ralph process
    pub fn mark_merging(&self, loop_id: &str, pid: u32) -> Result<(), MergeQueueError> {
        // Verify loop is in queued or needs_review state
        let entry = self.get_entry(loop_id)?;
        match entry {
            Some(e) if e.state == MergeState::Queued || e.state == MergeState::NeedsReview => {}
            Some(e) => {
                return Err(MergeQueueError::InvalidTransition(
                    loop_id.to_string(),
                    e.state,
                    MergeState::Merging,
                ));
            }
            None => return Err(MergeQueueError::NotFound(loop_id.to_string())),
        }

        let event = MergeEvent {
            ts: Utc::now(),
            loop_id: loop_id.to_string(),
            event: MergeEventType::Merging { pid },
        };
        self.append_event(&event)
    }

    /// Marks a loop as successfully merged.
    ///
    /// # Arguments
    ///
    /// * `loop_id` - The loop identifier
    /// * `commit` - The merge commit SHA
    pub fn mark_merged(&self, loop_id: &str, commit: &str) -> Result<(), MergeQueueError> {
        // Verify loop is in merging state
        let entry = self.get_entry(loop_id)?;
        match entry {
            Some(e) if e.state == MergeState::Merging => {}
            Some(e) => {
                return Err(MergeQueueError::InvalidTransition(
                    loop_id.to_string(),
                    e.state,
                    MergeState::Merged,
                ));
            }
            None => return Err(MergeQueueError::NotFound(loop_id.to_string())),
        }

        let event = MergeEvent {
            ts: Utc::now(),
            loop_id: loop_id.to_string(),
            event: MergeEventType::Merged {
                commit: commit.to_string(),
            },
        };
        self.append_event(&event)
    }

    /// Marks a loop as needing manual review.
    ///
    /// # Arguments
    ///
    /// * `loop_id` - The loop identifier
    /// * `reason` - Reason for the failure
    pub fn mark_needs_review(&self, loop_id: &str, reason: &str) -> Result<(), MergeQueueError> {
        // Verify loop is in merging state
        let entry = self.get_entry(loop_id)?;
        match entry {
            Some(e) if e.state == MergeState::Merging => {}
            Some(e) => {
                return Err(MergeQueueError::InvalidTransition(
                    loop_id.to_string(),
                    e.state,
                    MergeState::NeedsReview,
                ));
            }
            None => return Err(MergeQueueError::NotFound(loop_id.to_string())),
        }

        let event = MergeEvent {
            ts: Utc::now(),
            loop_id: loop_id.to_string(),
            event: MergeEventType::NeedsReview {
                reason: reason.to_string(),
            },
        };
        self.append_event(&event)
    }

    /// Marks a loop as discarded.
    ///
    /// # Arguments
    ///
    /// * `loop_id` - The loop identifier
    /// * `reason` - Optional reason for discarding
    pub fn discard(&self, loop_id: &str, reason: Option<&str>) -> Result<(), MergeQueueError> {
        // Can discard from queued or needs_review states
        let entry = self.get_entry(loop_id)?;
        match entry {
            Some(e) if e.state == MergeState::Queued || e.state == MergeState::NeedsReview => {}
            Some(e) => {
                return Err(MergeQueueError::InvalidTransition(
                    loop_id.to_string(),
                    e.state,
                    MergeState::Discarded,
                ));
            }
            None => return Err(MergeQueueError::NotFound(loop_id.to_string())),
        }

        let event = MergeEvent {
            ts: Utc::now(),
            loop_id: loop_id.to_string(),
            event: MergeEventType::Discarded {
                reason: reason.map(String::from),
            },
        };
        self.append_event(&event)
    }

    /// Gets the next pending loop ready for merge (FIFO order).
    ///
    /// Returns the oldest loop in `Queued` state.
    pub fn next_pending(&self) -> Result<Option<MergeEntry>, MergeQueueError> {
        let entries = self.list()?;
        Ok(entries.into_iter().find(|e| e.state == MergeState::Queued))
    }

    /// Gets the entry for a specific loop.
    pub fn get_entry(&self, loop_id: &str) -> Result<Option<MergeEntry>, MergeQueueError> {
        let entries = self.list()?;
        Ok(entries.into_iter().find(|e| e.loop_id == loop_id))
    }

    /// Lists all entries in the merge queue.
    ///
    /// Returns entries in chronological order (oldest first).
    pub fn list(&self) -> Result<Vec<MergeEntry>, MergeQueueError> {
        let events = self.read_all_events()?;
        Ok(Self::derive_state(&events))
    }

    /// Lists entries filtered by state.
    pub fn list_by_state(&self, state: MergeState) -> Result<Vec<MergeEntry>, MergeQueueError> {
        let entries = self.list()?;
        Ok(entries.into_iter().filter(|e| e.state == state).collect())
    }

    /// Reads all events from the queue file.
    fn read_all_events(&self) -> Result<Vec<MergeEvent>, MergeQueueError> {
        if !self.queue_path.exists() {
            return Ok(Vec::new());
        }

        self.with_shared_lock(|file| {
            let reader = BufReader::new(file);
            let mut events = Vec::new();

            for (line_num, line) in reader.lines().enumerate() {
                let line = line?;
                if line.trim().is_empty() {
                    continue;
                }

                let event: MergeEvent = serde_json::from_str(&line).map_err(|e| {
                    MergeQueueError::ParseError(format!("Line {}: {}", line_num + 1, e))
                })?;
                events.push(event);
            }

            Ok(events)
        })
    }

    /// Derives the current state of all loops from the event history.
    fn derive_state(events: &[MergeEvent]) -> Vec<MergeEntry> {
        use std::collections::HashMap;

        // Build up state for each loop
        let mut loop_states: HashMap<String, MergeEntry> = HashMap::new();

        for event in events {
            let entry = loop_states
                .entry(event.loop_id.clone())
                .or_insert_with(|| MergeEntry {
                    loop_id: event.loop_id.clone(),
                    prompt: String::new(),
                    state: MergeState::Queued,
                    queued_at: event.ts,
                    merge_pid: None,
                    merge_commit: None,
                    failure_reason: None,
                    discard_reason: None,
                });

            match &event.event {
                MergeEventType::Queued { prompt } => {
                    entry.prompt = prompt.clone();
                    entry.state = MergeState::Queued;
                    entry.queued_at = event.ts;
                }
                MergeEventType::Merging { pid } => {
                    entry.state = MergeState::Merging;
                    entry.merge_pid = Some(*pid);
                }
                MergeEventType::Merged { commit } => {
                    entry.state = MergeState::Merged;
                    entry.merge_commit = Some(commit.clone());
                }
                MergeEventType::NeedsReview { reason } => {
                    entry.state = MergeState::NeedsReview;
                    entry.failure_reason = Some(reason.clone());
                }
                MergeEventType::Discarded { reason } => {
                    entry.state = MergeState::Discarded;
                    entry.discard_reason = reason.clone();
                }
            }
        }

        // Sort by queued_at to maintain FIFO order
        let mut entries: Vec<_> = loop_states.into_values().collect();
        entries.sort_by(|a, b| a.queued_at.cmp(&b.queued_at));
        entries
    }

    /// Appends an event to the queue file.
    fn append_event(&self, event: &MergeEvent) -> Result<(), MergeQueueError> {
        self.with_exclusive_lock(|mut file| {
            // Seek to end
            file.seek(SeekFrom::End(0))?;

            // Write event as JSON line
            let json = serde_json::to_string(event)
                .map_err(|e| MergeQueueError::ParseError(e.to_string()))?;
            writeln!(file, "{}", json)?;

            file.sync_all()?;
            Ok(())
        })
    }

    /// Executes an operation with a shared (read) lock on the queue file.
    #[cfg(unix)]
    fn with_shared_lock<T, F>(&self, f: F) -> Result<T, MergeQueueError>
    where
        F: FnOnce(&File) -> Result<T, MergeQueueError>,
    {
        use nix::fcntl::{Flock, FlockArg};

        let file = File::open(&self.queue_path)?;

        // Acquire shared lock (blocking)
        let flock = Flock::lock(file, FlockArg::LockShared).map_err(|(_, errno)| {
            MergeQueueError::Io(io::Error::new(
                io::ErrorKind::Other,
                format!("flock failed: {}", errno),
            ))
        })?;

        // Get a reference to the inner file
        use std::os::fd::AsFd;
        let borrowed_fd = flock.as_fd();
        let owned_fd = borrowed_fd.try_clone_to_owned()?;
        let file: File = owned_fd.into();

        f(&file)
    }

    #[cfg(not(unix))]
    fn with_shared_lock<T, F>(&self, _f: F) -> Result<T, MergeQueueError>
    where
        F: FnOnce(&File) -> Result<T, MergeQueueError>,
    {
        Err(MergeQueueError::UnsupportedPlatform)
    }

    /// Executes an operation with an exclusive (write) lock on the queue file.
    #[cfg(unix)]
    fn with_exclusive_lock<T, F>(&self, f: F) -> Result<T, MergeQueueError>
    where
        F: FnOnce(File) -> Result<T, MergeQueueError>,
    {
        use nix::fcntl::{Flock, FlockArg};

        // Ensure .ralph directory exists
        if let Some(parent) = self.queue_path.parent() {
            fs::create_dir_all(parent)?;
        }

        // Open or create the file
        let file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .open(&self.queue_path)?;

        // Acquire exclusive lock (blocking)
        let flock = Flock::lock(file, FlockArg::LockExclusive).map_err(|(_, errno)| {
            MergeQueueError::Io(io::Error::new(
                io::ErrorKind::Other,
                format!("flock failed: {}", errno),
            ))
        })?;

        // Get a clone of the underlying file
        use std::os::fd::AsFd;
        let borrowed_fd = flock.as_fd();
        let owned_fd = borrowed_fd.try_clone_to_owned()?;
        let file: File = owned_fd.into();

        f(file)
    }

    #[cfg(not(unix))]
    fn with_exclusive_lock<T, F>(&self, _f: F) -> Result<T, MergeQueueError>
    where
        F: FnOnce(File) -> Result<T, MergeQueueError>,
    {
        Err(MergeQueueError::UnsupportedPlatform)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_enqueue() {
        let temp_dir = TempDir::new().unwrap();
        let queue = MergeQueue::new(temp_dir.path());

        queue.enqueue("loop-123", "implement auth").unwrap();

        let entries = queue.list().unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].loop_id, "loop-123");
        assert_eq!(entries[0].prompt, "implement auth");
        assert_eq!(entries[0].state, MergeState::Queued);
    }

    #[test]
    fn test_full_merge_lifecycle() {
        let temp_dir = TempDir::new().unwrap();
        let queue = MergeQueue::new(temp_dir.path());

        // Enqueue
        queue.enqueue("loop-abc", "test prompt").unwrap();
        let entry = queue.get_entry("loop-abc").unwrap().unwrap();
        assert_eq!(entry.state, MergeState::Queued);

        // Start merging
        queue.mark_merging("loop-abc", 12345).unwrap();
        let entry = queue.get_entry("loop-abc").unwrap().unwrap();
        assert_eq!(entry.state, MergeState::Merging);
        assert_eq!(entry.merge_pid, Some(12345));

        // Complete merge
        queue.mark_merged("loop-abc", "commit-sha-123").unwrap();
        let entry = queue.get_entry("loop-abc").unwrap().unwrap();
        assert_eq!(entry.state, MergeState::Merged);
        assert_eq!(entry.merge_commit, Some("commit-sha-123".to_string()));
    }

    #[test]
    fn test_merge_needs_review() {
        let temp_dir = TempDir::new().unwrap();
        let queue = MergeQueue::new(temp_dir.path());

        queue.enqueue("loop-def", "test").unwrap();
        queue.mark_merging("loop-def", 99999).unwrap();
        queue
            .mark_needs_review("loop-def", "Conflicting changes in src/auth.rs")
            .unwrap();

        let entry = queue.get_entry("loop-def").unwrap().unwrap();
        assert_eq!(entry.state, MergeState::NeedsReview);
        assert_eq!(
            entry.failure_reason,
            Some("Conflicting changes in src/auth.rs".to_string())
        );
    }

    #[test]
    fn test_discard_from_queued() {
        let temp_dir = TempDir::new().unwrap();
        let queue = MergeQueue::new(temp_dir.path());

        queue.enqueue("loop-xyz", "test").unwrap();
        queue.discard("loop-xyz", Some("No longer needed")).unwrap();

        let entry = queue.get_entry("loop-xyz").unwrap().unwrap();
        assert_eq!(entry.state, MergeState::Discarded);
        assert_eq!(entry.discard_reason, Some("No longer needed".to_string()));
    }

    #[test]
    fn test_discard_from_needs_review() {
        let temp_dir = TempDir::new().unwrap();
        let queue = MergeQueue::new(temp_dir.path());

        queue.enqueue("loop-xyz", "test").unwrap();
        queue.mark_merging("loop-xyz", 123).unwrap();
        queue.mark_needs_review("loop-xyz", "conflicts").unwrap();
        queue.discard("loop-xyz", None).unwrap();

        let entry = queue.get_entry("loop-xyz").unwrap().unwrap();
        assert_eq!(entry.state, MergeState::Discarded);
    }

    #[test]
    fn test_next_pending_fifo() {
        let temp_dir = TempDir::new().unwrap();
        let queue = MergeQueue::new(temp_dir.path());

        queue.enqueue("loop-1", "first").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(10));
        queue.enqueue("loop-2", "second").unwrap();
        std::thread::sleep(std::time::Duration::from_millis(10));
        queue.enqueue("loop-3", "third").unwrap();

        // First pending should be loop-1
        let pending = queue.next_pending().unwrap().unwrap();
        assert_eq!(pending.loop_id, "loop-1");

        // Mark loop-1 as merging
        queue.mark_merging("loop-1", 123).unwrap();

        // Next pending should be loop-2
        let pending = queue.next_pending().unwrap().unwrap();
        assert_eq!(pending.loop_id, "loop-2");
    }

    #[test]
    fn test_invalid_transition_queued_to_merged() {
        let temp_dir = TempDir::new().unwrap();
        let queue = MergeQueue::new(temp_dir.path());

        queue.enqueue("loop-xyz", "test").unwrap();

        // Can't go directly from queued to merged
        let result = queue.mark_merged("loop-xyz", "commit");
        assert!(matches!(
            result,
            Err(MergeQueueError::InvalidTransition(
                _,
                MergeState::Queued,
                MergeState::Merged
            ))
        ));
    }

    #[test]
    fn test_invalid_transition_merged_to_needs_review() {
        let temp_dir = TempDir::new().unwrap();
        let queue = MergeQueue::new(temp_dir.path());

        queue.enqueue("loop-xyz", "test").unwrap();
        queue.mark_merging("loop-xyz", 123).unwrap();
        queue.mark_merged("loop-xyz", "abc").unwrap();

        // Can't go from merged to needs_review
        let result = queue.mark_needs_review("loop-xyz", "error");
        assert!(matches!(
            result,
            Err(MergeQueueError::InvalidTransition(
                _,
                MergeState::Merged,
                MergeState::NeedsReview
            ))
        ));
    }

    #[test]
    fn test_not_found() {
        let temp_dir = TempDir::new().unwrap();
        let queue = MergeQueue::new(temp_dir.path());

        let result = queue.mark_merging("nonexistent", 123);
        assert!(matches!(result, Err(MergeQueueError::NotFound(_))));
    }

    #[test]
    fn test_retry_from_needs_review() {
        let temp_dir = TempDir::new().unwrap();
        let queue = MergeQueue::new(temp_dir.path());

        queue.enqueue("loop-retry", "test").unwrap();
        queue.mark_merging("loop-retry", 100).unwrap();
        queue.mark_needs_review("loop-retry", "conflicts").unwrap();

        // Can retry (mark_merging) from needs_review
        queue.mark_merging("loop-retry", 200).unwrap();
        let entry = queue.get_entry("loop-retry").unwrap().unwrap();
        assert_eq!(entry.state, MergeState::Merging);
        assert_eq!(entry.merge_pid, Some(200));
    }

    #[test]
    fn test_list_by_state() {
        let temp_dir = TempDir::new().unwrap();
        let queue = MergeQueue::new(temp_dir.path());

        queue.enqueue("loop-1", "test 1").unwrap();
        queue.enqueue("loop-2", "test 2").unwrap();
        queue.enqueue("loop-3", "test 3").unwrap();

        queue.mark_merging("loop-1", 123).unwrap();
        queue.mark_merged("loop-1", "abc").unwrap();

        queue.mark_merging("loop-2", 456).unwrap();

        let queued = queue.list_by_state(MergeState::Queued).unwrap();
        assert_eq!(queued.len(), 1);
        assert_eq!(queued[0].loop_id, "loop-3");

        let merging = queue.list_by_state(MergeState::Merging).unwrap();
        assert_eq!(merging.len(), 1);
        assert_eq!(merging[0].loop_id, "loop-2");

        let merged = queue.list_by_state(MergeState::Merged).unwrap();
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].loop_id, "loop-1");
    }

    #[test]
    fn test_empty_queue() {
        let temp_dir = TempDir::new().unwrap();
        let queue = MergeQueue::new(temp_dir.path());

        let entries = queue.list().unwrap();
        assert!(entries.is_empty());

        let pending = queue.next_pending().unwrap();
        assert!(pending.is_none());
    }

    #[test]
    fn test_persistence() {
        let temp_dir = TempDir::new().unwrap();

        {
            let queue = MergeQueue::new(temp_dir.path());
            queue.enqueue("loop-persist", "test persistence").unwrap();
        }

        // Load again and verify data persisted
        {
            let queue = MergeQueue::new(temp_dir.path());
            let entries = queue.list().unwrap();
            assert_eq!(entries.len(), 1);
            assert_eq!(entries[0].loop_id, "loop-persist");
            assert_eq!(entries[0].prompt, "test persistence");
        }
    }

    #[test]
    fn test_event_serialization() {
        let event = MergeEvent {
            ts: Utc::now(),
            loop_id: "loop-test".to_string(),
            event: MergeEventType::Queued {
                prompt: "test prompt".to_string(),
            },
        };

        let json = serde_json::to_string(&event).unwrap();
        let parsed: MergeEvent = serde_json::from_str(&json).unwrap();

        assert_eq!(parsed.loop_id, event.loop_id);
        match parsed.event {
            MergeEventType::Queued { prompt } => assert_eq!(prompt, "test prompt"),
            _ => panic!("Wrong event type"),
        }
    }

    #[test]
    fn test_creates_ralph_directory() {
        let temp_dir = TempDir::new().unwrap();
        let ralph_dir = temp_dir.path().join(".ralph");
        let queue_file = ralph_dir.join("merge-queue.jsonl");

        assert!(!ralph_dir.exists());
        assert!(!queue_file.exists());

        let queue = MergeQueue::new(temp_dir.path());
        queue.enqueue("loop-dir", "test").unwrap();

        assert!(ralph_dir.exists());
        assert!(queue_file.exists());
    }
}
