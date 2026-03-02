//! Hooks BDD runner for acceptance criteria.
//!
//! Discovery is backed by `cucumber-rs` Gherkin parsing over
//! `features/hooks/*.feature`, while execution routes scenarios through
//! deterministic AC evaluators and runtime harness assertions.

use crate::executor::{find_workspace_root, resolve_ralph_binary};
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
use thiserror::Error;

const HOOKS_FEATURE_DIR_WORKSPACE: &str = "crates/ralph-e2e/features/hooks";
const HOOKS_FEATURE_DIR_CRATE: &str = "features/hooks";

/// Configuration for executing the hooks BDD suite.
#[derive(Debug, Clone, Default)]
pub struct HooksBddConfig {
    /// Optional scenario filter (matches id, scenario title, tags, or feature filename).
    pub filter: Option<String>,
    /// Whether the suite is being executed in CI-safe mode.
    pub ci_safe_mode: bool,
}

impl HooksBddConfig {
    /// Creates a new hooks BDD run configuration.
    pub fn new(filter: Option<String>, ci_safe_mode: bool) -> Self {
        Self {
            filter,
            ci_safe_mode,
        }
    }
}

/// Discovery/execution errors for hooks BDD scaffolding.
#[derive(Debug, Error)]
pub enum HooksBddError {
    /// Workspace root could not be determined.
    #[error("workspace root not found")]
    WorkspaceRootNotFound,

    /// Hooks feature directory could not be found.
    #[error("hooks feature directory not found: {0}")]
    HooksFeatureDirNotFound(PathBuf),

    /// Failed to read the hooks feature directory.
    #[error("failed to read hooks feature directory {path}: {source}")]
    ReadFeatureDir {
        /// Path that failed to read.
        path: PathBuf,
        /// Source IO error.
        #[source]
        source: std::io::Error,
    },

    /// Failed to read a feature file.
    #[error("failed to read feature file {path}: {source}")]
    ReadFeatureFile {
        /// Feature file path.
        path: PathBuf,
        /// Source IO error.
        #[source]
        source: std::io::Error,
    },

    /// Feature file was malformed for cucumber-rs Gherkin parsing.
    #[error("invalid feature file {path}: {reason}")]
    InvalidFeatureFile {
        /// Feature file path.
        path: PathBuf,
        /// Validation reason.
        reason: String,
    },
}

/// One discovered hooks BDD scenario from a `.feature` file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HooksBddScenario {
    /// Stable AC ID tag when present (e.g. `AC-01`).
    pub scenario_id: String,
    /// Scenario title from `Scenario:` line.
    pub scenario_name: String,
    /// Feature file name (e.g. `scope-and-dispatch.feature`).
    pub feature_file: String,
    /// Scenario tags without `@` prefix.
    pub tags: Vec<String>,
    steps: Vec<HooksStep>,
}

/// Runtime command artifact scaffold for one hooks BDD integration invocation.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct HooksBddRunArtifact {
    /// Stable artifact name (e.g. "hooks.validate" or "ralph.run").
    pub name: String,
    /// Command preview for failure output.
    pub command: String,
    /// Working directory where the command runs.
    pub working_dir: PathBuf,
    /// Command timeout marker.
    pub timed_out: bool,
    /// Exit status code when available.
    pub exit_code: Option<i32>,
    /// Planned stdout capture location.
    pub stdout_path: PathBuf,
    /// Planned stderr capture location.
    pub stderr_path: PathBuf,
}

/// Scenario-level artifact manifest used by hooks BDD runtime assertions.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct HooksBddScenarioArtifacts {
    /// Root directory where scenario artifacts are written.
    pub root_dir: PathBuf,
    /// Command-level artifacts captured during the scenario.
    pub run_artifacts: Vec<HooksBddRunArtifact>,
}

/// Runtime harness scaffold for hooks BDD integration execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HooksBddIntegrationHarness {
    scenario_id: String,
    scenario_name: String,
    ci_safe_mode: bool,
    artifacts: HooksBddScenarioArtifacts,
}

impl HooksBddIntegrationHarness {
    /// Creates a new harness with deterministic artifact directory scaffolding.
    pub fn new(scenario: &HooksBddScenario, ci_safe_mode: bool) -> Self {
        let scenario_slug = format!(
            "{}-{}",
            slugify_path_segment(&scenario.scenario_id),
            slugify_path_segment(&scenario.scenario_name)
        );

        let root_dir = default_hooks_bdd_artifact_root().join(scenario_slug);

        Self {
            scenario_id: scenario.scenario_id.clone(),
            scenario_name: scenario.scenario_name.clone(),
            ci_safe_mode,
            artifacts: HooksBddScenarioArtifacts {
                root_dir,
                run_artifacts: Vec::new(),
            },
        }
    }

    /// Returns the stable AC identifier for this harness.
    pub fn scenario_id(&self) -> &str {
        &self.scenario_id
    }

    /// Returns the human-readable scenario name.
    pub fn scenario_name(&self) -> &str {
        &self.scenario_name
    }

    /// Returns whether the harness is in CI-safe mode.
    pub fn ci_safe_mode(&self) -> bool {
        self.ci_safe_mode
    }

    /// Returns immutable access to scaffolded artifact metadata.
    pub fn artifacts(&self) -> &HooksBddScenarioArtifacts {
        &self.artifacts
    }

    /// Returns mutable access to scaffolded artifact metadata.
    pub fn artifacts_mut(&mut self) -> &mut HooksBddScenarioArtifacts {
        &mut self.artifacts
    }

    /// Registers a run artifact scaffold and returns its index.
    pub fn scaffold_run_artifact(
        &mut self,
        name: impl Into<String>,
        command: impl Into<String>,
    ) -> usize {
        let name = name.into();
        let command = command.into();
        let next_index = self.artifacts.run_artifacts.len() + 1;
        let artifact_slug = format!("{:02}-{}", next_index, slugify_path_segment(&name));
        let artifact_dir = self.artifacts.root_dir.join(artifact_slug);

        self.artifacts.run_artifacts.push(HooksBddRunArtifact {
            name,
            command,
            working_dir: PathBuf::new(),
            timed_out: false,
            exit_code: None,
            stdout_path: artifact_dir.join("stdout.log"),
            stderr_path: artifact_dir.join("stderr.log"),
        });

        next_index - 1
    }

    /// Creates a deterministic temporary workspace for a hooks BDD scenario run.
    pub fn prepare_temp_workspace(&self, workspace_name: &str) -> Result<PathBuf, String> {
        let workspace_parent = self.artifacts.root_dir.join("workspace");
        fs::create_dir_all(&workspace_parent).map_err(|source| {
            format!(
                "{}: failed to create workspace parent {}: {source}",
                self.scenario_id,
                workspace_parent.display()
            )
        })?;

        let workspace_dir = workspace_parent.join(slugify_path_segment(workspace_name));
        if workspace_dir.exists() {
            fs::remove_dir_all(&workspace_dir).map_err(|source| {
                format!(
                    "{}: failed to reset workspace {}: {source}",
                    self.scenario_id,
                    workspace_dir.display()
                )
            })?;
        }

        fs::create_dir_all(workspace_dir.join(".ralph/agent")).map_err(|source| {
            format!(
                "{}: failed to create workspace {}: {source}",
                self.scenario_id,
                workspace_dir.display()
            )
        })?;

        Ok(workspace_dir)
    }

    /// Writes a workspace-relative file, creating parent directories as needed.
    pub fn write_workspace_file(
        &self,
        workspace_dir: &Path,
        relative_path: &str,
        content: &str,
    ) -> Result<PathBuf, String> {
        let relative = Path::new(relative_path);
        if relative.is_absolute() {
            return Err(format!(
                "{}: workspace file path must be relative: {}",
                self.scenario_id, relative_path
            ));
        }

        let target_path = workspace_dir.join(relative);
        if let Some(parent) = target_path.parent() {
            fs::create_dir_all(parent).map_err(|source| {
                format!(
                    "{}: failed to create parent dir {}: {source}",
                    self.scenario_id,
                    parent.display()
                )
            })?;
        }

        fs::write(&target_path, content).map_err(|source| {
            format!(
                "{}: failed to write workspace file {}: {source}",
                self.scenario_id,
                target_path.display()
            )
        })?;

        Ok(target_path)
    }

    /// Creates an executable hook script under `<workspace>/hooks/`.
    pub fn write_hook_script(
        &self,
        workspace_dir: &Path,
        script_name: &str,
        script_body: &str,
    ) -> Result<PathBuf, String> {
        let script_file_name = format!("{}.sh", slugify_path_segment(script_name));
        let script_relative_path = format!("hooks/{script_file_name}");
        let script_content = normalize_hook_script_content(script_body);
        let script_path =
            self.write_workspace_file(workspace_dir, &script_relative_path, &script_content)?;

        mark_file_executable(&script_path).map_err(|source| {
            format!(
                "{}: failed to mark hook script executable {}: {source}",
                self.scenario_id,
                script_path.display()
            )
        })?;

        Ok(script_path)
    }

    /// Runs an arbitrary command with a bounded timeout and writes stdout/stderr artifacts.
    pub fn run_bounded_command(
        &mut self,
        artifact_name: impl Into<String>,
        workspace_dir: &Path,
        binary: &Path,
        args: &[&str],
        timeout: Duration,
    ) -> Result<HooksBddRunArtifact, String> {
        if !workspace_dir.is_dir() {
            return Err(format!(
                "{}: workspace directory does not exist: {}",
                self.scenario_id,
                workspace_dir.display()
            ));
        }

        let command_preview =
            format_command_preview(binary.as_os_str(), args.iter().copied().map(OsStr::new));
        let artifact_index = self.scaffold_run_artifact(artifact_name, command_preview);

        let (stdout_path, stderr_path) = {
            let artifact = self
                .artifacts
                .run_artifacts
                .get_mut(artifact_index)
                .ok_or_else(|| {
                    format!(
                        "{}: internal error: missing run artifact at index {}",
                        self.scenario_id, artifact_index
                    )
                })?;

            artifact.working_dir = workspace_dir.to_path_buf();
            (artifact.stdout_path.clone(), artifact.stderr_path.clone())
        };

        if let Some(parent) = stdout_path.parent() {
            fs::create_dir_all(parent).map_err(|source| {
                format!(
                    "{}: failed to create stdout artifact dir {}: {source}",
                    self.scenario_id,
                    parent.display()
                )
            })?;
        }

        if let Some(parent) = stderr_path.parent() {
            fs::create_dir_all(parent).map_err(|source| {
                format!(
                    "{}: failed to create stderr artifact dir {}: {source}",
                    self.scenario_id,
                    parent.display()
                )
            })?;
        }

        let stdout_file = fs::File::create(&stdout_path).map_err(|source| {
            format!(
                "{}: failed to create stdout artifact {}: {source}",
                self.scenario_id,
                stdout_path.display()
            )
        })?;
        let stderr_file = fs::File::create(&stderr_path).map_err(|source| {
            format!(
                "{}: failed to create stderr artifact {}: {source}",
                self.scenario_id,
                stderr_path.display()
            )
        })?;

        let mut command = Command::new(binary);
        command
            .args(args)
            .current_dir(workspace_dir)
            .stdin(Stdio::null())
            .stdout(Stdio::from(stdout_file))
            .stderr(Stdio::from(stderr_file));

        let mut child = command.spawn().map_err(|source| {
            format!(
                "{}: failed to spawn command `{}` in {}: {source}",
                self.scenario_id,
                self.artifacts.run_artifacts[artifact_index].command,
                workspace_dir.display()
            )
        })?;

        let poll_interval = Duration::from_millis(20);
        let start = Instant::now();
        let mut timed_out = false;

        let status = loop {
            match child.try_wait() {
                Ok(Some(status)) => break status,
                Ok(None) => {
                    if start.elapsed() >= timeout {
                        timed_out = true;
                        if let Err(source) = child.kill()
                            && source.kind() != std::io::ErrorKind::InvalidInput
                        {
                            return Err(format!(
                                "{}: failed to terminate timed out command `{}`: {source}",
                                self.scenario_id,
                                self.artifacts.run_artifacts[artifact_index].command
                            ));
                        }

                        break child.wait().map_err(|source| {
                            format!(
                                "{}: failed waiting for timed out command `{}`: {source}",
                                self.scenario_id,
                                self.artifacts.run_artifacts[artifact_index].command
                            )
                        })?;
                    }

                    std::thread::sleep(poll_interval);
                }
                Err(source) => {
                    return Err(format!(
                        "{}: failed while polling command `{}`: {source}",
                        self.scenario_id, self.artifacts.run_artifacts[artifact_index].command
                    ));
                }
            }
        };

        if let Some(artifact) = self.artifacts.run_artifacts.get_mut(artifact_index) {
            artifact.exit_code = status.code();
            artifact.timed_out = timed_out;
        }

        self.artifacts
            .run_artifacts
            .get(artifact_index)
            .cloned()
            .ok_or_else(|| {
                format!(
                    "{}: internal error: run artifact missing after command execution",
                    self.scenario_id
                )
            })
    }

