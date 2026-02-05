//! # hats-e2e
//!
//! End-to-end test harness library for the Hats.
//!
//! This crate provides the core functionality for validating Hats's behavior
//! against real AI backends. It is designed to be used both as a CLI tool
//! and as a library for programmatic test execution.
//!
//! ## Architecture
//!
//! ```text
//! ┌─────────────┐     ┌─────────────┐     ┌─────────────┐
//! │  TestRunner │────▶│  Scenarios  │────▶│  Executor   │
//! └─────────────┘     └─────────────┘     └─────────────┘
//!        │                                       │
//!        ▼                                       ▼
//! ┌─────────────┐                         ┌─────────────┐
//! │  Reporter   │                         │   Backend   │
//! └─────────────┘                         └─────────────┘
//! ```
//!
//! ## Modules (to be implemented)
//!
//! - `workspace`: Manages isolated test workspaces in `.e2e-tests/`
//! - `auth`: Checks backend availability and authentication
//! - `executor`: Invokes `hats run` with test configurations
//! - `scenarios`: Defines test scenarios (TestScenario trait)
//! - `runner`: Orchestrates test execution
//! - `reporter`: Generates terminal and file reports
//! - `analyzer`: Meta-Hats analysis for rich diagnostics

// Re-export common types for library consumers
pub use crate::analyzer::{
    AnalyzedResult, AnalyzerConfig, AnalyzerError, Diagnosis, FailedAnalysis, FailureType,
    MetaHatsAnalyzer, Optimization, PassedAnalysis, PassedTestAnalysis, Pattern, PotentialFix,
    QualityScore, Recommendation, Severity, TestMetrics, Warning, WarningCategory,
};
pub use crate::auth::{AuthChecker, BackendInfo};
pub use crate::backend::Backend;
pub use crate::executor::{
    EventRecord, ExecutionResult, ExecutorError, PromptSource, HatsExecutor, ScenarioConfig,
    find_workspace_root, resolve_hats_binary,
};
pub use crate::mock::{
    CassetteError, CassetteResolver, DEFAULT_CASSETTE_DIR, MockConfig, build_mock_cli_args,
};
pub use crate::mock_cli::{MockCliError, run as run_mock_cli};
pub use crate::models::{Assertion, ReportFormat, TestResult};
pub use crate::reporter::{
    AnalyzedResultData, BackendSummary, JsonReporter, MarkdownReporter, QualityBreakdown,
    ReportSummary, ReportWriter, ReporterError, TerminalReporter, TestReport, TierSummary,
    Verbosity, create_incremental_progress_callback, create_progress_callback,
};
pub use crate::runner::{
    ProgressCallback, ProgressEvent, RunConfig, RunResults, RunnerError, TestRunner,
};
pub use crate::scenarios::{
    // Core traits and helpers
    Assertions,
    // Tier 8: Error Handling (backend-agnostic)
    AuthFailureScenario,
    BackendUnavailableScenario,
    // Tier 3: Events (backend-agnostic)
    BackpressureScenario,
    // Tier 7: Incremental Development (backend-agnostic)
    ChainedLoopScenario,
    // Tier 2: Orchestration Loop (backend-agnostic)
    CompletionScenario,
    // Tier 1: Connectivity (backend-agnostic)
    ConnectivityScenario,
    EventsScenario,
    // Tier 5: Hat Collections (backend-agnostic)
    HatBackendOverrideScenario,
    HatEventRoutingScenario,
    HatInstructionsScenario,
    HatMultiWorkflowScenario,
    HatSingleScenario,
    IncrementalFeatureScenario,
    MaxIterationsScenario,
    // Tier 6: Memory System (backend-agnostic)
    MemoryAddScenario,
    MemoryCorruptedFileScenario,
    MemoryInjectionScenario,
    MemoryLargeContentScenario,
    MemoryMissingFileScenario,
    MemoryPersistenceScenario,
    MemoryRapidWriteScenario,
    MemorySearchScenario,
    MultiIterScenario,
    ScenarioError,
    SingleIterScenario,
    // Tier 4: Capabilities (backend-agnostic)
    StreamingScenario,
    // Tier 6: Task System (backend-agnostic)
    TaskAddScenario,
    TaskCloseScenario,
    TaskCompletionScenario,
    TaskReadyScenario,
    TestScenario,
    TimeoutScenario,
    ToolUseScenario,
};
pub use crate::workspace::WorkspaceManager;

pub mod analyzer;
pub mod auth;
mod backend;
pub mod executor;
pub mod mock;
pub mod mock_cli;
mod models;
pub mod reporter;
pub mod runner;
pub mod scenarios;
pub mod workspace;

/// Library version, matching the crate version.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
