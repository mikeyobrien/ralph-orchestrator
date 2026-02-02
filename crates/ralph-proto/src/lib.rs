//! # ralph-proto
//!
//! Shared types, error definitions, and traits for the Ralph Orchestrator framework.
//!
//! This crate provides the foundational abstractions used across all Ralph crates,
//! including:
//! - Event and `EventBus` types for pub/sub messaging
//! - Hat definitions for agent personas
//! - Topic matching for event routing
//! - Common error types

pub mod daemon;
mod error;
mod event;
mod event_bus;
mod hat;
pub mod robot;
mod topic;
mod ux_event;

pub use daemon::{DaemonAdapter, StartLoopFn};
pub use error::{Error, Result};
pub use event::Event;
pub use event_bus::EventBus;
pub use hat::{Hat, HatId};
pub use robot::{CheckinContext, RobotService};
pub use topic::Topic;
pub use ux_event::{
    FrameCapture, TerminalColorMode, TerminalResize, TerminalWrite, TuiFrame, UxEvent,
};
