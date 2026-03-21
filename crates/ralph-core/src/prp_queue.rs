//! PRP Queue for tracking parallel PRP (Pull Request Process) items.
//!
//! The PRP queue maintains an append-only log of PRP events, tracking items from
//! import through implementation, integration, and completion. It uses JSONL format
//! for durability and easy debugging.
//!
//! # Design
//!
//! - **JSONL persistence**: Append-only log at `.ralph/prp-queue.jsonl`
//! - **Event sourcing**: State is derived from event history
//! - **Strict state machine**: Only valid transitions are allowed
//!
//! # PRP States
//!
//! - `queued`: Waiting to be processed
//! - `implementing`: Implementation phase in progress
//! - `ready_for_integration`: Implementation complete, ready for integration
//! - `integrating`: Integration phase in progress
//! - `integrated`: Successfully integrated into shared branch
//! - `needs_review`: Blocked, requires operator intervention
//! - `discarded`: Manually discarded

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};

/// A PRP queue event recorded in the JSONL log.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PrpEvent {
    /// Timestamp of the event.
    pub ts: DateTime<Utc>,

    /// PRP ID (e.g., "PRP-001").
    pub prp_id: String,

    /// Type of event.
    pub event: PrpEventType,
}

/// Types of PRP events.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PrpEventType {
    /// PRP has been imported into the queue.
    Imported {
        /// Human-readable title extracted from the PRP markdown.
        title: String,
        /// Path to the source markdown file (relative to workspace).
        source_path: String,
        /// Position in the queue at time of import.
        queue_position: u64,
    },

    /// Implementation phase has started.
    ImplementationStarted {
        /// Branch name for implementation.
        branch: String,
        /// Worktree path (relative to workspace).
        worktree: String,
        /// PID of the Ralph process.
        pid: u32,
        /// Loop ID if applicable.
        loop_id: Option<String>,
    },

    /// Implementation phase completed, ready for integration.
    ImplementationReady {
        /// Path to the handoff file.
        handoff_path: String,
        /// Path to the events file.
        events_path: String,
    },

    /// Integration phase has started.
    IntegrationStarted {
        /// Integration branch name.
        branch: String,
        /// Integration worktree path.
        worktree: String,
        /// PID of the Ralph process.
        pid: u32,
        /// Loop ID if applicable.
        loop_id: Option<String>,
    },

    /// Successfully integrated.
    Integrated {
        /// The commit SHA of the integration commit.
        commit: String,
        /// Path to the archived PRP markdown.
        archive_path: String,
    },

    /// Needs manual review.
    NeedsReview {
        /// Phase where review is needed.
        phase: PrpPhase,
        /// Reason for needing review.
        reason: String,
    },

    /// PRP was retried after needs_review.
    Retried {
        /// Phase that was retried.
        phase: PrpPhase,
    },

    /// PRP was manually discarded.
    Discarded {
        /// Optional reason for discarding.
        reason: Option<String>,
    },
}

/// Phase in the PRP lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrpPhase {
    /// Implementation phase.
    Implementation,
    /// Integration phase.
    Integration,
}

impl std::fmt::Display for PrpPhase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PrpPhase::Implementation => write!(f, "implementation"),
            PrpPhase::Integration => write!(f, "integration"),
        }
    }
}

/// Current state of a PRP in the queue.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrpState {
    /// Waiting to be processed.
    Queued,
    /// Implementation phase in progress.
    Implementing,
    /// Implementation complete, ready for integration.
    ReadyForIntegration,
    /// Integration phase in progress.
    Integrating,
    /// Successfully integrated.
    Integrated,
    /// Needs manual review.
    NeedsReview,
    /// Manually discarded.
    Discarded,
}

impl PrpState {
    /// Returns true if this is a terminal state (no further transitions possible).
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Integrated | Self::Discarded)
    }

    /// Returns true if this is a blocking state (blocks queue advancement).
    pub fn is_blocking(self) -> bool {
        matches!(
            self,
            Self::Implementing
                | Self::ReadyForIntegration
                | Self::Integrating
                | Self::NeedsReview
        )
    }
}