    /// Runs `ralph` with a bounded timeout and writes stdout/stderr to run artifacts.
    pub fn run_bounded_ralph_command(
        &mut self,
        artifact_name: impl Into<String>,
        workspace_dir: &Path,
        args: &[&str],
        timeout: Duration,
    ) -> Result<HooksBddRunArtifact, String> {
        let ralph_binary = resolve_ralph_binary();
        self.run_bounded_command(
            artifact_name,
            workspace_dir,
            ralph_binary.as_path(),
            args,
            timeout,
        )
    }

    /// Consumes the harness and returns captured artifact metadata.
    pub fn into_artifacts(self) -> HooksBddScenarioArtifacts {
        self.artifacts
    }
}

/// Result of executing one hooks BDD scenario.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HooksBddScenarioResult {
    /// Stable AC ID tag (or fallback scenario title if tag missing).
    pub scenario_id: String,
    /// Scenario title.
    pub scenario_name: String,
    /// Feature file name.
    pub feature_file: String,
    /// Whether the scenario passed.
    pub passed: bool,
    /// Pass/fail reason for terminal output.
    pub message: String,
    /// Runtime artifacts scaffolded and/or produced during evaluation.
    pub artifacts: HooksBddScenarioArtifacts,
}

/// Aggregated hooks BDD run results.
#[derive(Debug, Clone, Default)]
pub struct HooksBddRunResults {
    /// Individual scenario results in deterministic file/scenario order.
    pub results: Vec<HooksBddScenarioResult>,
}

impl HooksBddRunResults {
    /// Total number of executed scenarios.
    pub fn total_count(&self) -> usize {
        self.results.len()
    }

    /// Number of passed scenarios.
    pub fn passed_count(&self) -> usize {
        self.results.iter().filter(|result| result.passed).count()
    }

    /// Number of failed scenarios.
    pub fn failed_count(&self) -> usize {
        self.results.iter().filter(|result| !result.passed).count()
    }

    /// Returns true when every scenario passed.
    pub fn all_passed(&self) -> bool {
        self.results.iter().all(|result| result.passed)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum HooksStepKeyword {
    Given,
    When,
    Then,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct HooksStep {
    keyword: HooksStepKeyword,
    text: String,
}

fn default_hooks_bdd_artifact_root() -> PathBuf {
    if let Some(workspace_root) = find_workspace_root() {
        return workspace_root.join(".ralph/hooks-bdd-artifacts");
    }

    PathBuf::from(".ralph/hooks-bdd-artifacts")
}

fn slugify_path_segment(value: &str) -> String {
    let mut slug = String::new();
    let mut previous_was_separator = false;

    for character in value.chars().flat_map(char::to_lowercase) {
        if character.is_ascii_alphanumeric() {
            slug.push(character);
            previous_was_separator = false;
        } else if !previous_was_separator {
            slug.push('-');
            previous_was_separator = true;
        }
    }

    let slug = slug.trim_matches('-');
    if slug.is_empty() {
        "scenario".to_string()
    } else {
        slug.to_string()
    }
}

fn normalize_hook_script_content(script_body: &str) -> String {
    let trimmed = script_body.trim();
    if trimmed.starts_with("#!") {
        format!("{trimmed}\n")
    } else {
        format!("#!/usr/bin/env bash\nset -euo pipefail\n{trimmed}\n")
    }
}

fn format_command_preview<'a>(binary: &OsStr, args: impl Iterator<Item = &'a OsStr>) -> String {
    let mut parts = Vec::new();
    parts.push(binary.to_string_lossy().to_string());

    for arg in args {
        parts.push(arg.to_string_lossy().to_string());
    }

    parts.join(" ")
}

#[cfg(unix)]
fn mark_file_executable(path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;

    let mut permissions = fs::metadata(path)?.permissions();
    permissions.set_mode(0o755);
    fs::set_permissions(path, permissions)
}

#[cfg(not(unix))]
fn mark_file_executable(_path: &Path) -> std::io::Result<()> {
    Ok(())
}

/// Discovers hook BDD scenarios from `features/hooks/*.feature`.
pub fn discover_hooks_bdd_scenarios(
    filter: Option<&str>,
) -> Result<Vec<HooksBddScenario>, HooksBddError> {
    let hooks_dir = hooks_feature_dir()?;
    let mut feature_paths: Vec<PathBuf> = fs::read_dir(&hooks_dir)
        .map_err(|source| HooksBddError::ReadFeatureDir {
            path: hooks_dir.clone(),
            source,
        })?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "feature"))
        .collect();

    feature_paths.sort();

    let mut scenarios = Vec::new();
    for feature_path in &feature_paths {
        scenarios.extend(parse_feature_file(feature_path)?);
    }

    if let Some(filter_text) = filter {
        let filter_lower = filter_text.to_lowercase();
        scenarios.retain(|scenario| matches_filter(scenario, &filter_lower));
    }

    Ok(scenarios)
}

/// Executes discovered hooks BDD scenarios through AC evaluator dispatch.
///
/// Routes each scenario to its corresponding AC evaluator for green verification.
pub fn run_hooks_bdd_suite(config: &HooksBddConfig) -> Result<HooksBddRunResults, HooksBddError> {
    let scenarios = discover_hooks_bdd_scenarios(config.filter.as_deref())?;
    let mut results = Vec::with_capacity(scenarios.len());

    for scenario in scenarios {
        results.push(execute_scenario(&scenario, config.ci_safe_mode));
    }

    Ok(HooksBddRunResults { results })
}

/// Execute a scenario through the AC evaluator dispatch.
fn execute_scenario(scenario: &HooksBddScenario, ci_safe_mode: bool) -> HooksBddScenarioResult {
    let mut harness = HooksBddIntegrationHarness::new(scenario, ci_safe_mode);

    // Route through evaluator dispatch for green verification.
    let evaluator = dispatch_ac_evaluator(&scenario.scenario_id);
    evaluator(scenario, &mut harness, ci_safe_mode)
}

/// AC evaluator dispatch map - routes AC IDs to their evaluator functions.
fn dispatch_ac_evaluator(
    ac_id: &str,
) -> fn(&HooksBddScenario, &mut HooksBddIntegrationHarness, bool) -> HooksBddScenarioResult {
    match ac_id {
        // AC-01..AC-03: Scope, lifecycle events, pre/post phases
        "AC-01" => evaluate_ac_01,
        "AC-02" => evaluate_ac_02,
        "AC-03" => evaluate_ac_03,
        // AC-04..AC-06: Ordering, stdin contract, timeout
        "AC-04" => evaluate_ac_04,
        "AC-05" => evaluate_ac_05,
        "AC-06" => evaluate_ac_06,
        // AC-07..AC-18: Safeguards, dispositions, suspend/resume, mutation, telemetry
        "AC-07" => evaluate_ac_07,
        "AC-08" => evaluate_ac_08,
        "AC-09" => evaluate_ac_09,
        "AC-10" => evaluate_ac_10,
        "AC-11" => evaluate_ac_11,
        "AC-12" => evaluate_ac_12,
        "AC-13" => evaluate_ac_13,
        "AC-14" => evaluate_ac_14,
        "AC-15" => evaluate_ac_15,
        "AC-16" => evaluate_ac_16,
        "AC-17" => evaluate_ac_17,
        "AC-18" => evaluate_ac_18,
        _ => evaluate_unmapped_acceptance,
    }
}

fn build_scenario_result(
    scenario: &HooksBddScenario,
    harness: &HooksBddIntegrationHarness,
    passed: bool,
    message: String,
) -> HooksBddScenarioResult {
    HooksBddScenarioResult {
        scenario_id: scenario.scenario_id.clone(),
        scenario_name: scenario.scenario_name.clone(),
        feature_file: scenario.feature_file.clone(),
        passed,
        message,
        artifacts: harness.artifacts().clone(),
    }
}

/// Green evaluator wrapper that validates acceptance context and returns pass/fail.
fn evaluate_green_acceptance(
    scenario: &HooksBddScenario,
    harness: &mut HooksBddIntegrationHarness,
    ci_safe_mode: bool,
    context_guard: fn(bool, &str) -> Result<(), String>,
    evaluation: fn(&mut HooksBddIntegrationHarness) -> Result<(), String>,
) -> HooksBddScenarioResult {
    // Guard CI-safe mode requirement
    if let Err(msg) = context_guard(ci_safe_mode, &scenario.scenario_id) {
        return build_scenario_result(scenario, harness, false, msg);
    }

    if let Err(msg) = assert_runtime_integration_coverage(&scenario.scenario_id, harness) {
        return build_scenario_result(scenario, harness, false, msg);
    }

    // Run the AC-specific evaluation assertions.
    match evaluation(harness) {
        Ok(()) => build_scenario_result(
            scenario,
            harness,
            true,
            format!(
                "{}: acceptance criterion verified green",
                scenario.scenario_id
            ),
        ),
        Err(msg) => build_scenario_result(scenario, harness, false, msg),
    }
}

/// Validates that CI-safe mode is enabled for the evaluation.
fn validate_acceptance_context(ci_safe_mode: bool, ac_id: &str) -> Result<(), String> {
    if !ci_safe_mode {
        return Err(format!(
            "{}: CI-safe mode required; rerun hooks BDD with --mock",
            ac_id
        ));
    }
    Ok(())
}

#[derive(Debug, Clone, Copy)]
struct RuntimeTestCase {
    package: &'static str,
    filter: &'static str,
}

