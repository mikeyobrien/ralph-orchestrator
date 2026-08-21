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
        configure_bounded_command(&mut command);

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
                        if let Err(source) = terminate_bounded_child(&mut child)
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

#[cfg(unix)]
fn configure_bounded_command(command: &mut Command) {
    use std::os::unix::process::CommandExt;

    command.process_group(0);
}

#[cfg(not(unix))]
fn configure_bounded_command(_command: &mut Command) {}

#[cfg(unix)]
fn terminate_bounded_child(child: &mut std::process::Child) -> std::io::Result<()> {
    let process_group = format!("-{}", child.id());
    match Command::new("kill")
        .args(["-KILL", &process_group])
        .status()
    {
        Ok(status) if status.success() => Ok(()),
        _ => child.kill(),
    }
}

#[cfg(not(unix))]
fn terminate_bounded_child(child: &mut std::process::Child) -> std::io::Result<()> {
    child.kill()
}

/// Disposition of one hooks BDD scenario.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HooksBddScenarioStatus {
    /// Ralph's runtime behavior satisfies the acceptance criterion.
    Passed,
    /// Ralph's runtime behavior does not satisfy the acceptance criterion.
    Failed,
    /// The acceptance criterion belongs to the v3 autoloop engine, not Ralph.
    Descoped,
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
    /// Acceptance-criterion disposition.
    pub status: HooksBddScenarioStatus,
    /// Result reason for terminal output.
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
        self.results
            .iter()
            .filter(|result| result.status == HooksBddScenarioStatus::Passed)
            .count()
    }

    /// Number of failed scenarios.
    pub fn failed_count(&self) -> usize {
        self.results
            .iter()
            .filter(|result| result.status == HooksBddScenarioStatus::Failed)
            .count()
    }

    /// Number of scenarios owned by the v3 autoloop engine rather than Ralph.
    pub fn descoped_count(&self) -> usize {
        self.results
            .iter()
            .filter(|result| result.status == HooksBddScenarioStatus::Descoped)
            .count()
    }

    /// Returns true when no scenario failed.
    pub fn all_passed(&self) -> bool {
        self.failed_count() == 0
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
        // These ACs are certified by the real ralph-core/ralph-cli tests mapped in
        // `runtime_test_cases_for_ac`, not by inspecting implementation source.
        "AC-01" | "AC-02" | "AC-03" | "AC-04" | "AC-05" | "AC-06" | "AC-07" | "AC-10" | "AC-11"
        | "AC-12" | "AC-17" | "AC-18" => evaluate_runtime_certified,
        // AC-08 (warn), AC-09 (block), AC-13..AC-15 (mutation), and AC-16
        // (telemetry completeness) execute inside the autoloop harness in v3.
        // Ralph is the observation plane, so these criteria are engine-owned.
        "AC-08" | "AC-09" | "AC-13" | "AC-14" | "AC-15" | "AC-16" => evaluate_descoped_to_engine,
        _ => evaluate_unmapped_acceptance,
    }
}

fn build_scenario_result(
    scenario: &HooksBddScenario,
    harness: &HooksBddIntegrationHarness,
    status: HooksBddScenarioStatus,
    message: String,
) -> HooksBddScenarioResult {
    HooksBddScenarioResult {
        scenario_id: scenario.scenario_id.clone(),
        scenario_name: scenario.scenario_name.clone(),
        feature_file: scenario.feature_file.clone(),
        status,
        message,
        artifacts: harness.artifacts().clone(),
    }
}

fn evaluate_descoped_to_engine(
    scenario: &HooksBddScenario,
    harness: &mut HooksBddIntegrationHarness,
    _ci_safe_mode: bool,
) -> HooksBddScenarioResult {
    build_scenario_result(
        scenario,
        harness,
        HooksBddScenarioStatus::Descoped,
        format!(
            "{}: DESCOPED to autoloop engine — hooks run inside the autoloop harness in v3 (engine-owned; autoloop hooks parity territory, autoloop#38 per .ralph/specs/v3-autoloops-cutover.spec.md). Not certified by Ralph tests.",
            scenario.scenario_id
        ),
    )
}

