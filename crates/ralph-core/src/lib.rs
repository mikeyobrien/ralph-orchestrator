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
mod hat_registry;
mod hatless_ralph;
mod instructions;
mod memory;
pub mod memory_parser;
mod memory_store;
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

pub use cli_capture::{CliCapture, CliCapturePair};
pub use config::{
    CliConfig, CoreConfig, EventLoopConfig, EventMetadata, HatBackend, HatConfig, InjectMode,
    MemoriesConfig, MemoriesFilter, RalphConfig,
};
pub use diagnostics::DiagnosticsCollector;
pub use event_logger::{EventHistory, EventLogger, EventRecord};
pub use event_loop::{EventLoop, LoopState, TerminationReason};
pub use event_parser::EventParser;
pub use event_reader::{Event, EventReader, MalformedLine, ParseResult};
pub use hat_registry::HatRegistry;
pub use hatless_ralph::{HatInfo, HatTopology, HatlessRalph};
pub use instructions::InstructionBuilder;
pub use memory::{Memory, MemoryType};
pub use memory_store::{
    DEFAULT_MEMORIES_PATH, MarkdownMemoryStore, format_memories_as_markdown, truncate_to_budget,
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