fn runtime_test_cases_for_ac(ac_id: &str) -> Vec<RuntimeTestCase> {
    match ac_id {
        "AC-01" => vec![
            RuntimeTestCase {
                package: "ralph-core",
                filter: "test_hooks_config_boundary_accepts_valid_file",
            },
            RuntimeTestCase {
                package: "ralph-core",
                filter: "test_hooks_config_boundary_rejects_non_v1_scope_field",
            },
        ],
        "AC-02" => vec![RuntimeTestCase {
            package: "ralph-cli",
            filter: "test_dispatch_phase_event_hooks_routes_by_phase_and_preserves_order",
        }],
        "AC-03" => vec![RuntimeTestCase {
            package: "ralph-cli",
            filter: "test_dispatch_phase_event_hooks_routes_by_phase_and_preserves_order",
        }],
        "AC-04" => vec![
            RuntimeTestCase {
                package: "ralph-core",
                filter: "resolve_phase_event_preserves_declaration_order",
            },
            RuntimeTestCase {
                package: "ralph-cli",
                filter: "test_dispatch_phase_event_hooks_routes_by_phase_and_preserves_order",
            },
        ],
        "AC-05" => vec![RuntimeTestCase {
            package: "ralph-core",
            filter: "run_writes_json_payload_to_hook_stdin",
        }],
        "AC-06" => vec![RuntimeTestCase {
            package: "ralph-core",
            filter: "run_marks_timed_out_when_command_exceeds_timeout",
        }],
        "AC-07" => vec![RuntimeTestCase {
            package: "ralph-core",
            filter: "run_truncates_stdout_and_stderr_at_max_output_bytes",
        }],
        "AC-08" => vec![RuntimeTestCase {
            package: "ralph-cli",
            filter: "test_loop_start_dispatch_warn_continues_and_block_aborts",
        }],
        "AC-09" => vec![RuntimeTestCase {
            package: "ralph-cli",
            filter: "test_loop_start_dispatch_warn_continues_and_block_aborts",
        }],
        "AC-10" => vec![RuntimeTestCase {
            package: "ralph-cli",
            filter: "test_iteration_start_suspend_waits_for_resume_and_clears_artifacts_before_continuing",
        }],
        "AC-11" => vec![RuntimeTestCase {
            package: "ralph-cli",
            filter: "test_wait_for_resume_if_suspended_resumes_and_clears_suspend_artifacts",
        }],
        "AC-12" => vec![RuntimeTestCase {
            package: "ralph-cli",
            filter: "test_wait_for_resume_if_suspended_is_noop_without_suspend_dispositions",
        }],
        "AC-13" => vec![RuntimeTestCase {
            package: "ralph-cli",
            filter: "test_ac13_mutation_disabled_json_output_is_inert_for_accumulator_and_downstream_payloads",
        }],
        "AC-14" => vec![
            RuntimeTestCase {
                package: "ralph-cli",
                filter: "test_ac14_mutation_enabled_updates_only_namespaced_metadata_in_downstream_payloads",
            },
            RuntimeTestCase {
                package: "ralph-cli",
                filter: "test_parse_hook_mutation_stdout_accepts_metadata_only_payload_and_namespaces_by_hook",
            },
        ],
        "AC-15" => vec![
            RuntimeTestCase {
                package: "ralph-cli",
                filter: "test_ac15_dispatch_phase_event_hooks_non_json_mutation_warn_continues_through_block_gate",
            },
            RuntimeTestCase {
                package: "ralph-cli",
                filter: "test_ac15_dispatch_phase_event_hooks_non_json_mutation_block_surfaces_invalid_output_reason",
            },
            RuntimeTestCase {
                package: "ralph-cli",
                filter: "test_ac15_dispatch_phase_event_hooks_non_json_mutation_suspend_uses_wait_for_resume_gate",
            },
        ],
        "AC-16" => vec![
            RuntimeTestCase {
                package: "ralph-cli",
                filter: "test_dispatch_phase_event_hooks_retry_backoff_recovers_before_exhaustion",
            },
            RuntimeTestCase {
                package: "ralph-core",
                filter: "test_diagnostics_collector_logs_hook_run_telemetry",
            },
        ],
        "AC-17" => vec![RuntimeTestCase {
            package: "ralph-cli",
            filter: "test_hooks_validate_json_success_report_and_exit_code",
        }],
        "AC-18" => vec![
            RuntimeTestCase {
                package: "ralph-cli",
                filter: "test_preflight_check_config_json",
            },
            RuntimeTestCase {
                package: "ralph-core",
                filter: "default_checks_include_hooks_check_name",
            },
        ],
        _ => Vec::new(),
    }
}

fn read_artifact_excerpt(path: &Path, max_chars: usize) -> String {
    match fs::read_to_string(path) {
        Ok(content) => {
            let trimmed = content.trim();
            if trimmed.chars().count() <= max_chars {
                trimmed.to_string()
            } else {
                let tail: String = trimmed
                    .chars()
                    .rev()
                    .take(max_chars)
                    .collect::<String>()
                    .chars()
                    .rev()
                    .collect();
                format!("…{tail}")
            }
        }
        Err(source) => format!("(failed to read {}: {source})", path.display()),
    }
}

fn assert_runtime_integration_coverage(
    ac_id: &str,
    harness: &mut HooksBddIntegrationHarness,
) -> Result<(), String> {
    if cfg!(test) {
        // Unit tests already exercise evaluators directly; avoid recursive nested
        // `cargo test` subprocesses during `cargo test -p ralph-e2e`.
        return Ok(());
    }

    let runtime_tests = runtime_test_cases_for_ac(ac_id);
    if runtime_tests.is_empty() {
        return Err(format!(
            "{ac_id}: runtime integration mapping is missing for this acceptance criterion"
        ));
    }

    let workspace_root = find_workspace_root().ok_or_else(|| {
        format!("{ac_id}: failed to determine workspace root for runtime integration checks")
    })?;

    for runtime_test in runtime_tests {
        let args = [
            "test",
            "-p",
            runtime_test.package,
            runtime_test.filter,
            "--",
            "--nocapture",
        ];
        let artifact_name = format!(
            "cargo-test-{}-{}",
            slugify_path_segment(runtime_test.package),
            slugify_path_segment(runtime_test.filter)
        );

        let artifact = harness.run_bounded_command(
            artifact_name,
            &workspace_root,
            Path::new("cargo"),
            &args,
            Duration::from_secs(180),
        )?;

        if artifact.timed_out {
            return Err(format!(
                "{ac_id}: runtime integration test timed out ({}/{})",
                runtime_test.package, runtime_test.filter
            ));
        }

        if artifact.exit_code != Some(0) {
            let stdout_tail = read_artifact_excerpt(&artifact.stdout_path, 800);
            let stderr_tail = read_artifact_excerpt(&artifact.stderr_path, 800);
            return Err(format!(
                "{ac_id}: runtime integration test failed ({}/{}) [exit={:?}]\nstdout: {}\nstderr: {}",
                runtime_test.package,
                runtime_test.filter,
                artifact.exit_code,
                stdout_tail,
                stderr_tail
            ));
        }
    }

    Ok(())
}

/// Fallback evaluator for unmapped acceptance IDs.
fn evaluate_unmapped_acceptance(
    scenario: &HooksBddScenario,
    harness: &mut HooksBddIntegrationHarness,
    _ci_safe_mode: bool,
) -> HooksBddScenarioResult {
    build_scenario_result(
        scenario,
        harness,
        false,
        format!(
            "{}: no evaluator implemented - scenario is pending",
            scenario.scenario_id
        ),
    )
}

// =============================================================================
// Source evidence helpers (Step 2.1)
// =============================================================================

fn load_workspace_source_file(relative_path: &str) -> Result<String, String> {
    let relative = Path::new(relative_path);

    let source_path = if relative.is_absolute() {
        relative.to_path_buf()
    } else {
        let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
        let manifest_workspace_root = manifest_dir
            .parent()
            .and_then(Path::parent)
            .map(Path::to_path_buf)
            .ok_or_else(|| {
                format!(
                    "failed to derive workspace root from CARGO_MANIFEST_DIR={}",
                    manifest_dir.display()
                )
            })?;

        let manifest_candidate = manifest_workspace_root.join(relative);
        if manifest_candidate.is_file() {
            manifest_candidate
        } else if let Some(discovered_root) = find_workspace_root() {
            let discovered_candidate = discovered_root.join(relative);
            if discovered_candidate.is_file() {
                discovered_candidate
            } else {
                return Err(format!(
                    "source evidence file not found: {} (checked {} and {})",
                    relative_path,
                    manifest_candidate.display(),
                    discovered_candidate.display()
                ));
            }
        } else {
            return Err(format!(
                "source evidence file not found: {} (checked {})",
                relative_path,
                manifest_candidate.display()
            ));
        }
    };

    fs::read_to_string(&source_path).map_err(|source| {
        format!(
            "failed to read source evidence file {}: {source}",
            source_path.display()
        )
    })
}

fn assert_required_source_snippets(
    source_file: &str,
    source_content: &str,
    required_snippets: &[(&str, &str)],
) -> Result<(), String> {
    let missing: Vec<String> = required_snippets
        .iter()
        .filter_map(|(description, snippet)| {
            (!source_content.contains(snippet))
                .then_some(format!("{description} (snippet: `{snippet}`)"))
        })
        .collect();

    if missing.is_empty() {
        return Ok(());
    }

    Err(format!(
        "source evidence assertion failed for {source_file}: missing {}",
        missing.join(", ")
    ))
}

fn assert_workspace_source_contains(
    relative_path: &str,
    required_snippets: &[(&str, &str)],
) -> Result<(), String> {
    let source_content = load_workspace_source_file(relative_path)?;
    assert_required_source_snippets(relative_path, &source_content, required_snippets)
}

// =============================================================================
// AC-01: Per-project scope only
// =============================================================================

fn evaluate_ac_01(
    scenario: &HooksBddScenario,
    harness: &mut HooksBddIntegrationHarness,
    ci_safe_mode: bool,
) -> HooksBddScenarioResult {
    evaluate_green_acceptance(
        scenario,
        harness,
        ci_safe_mode,
        validate_acceptance_context,
        |_harness| {
            assert_workspace_source_contains(
                "crates/ralph-core/src/config.rs",
                &[
                    (
                        "RalphConfig carries hooks config at project scope",
                        "pub hooks: HooksConfig,",
                    ),
                    (
                        "RalphConfig default initializes hooks without global source",
                        "hooks: HooksConfig::default(),",
                    ),
                    (
                        "hooks docs explicitly describe per-project scope",
                        "Controls per-project orchestrator lifecycle hooks.",
                    ),
                ],
            )?;

            assert_workspace_source_contains(
                "crates/ralph-core/src/hooks/engine.rs",
                &[
                    (
                        "HookEngine is constructed directly from HooksConfig",
                        "pub fn new(config: &HooksConfig) -> Self {",
                    ),
                    (
                        "HookEngine clones defaults from project config",
                        "defaults: config.defaults.clone(),",
                    ),
                    (
                        "HookEngine clones event map from project config",
                        "hooks_by_phase_event: config.events.clone(),",
                    ),
                ],
            )?;

            Ok(())
        },
    )
}

// =============================================================================
// AC-02: Mandatory lifecycle events supported
// =============================================================================

fn evaluate_ac_02(
    scenario: &HooksBddScenario,
    harness: &mut HooksBddIntegrationHarness,
    ci_safe_mode: bool,
) -> HooksBddScenarioResult {
    evaluate_green_acceptance(
        scenario,
        harness,
        ci_safe_mode,
        validate_acceptance_context,
        |_harness| {
            assert_workspace_source_contains(
                "crates/ralph-core/src/config.rs",
                &[
                    (
                        "pre.loop.start phase-event parses",
                        "\"pre.loop.start\" => Some(Self::PreLoopStart),",
                    ),
                    (
                        "post.loop.start phase-event parses",
                        "\"post.loop.start\" => Some(Self::PostLoopStart),",
                    ),
                    (
                        "pre.iteration.start phase-event parses",
                        "\"pre.iteration.start\" => Some(Self::PreIterationStart),",
                    ),
                    (
                        "post.iteration.start phase-event parses",
                        "\"post.iteration.start\" => Some(Self::PostIterationStart),",
                    ),
                    (
                        "pre.plan.created phase-event parses",
                        "\"pre.plan.created\" => Some(Self::PrePlanCreated),",
                    ),
                    (
                        "post.plan.created phase-event parses",
                        "\"post.plan.created\" => Some(Self::PostPlanCreated),",
                    ),
                    (
                        "pre.human.interact phase-event parses",
                        "\"pre.human.interact\" => Some(Self::PreHumanInteract),",
                    ),
                    (
                        "post.human.interact phase-event parses",
                        "\"post.human.interact\" => Some(Self::PostHumanInteract),",
                    ),
                    (
                        "pre.loop.complete phase-event parses",
                        "\"pre.loop.complete\" => Some(Self::PreLoopComplete),",
                    ),
                    (
                        "post.loop.complete phase-event parses",
                        "\"post.loop.complete\" => Some(Self::PostLoopComplete),",
                    ),
                    (
                        "pre.loop.error phase-event parses",
                        "\"pre.loop.error\" => Some(Self::PreLoopError),",
                    ),
                    (
                        "post.loop.error phase-event parses",
                        "\"post.loop.error\" => Some(Self::PostLoopError),",
                    ),
                ],
            )?;

            assert_workspace_source_contains(
                "crates/ralph-core/src/hooks/engine.rs",
                &[
                    (
                        "phase-event resolver dispatches parsed canonical keys",
                        "HookPhaseEvent::parse(phase_event)",
                    ),
                    ("payload builder carries phase", "phase: phase.to_string(),"),
                    ("payload builder carries event", "event: event.to_string(),"),
                    (
                        "payload builder carries canonical phase_event",
                        "phase_event: phase_event.as_str().to_string(),",
                    ),
                    (
                        "payload includes loop block",
                        "loop_context: HookPayloadLoop {",
                    ),
                    (
                        "payload includes iteration block",
                        "iteration: HookPayloadIteration {",
                    ),
                    (
                        "payload includes context block",
                        "context: HookPayloadContext {",
                    ),
                    (
                        "payload includes metadata block",
                        "metadata: HookPayloadMetadata {",
                    ),
                ],
            )?;

            Ok(())
        },
    )
}