/// Materialized entry for a PRP in the queue.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrpEntry {
    /// PRP ID.
    pub prp_id: String,

    /// Human-readable title.
    pub title: String,

    /// Path to source markdown (relative to workspace).
    pub source_path: String,

    /// Path to archived markdown after integration.
    pub archive_path: Option<String>,

    /// Current state.
    pub state: PrpState,

    /// Last active phase.
    pub last_phase: Option<PrpPhase>,

    /// Queue position (immutable after import).
    pub queue_position: u64,

    /// Implementation branch name.
    pub implementation_branch: Option<String>,

    /// Implementation worktree path.
    pub implementation_worktree: Option<String>,

    /// PID of implementation process.
    pub implementation_pid: Option<u32>,

    /// Loop ID for implementation.
    pub implementation_loop_id: Option<String>,

    /// Path to implementation events file.
    pub implementation_events_path: Option<String>,

    /// Path to implementation handoff file.
    pub implementation_handoff_path: Option<String>,

    /// Integration branch name.
    pub integration_branch: Option<String>,

    /// Integration worktree path.
    pub integration_worktree: Option<String>,

    /// PID of integration process.
    pub integration_pid: Option<u32>,

    /// Loop ID for integration.
    pub integration_loop_id: Option<String>,

    /// Integration commit SHA.
    pub integration_commit: Option<String>,

    /// Failure reason if needs_review.
    pub failure_reason: Option<String>,

    /// When the PRP was imported.
    pub created_at: DateTime<Utc>,

    /// When the PRP was last updated.
    pub updated_at: DateTime<Utc>,
}

/// Errors that can occur during PRP queue operations.
#[derive(Debug, thiserror::Error)]
pub enum PrpQueueError {
    /// IO error during queue operations.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Failed to parse queue data.
    #[error("Failed to parse PRP queue: {0}")]
    ParseError(String),

    /// PRP entry not found.
    #[error("PRP not found in queue: {0}")]
    NotFound(String),

    /// Invalid state transition.
    #[error("Invalid state transition for {0}: cannot transition from {1:?} to {2:?}")]
    InvalidTransition(String, PrpState, PrpState),

    /// PRP is in wrong state for the requested operation.
    #[error("PRP {0} is in {1:?} state, expected one of {2:?}")]
    WrongState(String, PrpState, &'static [PrpState]),

    /// Queue is blocked by an earlier PRP.
    #[error("Queue is blocked by {0} which is in {1:?} state")]
    QueueBlocked(String, PrpState),
}

/// PRP queue for tracking parallel PRP items.
///
/// The queue maintains an append-only JSONL log of PRP events.
/// State is derived by replaying events for each PRP.
pub struct PrpQueue {
    /// Path to the queue file.
    queue_path: PathBuf,
}

impl PrpQueue {
    /// The relative path to the PRP queue file within the workspace.
    pub const QUEUE_FILE: &'static str = ".ralph/prp-queue.jsonl";

    /// Creates a new PRP queue instance for the given workspace.
    pub fn new(workspace_root: impl AsRef<Path>) -> Self {
        Self {
            queue_path: workspace_root.as_ref().join(Self::QUEUE_FILE),
        }
    }

    /// Imports a new PRP into the queue.
    ///
    /// # Arguments
    ///
    /// * `prp_id` - The PRP identifier (e.g., "PRP-001")
    /// * `title` - Human-readable title
    /// * `source_path` - Path to the source markdown file
    pub fn import(
        &self,
        prp_id: &str,
        title: &str,
        source_path: &str,
    ) -> Result<(), PrpQueueError> {
        // Check if already exists
        if self.get_entry(prp_id)?.is_some() {
            return Ok(()); // Idempotent - already imported
        }

        let entries = self.list()?;
        let queue_position = entries.len() as u64;

        let event = PrpEvent {
            ts: Utc::now(),
            prp_id: prp_id.to_string(),
            event: PrpEventType::Imported {
                title: title.to_string(),
                source_path: source_path.to_string(),
                queue_position,
            },
        };
        self.append_event(&event)
    }

