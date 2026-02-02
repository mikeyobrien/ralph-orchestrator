//! Testing utilities for deterministic E2E tests.

pub mod mock_backend;
#[cfg(feature = "recording")]
pub mod replay_backend;
pub mod scenario;
#[cfg(feature = "recording")]
pub mod smoke_runner;

pub use mock_backend::{ExecutionRecord, MockBackend};
#[cfg(feature = "recording")]
pub use replay_backend::{ReplayBackend, ReplayTimingMode};
pub use scenario::{ExecutionTrace, Scenario, ScenarioRunner};
#[cfg(feature = "recording")]
pub use smoke_runner::{
    SmokeRunner, SmokeTestConfig, SmokeTestError, SmokeTestResult, TerminationReason, list_fixtures,
};