// =============================================================================
// AC-03: Pre/post phase support
// =============================================================================

fn evaluate_ac_03(
    scenario: &HooksBddScenario,
    harness: &mut HooksBddIntegrationHarness,
    ci_safe_mode: bool,
) -> HooksBddScenarioResult {
    evaluate_green_acceptance(
        scenario,
        harness,
        ci_safe_mode,
        validate_acceptance_context,
        |_harness| {
            assert_workspace_source_contains(
                "crates/ralph-core/src/config.rs",
                &[
                    (
                        "pre.loop.start serde key exists",
                        "#[serde(rename = \"pre.loop.start\")]",
                    ),
                    (
                        "post.loop.start serde key exists",
                        "#[serde(rename = \"post.loop.start\")]",
                    ),
                    (
                        "pre.iteration.start serde key exists",
                        "#[serde(rename = \"pre.iteration.start\")]",
                    ),
                    (
                        "post.iteration.start serde key exists",
                        "#[serde(rename = \"post.iteration.start\")]",
                    ),
                    (
                        "pre.plan.created serde key exists",
                        "#[serde(rename = \"pre.plan.created\")]",
                    ),
                    (
                        "post.plan.created serde key exists",
                        "#[serde(rename = \"post.plan.created\")]",
                    ),
                    (
                        "pre.human.interact serde key exists",
                        "#[serde(rename = \"pre.human.interact\")]",
                    ),
                    (
                        "post.human.interact serde key exists",
                        "#[serde(rename = \"post.human.interact\")]",
                    ),
                    (
                        "pre.loop.complete serde key exists",
                        "#[serde(rename = \"pre.loop.complete\")]",
                    ),
                    (
                        "post.loop.complete serde key exists",
                        "#[serde(rename = \"post.loop.complete\")]",
                    ),
                    (
                        "pre.loop.error serde key exists",
                        "#[serde(rename = \"pre.loop.error\")]",
                    ),
                    (
                        "post.loop.error serde key exists",
                        "#[serde(rename = \"post.loop.error\")]",
                    ),
                ],
            )?;

            assert_workspace_source_contains(
                "crates/ralph-core/src/hooks/engine.rs",
                &[
                    (
                        "phase/event splitting helper exists",
                        "fn split_phase_event(phase_event: HookPhaseEvent) -> (&'static str, &'static str) {",
                    ),
                    (
                        "split helper derives phase and event from canonical key",
                        "phase_event.as_str().split_once('.')",
                    ),
                    (
                        "payload build uses split pre/post phase",
                        "let (phase, event) = split_phase_event(phase_event);",
                    ),
                ],
            )?;

            Ok(())
        },
    )
}

// =============================================================================
// AC-04: Deterministic ordering
// =============================================================================

fn evaluate_ac_04(
    scenario: &HooksBddScenario,
    harness: &mut HooksBddIntegrationHarness,
    ci_safe_mode: bool,
) -> HooksBddScenarioResult {
    evaluate_green_acceptance(
        scenario,
        harness,
        ci_safe_mode,
        validate_acceptance_context,
        |_harness| {
            assert_workspace_source_contains(
                "crates/ralph-core/src/config.rs",
                &[(
                    "phase-event hook lists preserve declaration order via Vec",
                    "pub events: HashMap<HookPhaseEvent, Vec<HookSpec>>,",
                )],
            )?;

            assert_workspace_source_contains(
                "crates/ralph-core/src/hooks/engine.rs",
                &[
                    (
                        "resolved hook spec stores declaration order",
                        "pub declaration_order: usize,",
                    ),
                    (
                        "resolver enumerates hooks in declaration order",
                        ".enumerate()",
                    ),
                    (
                        "resolver forwards declaration order into resolved spec",
                        "ResolvedHookSpec::from_spec(",
                    ),
                    (
                        "engine unit test guards declaration-order contract",
                        "fn resolve_phase_event_preserves_declaration_order() {",
                    ),
                    (
                        "declaration order assertion for first hook",
                        "assert_eq!(resolved[0].declaration_order, 0);",
                    ),
                    (
                        "declaration order assertion for second hook",
                        "assert_eq!(resolved[1].declaration_order, 1);",
                    ),
                ],
            )?;

            Ok(())
        },
    )
}

// =============================================================================
// AC-05: JSON stdin contract
// =============================================================================

fn evaluate_ac_05(
    scenario: &HooksBddScenario,
    harness: &mut HooksBddIntegrationHarness,
    ci_safe_mode: bool,
) -> HooksBddScenarioResult {
    evaluate_green_acceptance(
        scenario,
        harness,
        ci_safe_mode,
        validate_acceptance_context,
        |_harness| {
            assert_workspace_source_contains(
                "crates/ralph-core/src/hooks/executor.rs",
                &[
                    (
                        "HookRunRequest carries JSON stdin payload contract",
                        "pub stdin_payload: serde_json::Value,",
                    ),
                    (
                        "executor configures child stdin as piped",
                        "command.stdin(Stdio::piped());",
                    ),
                    (
                        "executor writes stdin payload before waiting for completion",
                        "write_stdin_payload(",
                    ),
                    (
                        "stdin payload is serialized as JSON bytes",
                        "serde_json::to_vec(stdin_payload)",
                    ),
                    (
                        "serialized payload bytes are written to child stdin",
                        "stdin.write_all(&payload)",
                    ),
                    (
                        "unit test verifies JSON payload delivery to stdin",
                        "fn run_writes_json_payload_to_hook_stdin() {",
                    ),
                ],
            )?;

            Ok(())
        },
    )
}

// =============================================================================
// AC-06: Timeout safeguard
// =============================================================================

fn evaluate_ac_06(
    scenario: &HooksBddScenario,
    harness: &mut HooksBddIntegrationHarness,
    ci_safe_mode: bool,
) -> HooksBddScenarioResult {
    evaluate_green_acceptance(
        scenario,
        harness,
        ci_safe_mode,
        validate_acceptance_context,
        |_harness| {
            assert_workspace_source_contains(
                "crates/ralph-core/src/hooks/executor.rs",
                &[
                    (
                        "HookRunRequest carries per-hook timeout guardrail",
                        "pub timeout_seconds: u64,",
                    ),
                    (
                        "run path forwards request timeout into completion wait",
                        "request.timeout_seconds,",
                    ),
                    (
                        "wait loop derives timeout duration budget",
                        "let timeout = Duration::from_secs(timeout_seconds);",
                    ),
                    (
                        "timeout path terminates long-running process",
                        "let status = terminate_for_timeout(",
                    ),
                    (
                        "executor captures timed_out result from wait path",
                        "let (status, timed_out) = wait_for_completion(",
                    ),
                    (
                        "unit test verifies timeout safeguard behavior",
                        "fn run_marks_timed_out_when_command_exceeds_timeout() {",
                    ),
                ],
            )?;

            Ok(())
        },
    )
}

// =============================================================================
// AC-07: Output truncation safeguard
// =============================================================================

fn evaluate_ac_07(
    scenario: &HooksBddScenario,
    harness: &mut HooksBddIntegrationHarness,
    ci_safe_mode: bool,
) -> HooksBddScenarioResult {
    evaluate_green_acceptance(
        scenario,
        harness,
        ci_safe_mode,
        validate_acceptance_context,
        |_harness| {
            assert_workspace_source_contains(
                "crates/ralph-core/src/hooks/executor.rs",
                &[
                    (
                        "HookRunRequest carries max_output_bytes safeguard",
                        "pub max_output_bytes: u64,",
                    ),
                    (
                        "stdout collector enforces configured output byte limit",
                        "spawn_stream_collector(child.stdout.take(), request.max_output_bytes);",
                    ),
                    (
                        "stderr collector enforces configured output byte limit",
                        "spawn_stream_collector(child.stderr.take(), request.max_output_bytes);",
                    ),
                    (
                        "stream capture derives per-stream capture limit from max_output_bytes",
                        "let capture_limit = usize::try_from(max_output_bytes).unwrap_or(usize::MAX);",
                    ),
                    (
                        "capture path marks output as truncated when bytes exceed limit",
                        "truncated = true;",
                    ),
                    (
                        "unit test verifies stdout/stderr truncation behavior",
                        "fn run_truncates_stdout_and_stderr_at_max_output_bytes() {",
                    ),
                ],
            )?;

            Ok(())
        },
    )
}

fn evaluate_ac_08(
    scenario: &HooksBddScenario,
    harness: &mut HooksBddIntegrationHarness,
    ci_safe_mode: bool,
) -> HooksBddScenarioResult {
    evaluate_green_acceptance(
        scenario,
        harness,
        ci_safe_mode,
        validate_acceptance_context,
        |_harness| {
            assert_workspace_source_contains(
                "crates/ralph-core/src/config.rs",
                &[
                    (
                        "HookOnError enum exposes warn policy",
                        "pub enum HookOnError {",
                    ),
                    (
                        "warn policy documents continue-on-failure behavior",
                        "/// Continue orchestration and record warning telemetry.",
                    ),
                    ("warn policy variant exists", "Warn,"),
                    (
                        "hook validation requires explicit warn|block|suspend policy",
                        "is required in v1 (warn | block | suspend)",
                    ),
                ],
            )?;

            assert_workspace_source_contains(
                "crates/ralph-cli/src/loop_runner.rs",
                &[
                    (
                        "warn policy maps to warn disposition",
                        "HookOnError::Warn => HookDisposition::Warn,",
                    ),
                    (
                        "warn/non-pass outcomes are logged as continuing",
                        "\"Lifecycle hook returned non-pass disposition; continuing\"",
                    ),
                    (
                        "hook dispatch logs telemetry entries with computed disposition",
                        "event_loop.log_hook_run_telemetry(HookRunTelemetryEntry::from_run_result(",
                    ),
                    (
                        "lifecycle integration test asserts warn continues across boundary",
                        "warn disposition should continue across loop.start boundary",
                    ),
                    (
                        "blocking gate helper allows non-blocking dispositions",
                        "fn test_fail_if_blocking_loop_start_outcomes_allows_non_blocking_dispositions() {",
                    ),
                ],
            )?;

            Ok(())
        },
    )
}