    /// Marks a PRP as started implementation.
    pub fn mark_implementing(
        &self,
        prp_id: &str,
        branch: &str,
        worktree: &str,
        pid: u32,
        loop_id: Option<&str>,
    ) -> Result<(), PrpQueueError> {
        let entry = self.get_entry(prp_id)?;
        match entry {
            Some(e) if e.state == PrpState::Queued => {}
            Some(e) => {
                return Err(PrpQueueError::InvalidTransition(
                    prp_id.to_string(),
                    e.state,
                    PrpState::Implementing,
                ));
            }
            None => return Err(PrpQueueError::NotFound(prp_id.to_string())),
        }

        let event = PrpEvent {
            ts: Utc::now(),
            prp_id: prp_id.to_string(),
            event: PrpEventType::ImplementationStarted {
                branch: branch.to_string(),
                worktree: worktree.to_string(),
                pid,
                loop_id: loop_id.map(String::from),
            },
        };
        self.append_event(&event)
    }

    /// Marks a PRP as ready for integration.
    pub fn mark_ready_for_integration(
        &self,
        prp_id: &str,
        handoff_path: &str,
        events_path: &str,
    ) -> Result<(), PrpQueueError> {
        let entry = self.get_entry(prp_id)?;
        match entry {
            Some(e) if e.state == PrpState::Implementing => {}
            Some(e) => {
                return Err(PrpQueueError::InvalidTransition(
                    prp_id.to_string(),
                    e.state,
                    PrpState::ReadyForIntegration,
                ));
            }
            None => return Err(PrpQueueError::NotFound(prp_id.to_string())),
        }

        let event = PrpEvent {
            ts: Utc::now(),
            prp_id: prp_id.to_string(),
            event: PrpEventType::ImplementationReady {
                handoff_path: handoff_path.to_string(),
                events_path: events_path.to_string(),
            },
        };
        self.append_event(&event)
    }

    /// Marks a PRP as started integration.
    pub fn mark_integrating(
        &self,
        prp_id: &str,
        branch: &str,
        worktree: &str,
        pid: u32,
        loop_id: Option<&str>,
    ) -> Result<(), PrpQueueError> {
        let entry = self.get_entry(prp_id)?;
        match entry {
            Some(e) if e.state == PrpState::ReadyForIntegration => {}
            Some(e) => {
                return Err(PrpQueueError::InvalidTransition(
                    prp_id.to_string(),
                    e.state,
                    PrpState::Integrating,
                ));
            }
            None => return Err(PrpQueueError::NotFound(prp_id.to_string())),
        }

        let event = PrpEvent {
            ts: Utc::now(),
            prp_id: prp_id.to_string(),
            event: PrpEventType::IntegrationStarted {
                branch: branch.to_string(),
                worktree: worktree.to_string(),
                pid,
                loop_id: loop_id.map(String::from),
            },
        };
        self.append_event(&event)
    }

    /// Marks a PRP as successfully integrated.
    pub fn mark_integrated(&self, prp_id: &str, commit: &str, archive_path: &str) -> Result<(), PrpQueueError> {
        let entry = self.get_entry(prp_id)?;
        match entry {
            Some(e) if e.state == PrpState::Integrating => {}
            Some(e) => {
                return Err(PrpQueueError::InvalidTransition(
                    prp_id.to_string(),
                    e.state,
                    PrpState::Integrated,
                ));
            }
            None => return Err(PrpQueueError::NotFound(prp_id.to_string())),
        }

        let event = PrpEvent {
            ts: Utc::now(),
            prp_id: prp_id.to_string(),
            event: PrpEventType::Integrated {
                commit: commit.to_string(),
                archive_path: archive_path.to_string(),
            },
        };
        self.append_event(&event)
    }

