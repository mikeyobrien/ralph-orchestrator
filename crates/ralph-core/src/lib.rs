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
mod event_logger;
mod event_loop;
mod event_parser;
mod event_reader;
mod hat_registry;
mod hatless_ralph;
mod instructions;
mod session_player;
mod session_recorder;
mod summary_writer;
pub mod task_definition;
pub mod testing;
pub mod workspace;

pub use cli_capture::{CliCapture, CliCapturePair};
pub use config::{
    CliConfig, CoreConfig, EventLoopConfig, EventMetadata, HatBackend, HatConfig, RalphConfig,
};
pub use event_logger::{EventHistory, EventLogger, EventRecord};
pub use event_loop::{EventLoop, LoopState, TerminationReason};
pub use event_parser::EventParser;
pub use event_reader::{Event, EventReader, MalformedLine, ParseResult};
pub use hat_registry::HatRegistry;
pub use hatless_ralph::{HatInfo, HatTopology, HatlessRalph};
pub use instructions::InstructionBuilder;
pub use session_player::{PlayerConfig, ReplayMode, SessionPlayer, TimestampedRecord};
pub use session_recorder::{Record, SessionRecorder};
pub use summary_writer::SummaryWriter;
pub use task_definition::{
    TaskDefinition, TaskDefinitionError, TaskSetup, TaskSuite, Verification,
};
pub use workspace::{
    CleanupPolicy, TaskWorkspace, VerificationResult, WorkspaceError, WorkspaceInfo,
    WorkspaceManager,
};