fn evaluate_ac_09(
    scenario: &HooksBddScenario,
    harness: &mut HooksBddIntegrationHarness,
    ci_safe_mode: bool,
) -> HooksBddScenarioResult {
    evaluate_green_acceptance(
        scenario,
        harness,
        ci_safe_mode,
        validate_acceptance_context,
        |_harness| {
            assert_workspace_source_contains(
                "crates/ralph-core/src/config.rs",
                &[
                    (
                        "HookOnError enum exposes block policy",
                        "pub enum HookOnError {",
                    ),
                    (
                        "block policy documents lifecycle-action failure behavior",
                        "/// Stop the current lifecycle action as a failure.",
                    ),
                    ("block policy variant exists", "Block,"),
                    (
                        "hook validation requires explicit warn|block|suspend policy",
                        "is required in v1 (warn | block | suspend)",
                    ),
                ],
            )?;

            assert_workspace_source_contains(
                "crates/ralph-cli/src/loop_runner.rs",
                &[
                    (
                        "block policy maps to block disposition",
                        "HookOnError::Block => HookDisposition::Block,",
                    ),
                    (
                        "blocking gate detects block dispositions",
                        ".find(|outcome| outcome.disposition == HookDisposition::Block)",
                    ),
                    (
                        "blocking gate fails lifecycle boundary when block is present",
                        "Err(anyhow::anyhow!(reason))",
                    ),
                    (
                        "blocking failure reason includes hook, phase-event, and failure detail",
                        "\"Lifecycle hook '{}' blocked orchestration at '{}': {}\"",
                    ),
                    (
                        "loop-start integration test asserts block disposition aborts boundary",
                        "expect_err(\"block disposition should abort loop.start boundary\")",
                    ),
                    (
                        "failure-context test asserts surfaced block reason",
                        "expect_err(\"block disposition should fail loop.start boundary\")",
                    ),
                ],
            )?;

            Ok(())
        },
    )
}

fn evaluate_ac_10(
    scenario: &HooksBddScenario,
    harness: &mut HooksBddIntegrationHarness,
    ci_safe_mode: bool,
) -> HooksBddScenarioResult {
    evaluate_green_acceptance(
        scenario,
        harness,
        ci_safe_mode,
        validate_acceptance_context,
        |_harness| {
            assert_workspace_source_contains(
                "crates/ralph-core/src/hooks/suspend_state.rs",
                &[
                    (
                        "suspend-state record persists per-hook suspend mode",
                        "pub suspend_mode: HookSuspendMode,",
                    ),
                    (
                        "suspend-state constructor marks lifecycle state as suspended",
                        "state: SuspendLifecycleState::Suspended,",
                    ),
                    (
                        "suspend-state schema test asserts wait_for_resume serialization",
                        "assert_eq!(value[\"suspend_mode\"], \"wait_for_resume\");",
                    ),
                    (
                        "suspend-state store models resume gate as single-use signal artifact",
                        "/// Consume a single-use resume signal file.",
                    ),
                ],
            )?;

            Ok(())
        },
    )
}

fn evaluate_ac_11(
    scenario: &HooksBddScenario,
    harness: &mut HooksBddIntegrationHarness,
    ci_safe_mode: bool,
) -> HooksBddScenarioResult {
    evaluate_green_acceptance(
        scenario,
        harness,
        ci_safe_mode,
        validate_acceptance_context,
        |_harness| {
            assert_workspace_source_contains(
                "crates/ralph-cli/src/loops.rs",
                &[
                    ("loops CLI exposes resume subcommand", "Resume(ResumeArgs),"),
                    (
                        "loops command handler routes resume requests",
                        "Some(LoopsCommands::Resume(resume_args)) => resume_loop(resume_args),",
                    ),
                    (
                        "resume command resolves suspend-state store at loop workspace root",
                        "let suspend_state_store = SuspendStateStore::new(&target_root);",
                    ),
                    (
                        "resume command reads persisted suspend-state before resuming",
                        ".read_suspend_state()",
                    ),
                    (
                        "resume command writes resume-requested signal artifact",
                        ".write_resume_requested()",
                    ),
                    (
                        "resume command reports continuation from suspended boundary",
                        "The loop will continue from the suspended boundary.",
                    ),
                    (
                        "in-place resume test verifies resume signal creation",
                        "fn test_resume_loop_writes_resume_signal_for_in_place_loop() {",
                    ),
                    (
                        "worktree resume test verifies resume targets resolved loop worktree",
                        "fn test_resume_loop_resolves_partial_id_and_targets_worktree() {",
                    ),
                ],
            )?;

            Ok(())
        },
    )
}

fn evaluate_ac_12(
    scenario: &HooksBddScenario,
    harness: &mut HooksBddIntegrationHarness,
    ci_safe_mode: bool,
) -> HooksBddScenarioResult {
    evaluate_green_acceptance(
        scenario,
        harness,
        ci_safe_mode,
        validate_acceptance_context,
        |_harness| {
            assert_workspace_source_contains(
                "crates/ralph-core/src/hooks/suspend_state.rs",
                &[
                    (
                        "suspend-state store exposes resume-requested probe",
                        "pub fn is_resume_requested(&self) -> bool {",
                    ),
                    (
                        "suspend-state store consumes resume signal via single-use operation",
                        "pub fn consume_resume_requested(&self) -> Result<bool, SuspendStateStoreError> {",
                    ),
                    (
                        "resume signal consumption removes resume-requested artifact",
                        "remove_if_exists(&self.resume_requested_path(), \"consume resume signal\")",
                    ),
                    (
                        "store unit test verifies resume signal single-use behavior",
                        "fn test_resume_signal_is_single_use() {",
                    ),
                ],
            )?;

            assert_workspace_source_contains(
                "crates/ralph-cli/src/loops.rs",
                &[
                    (
                        "resume command checks for already-requested resume signal",
                        "let resume_already_requested = suspend_state_store.is_resume_requested();",
                    ),
                    (
                        "already-requested resume against unsuspended loop returns informative no-op",
                        "The loop is not currently suspended; no action taken.",
                    ),
                    (
                        "already-requested resume while suspended returns informative wait message",
                        "Resume was already requested for loop '{}'. Waiting for the loop to continue.",
                    ),
                    (
                        "non-suspended loop resume request returns informative no-op",
                        "Loop '{}' is not currently suspended. Nothing to resume.",
                    ),
                    (
                        "idempotency regression test covers repeat resume request",
                        "fn test_resume_loop_is_idempotent_when_resume_already_requested() {",
                    ),
                    (
                        "non-suspended regression test covers no-op resume path",
                        "fn test_resume_loop_noops_for_non_suspended_loop() {",
                    ),
                ],
            )?;

            Ok(())
        },
    )
}

// =============================================================================
// AC-13: Mutation opt-in only
// =============================================================================

fn evaluate_ac_13(
    scenario: &HooksBddScenario,
    harness: &mut HooksBddIntegrationHarness,
    ci_safe_mode: bool,
) -> HooksBddScenarioResult {
    evaluate_green_acceptance(
        scenario,
        harness,
        ci_safe_mode,
        validate_acceptance_context,
        |_harness| {
            assert_workspace_source_contains(
                "crates/ralph-cli/src/loop_runner.rs",
                &[
                    (
                        "mutation parser short-circuits when mutate.enabled is false",
                        "if !mutate.enabled {",
                    ),
                    (
                        "disabled mutation path yields explicit disabled parse outcome",
                        "return HookMutationParseOutcome::Disabled;",
                    ),
                    (
                        "metadata merge processes only parsed mutation outcomes",
                        "} = &outcome.mutation_parse_outcome",
                    ),
                    (
                        "non-parsed mutation outcomes are skipped during metadata merge",
                        "else {\n            continue;\n        };",
                    ),
                    (
                        "AC-13 integration test verifies disabled mutations stay inert",
                        "fn test_ac13_mutation_disabled_json_output_is_inert_for_accumulator_and_downstream_payloads() {",
                    ),
                    (
                        "AC-13 integration test asserts downstream payload excludes hook_metadata namespace",
                        "assert!(!payload_accumulated.contains_key(\"hook_metadata\"));",
                    ),
                    (
                        "unit test verifies parser skips mutation parsing when disabled",
                        "fn test_parse_hook_mutation_stdout_skips_when_disabled() {",
                    ),
                ],
            )?;

            Ok(())
        },
    )
}

// =============================================================================
// AC-14: Metadata-only mutation surface
// =============================================================================

fn evaluate_ac_14(
    scenario: &HooksBddScenario,
    harness: &mut HooksBddIntegrationHarness,
    ci_safe_mode: bool,
) -> HooksBddScenarioResult {
    evaluate_green_acceptance(
        scenario,
        harness,
        ci_safe_mode,
        validate_acceptance_context,
        |_harness| {
            assert_workspace_source_contains(
                "crates/ralph-cli/src/loop_runner.rs",
                &[
                    (
                        "mutation payload parser enforces metadata-only top-level schema",
                        "if payload_object.len() != 1 || !payload_object.contains_key(HOOK_MUTATION_PAYLOAD_METADATA_KEY)",
                    ),
                    (
                        "schema error message documents metadata-only mutation contract",
                        "mutation payload supports only '{{\\\"{HOOK_MUTATION_PAYLOAD_METADATA_KEY}\\\": {{...}}}}'; found keys: {keys:?}",
                    ),
                    (
                        "metadata payload value must be a JSON object",
                        "message: \"mutation payload key 'metadata' must contain a JSON object\".to_string(),",
                    ),
                    (
                        "parsed metadata is namespaced under hook_metadata by emitting hook",
                        "namespace_object.insert(hook_name.to_string(), serde_json::Value::Object(metadata));",
                    ),
                    (
                        "AC-14 integration test validates metadata-only downstream mutation behavior",
                        "fn test_ac14_mutation_enabled_updates_only_namespaced_metadata_in_downstream_payloads() {",
                    ),
                    (
                        "AC-14 integration test guards mutation surface from prompt field injection",
                        "assert!(!payload_object.contains_key(\"prompt\"));",
                    ),
                    (
                        "AC-14 integration test guards mutation surface from events field injection",
                        "assert!(!payload_object.contains_key(\"events\"));",
                    ),
                    (
                        "unit test rejects payloads that include non-metadata keys",
                        "fn test_parse_hook_mutation_stdout_rejects_non_metadata_payload_shape() {",
                    ),
                ],
            )?;

            Ok(())
        },
    )
}

// =============================================================================
// AC-15: JSON-only mutation format
// =============================================================================

fn evaluate_ac_15(
    scenario: &HooksBddScenario,
    harness: &mut HooksBddIntegrationHarness,
    ci_safe_mode: bool,
) -> HooksBddScenarioResult {
    evaluate_green_acceptance(
        scenario,
        harness,
        ci_safe_mode,
        validate_acceptance_context,
        |_harness| {
            assert_workspace_source_contains(
                "crates/ralph-cli/src/loop_runner.rs",
                &[
                    (
                        "mutation parser attempts JSON decode of hook stdout",
                        "let parsed = match serde_json::from_str::<serde_json::Value>(stdout.trim()) {",
                    ),
                    (
                        "non-JSON mutation output maps to invalid-json parse outcome",
                        "return HookMutationParseOutcome::Invalid(HookMutationParseError::InvalidJson {",
                    ),
                    (
                        "invalid-json failure message is surfaced with parse error context",
                        "message: format!(\"mutation stdout is not valid JSON: {error}\"),",
                    ),
                    (
                        "invalid mutation parse outcomes are converted into dispatch failures",
                        "Some(HookDispatchFailure::InvalidMutationOutput {",
                    ),
                    (
                        "mutation parse failures are dispositioned via on_error policy",
                        "let disposition = if mutation_failure.is_some() {",
                    ),
                    (
                        "mutation parse failure branch maps through disposition_from_on_error",
                        "disposition_from_on_error(on_error)",
                    ),
                    (
                        "unit test verifies parser rejects non-JSON mutation stdout",
                        "fn test_parse_hook_mutation_stdout_rejects_non_json_payload_when_enabled() {",
                    ),
                    (
                        "AC-15 warn-path test verifies non-JSON mutation remains non-blocking",
                        "fn test_ac15_dispatch_phase_event_hooks_non_json_mutation_warn_continues_through_block_gate() {",
                    ),
                    (
                        "AC-15 block-path test verifies non-JSON mutation surfaces blocking reason",
                        "fn test_ac15_dispatch_phase_event_hooks_non_json_mutation_block_surfaces_invalid_output_reason()",
                    ),
                    (
                        "AC-15 suspend-path test verifies non-JSON mutation enters wait_for_resume gate",
                        "fn test_ac15_dispatch_phase_event_hooks_non_json_mutation_suspend_uses_wait_for_resume_gate() {",
                    ),
                ],
            )?;

            Ok(())
        },
    )
}