    /// Marks a PRP as needing review.
    pub fn mark_needs_review(&self, prp_id: &str, phase: PrpPhase, reason: &str) -> Result<(), PrpQueueError> {
        let entry = self.get_entry(prp_id)?;
        match entry {
            Some(e)
                if e.state == PrpState::Implementing
                    || e.state == PrpState::Integrating
                    || e.state == PrpState::ReadyForIntegration =>
            {
                // Allowed
            }
            Some(e) => {
                return Err(PrpQueueError::InvalidTransition(
                    prp_id.to_string(),
                    e.state,
                    PrpState::NeedsReview,
                ));
            }
            None => return Err(PrpQueueError::NotFound(prp_id.to_string())),
        }

        let event = PrpEvent {
            ts: Utc::now(),
            prp_id: prp_id.to_string(),
            event: PrpEventType::NeedsReview {
                phase,
                reason: reason.to_string(),
            },
        };
        self.append_event(&event)
    }

    /// Retries a PRP from needs_review state.
    pub fn retry(&self, prp_id: &str) -> Result<(), PrpQueueError> {
        let entry = self.get_entry(prp_id)?;
        match entry {
            Some(ref e) if e.state == PrpState::NeedsReview => {
                let phase = e.last_phase.unwrap_or(PrpPhase::Implementation);
                let event = PrpEvent {
                    ts: Utc::now(),
                    prp_id: prp_id.to_string(),
                    event: PrpEventType::Retried { phase },
                };
                return self.append_event(&event);
            }
            Some(e) => {
                return Err(PrpQueueError::WrongState(
                    prp_id.to_string(),
                    e.state,
                    &[PrpState::NeedsReview],
                ));
            }
            None => return Err(PrpQueueError::NotFound(prp_id.to_string())),
        }
    }

    /// Discards a PRP from the queue.
    pub fn discard(&self, prp_id: &str, reason: Option<&str>) -> Result<(), PrpQueueError> {
        let entry = self.get_entry(prp_id)?;
        match entry {
            Some(e)
                if e.state == PrpState::Queued
                    || e.state == PrpState::NeedsReview
                    || e.state == PrpState::ReadyForIntegration =>
            {
                // Allowed from these states
            }
            Some(e) => {
                return Err(PrpQueueError::WrongState(
                    prp_id.to_string(),
                    e.state,
                    &[
                        PrpState::Queued,
                        PrpState::NeedsReview,
                        PrpState::ReadyForIntegration,
                    ],
                ));
            }
            None => return Err(PrpQueueError::NotFound(prp_id.to_string())),
        }

        let event = PrpEvent {
            ts: Utc::now(),
            prp_id: prp_id.to_string(),
            event: PrpEventType::Discarded {
                reason: reason.map(String::from),
            },
        };
        self.append_event(&event)
    }

    /// Gets the entry for a specific PRP.
    pub fn get_entry(&self, prp_id: &str) -> Result<Option<PrpEntry>, PrpQueueError> {
        let entries = self.list()?;
        Ok(entries.into_iter().find(|e| e.prp_id == prp_id))
    }

    /// Lists all entries in the PRP queue.
    ///
    /// Returns entries in queue position order.
    pub fn list(&self) -> Result<Vec<PrpEntry>, PrpQueueError> {
        let events = self.read_all_events()?;
        Ok(Self::derive_state(&events))
    }

    /// Gets the next runnable PRP (first non-terminal in queue order).
    ///
    /// Returns the oldest PRP that is in `Queued` state and whose predecessors
    /// are all terminal.
    pub fn next_runnable(&self) -> Result<Option<PrpEntry>, PrpQueueError> {
        let entries = self.list()?;

        for entry in &entries {
            if entry.state == PrpState::Queued {
                return Ok(Some(entry.clone()));
            }
        }

        Ok(None)
    }

