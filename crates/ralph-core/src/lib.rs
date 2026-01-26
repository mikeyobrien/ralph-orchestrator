//! # ralph-core
//!
//! Core orchestration functionality for the Ralph Orchestrator framework.
//!
//! This crate provides:
//! - The main orchestration loop for coordinating multiple agents
//! - Configuration loading and management
//! - State management for agent sessions
//! - Message routing between agents
//! - Terminal capture for session recording
//! - Benchmark task definitions and workspace isolation

mod cli_capture;
mod config;
pub mod diagnostics;
mod event_logger;
mod event_loop;
mod event_parser;
mod event_reader;
pub mod file_lock;
mod hat_registry;
mod hatless_ralph;
mod instructions;
pub mod loop_completion;
pub mod loop_context;
pub mod loop_history;
pub mod loop_lock;
pub mod loop_registry;
mod memory;
pub mod memory_parser;
mod memory_store;
pub mod merge_queue;
mod session_player;
mod session_recorder;
mod summary_writer;
pub mod task;
pub mod task_definition;
pub mod task_store;
pub mod testing;
mod text;
pub mod utils;
pub mod workspace;
pub mod worktree;

pub use cli_capture::{CliCapture, CliCapturePair};
pub use config::{
    CliConfig, CoreConfig, EventLoopConfig, EventMetadata, FeaturesConfig, HatBackend, HatConfig,
    InjectMode, MemoriesConfig, MemoriesFilter, RalphConfig,
};
pub use diagnostics::DiagnosticsCollector;
pub use event_logger::{EventHistory, EventLogger, EventRecord};
pub use event_loop::{EventLoop, LoopState, TerminationReason};
pub use event_parser::EventParser;
pub use event_reader::{Event, EventReader, MalformedLine, ParseResult};
pub use file_lock::{FileLock, LockGuard as FileLockGuard, LockedFile};
pub use hat_registry::HatRegistry;
pub use hatless_ralph::{HatInfo, HatTopology, HatlessRalph};
pub use instructions::InstructionBuilder;
pub use loop_completion::{CompletionAction, CompletionError, LoopCompletionHandler};
pub use loop_context::LoopContext;
pub use loop_history::{HistoryError, HistoryEvent, HistoryEventType, HistorySummary, LoopHistory};
pub use loop_lock::{LockError, LockGuard, LockMetadata, LoopLock};
pub use loop_registry::{LoopEntry, LoopRegistry, RegistryError};
pub use memory::{Memory, MemoryType};
pub use memory_store::{
    DEFAULT_MEMORIES_PATH, MarkdownMemoryStore, format_memories_as_markdown, truncate_to_budget,
};
pub use merge_queue::{
    MergeEntry, MergeEvent, MergeEventType, MergeQueue, MergeQueueError, MergeState,
};
pub use session_player::{PlayerConfig, ReplayMode, SessionPlayer, TimestampedRecord};
pub use session_recorder::{Record, SessionRecorder};
pub use summary_writer::SummaryWriter;
pub use task::{Task, TaskStatus};
pub use task_definition::{
    TaskDefinition, TaskDefinitionError, TaskSetup, TaskSuite, Verification,
};
pub use task_store::TaskStore;
pub use text::truncate_with_ellipsis;
pub use workspace::{
    CleanupPolicy, TaskWorkspace, VerificationResult, WorkspaceError, WorkspaceInfo,
    WorkspaceManager,
};
pub use worktree::{
    Worktree, WorktreeConfig, WorktreeError, create_worktree, ensure_gitignore,
    list_ralph_worktrees, list_worktrees, remove_worktree, worktree_exists,
};