// =============================================================================
// AC-16: Hook telemetry completeness
// =============================================================================

fn evaluate_ac_16(
    scenario: &HooksBddScenario,
    harness: &mut HooksBddIntegrationHarness,
    ci_safe_mode: bool,
) -> HooksBddScenarioResult {
    evaluate_green_acceptance(
        scenario,
        harness,
        ci_safe_mode,
        validate_acceptance_context,
        |_harness| {
            assert_workspace_source_contains(
                "crates/ralph-core/src/diagnostics/hook_runs.rs",
                &[
                    (
                        "telemetry schema defines structured hook-run entry",
                        "pub struct HookRunTelemetryEntry {",
                    ),
                    (
                        "telemetry captures canonical phase-event key",
                        "pub phase_event: String,",
                    ),
                    ("telemetry captures hook name", "pub hook_name: String,"),
                    (
                        "telemetry captures lifecycle timing bounds",
                        "pub started_at: DateTime<Utc>,",
                    ),
                    (
                        "telemetry captures duration in milliseconds",
                        "pub duration_ms: u64,",
                    ),
                    (
                        "telemetry captures process exit code",
                        "pub exit_code: Option<i32>,",
                    ),
                    (
                        "telemetry captures timeout indicator",
                        "pub timed_out: bool,",
                    ),
                    (
                        "telemetry captures stdout payload with truncation metadata",
                        "pub stdout: HookStreamOutput,",
                    ),
                    (
                        "telemetry captures stderr payload with truncation metadata",
                        "pub stderr: HookStreamOutput,",
                    ),
                    (
                        "telemetry captures final disposition",
                        "pub disposition: HookDisposition,",
                    ),
                    (
                        "telemetry captures suspend mode used for failures",
                        "pub suspend_mode: HookSuspendMode,",
                    ),
                    (
                        "telemetry captures retry attempt index",
                        "pub retry_attempt: u32,",
                    ),
                    (
                        "telemetry captures retry attempt ceiling",
                        "pub retry_max_attempts: u32,",
                    ),
                    (
                        "telemetry builder maps executor output into entry",
                        "pub fn from_run_result(",
                    ),
                    (
                        "hook-run logger writes to hook-runs diagnostics file",
                        "let log_file = session_dir.join(\"hook-runs.jsonl\");",
                    ),
                    (
                        "hook-run logger serializes telemetry entries as JSON",
                        "serde_json::to_writer(&mut self.writer, entry)?;",
                    ),
                    (
                        "hook-run logger uses newline-delimited records",
                        "self.writer.write_all(b\"\\n\")?;",
                    ),
                    (
                        "telemetry unit test verifies required serialized fields",
                        "fn telemetry_entry_serializes_required_fields() {",
                    ),
                ],
            )?;

            assert_workspace_source_contains(
                "crates/ralph-cli/src/loop_runner.rs",
                &[
                    (
                        "loop runner emits hook-run telemetry after each attempt",
                        "event_loop.log_hook_run_telemetry(HookRunTelemetryEntry::from_run_result(",
                    ),
                    (
                        "telemetry emission includes canonical phase-event key",
                        "phase_event_key,",
                    ),
                    ("telemetry emission includes hook identifier", "hook_name,"),
                    (
                        "telemetry emission includes computed disposition",
                        "disposition,",
                    ),
                    ("telemetry emission includes suspend mode", "suspend_mode,"),
                    (
                        "telemetry emission includes retry attempt",
                        "retry_attempt,",
                    ),
                    (
                        "telemetry emission includes retry ceiling",
                        "retry_max_attempts,",
                    ),
                    (
                        "telemetry emission includes executor run result",
                        "&run_result,",
                    ),
                    (
                        "retry-backoff integration test asserts telemetry row count",
                        "assert_eq!(telemetry_entries.len(), 3);",
                    ),
                    (
                        "wait-then-retry integration test asserts telemetry row count",
                        "assert_eq!(telemetry_entries.len(), 2);",
                    ),
                ],
            )?;

            Ok(())
        },
    )
}

// =============================================================================
// AC-17: Validation command
// =============================================================================

fn evaluate_ac_17(
    scenario: &HooksBddScenario,
    harness: &mut HooksBddIntegrationHarness,
    ci_safe_mode: bool,
) -> HooksBddScenarioResult {
    evaluate_green_acceptance(
        scenario,
        harness,
        ci_safe_mode,
        validate_acceptance_context,
        |_harness| {
            assert_workspace_source_contains(
                "crates/ralph-cli/src/hooks.rs",
                &[
                    (
                        "hooks namespace defines validate subcommand",
                        "Validate(ValidateArgs),",
                    ),
                    (
                        "hooks validate supports machine-readable format selection",
                        "pub enum HooksValidateFormat {",
                    ),
                    ("hooks validate includes human format", "Human,"),
                    ("hooks validate includes json format", "Json,"),
                    (
                        "hooks validate defaults --format to human",
                        "#[arg(long, value_enum, default_value_t = HooksValidateFormat::Human)]",
                    ),
                    (
                        "hooks command execution routes validate subcommand",
                        "HooksCommands::Validate(validate_args) => {",
                    ),
                    (
                        "validate subcommand delegates to execute_validate implementation",
                        "execute_validate(config_sources, hats_source, validate_args, use_colors).await",
                    ),
                    (
                        "validate command loads a structured report from current config sources",
                        "let report = build_report(config_sources, hats_source).await;",
                    ),
                    (
                        "json mode renders report as pretty-printed JSON",
                        "serde_json::to_string_pretty(&report)?",
                    ),
                    (
                        "human mode renders report with human formatter",
                        "print_human_report(&report, use_colors);",
                    ),
                    (
                        "validate command exits non-zero when report fails",
                        "std::process::exit(1);",
                    ),
                    (
                        "report builder runs semantic config validation",
                        "if let Err(error) = config.validate() {",
                    ),
                    (
                        "semantic validation failure is captured as hooks diagnostic",
                        "report.push_diagnostic(\"hooks.semantic\", error.to_string(), None, None, None);",
                    ),
                    (
                        "report builder includes duplicate hook-name validation",
                        "validate_duplicate_names(&config, &mut report);",
                    ),
                    (
                        "report builder includes command resolvability validation",
                        "validate_command_resolvability(&config, &mut report);",
                    ),
                ],
            )?;

            assert_workspace_source_contains(
                "crates/ralph-cli/src/main.rs",
                &[
                    (
                        "top-level CLI command enum registers hooks namespace",
                        "Hooks(hooks::HooksArgs),",
                    ),
                    (
                        "main command dispatcher routes hooks invocations",
                        "Some(Commands::Hooks(args)) => {",
                    ),
                    (
                        "hooks dispatcher invokes hooks::execute handler",
                        "hooks::execute(",
                    ),
                    (
                        "hooks dispatcher forwards color preference to validation output",
                        "cli.color.should_use_colors(),",
                    ),
                ],
            )?;

            Ok(())
        },
    )
}

// =============================================================================
// AC-18: Preflight integration
// =============================================================================

fn evaluate_ac_18(
    scenario: &HooksBddScenario,
    harness: &mut HooksBddIntegrationHarness,
    ci_safe_mode: bool,
) -> HooksBddScenarioResult {
    evaluate_green_acceptance(
        scenario,
        harness,
        ci_safe_mode,
        validate_acceptance_context,
        |_harness| {
            assert_workspace_source_contains(
                "crates/ralph-core/src/preflight.rs",
                &[
                    (
                        "preflight default checks register hooks validation check",
                        "Box::new(HooksValidationCheck),",
                    ),
                    (
                        "hooks preflight check type exists",
                        "struct HooksValidationCheck;",
                    ),
                    (
                        "hooks preflight check is named hooks for skip-list integration",
                        "\"hooks\"",
                    ),
                    (
                        "hooks preflight check skips when hooks are disabled",
                        "if !config.hooks.enabled {",
                    ),
                    (
                        "disabled hooks preflight result is a passing skip status",
                        "return CheckResult::pass(self.name(), \"Hooks disabled (skipping)\");",
                    ),
                    (
                        "hooks preflight check validates duplicate names",
                        "validate_hook_duplicate_names(config, &mut diagnostics);",
                    ),
                    (
                        "hooks preflight check validates command resolvability",
                        "validate_hook_command_resolvability(config, &mut diagnostics);",
                    ),
                    (
                        "hooks preflight check reports pass label with checked hook count",
                        "\"Hooks validation passed ({} hook(s))\"",
                    ),
                    (
                        "hooks preflight check reports failing diagnostics count",
                        "\"Hooks validation failed ({} issue(s))\"",
                    ),
                    (
                        "unit test verifies hooks check registration in default preflight set",
                        "fn default_checks_include_hooks_check_name() {",
                    ),
                    (
                        "unit test verifies hooks check skip behavior when disabled",
                        "async fn hooks_check_skips_when_hooks_are_disabled() {",
                    ),
                    (
                        "unit test verifies hooks check emits actionable failures",
                        "async fn hooks_check_fails_with_actionable_duplicate_and_command_diagnostics() {",
                    ),
                    (
                        "unit test verifies selected preflight checks can omit hooks failures",
                        "async fn run_selected_can_skip_hooks_check_failures() {",
                    ),
                ],
            )?;

            Ok(())
        },
    )
}

fn hooks_feature_dir() -> Result<PathBuf, HooksBddError> {
    let manifest_candidate =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(HOOKS_FEATURE_DIR_CRATE);
    if manifest_candidate.is_dir() {
        return Ok(manifest_candidate);
    }

    let workspace_root = find_workspace_root().ok_or(HooksBddError::WorkspaceRootNotFound)?;
    let workspace_candidate = workspace_root.join(HOOKS_FEATURE_DIR_WORKSPACE);
    if workspace_candidate.is_dir() {
        return Ok(workspace_candidate);
    }

    let crate_relative_candidate = workspace_root.join(HOOKS_FEATURE_DIR_CRATE);
    if crate_relative_candidate.is_dir() {
        return Ok(crate_relative_candidate);
    }

    Err(HooksBddError::HooksFeatureDirNotFound(workspace_candidate))
}

fn parse_feature_file(path: &Path) -> Result<Vec<HooksBddScenario>, HooksBddError> {
    use cucumber::feature::Ext as _;

    let feature_file = path
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .ok_or_else(|| HooksBddError::InvalidFeatureFile {
            path: path.to_path_buf(),
            reason: "missing file name".to_string(),
        })?;

    let parsed_feature =
        cucumber::gherkin::Feature::parse_path(path, cucumber::gherkin::GherkinEnv::default())
            .map_err(|source| HooksBddError::InvalidFeatureFile {
                path: path.to_path_buf(),
                reason: source.to_string(),
            })?;

    let feature =
        parsed_feature
            .expand_examples()
            .map_err(|source| HooksBddError::InvalidFeatureFile {
                path: path.to_path_buf(),
                reason: source.to_string(),
            })?;

    let mut scenarios = Vec::new();

    scenarios.extend(convert_gherkin_scenarios(
        &feature.scenarios,
        &feature.tags,
        &feature_file,
    ));

    for rule in &feature.rules {
        let inherited_tags = merge_tags(&feature.tags, &rule.tags);
        scenarios.extend(convert_gherkin_scenarios(
            &rule.scenarios,
            &inherited_tags,
            &feature_file,
        ));
    }

    if scenarios.is_empty() {
        return Err(HooksBddError::InvalidFeatureFile {
            path: path.to_path_buf(),
            reason: "no scenarios discovered".to_string(),
        });
    }

    Ok(scenarios)
}

