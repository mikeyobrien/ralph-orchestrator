//! Testing utilities for deterministic E2E tests.

pub mod mock_backend;
pub mod replay_backend;
pub mod scenario;
pub mod smoke_runner;

pub use mock_backend::{ExecutionRecord, MockBackend};
pub use replay_backend::{ReplayBackend, ReplayTimingMode};
pub use scenario::{ExecutionTrace, Scenario, ScenarioRunner};
pub use smoke_runner::{
    SmokeRunner, SmokeTestConfig, SmokeTestError, SmokeTestResult, TerminationReason, list_fixtures,
};