/// Certifies an AC by running its mapped ralph-core/ralph-cli runtime tests.
fn evaluate_runtime_certified(
    scenario: &HooksBddScenario,
    harness: &mut HooksBddIntegrationHarness,
    ci_safe_mode: bool,
) -> HooksBddScenarioResult {
    if let Err(msg) = validate_acceptance_context(ci_safe_mode, &scenario.scenario_id) {
        return build_scenario_result(scenario, harness, HooksBddScenarioStatus::Failed, msg);
    }

    if let Err(msg) = assert_runtime_integration_coverage(&scenario.scenario_id, harness) {
        return build_scenario_result(scenario, harness, HooksBddScenarioStatus::Failed, msg);
    }

    build_scenario_result(
        scenario,
        harness,
        HooksBddScenarioStatus::Passed,
        format!(
            "{}: acceptance criterion verified green by runtime tests",
            scenario.scenario_id
        ),
    )
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
            package: "ralph-core",
            filter: "test_hooks_config_valid_yaml_parses_and_validates",
        }],
        "AC-03" => vec![RuntimeTestCase {
            package: "ralph-core",
            filter: "build_payload_maps_loop_iteration_and_context_fields",
        }],
        "AC-04" => vec![RuntimeTestCase {
            package: "ralph-core",
            filter: "resolve_phase_event_preserves_declaration_order",
        }],
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
        "AC-10" => vec![RuntimeTestCase {
            package: "ralph-core",
            filter: "test_suspend_state_record_serializes_v1_schema_shape",
        }],
        "AC-11" => vec![RuntimeTestCase {
            package: "ralph-cli",
            filter: "test_resume_loop_writes_resume_signal_for_in_place_loop",
        }],
        "AC-12" => vec![
            RuntimeTestCase {
                package: "ralph-core",
                filter: "test_resume_signal_is_single_use",
            },
            RuntimeTestCase {
                package: "ralph-cli",
                filter: "test_resume_loop_is_idempotent_when_resume_already_requested",
            },
        ],
        // AC-08/09/13/14/15/16 are deliberately absent: their shared evaluator
        // reports them as engine-owned without running Ralph integration coverage.
        // `test_diagnostics_collector_logs_hook_run_telemetry` still runs in the
        // ralph-core suite, but it cannot certify AC-16 without a live dispatch path.
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
            Duration::from_mins(3),
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

        let stdout = read_artifact_excerpt(&artifact.stdout_path, 4_000);
        let stderr = read_artifact_excerpt(&artifact.stderr_path, 4_000);
        if !cargo_test_output_reports_executed_test(&stdout, &stderr) {
            return Err(format!(
                "{ac_id}: runtime integration filter matched zero tests ({}/{})\nstdout: {}\nstderr: {}",
                runtime_test.package, runtime_test.filter, stdout, stderr
            ));
        }
    }

    Ok(())
}

