//! Test runner for E2E test execution.
//!
//! The TestRunner orchestrates scenario execution: it manages workspaces,
//! runs scenarios, and collects results for reporting.
//!
//! # Example
//!
//! ```no_run
//! use ralph_e2e::{TestRunner, RunConfig, ConnectivityScenario, TestScenario, WorkspaceManager};
//! use std::path::PathBuf;
//!
//! #[tokio::main]
//! async fn main() {
//!     let workspace_mgr = WorkspaceManager::new(PathBuf::from(".e2e-tests"));
//!     let scenarios: Vec<Box<dyn TestScenario>> = vec![
//!         Box::new(ConnectivityScenario::new()),
//!     ];
//!
//!     let runner = TestRunner::new(workspace_mgr, scenarios);
//!     let results = runner.run_all().await.unwrap();
//!
//!     println!("Passed: {}", results.passed_count());
//! }
//! ```

use crate::Backend;
use crate::executor::RalphExecutor;
use crate::models::TestResult;
use crate::scenarios::{ScenarioError, TestScenario};
use crate::workspace::WorkspaceManager;
use std::path::PathBuf;
use std::time::{Duration, Instant};
use thiserror::Error;

/// Errors that can occur during test execution.
#[derive(Debug, Error)]
pub enum RunnerError {
    /// Failed to create workspace.
    #[error("workspace error: {0}")]
    WorkspaceError(String),

    /// Scenario execution failed.
    #[error("scenario error: {0}")]
    ScenarioError(#[from] ScenarioError),

    /// No scenarios matched the filter.
    #[error("no scenarios matched filter: {0}")]
    NoMatchingScenarios(String),
}

/// Configuration for a test run.
#[derive(Debug, Clone, Default)]
pub struct RunConfig {
    /// Filter scenarios by pattern (matches scenario ID or description).
    pub filter: Option<String>,

    /// Only run scenarios for this backend.
    pub backend: Option<Backend>,

    /// Keep workspaces after tests complete.
    pub keep_workspaces: bool,

    /// Skip scenarios that require unavailable backends.
    pub skip_unavailable: bool,
}

impl RunConfig {
    /// Creates a new run configuration with defaults.
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the filter pattern.
    pub fn with_filter(mut self, filter: impl Into<String>) -> Self {
        self.filter = Some(filter.into());
        self
    }

    /// Sets the backend filter.
    pub fn with_backend(mut self, backend: Backend) -> Self {
        self.backend = Some(backend);
        self
    }

    /// Sets whether to keep workspaces.
    pub fn keep_workspaces(mut self, keep: bool) -> Self {
        self.keep_workspaces = keep;
        self
    }
}

/// Aggregated results from a test run.
#[derive(Debug, Clone, Default)]
pub struct RunResults {
    /// Individual test results.
    pub results: Vec<TestResult>,

    /// Total duration of the run.
    pub duration: Duration,

    /// Number of scenarios that were skipped.
    pub skipped_count: usize,
}

impl RunResults {
    /// Returns the number of passed tests.
    pub fn passed_count(&self) -> usize {
        self.results.iter().filter(|r| r.passed).count()
    }

    /// Returns the number of failed tests.
    pub fn failed_count(&self) -> usize {
        self.results.iter().filter(|r| !r.passed).count()
    }

    /// Returns the total number of tests run.
    pub fn total_count(&self) -> usize {
        self.results.len()
    }

    /// Returns true if all tests passed.
    pub fn all_passed(&self) -> bool {
        self.results.iter().all(|r| r.passed)
    }

    /// Returns results grouped by tier.
    pub fn by_tier(&self) -> Vec<(&str, Vec<&TestResult>)> {
        let mut tiers: std::collections::HashMap<&str, Vec<&TestResult>> =
            std::collections::HashMap::new();

        for result in &self.results {
            tiers.entry(&result.tier).or_default().push(result);
        }

        let mut sorted: Vec<_> = tiers.into_iter().collect();
        sorted.sort_by_key(|(tier, _)| *tier);
        sorted
    }

    /// Returns only failed results.
    pub fn failures(&self) -> Vec<&TestResult> {
        self.results.iter().filter(|r| !r.passed).collect()
    }
}

/// Progress callback for test execution updates.
pub type ProgressCallback = Box<dyn Fn(ProgressEvent) + Send + Sync>;

/// Events emitted during test execution.
#[derive(Debug, Clone)]
pub enum ProgressEvent {
    /// A test run has started.
    RunStarted { total_scenarios: usize },