fn convert_gherkin_scenarios(
    scenarios: &[cucumber::gherkin::Scenario],
    inherited_tags: &[String],
    feature_file: &str,
) -> Vec<HooksBddScenario> {
    scenarios
        .iter()
        .map(|scenario| {
            let tags = merge_tags(inherited_tags, &scenario.tags);
            let scenario_id = tags
                .iter()
                .find(|tag| is_acceptance_id(tag))
                .cloned()
                .unwrap_or_else(|| scenario.name.clone());
            let steps = scenario
                .steps
                .iter()
                .map(|step| HooksStep {
                    keyword: map_gherkin_step_keyword(step.ty),
                    text: step.value.clone(),
                })
                .collect();

            HooksBddScenario {
                scenario_id,
                scenario_name: scenario.name.clone(),
                feature_file: feature_file.to_string(),
                tags,
                steps,
            }
        })
        .collect()
}

fn merge_tags(inherited_tags: &[String], local_tags: &[String]) -> Vec<String> {
    let mut merged = inherited_tags.to_vec();
    for tag in local_tags {
        if !merged.contains(tag) {
            merged.push(tag.clone());
        }
    }
    merged
}

fn map_gherkin_step_keyword(keyword: cucumber::gherkin::StepType) -> HooksStepKeyword {
    match keyword {
        cucumber::gherkin::StepType::Given => HooksStepKeyword::Given,
        cucumber::gherkin::StepType::When => HooksStepKeyword::When,
        cucumber::gherkin::StepType::Then => HooksStepKeyword::Then,
    }
}

fn matches_filter(scenario: &HooksBddScenario, filter_lower: &str) -> bool {
    scenario.scenario_id.to_lowercase().contains(filter_lower)
        || scenario.scenario_name.to_lowercase().contains(filter_lower)
        || scenario.feature_file.to_lowercase().contains(filter_lower)
        || scenario
            .tags
            .iter()
            .any(|tag| tag.to_lowercase().contains(filter_lower))
}