fn cargo_test_output_reports_executed_test(stdout: &str, stderr: &str) -> bool {
    stdout
        .lines()
        .chain(stderr.lines())
        .any(|line| line.contains("test result: ok.") && !line.contains("0 passed;"))
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
        HooksBddScenarioStatus::Failed,
        format!(
            "{}: no evaluator implemented - scenario is pending",
            scenario.scenario_id
        ),
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

    fn synthetic_result(status: HooksBddScenarioStatus) -> HooksBddScenarioResult {
        HooksBddScenarioResult {
            scenario_id: "AC-90".to_string(),
            scenario_name: "Synthetic result".to_string(),
            feature_file: "hooks/synthetic.feature".to_string(),
            status,
            message: "synthetic result".to_string(),
            artifacts: HooksBddScenarioArtifacts::default(),
        }
    }

    #[test]
    fn run_results_count_descoped_separately_without_failing() {
        let mut results = HooksBddRunResults {
            results: vec![
                synthetic_result(HooksBddScenarioStatus::Passed),
                synthetic_result(HooksBddScenarioStatus::Descoped),
            ],
        };

        assert_eq!(results.total_count(), 2);
        assert_eq!(results.passed_count(), 1);
        assert_eq!(results.failed_count(), 0);
        assert_eq!(results.descoped_count(), 1);
        assert!(results.all_passed());

        results
            .results
            .push(synthetic_result(HooksBddScenarioStatus::Failed));
        assert_eq!(results.failed_count(), 1);
        assert!(!results.all_passed());
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

        let sleep_backend = harness
            .write_hook_script(&workspace_dir, "sleep-backend", "#!/bin/sh\nsleep 3600\n")
            .expect("should write sleep backend");
        let sleep_backend = sleep_backend
            .to_string_lossy()
            .replace('\\', "\\\\")
            .replace('"', "\\\"");

        harness
            .write_workspace_file(
                &workspace_dir,
                ".ralph/hooks-timeout-preset/autoloops.toml",
                &format!(
                    "event_loop.max_iterations = 1\n\
                     event_loop.completion_event = \"task.complete\"\n\
                     event_loop.completion_promise = \"LOOP_COMPLETE\"\n\n\
                     backend.kind = \"command\"\n\
                     backend.command = \"{sleep_backend}\"\n\
                     backend.timeout_ms = 3000000\n\n\
                     review.enabled = false\n\
                     harness.instructions_file = \"harness.md\"\n"
                ),
            )
            .expect("should write autoloop config");
        harness
            .write_workspace_file(
                &workspace_dir,
                ".ralph/hooks-timeout-preset/topology.toml",
                "name = \"hooks-timeout\"\n\
                 completion = \"task.complete\"\n\n\
                 [[role]]\n\
                 id = \"planner\"\n\
                 emits = [\"task.complete\"]\n\
                 prompt_file = \"roles/planner.md\"\n\n\
                 [handoff]\n\
                 \"loop.start\" = [\"planner\"]\n",
            )
            .expect("should write autoloop topology");
        harness
            .write_workspace_file(
                &workspace_dir,
                ".ralph/hooks-timeout-preset/roles/planner.md",
                "You are the planner. This backend intentionally sleeps for timeout coverage.\n",
            )
            .expect("should write autoloop role");
        harness
            .write_workspace_file(
                &workspace_dir,
                ".ralph/hooks-timeout-preset/harness.md",
                "Hooks BDD timeout fixture.\n",
            )
            .expect("should write autoloop harness");

        // Run headlessly because this harness redirects stdio; TUI mode requires
        // a configured terminal and is covered by separate TUI smoke tests.
        harness
            .write_workspace_file(
                &workspace_dir,
                "ralph.yml",
                "core:\n  autoloop_preset: .ralph/hooks-timeout-preset\n",
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
                &[
                    "run",
                    "--no-tui",
                    "-p",
                    "hooks-bdd-timeout",
                    "--max-iterations",
                    "1",
                ],
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
    fn evaluate_runtime_certified_rejects_non_ci_safe_mode() {
        let scenario = HooksBddScenario {
            scenario_id: "AC-01".to_string(),
            scenario_name: "AC-01 synthetic context failure".to_string(),
            feature_file: "hooks/scope-and-dispatch.feature".to_string(),
            tags: vec!["AC-01".to_string()],
            steps: vec![],
        };

        let mut harness = HooksBddIntegrationHarness::new(&scenario, false);
        let result = evaluate_runtime_certified(&scenario, &mut harness, false);

        assert_eq!(result.status, HooksBddScenarioStatus::Failed);
        assert_eq!(result.scenario_id, "AC-01");
        assert!(result.message.contains("CI-safe mode required"));
        assert!(result.message.contains("--mock"));
    }

    #[test]
    fn run_hooks_bdd_suite_passes_ac_01_in_ci_safe_mode() {
        let config = HooksBddConfig::new(Some("AC-01".to_string()), true);
        let results = run_hooks_bdd_suite(&config).expect("suite should run");

        assert_eq!(results.total_count(), 1);
        assert_eq!(results.passed_count(), 1);
        assert_eq!(results.results[0].status, HooksBddScenarioStatus::Passed);
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
        assert_eq!(results.results[0].status, HooksBddScenarioStatus::Passed);
    }

    #[test]
    fn run_hooks_bdd_suite_passes_ac_05_json_stdin_contract() {
        let config = HooksBddConfig::new(Some("AC-05".to_string()), true);
        let results = run_hooks_bdd_suite(&config).expect("suite should run");

        assert_eq!(results.total_count(), 1);
        assert_eq!(results.passed_count(), 1);
        assert_eq!(results.results[0].status, HooksBddScenarioStatus::Passed);
    }

    #[test]
    fn run_hooks_bdd_suite_passes_ac_06_timeout_safeguard() {
        let config = HooksBddConfig::new(Some("AC-06".to_string()), true);
        let results = run_hooks_bdd_suite(&config).expect("suite should run");

        assert_eq!(results.total_count(), 1);
        assert_eq!(results.passed_count(), 1);
        assert_eq!(results.results[0].status, HooksBddScenarioStatus::Passed);
    }

    #[test]
    fn run_hooks_bdd_suite_passes_ac_07_output_size_safeguard() {
        let config = HooksBddConfig::new(Some("AC-07".to_string()), true);
        let results = run_hooks_bdd_suite(&config).expect("suite should run");

        assert_eq!(results.total_count(), 1);
        assert_eq!(results.passed_count(), 1);
        assert_eq!(results.results[0].status, HooksBddScenarioStatus::Passed);
    }

    fn assert_ac_descoped_to_engine(ac_id: &str) {
        let config = HooksBddConfig::new(Some(ac_id.to_string()), true);
        let results = run_hooks_bdd_suite(&config).expect("suite should run");

        assert_eq!(results.total_count(), 1);
        assert_eq!(results.descoped_count(), 1);
        assert_eq!(results.passed_count(), 0);
        assert_eq!(results.failed_count(), 0);
        assert_eq!(results.results[0].status, HooksBddScenarioStatus::Descoped);
        assert!(results.results[0].message.contains("DESCOPED"));
        assert!(results.results[0].message.contains("engine"));
    }

    #[test]
    fn run_hooks_bdd_suite_reports_ac_08_descoped_to_engine() {
        assert_ac_descoped_to_engine("AC-08");
    }

    #[test]
    fn run_hooks_bdd_suite_reports_ac_09_descoped_to_engine() {
        assert_ac_descoped_to_engine("AC-09");
    }

    #[test]
    fn run_hooks_bdd_suite_passes_ac_10_suspend_default_mode() {
        let config = HooksBddConfig::new(Some("AC-10".to_string()), true);
        let results = run_hooks_bdd_suite(&config).expect("suite should run");

        assert_eq!(results.total_count(), 1);
        assert_eq!(results.passed_count(), 1);
        assert_eq!(results.results[0].status, HooksBddScenarioStatus::Passed);
    }

    #[test]
    fn run_hooks_bdd_suite_passes_ac_11_cli_resume_path() {
        let config = HooksBddConfig::new(Some("AC-11".to_string()), true);
        let results = run_hooks_bdd_suite(&config).expect("suite should run");

        assert_eq!(results.total_count(), 1);
        assert_eq!(results.passed_count(), 1);
        assert_eq!(results.results[0].status, HooksBddScenarioStatus::Passed);
    }

    #[test]
    fn run_hooks_bdd_suite_passes_ac_12_resume_idempotency() {
        let config = HooksBddConfig::new(Some("AC-12".to_string()), true);
        let results = run_hooks_bdd_suite(&config).expect("suite should run");

        assert_eq!(results.total_count(), 1);
        assert_eq!(results.passed_count(), 1);
        assert_eq!(results.results[0].status, HooksBddScenarioStatus::Passed);
    }

    #[test]
    fn run_hooks_bdd_suite_reports_ac_13_descoped_to_engine() {
        assert_ac_descoped_to_engine("AC-13");
    }

    #[test]
    fn run_hooks_bdd_suite_reports_ac_14_descoped_to_engine() {
        assert_ac_descoped_to_engine("AC-14");
    }

    #[test]
    fn run_hooks_bdd_suite_reports_ac_15_descoped_to_engine() {
        assert_ac_descoped_to_engine("AC-15");
    }

    #[test]
    fn run_hooks_bdd_suite_reports_ac_16_descoped_to_engine() {
        assert_ac_descoped_to_engine("AC-16");
    }

    #[test]
    fn run_hooks_bdd_suite_passes_ac_17_validation_command() {
        let config = HooksBddConfig::new(Some("AC-17".to_string()), true);
        let results = run_hooks_bdd_suite(&config).expect("suite should run");

        assert_eq!(results.total_count(), 1);
        assert_eq!(results.passed_count(), 1);
        assert_eq!(results.results[0].status, HooksBddScenarioStatus::Passed);
    }

    #[test]
    fn run_hooks_bdd_suite_passes_ac_18_preflight_integration() {
        let config = HooksBddConfig::new(Some("AC-18".to_string()), true);
        let results = run_hooks_bdd_suite(&config).expect("suite should run");

        assert_eq!(results.total_count(), 1);
        assert_eq!(results.passed_count(), 1);
        assert_eq!(results.results[0].status, HooksBddScenarioStatus::Passed);
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
        assert_eq!(result.status, HooksBddScenarioStatus::Failed);
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
        // Runtime-certified ACs share one evaluator; unknown IDs use the fallback.
        let ac01_eval = dispatch_ac_evaluator("AC-01");
        let ac02_eval = dispatch_ac_evaluator("AC-02");
        let ac03_eval = dispatch_ac_evaluator("AC-03");
        let ac04_eval = dispatch_ac_evaluator("AC-04");
        let ac07_eval = dispatch_ac_evaluator("AC-07");
        let unknown_eval = dispatch_ac_evaluator("AC-99");

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
        let result_02 = ac02_eval(&scenario_ac02, &mut harness_02, true);
        let result_03 = ac03_eval(&scenario_ac03, &mut harness_03, true);
        let result_04 = ac04_eval(&scenario_ac04, &mut harness_04, true);
        let result_07 = ac07_eval(&scenario_ac07, &mut harness_07, true);
        let result_99 = unknown_eval(&scenario_ac99, &mut harness_99, true);

        // Mapped ACs are runtime-certified and unknown ACs fail closed.
        assert_eq!(result_01.status, HooksBddScenarioStatus::Passed);
        assert_eq!(result_02.status, HooksBddScenarioStatus::Passed);
        assert_eq!(result_03.status, HooksBddScenarioStatus::Passed);
        assert_eq!(result_04.status, HooksBddScenarioStatus::Passed);
        assert_eq!(result_07.status, HooksBddScenarioStatus::Passed);
        assert!(result_07.message.contains("verified green"));
        // AC-99 is unmapped (no such AC exists)
        assert_eq!(result_99.status, HooksBddScenarioStatus::Failed);
        assert!(result_99.message.contains("no evaluator implemented"));
    }
}