    /// A scenario is about to execute.
    ScenarioStarted { scenario_id: String, tier: String },

    /// A scenario has completed.
    ScenarioCompleted {
        scenario_id: String,
        passed: bool,
        duration: Duration,
        /// Full test result for incremental reporting.
        result: TestResult,
    },

    /// A scenario was skipped.
    ScenarioSkipped { scenario_id: String, reason: String },

    /// The test run has completed.
    RunCompleted { results: RunResults },
}

/// Orchestrates E2E test scenario execution.
pub struct TestRunner {
    /// Manages isolated test workspaces.
    workspace_mgr: WorkspaceManager,

    /// Registered test scenarios.
    scenarios: Vec<Box<dyn TestScenario>>,

    /// Progress callback for updates.
    on_progress: Option<ProgressCallback>,

    /// Path to the ralph binary to use for tests.
    ralph_binary: Option<PathBuf>,
}

impl TestRunner {
    /// Creates a new test runner.
    pub fn new(workspace_mgr: WorkspaceManager, scenarios: Vec<Box<dyn TestScenario>>) -> Self {
        Self {
            workspace_mgr,
            scenarios,
            on_progress: None,
            ralph_binary: None,
        }
    }

    /// Sets the ralph binary path to use for tests.
    ///
    /// If not set, tests will use `ralph` from PATH.
    /// Use `resolve_ralph_binary()` to automatically find the local build.
    pub fn with_binary(mut self, binary: PathBuf) -> Self {
        self.ralph_binary = Some(binary);
        self
    }

    /// Sets a callback for progress updates.
    pub fn on_progress(mut self, callback: ProgressCallback) -> Self {
        self.on_progress = Some(callback);
        self
    }

    /// Returns the number of registered scenarios.
    pub fn scenario_count(&self) -> usize {
        self.scenarios.len()
    }

    /// Returns scenarios matching the given config.
    pub fn matching_scenarios(&self, config: &RunConfig) -> Vec<&dyn TestScenario> {
        self.scenarios
            .iter()
            .filter(|s| self.matches_config(s.as_ref(), config))
            .map(|s| s.as_ref())
            .collect()
    }