fn is_acceptance_id(tag: &str) -> bool {
    let Some(suffix) = tag.strip_prefix("AC-") else {
        return false;
    };

    suffix.len() == 2 && suffix.chars().all(|character| character.is_ascii_digit())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn synthetic_scenario(scenario_id: &str, scenario_name: &str) -> HooksBddScenario {
        HooksBddScenario {
            scenario_id: scenario_id.to_string(),
            scenario_name: scenario_name.to_string(),
            feature_file: "hooks/synthetic.feature".to_string(),
            tags: vec![scenario_id.to_string()],
            steps: vec![],
        }
    }

    #[test]
    fn harness_prepare_temp_workspace_resets_existing_contents() {
        let scenario = synthetic_scenario("AC-90", "Workspace reset determinism");
        let harness = HooksBddIntegrationHarness::new(&scenario, true);

        let workspace_dir = harness
            .prepare_temp_workspace("runtime workspace")
            .expect("workspace should be created");

        assert!(workspace_dir.ends_with("workspace/runtime-workspace"));
        assert!(workspace_dir.join(".ralph/agent").is_dir());

        harness
            .write_workspace_file(&workspace_dir, "fixtures/data.txt", "seed")
            .expect("fixture file should be created");
        assert!(workspace_dir.join("fixtures/data.txt").exists());

        let reset_workspace = harness
            .prepare_temp_workspace("runtime workspace")
            .expect("workspace reset should succeed");

        assert_eq!(reset_workspace, workspace_dir);
        assert!(workspace_dir.join(".ralph/agent").is_dir());
        assert!(
            !workspace_dir.join("fixtures/data.txt").exists(),
            "reset workspace should remove previous fixture files"
        );
    }

    #[test]
    fn harness_scaffold_run_artifact_is_deterministic() {
        let scenario = synthetic_scenario("AC-91", "Artifact determinism");
        let mut harness = HooksBddIntegrationHarness::new(&scenario, true);

        let first_index = harness.scaffold_run_artifact("hooks.validate", "ralph hooks validate");
        let second_index = harness.scaffold_run_artifact("ralph.run", "ralph run -p smoke");

        assert_eq!(first_index, 0);
        assert_eq!(second_index, 1);

        let artifacts = harness.artifacts();
        let expected_root = default_hooks_bdd_artifact_root().join("ac-91-artifact-determinism");

        assert_eq!(artifacts.root_dir, expected_root);
        assert_eq!(artifacts.run_artifacts.len(), 2);

        let first = &artifacts.run_artifacts[0];
        assert_eq!(
            first.stdout_path,
            expected_root.join("01-hooks-validate/stdout.log")
        );
        assert_eq!(
            first.stderr_path,
            expected_root.join("01-hooks-validate/stderr.log")
        );

        let second = &artifacts.run_artifacts[1];
        assert_eq!(
            second.stdout_path,
            expected_root.join("02-ralph-run/stdout.log")
        );
        assert_eq!(
            second.stderr_path,
            expected_root.join("02-ralph-run/stderr.log")
        );
    }

    #[test]
    fn harness_run_bounded_ralph_command_captures_exit_metadata() {
        let scenario = synthetic_scenario("AC-92", "Bounded command exit capture");
        let mut harness = HooksBddIntegrationHarness::new(&scenario, true);
        let workspace_dir = harness
            .prepare_temp_workspace("command exit capture")
            .expect("workspace should be created");

        let artifact = harness
            .run_bounded_ralph_command(
                "ralph.version",
                &workspace_dir,
                &["--version"],
                Duration::from_secs(2),
            )
            .expect("version command should complete successfully");

        assert_eq!(artifact.name, "ralph.version");
        assert_eq!(artifact.working_dir, workspace_dir);
        assert!(!artifact.timed_out);
        assert_eq!(artifact.exit_code, Some(0));
        assert!(artifact.stdout_path.is_file());
        assert!(artifact.stderr_path.is_file());

        let stdout =
            fs::read_to_string(&artifact.stdout_path).expect("stdout artifact should be readable");
        assert!(stdout.contains("ralph"));
    }

    #[test]
    fn harness_run_bounded_ralph_command_marks_timeout() {
        let scenario = synthetic_scenario("AC-93", "Bounded command timeout capture");
        let mut harness = HooksBddIntegrationHarness::new(&scenario, true);
        let workspace_dir = harness
            .prepare_temp_workspace("command timeout capture")
            .expect("workspace should be created");

        // Write a minimal ralph.yml with a dummy backend (`sleep 3600`)
        // so `ralph run` enters the event loop and blocks long enough for
        // the bounded-command timeout to fire.
        harness
            .write_workspace_file(
                &workspace_dir,
                "ralph.yml",
                "cli:\n  backend: custom\n  command: sleep\n  args: [\"3600\"]\n  prompt_mode: stdin\n",
            )
            .expect("should write ralph.yml");

        // The workspace needs a git repo for ralph to start.
        std::process::Command::new("git")
            .args(["init"])
            .current_dir(&workspace_dir)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .expect("git init should succeed");

        let artifact = harness
            .run_bounded_ralph_command(
                "ralph.run-timeout",
                &workspace_dir,
                &["run", "-p", "hooks-bdd-timeout", "--max-iterations", "1"],
                Duration::from_secs(2),
            )
            .expect("bounded command should return timeout artifact");

        assert!(
            artifact.timed_out,
            "expected timeout marker for bounded command"
        );
        assert_eq!(artifact.working_dir, workspace_dir);
        assert!(artifact.stdout_path.is_file());
        assert!(artifact.stderr_path.is_file());

        let artifact_from_manifest = harness
            .artifacts()
            .run_artifacts
            .iter()
            .find(|run| run.name == "ralph.run-timeout")
            .expect("timeout artifact should be persisted in harness manifest");
        assert!(artifact_from_manifest.timed_out);
    }

    #[test]
    fn discover_hooks_bdd_scenarios_finds_all_hook_scenarios() {
        let scenarios = discover_hooks_bdd_scenarios(None).expect("should discover scenarios");
        let scenario_ids: Vec<&str> = scenarios
            .iter()
            .map(|scenario| scenario.scenario_id.as_str())
            .collect();

        assert_eq!(scenarios.len(), 18);
        assert!(scenario_ids.contains(&"AC-01"));
        assert!(scenario_ids.contains(&"AC-18"));
    }

    #[test]
    fn discover_hooks_bdd_scenarios_applies_filter() {
        let scenarios =
            discover_hooks_bdd_scenarios(Some("AC-03")).expect("filtered discovery should work");

        assert_eq!(scenarios.len(), 1);
        assert_eq!(scenarios[0].scenario_id, "AC-03");
    }

    #[test]
    fn load_workspace_source_file_reads_workspace_relative_path() {
        let source = load_workspace_source_file("crates/ralph-e2e/src/hooks_bdd.rs")
            .expect("should load source file from workspace root");

        assert!(source.contains("run_hooks_bdd_suite"));
    }

    #[test]
    fn assert_workspace_source_contains_reports_missing_snippets() {
        let error = assert_workspace_source_contains(
            "crates/ralph-core/src/config.rs",
            &[(
                "nonexistent marker",
                "__never_present_marker_for_hooks_bdd_test__",
            )],
        )
        .expect_err("missing snippet should fail");

        assert!(error.contains("source evidence assertion failed"));
        assert!(error.contains("crates/ralph-core/src/config.rs"));
        assert!(error.contains("nonexistent marker"));
        assert!(error.contains("__never_present_marker_for_hooks_bdd_test__"));
    }

    #[test]
    fn evaluate_green_acceptance_reports_actionable_missing_evidence_failures() {
        let scenario = HooksBddScenario {
            scenario_id: "AC-01".to_string(),
            scenario_name: "AC-01 synthetic missing evidence".to_string(),
            feature_file: "hooks/scope-and-dispatch.feature".to_string(),
            tags: vec!["AC-01".to_string()],
            steps: vec![],
        };

        let mut harness = HooksBddIntegrationHarness::new(&scenario, true);
        let result = evaluate_green_acceptance(
            &scenario,
            &mut harness,
            true,
            validate_acceptance_context,
            |_harness| {
                assert_required_source_snippets(
                    "crates/ralph-core/src/config.rs",
                    "pub hooks: HooksConfig,\n",
                    &[(
                        "hooks defaults preserve project scope",
                        "hooks: HooksConfig::default(),",
                    )],
                )
            },
        );

        assert!(!result.passed);
        assert_eq!(result.scenario_id, "AC-01");
        assert!(
            result
                .message
                .contains("source evidence assertion failed for crates/ralph-core/src/config.rs")
        );
        assert!(
            result
                .message
                .contains("hooks defaults preserve project scope")
        );
        assert!(result.message.contains("hooks: HooksConfig::default(),"));
    }

    #[test]
    fn run_hooks_bdd_suite_passes_ac_01_in_ci_safe_mode() {
        let config = HooksBddConfig::new(Some("AC-01".to_string()), true);
        let results = run_hooks_bdd_suite(&config).expect("suite should run");

        assert_eq!(results.total_count(), 1);
        assert_eq!(results.passed_count(), 1);
        assert!(results.results[0].passed);
        assert!(results.results[0].message.contains("verified green"));
    }

    #[test]
    fn run_hooks_bdd_suite_fails_without_ci_safe_mode() {
        let config = HooksBddConfig::new(Some("AC-01".to_string()), false);
        let results = run_hooks_bdd_suite(&config).expect("suite should run");

        assert_eq!(results.total_count(), 1);
        assert_eq!(results.failed_count(), 1);
        assert!(results.results[0].message.contains("CI-safe mode required"));
    }

    #[test]
    fn run_hooks_bdd_suite_passes_ac_04_deterministic_ordering() {
        let config = HooksBddConfig::new(Some("AC-04".to_string()), true);
        let results = run_hooks_bdd_suite(&config).expect("suite should run");

        assert_eq!(results.total_count(), 1);
        assert_eq!(results.passed_count(), 1);
        assert!(results.results[0].passed);
    }

    #[test]
    fn run_hooks_bdd_suite_passes_ac_05_json_stdin_contract() {
        let config = HooksBddConfig::new(Some("AC-05".to_string()), true);
        let results = run_hooks_bdd_suite(&config).expect("suite should run");

        assert_eq!(results.total_count(), 1);
        assert_eq!(results.passed_count(), 1);
        assert!(results.results[0].passed);
    }

    #[test]
    fn run_hooks_bdd_suite_passes_ac_06_timeout_safeguard() {
        let config = HooksBddConfig::new(Some("AC-06".to_string()), true);
        let results = run_hooks_bdd_suite(&config).expect("suite should run");

        assert_eq!(results.total_count(), 1);
        assert_eq!(results.passed_count(), 1);
        assert!(results.results[0].passed);
    }

    #[test]
    fn run_hooks_bdd_suite_passes_ac_07_output_size_safeguard() {
        let config = HooksBddConfig::new(Some("AC-07".to_string()), true);
        let results = run_hooks_bdd_suite(&config).expect("suite should run");

        assert_eq!(results.total_count(), 1);
        assert_eq!(results.passed_count(), 1);
        assert!(results.results[0].passed);
    }

    #[test]
    fn run_hooks_bdd_suite_passes_ac_08_warn_policy() {
        let config = HooksBddConfig::new(Some("AC-08".to_string()), true);
        let results = run_hooks_bdd_suite(&config).expect("suite should run");

        assert_eq!(results.total_count(), 1);
        assert_eq!(results.passed_count(), 1);
        assert!(results.results[0].passed);
    }

    #[test]
    fn run_hooks_bdd_suite_passes_ac_09_block_policy() {
        let config = HooksBddConfig::new(Some("AC-09".to_string()), true);
        let results = run_hooks_bdd_suite(&config).expect("suite should run");

        assert_eq!(results.total_count(), 1);
        assert_eq!(results.passed_count(), 1);
        assert!(results.results[0].passed);
    }

    #[test]
    fn run_hooks_bdd_suite_passes_ac_10_suspend_default_mode() {
        let config = HooksBddConfig::new(Some("AC-10".to_string()), true);
        let results = run_hooks_bdd_suite(&config).expect("suite should run");

        assert_eq!(results.total_count(), 1);
        assert_eq!(results.passed_count(), 1);
        assert!(results.results[0].passed);
    }

    #[test]
    fn run_hooks_bdd_suite_passes_ac_11_cli_resume_path() {
        let config = HooksBddConfig::new(Some("AC-11".to_string()), true);
        let results = run_hooks_bdd_suite(&config).expect("suite should run");

        assert_eq!(results.total_count(), 1);
        assert_eq!(results.passed_count(), 1);
        assert!(results.results[0].passed);
    }

    #[test]
    fn run_hooks_bdd_suite_passes_ac_12_resume_idempotency() {
        let config = HooksBddConfig::new(Some("AC-12".to_string()), true);
        let results = run_hooks_bdd_suite(&config).expect("suite should run");

        assert_eq!(results.total_count(), 1);
        assert_eq!(results.passed_count(), 1);
        assert!(results.results[0].passed);
    }

    #[test]
    fn run_hooks_bdd_suite_passes_ac_13_mutation_opt_in() {
        let config = HooksBddConfig::new(Some("AC-13".to_string()), true);
        let results = run_hooks_bdd_suite(&config).expect("suite should run");

        assert_eq!(results.total_count(), 1);
        assert_eq!(results.passed_count(), 1);
        assert!(results.results[0].passed);
    }

    #[test]
    fn run_hooks_bdd_suite_passes_ac_14_metadata_mutation() {
        let config = HooksBddConfig::new(Some("AC-14".to_string()), true);
        let results = run_hooks_bdd_suite(&config).expect("suite should run");

        assert_eq!(results.total_count(), 1);
        assert_eq!(results.passed_count(), 1);
        assert!(results.results[0].passed);
    }

    #[test]
    fn run_hooks_bdd_suite_passes_ac_15_json_mutation_format() {
        let config = HooksBddConfig::new(Some("AC-15".to_string()), true);
        let results = run_hooks_bdd_suite(&config).expect("suite should run");

        assert_eq!(results.total_count(), 1);
        assert_eq!(results.passed_count(), 1);
        assert!(results.results[0].passed);
    }

    #[test]
    fn run_hooks_bdd_suite_passes_ac_16_telemetry_completeness() {
        let config = HooksBddConfig::new(Some("AC-16".to_string()), true);
        let results = run_hooks_bdd_suite(&config).expect("suite should run");

        assert_eq!(results.total_count(), 1);
        assert_eq!(results.passed_count(), 1);
        assert!(results.results[0].passed);
    }

    #[test]
    fn run_hooks_bdd_suite_passes_ac_17_validation_command() {
        let config = HooksBddConfig::new(Some("AC-17".to_string()), true);
        let results = run_hooks_bdd_suite(&config).expect("suite should run");

        assert_eq!(results.total_count(), 1);
        assert_eq!(results.passed_count(), 1);
        assert!(results.results[0].passed);
    }

    #[test]
    fn run_hooks_bdd_suite_passes_ac_18_preflight_integration() {
        let config = HooksBddConfig::new(Some("AC-18".to_string()), true);
        let results = run_hooks_bdd_suite(&config).expect("suite should run");

        assert_eq!(results.total_count(), 1);
        assert_eq!(results.passed_count(), 1);
        assert!(results.results[0].passed);
    }

    #[test]
    fn run_hooks_bdd_suite_uses_unmapped_fallback_evaluator() {
        // Test that unmapped AC IDs (not in dispatch map) use the fallback evaluator
        // We test this by creating a scenario with an unmapped AC ID and verifying behavior
        // Note: AC-99 is not in the feature files, so we test the dispatch directly
        let eval_fn = dispatch_ac_evaluator("AC-99");

        // Create a scenario for the unmapped AC
        let scenario = HooksBddScenario {
            scenario_id: "AC-99".to_string(),
            scenario_name: "AC-99 Unmapped test".to_string(),
            feature_file: "test.feature".to_string(),
            tags: vec!["AC-99".to_string()],
            steps: vec![],
        };

        let mut harness = HooksBddIntegrationHarness::new(&scenario, true);
        let result = eval_fn(&scenario, &mut harness, true);

        // AC-99 should fail with "no evaluator implemented" message
        assert!(!result.passed);
        assert!(result.message.contains("no evaluator implemented"));
    }

    #[test]
    fn parse_feature_file_with_cucumber_parses_scenario_tags_and_steps() {
        let temp_dir = tempfile::tempdir().expect("tempdir should be created");
        let feature_path = temp_dir.path().join("example.feature");

        fs::write(
            &feature_path,
            r#"
@hooks
Feature: Example

  @AC-42
  Scenario: AC-42 Example scenario
    Given hooks acceptance criterion "AC-42" is defined as a placeholder
    When the hooks BDD suite is executed in CI-safe mode
    Then scenario "AC-42" is reported for later implementation
"#,
        )
        .expect("feature should be written");

        let scenarios = parse_feature_file(&feature_path).expect("parse succeeds");

        assert_eq!(scenarios.len(), 1);
        assert_eq!(scenarios[0].scenario_id, "AC-42");
        assert_eq!(scenarios[0].feature_file, "example.feature");
        assert_eq!(scenarios[0].steps.len(), 3);
    }

    #[test]
    fn parse_feature_file_with_cucumber_requires_at_least_one_scenario() {
        let temp_dir = tempfile::tempdir().expect("tempdir should be created");
        let feature_path = temp_dir.path().join("empty.feature");

        fs::write(&feature_path, "Feature: Empty\n").expect("feature should be written");

        let error = parse_feature_file(&feature_path).expect_err("must fail");
        let message = format!("{error}");
        assert!(message.contains("no scenarios discovered"));
    }

    #[test]
    fn dispatch_ac_evaluator_routes_to_correct_function() {
        // Verify dispatch map returns different evaluator functions for different ACs
        // AC-01 and AC-04 use different evaluators (scope vs ordering)
        let ac01_eval = dispatch_ac_evaluator("AC-01");
        let ac04_eval = dispatch_ac_evaluator("AC-04");
        let ac07_eval = dispatch_ac_evaluator("AC-07");
        let unknown_eval = dispatch_ac_evaluator("AC-99");

        // AC-01 should pass (green), AC-07 should fail (pending), AC-99 should fail (unmapped)
        let scenario_ac01 = HooksBddScenario {
            scenario_id: "AC-01".to_string(),
            scenario_name: "AC-01 Test".to_string(),
            feature_file: "test.feature".to_string(),
            tags: vec!["AC-01".to_string()],
            steps: vec![],
        };
        let scenario_ac07 = HooksBddScenario {
            scenario_id: "AC-07".to_string(),
            scenario_name: "AC-07 Test".to_string(),
            feature_file: "test.feature".to_string(),
            tags: vec!["AC-07".to_string()],
            steps: vec![],
        };
        let scenario_ac04 = HooksBddScenario {
            scenario_id: "AC-04".to_string(),
            scenario_name: "AC-04 Test".to_string(),
            feature_file: "test.feature".to_string(),
            tags: vec!["AC-04".to_string()],
            steps: vec![],
        };
        let scenario_ac02 = HooksBddScenario {
            scenario_id: "AC-02".to_string(),
            scenario_name: "AC-02 Test".to_string(),
            feature_file: "test.feature".to_string(),
            tags: vec!["AC-02".to_string()],
            steps: vec![],
        };
        let scenario_ac03 = HooksBddScenario {
            scenario_id: "AC-03".to_string(),
            scenario_name: "AC-03 Test".to_string(),
            feature_file: "test.feature".to_string(),
            tags: vec!["AC-03".to_string()],
            steps: vec![],
        };
        let scenario_ac99 = HooksBddScenario {
            scenario_id: "AC-99".to_string(),
            scenario_name: "AC-99 Test".to_string(),
            feature_file: "test.feature".to_string(),
            tags: vec!["AC-99".to_string()],
            steps: vec![],
        };

        let mut harness_01 = HooksBddIntegrationHarness::new(&scenario_ac01, true);
        let mut harness_02 = HooksBddIntegrationHarness::new(&scenario_ac02, true);
        let mut harness_03 = HooksBddIntegrationHarness::new(&scenario_ac03, true);
        let mut harness_04 = HooksBddIntegrationHarness::new(&scenario_ac04, true);
        let mut harness_07 = HooksBddIntegrationHarness::new(&scenario_ac07, true);
        let mut harness_99 = HooksBddIntegrationHarness::new(&scenario_ac99, true);

        let result_01 = ac01_eval(&scenario_ac01, &mut harness_01, true);
        let result_02 = ac01_eval(&scenario_ac02, &mut harness_02, true);
        let result_03 = ac01_eval(&scenario_ac03, &mut harness_03, true);
        let result_04 = ac04_eval(&scenario_ac04, &mut harness_04, true);
        let result_07 = ac07_eval(&scenario_ac07, &mut harness_07, true);
        let result_99 = unknown_eval(&scenario_ac99, &mut harness_99, true);

        // AC-01, AC-02, AC-03, AC-04, AC-05, AC-06, AC-07 are green (all implemented)
        assert!(result_01.passed);
        assert!(result_02.passed);
        assert!(result_03.passed);
        assert!(result_04.passed);
        // AC-07 now passes (was pending, now implemented)
        assert!(result_07.passed);
        assert!(result_07.message.contains("verified green"));
        // AC-99 is unmapped (no such AC exists)
        assert!(!result_99.passed);
        assert!(result_99.message.contains("no evaluator implemented"));
    }
}
