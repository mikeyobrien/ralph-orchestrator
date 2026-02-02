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

#[cfg(feature = "recording")]
mod cli_capture;
mod config;
pub mod diagnostics;
mod event_logger;
mod event_loop;
mod event_parser;
mod event_reader;
pub mod file_lock;
mod git_ops;
mod handoff;
mod hat_registry;
mod hatless_ralph;
mod instructions;
mod landing;
pub mod loop_completion;
pub mod loop_context;
pub mod loop_history;
pub mod loop_lock;
mod loop_name;
pub mod loop_registry;
mod memory;
pub mod memory_parser;
mod memory_store;
pub mod merge_queue;
pub mod planning_session;
pub mod preflight;
#[cfg(feature = "recording")]
mod session_player;
#[cfg(feature = "recording")]
mod session_recorder;
pub mod skill;
pub mod skill_registry;
mod summary_writer;
pub mod task;
pub mod task_definition;
pub mod task_store;
pub mod testing;
mod text;
pub mod utils;
pub mod workspace;
pub mod worktree;

#[cfg(feature = "recording")]
pub use cli_capture::{CliCapture, CliCapturePair};
pub use config::{
    CliConfig, ConfigError, CoreConfig, EventLoopConfig, EventMetadata, FeaturesConfig, HatBackend,
    HatConfig, InjectMode, MemoriesConfig, MemoriesFilter, RalphConfig, SkillOverride,
    SkillsConfig,
};
// Re-export loop_name types (also available via FeaturesConfig.loop_naming)
pub use diagnostics::DiagnosticsCollector;
pub use event_logger::{EventHistory, EventLogger, EventRecord};
pub use event_loop::{EventLoop, LoopState, TerminationReason, UserPrompt};
pub use event_parser::EventParser;
pub use event_reader::{Event, EventReader, MalformedLine, ParseResult};
pub use file_lock::{FileLock, LockGuard as FileLockGuard, LockedFile};
pub use git_ops::{
    AutoCommitResult, GitOpsError, auto_commit_changes, clean_stashes, get_commit_summary,
    get_current_branch, get_head_sha, get_recent_files, has_uncommitted_changes,
    is_working_tree_clean, prune_remote_refs,
};
pub use handoff::{HandoffError, HandoffResult, HandoffWriter};
pub use hat_registry::HatRegistry;
pub use hatless_ralph::{HatInfo, HatTopology, HatlessRalph};
pub use instructions::InstructionBuilder;
pub use landing::{LandingConfig, LandingError, LandingHandler, LandingResult};
pub use loop_completion::{CompletionAction, CompletionError, LoopCompletionHandler};
pub use loop_context::LoopContext;
pub use loop_history::{HistoryError, HistoryEvent, HistoryEventType, HistorySummary, LoopHistory};
pub use loop_lock::{LockError, LockGuard, LockMetadata, LoopLock};
pub use loop_name::{LoopNameGenerator, LoopNamingConfig};
pub use loop_registry::{LoopEntry, LoopRegistry, RegistryError};
pub use memory::{Memory, MemoryType};
pub use memory_store::{
    DEFAULT_MEMORIES_PATH, MarkdownMemoryStore, format_memories_as_markdown, truncate_to_budget,
};
pub use merge_queue::{
    MergeButtonState, MergeEntry, MergeEvent, MergeEventType, MergeOption, MergeQueue,
    MergeQueueError, MergeState, SteeringDecision, merge_button_state, merge_execution_summary,
    merge_needs_steering, smart_merge_summary,
};
pub use planning_session::{
    ConversationEntry, ConversationType, PlanningSession, PlanningSessionError, SessionMetadata,
    SessionStatus,
};
pub use preflight::{
    AcceptanceCriterion, CheckResult, CheckStatus, PreflightCheck, PreflightReport,
    PreflightRunner, extract_acceptance_criteria, extract_all_criteria, extract_criteria_from_file,
};
#[cfg(feature = "recording")]
pub use session_player::{PlayerConfig, ReplayMode, SessionPlayer, TimestampedRecord};
#[cfg(feature = "recording")]
pub use session_recorder::{Record, SessionRecorder};
pub use skill::{SkillEntry, SkillFrontmatter, SkillSource, parse_frontmatter};
pub use skill_registry::SkillRegistry;
pub use summary_writer::SummaryWriter;
pub use task::{Task, TaskStatus};
pub use task_definition::{
    TaskDefinition, TaskDefinitionError, TaskSetup, TaskSuite, Verification,
};
pub use task_store::TaskStore;
pub use text::{floor_char_boundary, truncate_with_ellipsis};
pub use workspace::{
    CleanupPolicy, TaskWorkspace, VerificationResult, WorkspaceError, WorkspaceInfo,
    WorkspaceManager,
};
pub use worktree::{
    SyncStats, Worktree, WorktreeConfig, WorktreeError, create_worktree, ensure_gitignore,
    list_ralph_worktrees, list_worktrees, remove_worktree, sync_working_directory_to_worktree,
    worktree_exists,
};