    /// Runs all scenarios matching the configuration.
    ///
    /// When a specific backend is set in `config`, each scenario runs once for that backend.
    /// When no backend is set (running "all"), each scenario runs once per supported backend.
    pub async fn run(&self, config: &RunConfig) -> Result<RunResults, RunnerError> {
        let start = Instant::now();
        let matching = self.matching_scenarios(config);

        if matching.is_empty() && config.filter.is_some() {
            return Err(RunnerError::NoMatchingScenarios(
                config.filter.clone().unwrap(),
            ));
        }

        // Calculate total scenarios: if no backend specified, multiply by supported backends
        let total_scenarios: usize = if config.backend.is_some() {
            matching.len()
        } else {
            matching.iter().map(|s| s.supported_backends().len()).sum()
        };

        self.emit_progress(ProgressEvent::RunStarted { total_scenarios });

        let mut results = Vec::new();
        let mut skipped_count = 0;

        for scenario in matching {
            // Determine which backends to run for this scenario
            let backends_to_run: Vec<Backend> = match &config.backend {
                Some(b) => vec![*b],
                None => scenario.supported_backends(),
            };

            for backend in backends_to_run {
                let scenario_id = if config.backend.is_some() {
                    // Single backend mode: use base scenario ID
                    scenario.id().to_string()
                } else {
                    // All backends mode: append backend name
                    format!("{}-{}", scenario.id(), backend.as_config_str())
                };
                let tier = scenario.tier().to_string();

                self.emit_progress(ProgressEvent::ScenarioStarted {
                    scenario_id: scenario_id.clone(),
                    tier: tier.clone(),
                });

                // Create workspace for this scenario
                let workspace_path = self
                    .workspace_mgr
                    .create_workspace(&scenario_id)
                    .map_err(|e| RunnerError::WorkspaceError(e.to_string()))?;

                // Setup the scenario with the target backend
                let setup_result = scenario.setup(&workspace_path, backend);
                let scenario_config = match setup_result {
                    Ok(cfg) => cfg,
                    Err(e) => {
                        self.emit_progress(ProgressEvent::ScenarioSkipped {
                            scenario_id: scenario_id.clone(),
                            reason: format!("Setup failed: {}", e),
                        });
                        skipped_count += 1;

                        if !config.keep_workspaces {
                            self.workspace_mgr.cleanup(&scenario_id).ok();
                        }
                        continue;
                    }
                };

                // Execute the scenario
                let executor = match &self.ralph_binary {
                    Some(binary) => {
                        RalphExecutor::with_binary(workspace_path.clone(), binary.clone())
                    }
                    None => RalphExecutor::new(workspace_path.clone()),
                };
                let scenario_start = Instant::now();

                let result = scenario.run(&executor, &scenario_config).await;
                let scenario_duration = scenario_start.elapsed();

                match result {
                    Ok(mut test_result) => {
                        // Update scenario_id to include backend suffix when running all
                        if config.backend.is_none() {
                            test_result.scenario_id = scenario_id.clone();
                        }
                        test_result.backend = backend.to_string();
                        let passed = test_result.passed;

                        self.emit_progress(ProgressEvent::ScenarioCompleted {
                            scenario_id: scenario_id.clone(),
                            passed,
                            duration: scenario_duration,
                            result: test_result.clone(),
                        });

                        results.push(test_result);
                    }
                    Err(e) => {
                        // Create a failed result for the scenario
                        let failed_result = TestResult {
                            scenario_id: scenario_id.clone(),
                            scenario_description: scenario.description().to_string(),
                            backend: backend.to_string(),
                            tier: tier.clone(),
                            passed: false,
                            assertions: vec![crate::models::Assertion {
                                name: "Execution".to_string(),
                                passed: false,
                                expected: "Scenario executes successfully".to_string(),
                                actual: format!("Error: {}", e),
                            }],
                            duration: scenario_duration,
                        };

                        self.emit_progress(ProgressEvent::ScenarioCompleted {
                            scenario_id: scenario_id.clone(),
                            passed: false,
                            duration: scenario_duration,
                            result: failed_result.clone(),
                        });

                        results.push(failed_result);
                    }
                }

                // Cleanup unless keeping workspaces
                if !config.keep_workspaces {
                    scenario.cleanup(&workspace_path).ok();
                    self.workspace_mgr.cleanup(&scenario_id).ok();
                }
            }
        }

        let run_results = RunResults {
            results,
            duration: start.elapsed(),
            skipped_count,
        };

        self.emit_progress(ProgressEvent::RunCompleted {
            results: run_results.clone(),
        });

        Ok(run_results)
    }

    /// Runs all registered scenarios with default configuration.
    pub async fn run_all(&self) -> Result<RunResults, RunnerError> {
        self.run(&RunConfig::default()).await
    }

    /// Checks if a scenario matches the run configuration.
    fn matches_config(&self, scenario: &dyn TestScenario, config: &RunConfig) -> bool {
        // Check backend filter: scenario must support the requested backend
        if let Some(backend) = &config.backend
            && !scenario.supported_backends().contains(backend)
        {
            return false;
        }

        // Check pattern filter
        if let Some(filter) = &config.filter {
            let filter_lower = filter.to_lowercase();
            let id_matches = scenario.id().to_lowercase().contains(&filter_lower);
            let desc_matches = scenario
                .description()
                .to_lowercase()
                .contains(&filter_lower);
            let tier_matches = scenario.tier().to_lowercase().contains(&filter_lower);

            if !id_matches && !desc_matches && !tier_matches {
                return false;
            }
        }

        true
    }