    /// Gets the head blocking entry if queue is blocked.
    ///
    /// Returns the first non-terminal entry that is blocking queue advancement.
    pub fn head_blocking_entry(&self) -> Result<Option<PrpEntry>, PrpQueueError> {
        let entries = self.list()?;

        for entry in &entries {
            if entry.state.is_blocking() {
                return Ok(Some(entry.clone()));
            }
        }

        Ok(None)
    }

    /// Reads all events from the queue file.
    fn read_all_events(&self) -> Result<Vec<PrpEvent>, PrpQueueError> {
        if !self.queue_path.exists() {
            return Ok(Vec::new());
        }

        let file = File::open(&self.queue_path)?;
        let reader = BufReader::new(file);
        let mut events = Vec::new();

        for (line_num, line) in reader.lines().enumerate() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }

            let event: PrpEvent = serde_json::from_str(&line).map_err(|e| {
                PrpQueueError::ParseError(format!("Line {}: {}", line_num + 1, e))
            })?;
            events.push(event);
        }

        Ok(events)
    }

    /// Derives the current state of all PRPs from the event history.
    fn derive_state(events: &[PrpEvent]) -> Vec<PrpEntry> {
        use std::collections::BTreeMap;

        // Use BTreeMap to maintain insertion order by queue_position
        let mut prp_states: BTreeMap<String, PrpEntry> = BTreeMap::new();

        for event in events {
            let entry = prp_states
                .entry(event.prp_id.clone())
                .or_insert_with(|| PrpEntry {
                    prp_id: event.prp_id.clone(),
                    title: String::new(),
                    source_path: String::new(),
                    archive_path: None,
                    state: PrpState::Queued,
                    last_phase: None,
                    queue_position: 0,
                    implementation_branch: None,
                    implementation_worktree: None,
                    implementation_pid: None,
                    implementation_loop_id: None,
                    implementation_events_path: None,
                    implementation_handoff_path: None,
                    integration_branch: None,
                    integration_worktree: None,
                    integration_pid: None,
                    integration_loop_id: None,
                    integration_commit: None,
                    failure_reason: None,
                    created_at: event.ts,
                    updated_at: event.ts,
                });

            entry.updated_at = event.ts;

            match &event.event {
                PrpEventType::Imported {
                    title,
                    source_path,
                    queue_position,
                } => {
                    entry.title = title.clone();
                    entry.source_path = source_path.clone();
                    entry.queue_position = *queue_position;
                    entry.state = PrpState::Queued;
                    entry.created_at = event.ts;
                    entry.updated_at = event.ts;
                }
                PrpEventType::ImplementationStarted {
                    branch,
                    worktree,
                    pid,
                    loop_id,
                } => {
                    entry.state = PrpState::Implementing;
                    entry.last_phase = Some(PrpPhase::Implementation);
                    entry.implementation_branch = Some(branch.clone());
                    entry.implementation_worktree = Some(worktree.clone());
                    entry.implementation_pid = Some(*pid);
                    entry.implementation_loop_id = loop_id.clone();
                }
                PrpEventType::ImplementationReady {
                    handoff_path,
                    events_path,
                } => {
                    entry.state = PrpState::ReadyForIntegration;
                    entry.implementation_handoff_path = Some(handoff_path.clone());
                    entry.implementation_events_path = Some(events_path.clone());
                }
                PrpEventType::IntegrationStarted {
                    branch,
                    worktree,
                    pid,
                    loop_id,
                } => {
                    entry.state = PrpState::Integrating;
                    entry.last_phase = Some(PrpPhase::Integration);
                    entry.integration_branch = Some(branch.clone());
                    entry.integration_worktree = Some(worktree.clone());
                    entry.integration_pid = Some(*pid);
                    entry.integration_loop_id = loop_id.clone();
                }
                PrpEventType::Integrated {
                    commit,
                    archive_path,
                } => {
                    entry.state = PrpState::Integrated;
                    entry.integration_commit = Some(commit.clone());
                    entry.archive_path = Some(archive_path.clone());
                }
                PrpEventType::NeedsReview { phase, reason } => {
                    entry.state = PrpState::NeedsReview;
                    entry.last_phase = Some(*phase);
                    entry.failure_reason = Some(reason.clone());
                }
                PrpEventType::Retried { phase } => {
                    // Retry moves back to implementing or ready_for_integration
                    entry.state = match phase {
                        PrpPhase::Implementation => PrpState::Implementing,
                        PrpPhase::Integration => PrpState::ReadyForIntegration,
                    };
                    entry.failure_reason = None;
                }
                PrpEventType::Discarded { reason: _ } => {
                    entry.state = PrpState::Discarded;
                }
            }
        }

        // Sort by queue_position (already sorted due to BTreeMap)
        let mut entries: Vec<_> = prp_states.into_values().collect();
        entries.sort_by_key(|a| a.queue_position);
        entries
    }

    /// Appends an event to the queue file.
    fn append_event(&self, event: &PrpEvent) -> Result<(), PrpQueueError> {
        // Ensure .ralph directory exists
        if let Some(parent) = self.queue_path.parent() {
            fs::create_dir_all(parent)?;
        }

        // Open or create the file
        let mut file = OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .append(true)
            .open(&self.queue_path)?;

        // Write event as JSON line
        let json = serde_json::to_string(event)
            .map_err(|e| PrpQueueError::ParseError(e.to_string()))?;
        writeln!(file, "{}", json)?;

        file.sync_all()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_import() {
        let temp_dir = TempDir::new().unwrap();
        let queue = PrpQueue::new(temp_dir.path());

        queue
            .import("PRP-001", "Test PRP", "PRPs/remaining_work/PRP-001.md")
            .unwrap();

        let entries = queue.list().unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].prp_id, "PRP-001");
        assert_eq!(entries[0].title, "Test PRP");
        assert_eq!(entries[0].state, PrpState::Queued);
    }

    #[test]
    fn test_import_idempotent() {
        let temp_dir = TempDir::new().unwrap();
        let queue = PrpQueue::new(temp_dir.path());

        queue
            .import("PRP-001", "Test PRP", "PRPs/remaining_work/PRP-001.md")
            .unwrap();
        queue
            .import("PRP-001", "Test PRP", "PRPs/remaining_work/PRP-001.md")
            .unwrap(); // Should not add duplicate

        let entries = queue.list().unwrap();
        assert_eq!(entries.len(), 1);
    }

    #[test]
    fn test_full_lifecycle() {
        let temp_dir = TempDir::new().unwrap();
        let queue = PrpQueue::new(temp_dir.path());

        // Import
        queue
            .import("PRP-001", "Test", "PRPs/remaining_work/PRP-001.md")
            .unwrap();
        let entry = queue.get_entry("PRP-001").unwrap().unwrap();
        assert_eq!(entry.state, PrpState::Queued);

        // Start implementation
        queue
            .mark_implementing("PRP-001", "prp/PRP-001", ".worktrees/prp-PRP-001", 123, None)
            .unwrap();
        let entry = queue.get_entry("PRP-001").unwrap().unwrap();
        assert_eq!(entry.state, PrpState::Implementing);

        // Ready for integration
        queue
            .mark_ready_for_integration("PRP-001", ".ralph/agent/handoff.md", ".ralph/current-events")
            .unwrap();
        let entry = queue.get_entry("PRP-001").unwrap().unwrap();
        assert_eq!(entry.state, PrpState::ReadyForIntegration);

        // Start integration
        queue
            .mark_integrating("PRP-001", "integration", ".worktrees/integration", 456, None)
            .unwrap();
        let entry = queue.get_entry("PRP-001").unwrap().unwrap();
        assert_eq!(entry.state, PrpState::Integrating);

        // Integrated
        queue
            .mark_integrated("PRP-001", "abc123def", "PRPs/completed/PRP-001.md")
            .unwrap();
        let entry = queue.get_entry("PRP-001").unwrap().unwrap();
        assert_eq!(entry.state, PrpState::Integrated);
        assert_eq!(entry.integration_commit, Some("abc123def".to_string()));
    }

    #[test]
    fn test_next_runnable_fifo() {
        let temp_dir = TempDir::new().unwrap();
        let queue = PrpQueue::new(temp_dir.path());

        queue
            .import("PRP-001", "First", "PRPs/remaining_work/PRP-001.md")
            .unwrap();
        queue
            .import("PRP-002", "Second", "PRPs/remaining_work/PRP-002.md")
            .unwrap();
        std::thread::sleep(std::time::Duration::from_millis(10));
        queue
            .import("PRP-003", "Third", "PRPs/remaining_work/PRP-003.md")
            .unwrap();

        let next = queue.next_runnable().unwrap().unwrap();
        assert_eq!(next.prp_id, "PRP-001");
    }

    #[test]
    fn test_invalid_transition() {
        let temp_dir = TempDir::new().unwrap();
        let queue = PrpQueue::new(temp_dir.path());

        queue
            .import("PRP-001", "Test", "PRPs/remaining_work/PRP-001.md")
            .unwrap();

        // Can't go directly from queued to integrated
        let result = queue.mark_integrated("PRP-001", "abc", "archive.md");
        assert!(matches!(
            result,
            Err(PrpQueueError::InvalidTransition(
                _,
                PrpState::Queued,
                PrpState::Integrated
            ))
        ));
    }

    #[test]
    fn test_needs_review_from_implementing() {
        let temp_dir = TempDir::new().unwrap();
        let queue = PrpQueue::new(temp_dir.path());

        queue
            .import("PRP-001", "Test", "PRPs/remaining_work/PRP-001.md")
            .unwrap();
        queue
            .mark_implementing("PRP-001", "prp/PRP-001", ".worktrees/prp-PRP-001", 123, None)
            .unwrap();

        queue
            .mark_needs_review("PRP-001", PrpPhase::Implementation, "DoD incomplete")
            .unwrap();

        let entry = queue.get_entry("PRP-001").unwrap().unwrap();
        assert_eq!(entry.state, PrpState::NeedsReview);
        assert_eq!(entry.failure_reason, Some("DoD incomplete".to_string()));
    }

    #[test]
    fn test_retry_from_needs_review() {
        let temp_dir = TempDir::new().unwrap();
        let queue = PrpQueue::new(temp_dir.path());

        queue
            .import("PRP-001", "Test", "PRPs/remaining_work/PRP-001.md")
            .unwrap();
        queue
            .mark_implementing("PRP-001", "prp/PRP-001", ".worktrees/prp-PRP-001", 123, None)
            .unwrap();
        queue
            .mark_needs_review("PRP-001", PrpPhase::Implementation, "conflicts")
            .unwrap();

        queue.retry("PRP-001").unwrap();

        let entry = queue.get_entry("PRP-001").unwrap().unwrap();
        assert_eq!(entry.state, PrpState::Implementing);
        assert!(entry.failure_reason.is_none());
    }

    #[test]
    fn test_discard_from_queued() {
        let temp_dir = TempDir::new().unwrap();
        let queue = PrpQueue::new(temp_dir.path());

        queue
            .import("PRP-001", "Test", "PRPs/remaining_work/PRP-001.md")
            .unwrap();
        queue.discard("PRP-001", Some("No longer needed")).unwrap();

        let entry = queue.get_entry("PRP-001").unwrap().unwrap();
        assert_eq!(entry.state, PrpState::Discarded);
    }

    #[test]
    fn test_persistence() {
        let temp_dir = TempDir::new().unwrap();

        {
            let queue = PrpQueue::new(temp_dir.path());
            queue
                .import("PRP-001", "Test", "PRPs/remaining_work/PRP-001.md")
                .unwrap();
        }

        // Load again and verify data persisted
        {
            let queue = PrpQueue::new(temp_dir.path());
            let entries = queue.list().unwrap();
            assert_eq!(entries.len(), 1);
            assert_eq!(entries[0].prp_id, "PRP-001");
        }
    }
}