    /// Emits a progress event if a callback is registered.
    fn emit_progress(&self, event: ProgressEvent) {
        if let Some(callback) = &self.on_progress {
            callback(event);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::executor::ScenarioConfig;
    use crate::models::Assertion;
    use async_trait::async_trait;
    use std::env;
    use std::path::{Path, PathBuf};
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};

    /// Mock scenario for testing.
    struct MockScenario {
        id: String,
        description: String,
        tier: String,
        supported_backends: Vec<Backend>,
        should_pass: bool,
    }

    impl MockScenario {
        fn new(id: &str, pass: bool) -> Self {
            Self {
                id: id.to_string(),
                description: format!("Mock scenario {}", id),
                tier: "Tier 0: Mock".to_string(),
                supported_backends: vec![Backend::Claude, Backend::Kiro, Backend::OpenCode],
                should_pass: pass,
            }
        }

        #[allow(dead_code)]
        fn with_tier(mut self, tier: &str) -> Self {
            self.tier = tier.to_string();
            self
        }

        fn with_backend(mut self, backend: Backend) -> Self {
            self.supported_backends = vec![backend];
            self
        }
    }

    #[async_trait]
    impl TestScenario for MockScenario {
        fn id(&self) -> &str {
            &self.id
        }

        fn description(&self) -> &str {
            &self.description
        }

        fn tier(&self) -> &str {
            &self.tier
        }

        fn supported_backends(&self) -> Vec<Backend> {
            self.supported_backends.clone()
        }

        fn setup(
            &self,
            workspace: &Path,
            backend: Backend,
        ) -> Result<ScenarioConfig, ScenarioError> {
            // Create a minimal ralph.yml for the mock
            std::fs::write(
                workspace.join("ralph.yml"),
                format!(
                    "cli:\n  backend: {}\n  max_iterations: 1\n",
                    backend.as_config_str()
                ),
            )?;

            Ok(ScenarioConfig::minimal("mock prompt"))
        }

        async fn run(
            &self,
            _executor: &RalphExecutor,
            _config: &ScenarioConfig,
        ) -> Result<TestResult, ScenarioError> {
            // Don't actually run ralph - return mock result
            Ok(TestResult {
                scenario_id: self.id.clone(),
                scenario_description: self.description.clone(),
                backend: self
                    .supported_backends
                    .first()
                    .map(|b| b.to_string())
                    .unwrap_or_default(),
                tier: self.tier.clone(),
                passed: self.should_pass,
                assertions: vec![Assertion {
                    name: "Mock assertion".to_string(),
                    passed: self.should_pass,
                    expected: "pass".to_string(),
                    actual: if self.should_pass { "pass" } else { "fail" }.to_string(),
                }],
                duration: Duration::from_millis(100),
            })
        }
    }

    /// Creates a unique test workspace path.
    fn test_workspace_base(test_name: &str) -> PathBuf {
        env::temp_dir().join(format!(
            "ralph-e2e-runner-{}-{}",
            test_name,
            std::process::id()
        ))
    }

    /// Cleans up a test workspace.
    fn cleanup_workspace(path: &PathBuf) {
        if path.exists() {
            std::fs::remove_dir_all(path).ok();
        }
    }

    #[test]
    fn test_run_config_defaults() {
        let config = RunConfig::new();
        assert!(config.filter.is_none());
        assert!(config.backend.is_none());
        assert!(!config.keep_workspaces);
    }

    #[test]
    fn test_run_config_with_filter() {
        let config = RunConfig::new().with_filter("connect");
        assert_eq!(config.filter, Some("connect".to_string()));
    }

    #[test]
    fn test_run_config_with_backend() {
        let config = RunConfig::new().with_backend(Backend::Claude);
        assert_eq!(config.backend, Some(Backend::Claude));
    }

    #[test]
    fn test_run_results_counts() {
        let results = RunResults {
            results: vec![
                TestResult {
                    scenario_id: "test-1".to_string(),
                    scenario_description: "Test 1".to_string(),
                    backend: "Claude".to_string(),
                    tier: "Tier 1".to_string(),
                    passed: true,
                    assertions: vec![],
                    duration: Duration::from_secs(1),
                },
                TestResult {
                    scenario_id: "test-2".to_string(),
                    scenario_description: "Test 2".to_string(),
                    backend: "Claude".to_string(),
                    tier: "Tier 1".to_string(),
                    passed: false,
                    assertions: vec![],
                    duration: Duration::from_secs(2),
                },
                TestResult {
                    scenario_id: "test-3".to_string(),
                    scenario_description: "Test 3".to_string(),
                    backend: "Claude".to_string(),
                    tier: "Tier 1".to_string(),
                    passed: true,
                    assertions: vec![],
                    duration: Duration::from_secs(1),
                },
            ],
            duration: Duration::from_secs(4),
            skipped_count: 0,
        };

        assert_eq!(results.passed_count(), 2);
        assert_eq!(results.failed_count(), 1);
        assert_eq!(results.total_count(), 3);
        assert!(!results.all_passed());
    }

    #[test]
    fn test_run_results_all_passed() {
        let results = RunResults {
            results: vec![TestResult {
                scenario_id: "test-1".to_string(),
                scenario_description: "Test 1".to_string(),
                backend: "Claude".to_string(),
                tier: "Tier 1".to_string(),
                passed: true,
                assertions: vec![],
                duration: Duration::from_secs(1),
            }],
            duration: Duration::from_secs(1),
            skipped_count: 0,
        };

        assert!(results.all_passed());
    }

    #[test]
    fn test_run_results_by_tier() {
        let results = RunResults {
            results: vec![
                TestResult {
                    scenario_id: "test-1".to_string(),
                    scenario_description: "Test 1".to_string(),
                    backend: "Claude".to_string(),
                    tier: "Tier 1: Connectivity".to_string(),
                    passed: true,
                    assertions: vec![],
                    duration: Duration::from_secs(1),
                },
                TestResult {
                    scenario_id: "test-2".to_string(),
                    scenario_description: "Test 2".to_string(),
                    backend: "Claude".to_string(),
                    tier: "Tier 2: Orchestration".to_string(),
                    passed: true,
                    assertions: vec![],
                    duration: Duration::from_secs(1),
                },
                TestResult {
                    scenario_id: "test-3".to_string(),
                    scenario_description: "Test 3".to_string(),
                    backend: "Claude".to_string(),
                    tier: "Tier 1: Connectivity".to_string(),
                    passed: true,
                    assertions: vec![],
                    duration: Duration::from_secs(1),
                },
            ],
            duration: Duration::from_secs(3),
            skipped_count: 0,
        };

        let by_tier = results.by_tier();
        assert_eq!(by_tier.len(), 2);
        // Tier 1 should have 2 results
        let tier1 = by_tier.iter().find(|(t, _)| t.contains("Tier 1")).unwrap();
        assert_eq!(tier1.1.len(), 2);
    }

    #[test]
    fn test_run_results_failures() {
        let results = RunResults {
            results: vec![
                TestResult {
                    scenario_id: "pass".to_string(),
                    scenario_description: "Pass".to_string(),
                    backend: "Claude".to_string(),
                    tier: "Tier 1".to_string(),
                    passed: true,
                    assertions: vec![],
                    duration: Duration::from_secs(1),
                },
                TestResult {
                    scenario_id: "fail".to_string(),
                    scenario_description: "Fail".to_string(),
                    backend: "Claude".to_string(),
                    tier: "Tier 1".to_string(),
                    passed: false,
                    assertions: vec![],
                    duration: Duration::from_secs(1),
                },
            ],
            duration: Duration::from_secs(2),
            skipped_count: 0,
        };

        let failures = results.failures();
        assert_eq!(failures.len(), 1);
        assert_eq!(failures[0].scenario_id, "fail");
    }

    #[test]
    fn test_runner_scenario_count() {
        let workspace = test_workspace_base("scenario-count");
        let workspace_mgr = WorkspaceManager::new(workspace.clone());
        let scenarios: Vec<Box<dyn TestScenario>> = vec![
            Box::new(MockScenario::new("mock-1", true)),
            Box::new(MockScenario::new("mock-2", true)),
        ];

        let runner = TestRunner::new(workspace_mgr, scenarios);
        assert_eq!(runner.scenario_count(), 2);

        cleanup_workspace(&workspace);
    }

    #[test]
    fn test_runner_matching_scenarios_no_filter() {
        let workspace = test_workspace_base("matching-no-filter");
        let workspace_mgr = WorkspaceManager::new(workspace.clone());
        let scenarios: Vec<Box<dyn TestScenario>> = vec![
            Box::new(MockScenario::new("mock-1", true)),
            Box::new(MockScenario::new("mock-2", true)),
        ];

        let runner = TestRunner::new(workspace_mgr, scenarios);
        let config = RunConfig::new();
        let matching = runner.matching_scenarios(&config);

        assert_eq!(matching.len(), 2);

        cleanup_workspace(&workspace);
    }

    #[test]
    fn test_runner_matching_scenarios_with_filter() {
        let workspace = test_workspace_base("matching-filter");
        let workspace_mgr = WorkspaceManager::new(workspace.clone());
        let scenarios: Vec<Box<dyn TestScenario>> = vec![
            Box::new(MockScenario::new("claude-connect", true)),
            Box::new(MockScenario::new("kiro-connect", true)),
            Box::new(MockScenario::new("claude-loop", true)),
        ];

        let runner = TestRunner::new(workspace_mgr, scenarios);

        // Filter by "connect" should match 2
        let config = RunConfig::new().with_filter("connect");
        let matching = runner.matching_scenarios(&config);
        assert_eq!(matching.len(), 2);

        // Filter by "claude" should match 2
        let config = RunConfig::new().with_filter("claude");
        let matching = runner.matching_scenarios(&config);
        assert_eq!(matching.len(), 2);

        // Filter by "kiro" should match 1
        let config = RunConfig::new().with_filter("kiro");
        let matching = runner.matching_scenarios(&config);
        assert_eq!(matching.len(), 1);

        cleanup_workspace(&workspace);
    }

    #[test]
    fn test_runner_matching_scenarios_with_backend() {
        let workspace = test_workspace_base("matching-backend");
        let workspace_mgr = WorkspaceManager::new(workspace.clone());
        let scenarios: Vec<Box<dyn TestScenario>> = vec![
            Box::new(MockScenario::new("claude-test", true).with_backend(Backend::Claude)),
            Box::new(MockScenario::new("kiro-test", true).with_backend(Backend::Kiro)),
        ];

        let runner = TestRunner::new(workspace_mgr, scenarios);

        let config = RunConfig::new().with_backend(Backend::Claude);
        let matching = runner.matching_scenarios(&config);
        assert_eq!(matching.len(), 1);
        assert_eq!(matching[0].id(), "claude-test");

        cleanup_workspace(&workspace);
    }

    #[tokio::test]
    async fn test_runner_run_all_empty() {
        let workspace = test_workspace_base("run-empty");
        let workspace_mgr = WorkspaceManager::new(workspace.clone());
        let scenarios: Vec<Box<dyn TestScenario>> = vec![];

        let runner = TestRunner::new(workspace_mgr, scenarios);
        let results = runner.run_all().await.unwrap();

        assert_eq!(results.total_count(), 0);
        assert!(results.all_passed()); // Vacuous truth

        cleanup_workspace(&workspace);
    }

    #[tokio::test]
    async fn test_runner_run_single_passing() {
        let workspace = test_workspace_base("run-single-pass");
        let workspace_mgr = WorkspaceManager::new(workspace.clone());
        let scenarios: Vec<Box<dyn TestScenario>> = vec![Box::new(
            MockScenario::new("mock-1", true).with_backend(Backend::Claude),
        )];

        let runner = TestRunner::new(workspace_mgr, scenarios);
        let results = runner.run_all().await.unwrap();

        assert_eq!(results.total_count(), 1);
        assert_eq!(results.passed_count(), 1);
        assert!(results.all_passed());

        cleanup_workspace(&workspace);
    }

    #[tokio::test]
    async fn test_runner_run_single_failing() {
        let workspace = test_workspace_base("run-single-fail");
        let workspace_mgr = WorkspaceManager::new(workspace.clone());
        let scenarios: Vec<Box<dyn TestScenario>> = vec![Box::new(
            MockScenario::new("mock-1", false).with_backend(Backend::Claude),
        )];

        let runner = TestRunner::new(workspace_mgr, scenarios);
        let results = runner.run_all().await.unwrap();

        assert_eq!(results.total_count(), 1);
        assert_eq!(results.failed_count(), 1);
        assert!(!results.all_passed());

        cleanup_workspace(&workspace);
    }

    #[tokio::test]
    async fn test_runner_run_multiple_mixed() {
        let workspace = test_workspace_base("run-mixed");
        let workspace_mgr = WorkspaceManager::new(workspace.clone());
        let scenarios: Vec<Box<dyn TestScenario>> = vec![
            Box::new(MockScenario::new("pass-1", true).with_backend(Backend::Claude)),
            Box::new(MockScenario::new("fail-1", false).with_backend(Backend::Claude)),
            Box::new(MockScenario::new("pass-2", true).with_backend(Backend::Claude)),
        ];

        let runner = TestRunner::new(workspace_mgr, scenarios);
        let results = runner.run_all().await.unwrap();

        assert_eq!(results.total_count(), 3);
        assert_eq!(results.passed_count(), 2);
        assert_eq!(results.failed_count(), 1);
        assert!(!results.all_passed());

        cleanup_workspace(&workspace);
    }

    #[tokio::test]
    async fn test_runner_run_with_filter() {
        let workspace = test_workspace_base("run-filter");
        let workspace_mgr = WorkspaceManager::new(workspace.clone());
        let scenarios: Vec<Box<dyn TestScenario>> = vec![
            Box::new(MockScenario::new("connect-1", true).with_backend(Backend::Claude)),
            Box::new(MockScenario::new("connect-2", true).with_backend(Backend::Claude)),
            Box::new(MockScenario::new("loop-1", true).with_backend(Backend::Claude)),
        ];

        let runner = TestRunner::new(workspace_mgr, scenarios);
        let config = RunConfig::new().with_filter("connect");
        let results = runner.run(&config).await.unwrap();

        // Only "connect" scenarios should run
        assert_eq!(results.total_count(), 2);

        cleanup_workspace(&workspace);
    }

    #[tokio::test]
    async fn test_runner_no_matching_scenarios_error() {
        let workspace = test_workspace_base("run-no-match");
        let workspace_mgr = WorkspaceManager::new(workspace.clone());
        let scenarios: Vec<Box<dyn TestScenario>> =
            vec![Box::new(MockScenario::new("mock-1", true))];

        let runner = TestRunner::new(workspace_mgr, scenarios);
        let config = RunConfig::new().with_filter("nonexistent");
        let result = runner.run(&config).await;

        assert!(matches!(result, Err(RunnerError::NoMatchingScenarios(_))));

        cleanup_workspace(&workspace);
    }

    #[tokio::test]
    async fn test_runner_progress_callback() {
        let workspace = test_workspace_base("run-progress");
        let workspace_mgr = WorkspaceManager::new(workspace.clone());
        let scenarios: Vec<Box<dyn TestScenario>> = vec![Box::new(
            MockScenario::new("mock-1", true).with_backend(Backend::Claude),
        )];

        let event_count = Arc::new(AtomicUsize::new(0));
        let counter = event_count.clone();

        let runner = TestRunner::new(workspace_mgr, scenarios).on_progress(Box::new(move |_| {
            counter.fetch_add(1, Ordering::SeqCst);
        }));

        runner.run_all().await.unwrap();

        // Should have: RunStarted, ScenarioStarted, ScenarioCompleted, RunCompleted = 4 events
        assert_eq!(event_count.load(Ordering::SeqCst), 4);

        cleanup_workspace(&workspace);
    }

    #[tokio::test]
    async fn test_runner_keep_workspaces() {
        let workspace = test_workspace_base("run-keep");
        let workspace_mgr = WorkspaceManager::new(workspace.clone());
        let scenarios: Vec<Box<dyn TestScenario>> = vec![Box::new(
            MockScenario::new("mock-1", true).with_backend(Backend::Claude),
        )];

        let runner = TestRunner::new(workspace_mgr, scenarios);
        let config = RunConfig::new().keep_workspaces(true);
        runner.run(&config).await.unwrap();

        // Workspace should still exist (with backend suffix)
        let scenario_workspace = workspace.join("mock-1-claude");
        assert!(scenario_workspace.exists());

        cleanup_workspace(&workspace);
    }

    #[tokio::test]
    async fn test_runner_cleanup_workspaces() {
        let workspace = test_workspace_base("run-cleanup");
        let workspace_mgr = WorkspaceManager::new(workspace.clone());
        let scenarios: Vec<Box<dyn TestScenario>> = vec![Box::new(
            MockScenario::new("mock-1", true).with_backend(Backend::Claude),
        )];

        let runner = TestRunner::new(workspace_mgr, scenarios);
        let config = RunConfig::default(); // keep_workspaces = false
        runner.run(&config).await.unwrap();

        // Workspace should be cleaned up (with backend suffix)
        let scenario_workspace = workspace.join("mock-1-claude");
        assert!(!scenario_workspace.exists());

        cleanup_workspace(&workspace);
    }
}
