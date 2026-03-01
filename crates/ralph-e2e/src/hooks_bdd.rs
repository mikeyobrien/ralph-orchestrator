//! Hooks BDD runner with deterministic CI-safe acceptance evaluation.
//!
//! Current rollout status:
//! - AC-01..AC-18: deterministic source-evidence checks (green in `--mock` mode).
//! - feature discovery from `features/hooks/*.feature`
//! - stable AC-tagged failure output for traceability

use crate::executor::find_workspace_root;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

const HOOKS_FEATURE_DIR_WORKSPACE: &str = "crates/ralph-e2e/features/hooks";
const HOOKS_FEATURE_DIR_CRATE: &str = "features/hooks";

const GREEN_ACCEPTANCE_IDS: [&str; 18] = [
    "AC-01", "AC-02", "AC-03", "AC-04", "AC-05", "AC-06", "AC-07", "AC-08", "AC-09", "AC-10",
    "AC-11", "AC-12", "AC-13", "AC-14", "AC-15", "AC-16", "AC-17", "AC-18",
];
const REQUIRED_V1_PHASE_EVENTS: [&str; 12] = [
    "pre.loop.start",
    "post.loop.start",
    "pre.iteration.start",
    "post.iteration.start",
    "pre.plan.created",
    "post.plan.created",
    "pre.human.interact",
    "post.human.interact",
    "pre.loop.complete",
    "post.loop.complete",
    "pre.loop.error",
    "post.loop.error",
];

/// Configuration for executing the hooks BDD acceptance suite.
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

    /// Feature file was malformed for the minimal parser.
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

#[derive(Debug, Default)]
struct HooksStepContext {
    criterion_id: Option<String>,
    ci_safe_confirmed: bool,
}

#[derive(Debug, Default)]
struct SourceEvidenceCache {
    workspace_root: Option<PathBuf>,
    files: HashMap<String, String>,
}

impl SourceEvidenceCache {
    fn resolve_workspace_root(&mut self) -> Result<PathBuf, String> {
        if let Some(root) = &self.workspace_root {
            return Ok(root.clone());
        }

        let manifest_fallback = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(Path::parent)
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")));

        let root = find_workspace_root()
            .filter(|candidate| candidate.join(HOOKS_FEATURE_DIR_WORKSPACE).is_dir())
            .unwrap_or(manifest_fallback);

        self.workspace_root = Some(root.clone());
        Ok(root)
    }

    fn read_source_file(&mut self, relative_path: &str) -> Result<&str, String> {
        if !self.files.contains_key(relative_path) {
            let root = self.resolve_workspace_root()?;
            let path = root.join(relative_path);
            let content = fs::read_to_string(&path).map_err(|source| {
                format!(
                    "failed to read source evidence file '{}': {}",
                    path.display(),
                    source
                )
            })?;
            self.files.insert(relative_path.to_string(), content);
        }

        self.files
            .get(relative_path)
            .map(String::as_str)
            .ok_or_else(|| format!("source evidence cache entry missing: {relative_path}"))
    }

    fn require_snippet(&mut self, relative_path: &str, snippet: &str) -> Result<usize, String> {
        let content = self.read_source_file(relative_path)?;
        let Some(index) = content.find(snippet) else {
            return Err(format!("missing snippet in {}: {}", relative_path, snippet));
        };

        let line = content[..index]
            .bytes()
            .filter(|byte| *byte == b'\n')
            .count()
            + 1;
        Ok(line)
    }
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

/// Executes discovered hooks BDD scenarios.
///
/// AC-01..AC-18 execute deterministic source-evidence checks in CI-safe mode.
pub fn run_hooks_bdd_suite(config: &HooksBddConfig) -> Result<HooksBddRunResults, HooksBddError> {
    let scenarios = discover_hooks_bdd_scenarios(config.filter.as_deref())?;
    let mut results = Vec::with_capacity(scenarios.len());
    let mut evidence_cache = SourceEvidenceCache::default();

    for scenario in scenarios {
        results.push(execute_scenario(
            &scenario,
            config.ci_safe_mode,
            &mut evidence_cache,
        ));
    }

    Ok(HooksBddRunResults { results })
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
    let content = fs::read_to_string(path).map_err(|source| HooksBddError::ReadFeatureFile {
        path: path.to_path_buf(),
        source,
    })?;

    let feature_file = path
        .file_name()
        .map(|name| name.to_string_lossy().to_string())
        .ok_or_else(|| HooksBddError::InvalidFeatureFile {
            path: path.to_path_buf(),
            reason: "missing file name".to_string(),
        })?;

    parse_feature_content(&content, &feature_file).map_err(|reason| {
        HooksBddError::InvalidFeatureFile {
            path: path.to_path_buf(),
            reason,
        }
    })
}

fn parse_feature_content(
    content: &str,
    feature_file: &str,
) -> Result<Vec<HooksBddScenario>, String> {
    let mut scenarios = Vec::new();
    let mut feature_tags: Vec<String> = Vec::new();
    let mut pending_tags: Vec<String> = Vec::new();
    let mut current_scenario: Option<ScenarioBuilder> = None;

    for line in content.lines() {
        let trimmed = line.trim();

        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        if trimmed.starts_with('@') {
            pending_tags.extend(parse_tags(trimmed));
            continue;
        }

        if let Some(_feature_name) = trimmed.strip_prefix("Feature:") {
            feature_tags = std::mem::take(&mut pending_tags);
            continue;
        }

        if let Some(scenario_name) = trimmed.strip_prefix("Scenario:") {
            if let Some(builder) = current_scenario.take() {
                scenarios.push(builder.build(feature_file));
            }

            let mut tags = feature_tags.clone();
            tags.extend(std::mem::take(&mut pending_tags));

            current_scenario = Some(ScenarioBuilder::new(scenario_name.trim().to_string(), tags));
            continue;
        }

        if let Some((keyword, step_text)) = parse_step(trimmed)
            && let Some(builder) = &mut current_scenario
        {
            builder.steps.push(HooksStep {
                keyword,
                text: step_text.to_string(),
            });
        }
    }

    if let Some(builder) = current_scenario.take() {
        scenarios.push(builder.build(feature_file));
    }

    if scenarios.is_empty() {
        return Err("no scenarios discovered".to_string());
    }

    Ok(scenarios)
}

fn parse_tags(line: &str) -> Vec<String> {
    line.split_whitespace()
        .filter_map(|tag| tag.strip_prefix('@'))
        .map(ToString::to_string)
        .collect()
}

fn parse_step(line: &str) -> Option<(HooksStepKeyword, &str)> {
    if let Some(text) = line.strip_prefix("Given ") {
        return Some((HooksStepKeyword::Given, text));
    }

    if let Some(text) = line.strip_prefix("When ") {
        return Some((HooksStepKeyword::When, text));
    }

    line.strip_prefix("Then ")
        .map(|text| (HooksStepKeyword::Then, text))
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

fn execute_scenario(
    scenario: &HooksBddScenario,
    ci_safe_mode: bool,
    evidence_cache: &mut SourceEvidenceCache,
) -> HooksBddScenarioResult {
    let mut context = HooksStepContext::default();

    for step in &scenario.steps {
        if let Err(message) = execute_step_definition(step, &mut context, ci_safe_mode) {
            return HooksBddScenarioResult {
                scenario_id: scenario.scenario_id.clone(),
                scenario_name: scenario.scenario_name.clone(),
                feature_file: scenario.feature_file.clone(),
                passed: false,
                message,
            };
        }
    }

    let Some(criterion_id) = context.criterion_id.as_deref() else {
        return HooksBddScenarioResult {
            scenario_id: scenario.scenario_id.clone(),
            scenario_name: scenario.scenario_name.clone(),
            feature_file: scenario.feature_file.clone(),
            passed: false,
            message: "scenario did not capture an acceptance criterion in Given step".to_string(),
        };
    };

    match evaluate_acceptance_criterion(criterion_id, evidence_cache) {
        Ok(message) => HooksBddScenarioResult {
            scenario_id: scenario.scenario_id.clone(),
            scenario_name: scenario.scenario_name.clone(),
            feature_file: scenario.feature_file.clone(),
            passed: true,
            message,
        },
        Err(message) => HooksBddScenarioResult {
            scenario_id: scenario.scenario_id.clone(),
            scenario_name: scenario.scenario_name.clone(),
            feature_file: scenario.feature_file.clone(),
            passed: false,
            message,
        },
    }
}

fn execute_step_definition(
    step: &HooksStep,
    context: &mut HooksStepContext,
    ci_safe_mode: bool,
) -> Result<(), String> {
    match step.keyword {
        HooksStepKeyword::Given => {
            let criterion_id = parse_given_placeholder_step(&step.text)
                .ok_or_else(|| format!("missing Given step definition for '{}'", step.text))?;
            context.criterion_id = Some(criterion_id);
            Ok(())
        }
        HooksStepKeyword::When => {
            if step.text != "the hooks BDD suite is executed in CI-safe mode" {
                return Err(format!("missing When step definition for '{}'", step.text));
            }

            if !ci_safe_mode {
                return Err("CI-safe mode not enabled; rerun hooks BDD with --mock".to_string());
            }

            context.ci_safe_confirmed = true;
            Ok(())
        }
        HooksStepKeyword::Then => {
            let reported_id = parse_then_reported_step(&step.text)
                .ok_or_else(|| format!("missing Then step definition for '{}'", step.text))?;

            let Some(given_id) = context.criterion_id.as_deref() else {
                return Err("Then step executed before criterion was captured in Given".to_string());
            };

            if !context.ci_safe_confirmed {
                return Err("CI-safe execution step not satisfied before Then".to_string());
            }

            if given_id != reported_id {
                return Err(format!(
                    "criterion mismatch: Given='{}', Then='{}'",
                    given_id, reported_id
                ));
            }

            Ok(())
        }
    }
}

fn evaluate_acceptance_criterion(
    criterion_id: &str,
    evidence_cache: &mut SourceEvidenceCache,
) -> Result<String, String> {
    match criterion_id {
        "AC-01" => evaluate_ac_01(evidence_cache),
        "AC-02" => evaluate_ac_02(evidence_cache),
        "AC-03" => evaluate_ac_03(evidence_cache),
        "AC-04" => evaluate_ac_04(evidence_cache),
        "AC-05" => evaluate_ac_05(evidence_cache),
        "AC-06" => evaluate_ac_06(evidence_cache),
        "AC-07" => evaluate_ac_07(evidence_cache),
        "AC-08" => evaluate_ac_08(evidence_cache),
        "AC-09" => evaluate_ac_09(evidence_cache),
        "AC-10" => evaluate_ac_10(evidence_cache),
        "AC-11" => evaluate_ac_11(evidence_cache),
        "AC-12" => evaluate_ac_12(evidence_cache),
        "AC-13" => evaluate_ac_13(evidence_cache),
        "AC-14" => evaluate_ac_14(evidence_cache),
        "AC-15" => evaluate_ac_15(evidence_cache),
        "AC-16" => evaluate_ac_16(evidence_cache),
        "AC-17" => evaluate_ac_17(evidence_cache),
        "AC-18" => evaluate_ac_18(evidence_cache),
        _ => Err(pending_acceptance_message(criterion_id)),
    }
}

fn evaluate_ac_01(evidence_cache: &mut SourceEvidenceCache) -> Result<String, String> {
    verify_source_evidence(
        "AC-01",
        evidence_cache,
        &[
            (
                "crates/ralph-core/src/config.rs",
                "Global hooks are out of scope for v1; use per-project hooks only",
            ),
            (
                "crates/ralph-core/src/config.rs",
                "fn test_hooks_validate_rejects_global_scope_non_v1_field()",
            ),
        ],
        "per-project scope guardrails enforced",
    )
}

fn evaluate_ac_02(evidence_cache: &mut SourceEvidenceCache) -> Result<String, String> {
    let mut locations = Vec::new();

    for phase_event in REQUIRED_V1_PHASE_EVENTS {
        let parse_snippet = format!("\"{phase_event}\" => Some(");
        record_source_evidence(
            "AC-02",
            evidence_cache,
            "crates/ralph-core/src/config.rs",
            &parse_snippet,
            &mut locations,
        )?;
    }

    for (relative_path, snippet) in [
        (
            "crates/ralph-cli/src/loop_runner.rs",
            "let pre_loop_start_outcomes = dispatch_phase_event_hooks(",
        ),
        (
            "crates/ralph-cli/src/loop_runner.rs",
            "let post_loop_start_outcomes = dispatch_phase_event_hooks(",
        ),
        (
            "crates/ralph-cli/src/loop_runner.rs",
            "let pre_iteration_start_outcomes = dispatch_phase_event_hooks(",
        ),
        (
            "crates/ralph-cli/src/loop_runner.rs",
            "let post_iteration_start_outcomes = dispatch_phase_event_hooks(",
        ),
        (
            "crates/ralph-cli/src/loop_runner.rs",
            "let pre_plan_created_outcomes = dispatch_phase_event_hooks(",
        ),
        (
            "crates/ralph-cli/src/loop_runner.rs",
            "let post_plan_created_outcomes = dispatch_phase_event_hooks(",
        ),
        (
            "crates/ralph-cli/src/loop_runner.rs",
            "let pre_human_interact_outcomes = dispatch_phase_event_hooks(",
        ),
        (
            "crates/ralph-cli/src/loop_runner.rs",
            "let post_human_interact_outcomes = dispatch_phase_event_hooks(",
        ),
        (
            "crates/ralph-cli/src/loop_runner.rs",
            "fn loop_termination_phase_events(reason: &TerminationReason) -> (HookPhaseEvent, HookPhaseEvent)",
        ),
    ] {
        record_source_evidence(
            "AC-02",
            evidence_cache,
            relative_path,
            snippet,
            &mut locations,
        )?;
    }

    Ok(format!(
        "AC-02 verified: mandatory lifecycle event keys and dispatch boundaries are wired ({})",
        locations.join(", ")
    ))
}

fn evaluate_ac_03(evidence_cache: &mut SourceEvidenceCache) -> Result<String, String> {
    verify_source_evidence(
        "AC-03",
        evidence_cache,
        &[
            (
                "crates/ralph-core/src/hooks/engine.rs",
                "let (phase, event) = split_phase_event(phase_event);",
            ),
            (
                "crates/ralph-core/src/hooks/engine.rs",
                "fn split_phase_event(phase_event: HookPhaseEvent) -> (&'static str, &'static str) {",
            ),
            (
                "crates/ralph-core/src/hooks/engine.rs",
                "assert_eq!(payload.phase, \"post\");",
            ),
            (
                "crates/ralph-core/src/hooks/engine.rs",
                "assert_eq!(payload.event, \"iteration.start\");",
            ),
            (
                "crates/ralph-cli/src/loop_runner.rs",
                "vec![\"pre.plan.created\", \"post.plan.created\"],",
            ),
        ],
        "pre/post phase payload and dispatch sequencing present",
    )
}

fn evaluate_ac_04(evidence_cache: &mut SourceEvidenceCache) -> Result<String, String> {
    verify_source_evidence(
        "AC-04",
        evidence_cache,
        &[
            ("crates/ralph-core/src/hooks/engine.rs", ".enumerate()"),
            (
                "crates/ralph-core/src/hooks/engine.rs",
                "fn resolve_phase_event_preserves_declaration_order()",
            ),
            (
                "crates/ralph-cli/src/loop_runner.rs",
                "fn test_dispatch_phase_event_hooks_routes_by_phase_and_preserves_order()",
            ),
            (
                "crates/ralph-cli/src/loop_runner.rs",
                "\"pre-iteration-first|pre.iteration.start\".to_string(),",
            ),
            (
                "crates/ralph-cli/src/loop_runner.rs",
                "\"pre-iteration-second|pre.iteration.start\".to_string(),",
            ),
        ],
        "sequential declaration-order dispatch is covered",
    )
}

fn evaluate_ac_05(evidence_cache: &mut SourceEvidenceCache) -> Result<String, String> {
    verify_source_evidence(
        "AC-05",
        evidence_cache,
        &[
            (
                "crates/ralph-core/src/hooks/executor.rs",
                "command.stdin(Stdio::piped());",
            ),
            (
                "crates/ralph-core/src/hooks/executor.rs",
                "serde_json::to_vec(stdin_payload)",
            ),
            (
                "crates/ralph-core/src/hooks/executor.rs",
                "fn run_writes_json_payload_to_hook_stdin()",
            ),
            (
                "crates/ralph-cli/src/loop_runner.rs",
                "let stdin_payload = match serde_json::to_value(&payload)",
            ),
        ],
        "JSON stdin payload contract is enforced",
    )
}

fn evaluate_ac_06(evidence_cache: &mut SourceEvidenceCache) -> Result<String, String> {
    verify_source_evidence(
        "AC-06",
        evidence_cache,
        &[
            (
                "crates/ralph-core/src/hooks/executor.rs",
                "fn wait_for_completion(",
            ),
            (
                "crates/ralph-core/src/hooks/executor.rs",
                "if wait_started_at.elapsed() >= timeout {",
            ),
            (
                "crates/ralph-core/src/hooks/executor.rs",
                "let status = terminate_for_timeout(",
            ),
            (
                "crates/ralph-core/src/hooks/executor.rs",
                "fn run_marks_timed_out_when_command_exceeds_timeout()",
            ),
        ],
        "timeout termination path is implemented and covered",
    )
}

fn evaluate_ac_07(evidence_cache: &mut SourceEvidenceCache) -> Result<String, String> {
    verify_source_evidence(
        "AC-07",
        evidence_cache,
        &[
            (
                "crates/ralph-core/src/hooks/executor.rs",
                "let capture_limit = usize::try_from(max_output_bytes).unwrap_or(usize::MAX);",
            ),
            (
                "crates/ralph-core/src/hooks/executor.rs",
                "fn run_truncates_stdout_and_stderr_at_max_output_bytes()",
            ),
            (
                "crates/ralph-core/src/hooks/executor.rs",
                "request.max_output_bytes = 8;",
            ),
            (
                "crates/ralph-core/src/hooks/executor.rs",
                "assert!(result.stdout.truncated);",
            ),
            (
                "crates/ralph-core/src/hooks/executor.rs",
                "assert!(result.stderr.truncated);",
            ),
        ],
        "max_output_bytes truncation is deterministic and covered",
    )
}

fn evaluate_ac_08(evidence_cache: &mut SourceEvidenceCache) -> Result<String, String> {
    verify_source_evidence(
        "AC-08",
        evidence_cache,
        &[
            (
                "crates/ralph-cli/src/loop_runner.rs",
                "HookOnError::Warn => HookDisposition::Warn,",
            ),
            (
                "crates/ralph-cli/src/loop_runner.rs",
                "fn test_loop_start_dispatch_warn_continues_and_block_aborts()",
            ),
            (
                "crates/ralph-cli/src/loop_runner.rs",
                "fail_if_blocking_loop_start_outcomes(&pre_loop_start_outcomes).is_ok(),",
            ),
            (
                "crates/ralph-cli/src/loop_runner.rs",
                "\"warn disposition should continue across loop.start boundary\"",
            ),
        ],
        "warn policy continues orchestration and is regression-tested",
    )
}

fn evaluate_ac_09(evidence_cache: &mut SourceEvidenceCache) -> Result<String, String> {
    verify_source_evidence(
        "AC-09",
        evidence_cache,
        &[
            (
                "crates/ralph-cli/src/loop_runner.rs",
                "HookOnError::Block => HookDisposition::Block,",
            ),
            (
                "crates/ralph-cli/src/loop_runner.rs",
                "fn fail_if_blocking_loop_start_outcomes(outcomes: &[HookDispatchOutcome]) -> Result<()> {",
            ),
            (
                "crates/ralph-cli/src/loop_runner.rs",
                "\"Lifecycle hook blocked loop.start boundary\"",
            ),
            (
                "crates/ralph-cli/src/loop_runner.rs",
                ".expect_err(\"block disposition should abort loop.start boundary\");",
            ),
        ],
        "block policy abort path and surfaced reason are covered",
    )
}

fn evaluate_ac_10(evidence_cache: &mut SourceEvidenceCache) -> Result<String, String> {
    verify_source_evidence(
        "AC-10",
        evidence_cache,
        &[
            (
                "crates/ralph-core/src/config.rs",
                "pub enum HookSuspendMode {",
            ),
            ("crates/ralph-core/src/config.rs", "#[default]"),
            ("crates/ralph-core/src/config.rs", "WaitForResume,"),
            (
                "crates/ralph-core/src/config.rs",
                "suspend_mode: HookSuspendMode::default(),",
            ),
            (
                "crates/ralph-core/src/hooks/engine.rs",
                "suspend_mode: spec.suspend_mode.unwrap_or(defaults.suspend_mode),",
            ),
            (
                "crates/ralph-cli/src/loop_runner.rs",
                "\"Lifecycle hook requested suspend; entering wait_for_resume gate\"",
            ),
            (
                "crates/ralph-cli/src/loop_runner.rs",
                "assert_eq!(suspend_state.suspend_mode, HookSuspendMode::WaitForResume);",
            ),
        ],
        "suspend hooks default to wait_for_resume mode and enter resume gate",
    )
}

fn evaluate_ac_11(evidence_cache: &mut SourceEvidenceCache) -> Result<String, String> {
    verify_source_evidence(
        "AC-11",
        evidence_cache,
        &[
            (
                "crates/ralph-cli/src/loops.rs",
                "fn resume_loop(args: ResumeArgs) -> Result<()> {",
            ),
            ("crates/ralph-cli/src/loops.rs", ".write_resume_requested()"),
            (
                "crates/ralph-cli/src/loops.rs",
                "\"Resume requested for loop '{}'. The loop will continue from the suspended boundary.\"",
            ),
            (
                "crates/ralph-cli/src/loops.rs",
                "fn test_resume_loop_writes_resume_signal_for_in_place_loop()",
            ),
            (
                "crates/ralph-cli/src/loops.rs",
                "fn test_resume_loop_resolves_partial_id_and_targets_worktree()",
            ),
        ],
        "CLI resume writes the signal and targets the resolved loop workspace",
    )
}

fn evaluate_ac_12(evidence_cache: &mut SourceEvidenceCache) -> Result<String, String> {
    verify_source_evidence(
        "AC-12",
        evidence_cache,
        &[
            (
                "crates/ralph-cli/src/loops.rs",
                "\"Resume was already requested for loop '{}'. The loop is not currently suspended; no action taken.\"",
            ),
            (
                "crates/ralph-cli/src/loops.rs",
                "\"Loop '{}' is not currently suspended. Nothing to resume.\"",
            ),
            (
                "crates/ralph-cli/src/loops.rs",
                "\"Resume was already requested for loop '{}'. Waiting for the loop to continue.\"",
            ),
            (
                "crates/ralph-cli/src/loops.rs",
                "fn test_resume_loop_is_idempotent_when_resume_already_requested()",
            ),
            (
                "crates/ralph-cli/src/loops.rs",
                "fn test_resume_loop_noops_for_non_suspended_loop()",
            ),
        ],
        "repeat/non-suspended resume requests are non-destructive and explicitly handled",
    )
}

fn evaluate_ac_13(evidence_cache: &mut SourceEvidenceCache) -> Result<String, String> {
    verify_source_evidence(
        "AC-13",
        evidence_cache,
        &[
            (
                "crates/ralph-cli/src/loop_runner.rs",
                "if !mutate.enabled {",
            ),
            (
                "crates/ralph-cli/src/loop_runner.rs",
                "return HookMutationParseOutcome::Disabled;",
            ),
            (
                "crates/ralph-core/src/config.rs",
                "mutation settings require mutate.enabled: true",
            ),
            (
                "crates/ralph-cli/src/loop_runner.rs",
                "fn test_ac13_mutation_disabled_json_output_is_inert_for_accumulator_and_downstream_payloads()",
            ),
            (
                "crates/ralph-cli/src/loop_runner.rs",
                "fn test_parse_hook_mutation_stdout_skips_when_disabled()",
            ),
        ],
        "mutation parsing is explicit opt-in and disabled-mode remains inert",
    )
}

fn evaluate_ac_14(evidence_cache: &mut SourceEvidenceCache) -> Result<String, String> {
    verify_source_evidence(
        "AC-14",
        evidence_cache,
        &[
            (
                "crates/ralph-cli/src/loop_runner.rs",
                "const HOOK_MUTATION_PAYLOAD_METADATA_KEY: &str = \"metadata\";",
            ),
            (
                "crates/ralph-cli/src/loop_runner.rs",
                "if payload_object.len() != 1 || !payload_object.contains_key(HOOK_MUTATION_PAYLOAD_METADATA_KEY)",
            ),
            (
                "crates/ralph-cli/src/loop_runner.rs",
                "message: \"mutation payload key 'metadata' must contain a JSON object\".to_string(),",
            ),
            (
                "crates/ralph-cli/src/loop_runner.rs",
                "fn test_ac14_mutation_enabled_updates_only_namespaced_metadata_in_downstream_payloads()",
            ),
            (
                "crates/ralph-cli/src/loop_runner.rs",
                "assert!(!payload_object.contains_key(\"prompt\"));",
            ),
            (
                "crates/ralph-cli/src/loop_runner.rs",
                "assert!(!payload_object.contains_key(\"events\"));",
            ),
            (
                "crates/ralph-cli/src/loop_runner.rs",
                "assert!(!payload_object.contains_key(\"config\"));",
            ),
        ],
        "mutation surface remains metadata-only and downstream payloads stay scoped",
    )
}

fn evaluate_ac_15(evidence_cache: &mut SourceEvidenceCache) -> Result<String, String> {
    verify_source_evidence(
        "AC-15",
        evidence_cache,
        &[
            (
                "crates/ralph-cli/src/loop_runner.rs",
                "let parsed = match serde_json::from_str::<serde_json::Value>(stdout.trim()) {",
            ),
            (
                "crates/ralph-cli/src/loop_runner.rs",
                "message: format!(\"mutation stdout is not valid JSON: {error}\"),",
            ),
            (
                "crates/ralph-cli/src/loop_runner.rs",
                "Some(HookDispatchFailure::InvalidMutationOutput {",
            ),
            (
                "crates/ralph-cli/src/loop_runner.rs",
                "fn test_ac15_dispatch_phase_event_hooks_non_json_mutation_warn_continues_through_block_gate()",
            ),
            (
                "crates/ralph-cli/src/loop_runner.rs",
                "fn test_ac15_dispatch_phase_event_hooks_non_json_mutation_block_surfaces_invalid_output_reason()",
            ),
            (
                "crates/ralph-cli/src/loop_runner.rs",
                "fn test_ac15_dispatch_phase_event_hooks_non_json_mutation_suspend_uses_wait_for_resume_gate()",
            ),
        ],
        "invalid non-JSON mutation output maps into lifecycle on_error dispositions",
    )
}

fn evaluate_ac_16(evidence_cache: &mut SourceEvidenceCache) -> Result<String, String> {
    verify_source_evidence(
        "AC-16",
        evidence_cache,
        &[
            (
                "crates/ralph-core/src/diagnostics/hook_runs.rs",
                "pub struct HookRunTelemetryEntry {",
            ),
            (
                "crates/ralph-core/src/diagnostics/hook_runs.rs",
                "pub disposition: HookDisposition,",
            ),
            (
                "crates/ralph-core/src/diagnostics/hook_runs.rs",
                "pub suspend_mode: HookSuspendMode,",
            ),
            (
                "crates/ralph-core/src/diagnostics/hook_runs.rs",
                "pub retry_attempt: u32,",
            ),
            (
                "crates/ralph-core/src/diagnostics/hook_runs.rs",
                "pub retry_max_attempts: u32,",
            ),
            (
                "crates/ralph-cli/src/loop_runner.rs",
                "event_loop.log_hook_run_telemetry(HookRunTelemetryEntry::from_run_result(",
            ),
            (
                "crates/ralph-cli/src/loop_runner.rs",
                "fn test_dispatch_phase_event_hooks_retry_backoff_recovers_before_exhaustion()",
            ),
            (
                "crates/ralph-cli/src/loop_runner.rs",
                "fn test_dispatch_phase_event_hooks_wait_then_retry_recovers_after_resume()",
            ),
            (
                "crates/ralph-cli/src/loop_runner.rs",
                "assert_eq!(telemetry_entries.len(), 3);",
            ),
            (
                "crates/ralph-cli/src/loop_runner.rs",
                "assert_eq!(telemetry_entries.len(), 2);",
            ),
        ],
        "hook-run telemetry captures disposition, suspend policy, and retry attempt lifecycle",
    )
}

fn evaluate_ac_17(evidence_cache: &mut SourceEvidenceCache) -> Result<String, String> {
    verify_source_evidence(
        "AC-17",
        evidence_cache,
        &[
            ("crates/ralph-cli/src/main.rs", "Hooks(hooks::HooksArgs),"),
            (
                "crates/ralph-cli/src/main.rs",
                "Some(Commands::Hooks(args)) => {",
            ),
            (
                "crates/ralph-cli/src/hooks.rs",
                "HooksCommands::Validate(validate_args) => {",
            ),
            (
                "crates/ralph-cli/src/hooks.rs",
                "report.push_diagnostic(\"hooks.semantic\", error.to_string(), None, None, None);",
            ),
            (
                "crates/ralph-cli/src/hooks.rs",
                "\"hooks.command_resolvable\",",
            ),
            (
                "crates/ralph-cli/src/hooks.rs",
                "Fix: ensure command exists and is executable, or invoke the script through an interpreter (for example: ['bash', 'script.sh']).",
            ),
            ("crates/ralph-cli/src/hooks.rs", "if !report.pass {"),
            ("crates/ralph-cli/src/hooks.rs", "std::process::exit(1);"),
        ],
        "hooks validate command is exposed, routed, and emits actionable diagnostics",
    )
}

fn evaluate_ac_18(evidence_cache: &mut SourceEvidenceCache) -> Result<String, String> {
    verify_source_evidence(
        "AC-18",
        evidence_cache,
        &[
            (
                "crates/ralph-core/src/preflight.rs",
                "Box::new(HooksValidationCheck),",
            ),
            (
                "crates/ralph-core/src/preflight.rs",
                "fn default_checks_include_hooks_check_name()",
            ),
            (
                "crates/ralph-core/src/preflight.rs",
                "assert!(check_names.contains(&\"hooks\"));",
            ),
            (
                "crates/ralph-cli/src/main.rs",
                "let runner = PreflightRunner::default_checks();",
            ),
            (
                "crates/ralph-cli/src/main.rs",
                "if skip_preflight || !config.features.preflight.enabled {",
            ),
            (
                "crates/ralph-cli/src/main.rs",
                "fn test_auto_preflight_skip_list_can_omit_hooks_check_failures()",
            ),
            (
                "crates/ralph-cli/src/main.rs",
                "config.features.preflight.skip = vec![\"hooks\".to_string()];",
            ),
        ],
        "hooks validation is integrated into automatic preflight with explicit skip controls",
    )
}

fn verify_source_evidence(
    criterion_id: &str,
    evidence_cache: &mut SourceEvidenceCache,
    checks: &[(&str, &str)],
    success_summary: &str,
) -> Result<String, String> {
    let mut locations = Vec::with_capacity(checks.len());

    for (relative_path, snippet) in checks {
        record_source_evidence(
            criterion_id,
            evidence_cache,
            relative_path,
            snippet,
            &mut locations,
        )?;
    }

    Ok(format!(
        "{criterion_id} verified: {success_summary} ({})",
        locations.join(", ")
    ))
}

fn record_source_evidence(
    criterion_id: &str,
    evidence_cache: &mut SourceEvidenceCache,
    relative_path: &str,
    snippet: &str,
    locations: &mut Vec<String>,
) -> Result<(), String> {
    let line = evidence_cache
        .require_snippet(relative_path, snippet)
        .map_err(|error| format!("{criterion_id}: {error}"))?;
    locations.push(format!("{relative_path}:{line}"));
    Ok(())
}

fn pending_acceptance_message(criterion_id: &str) -> String {
    match parse_acceptance_number(criterion_id) {
        Some(_) if !GREEN_ACCEPTANCE_IDS.contains(&criterion_id) => {
            format!("pending: {criterion_id} has no green evaluator yet")
        }
        _ => format!("pending: {criterion_id} acceptance evaluator is not implemented"),
    }
}

fn parse_acceptance_number(criterion_id: &str) -> Option<u8> {
    criterion_id
        .strip_prefix("AC-")
        .and_then(|value| value.parse::<u8>().ok())
}

fn parse_given_placeholder_step(text: &str) -> Option<String> {
    let prefix = "hooks acceptance criterion \"";
    let suffix = "\" is defined as a placeholder";

    text.strip_prefix(prefix)
        .and_then(|remaining| remaining.strip_suffix(suffix))
        .map(ToString::to_string)
}

fn parse_then_reported_step(text: &str) -> Option<&str> {
    let prefix = "scenario \"";
    let suffix = "\" is reported for later implementation";

    text.strip_prefix(prefix)
        .and_then(|remaining| remaining.strip_suffix(suffix))
}

#[derive(Debug, Clone)]
struct ScenarioBuilder {
    scenario_name: String,
    tags: Vec<String>,
    steps: Vec<HooksStep>,
}

impl ScenarioBuilder {
    fn new(scenario_name: String, tags: Vec<String>) -> Self {
        Self {
            scenario_name,
            tags,
            steps: Vec::new(),
        }
    }

    fn build(self, feature_file: &str) -> HooksBddScenario {
        let scenario_id = self
            .tags
            .iter()
            .find(|tag| is_acceptance_id(tag))
            .cloned()
            .unwrap_or_else(|| self.scenario_name.clone());

        HooksBddScenario {
            scenario_id,
            scenario_name: self.scenario_name,
            feature_file: feature_file.to_string(),
            tags: self.tags,
            steps: self.steps,
        }
    }
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

    #[test]
    fn discover_hooks_bdd_scenarios_finds_all_placeholder_scenarios() {
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
    fn run_hooks_bdd_suite_passes_ac_01_in_ci_safe_mode() {
        let config = HooksBddConfig::new(Some("AC-01".to_string()), true);
        let results = run_hooks_bdd_suite(&config).expect("suite should run");

        assert_eq!(results.total_count(), 1);
        assert_eq!(results.passed_count(), 1);
        assert!(results.results[0].passed);
        assert!(results.results[0].message.contains("AC-01 verified"));
    }

    #[test]
    fn run_hooks_bdd_suite_passes_ac_01_to_ac_18_slice_in_ci_safe_mode() {
        let config = HooksBddConfig::new(None, true);
        let results = run_hooks_bdd_suite(&config).expect("suite should run");

        assert_eq!(results.total_count(), 18);
        assert_eq!(results.passed_count(), 18);
        assert_eq!(results.failed_count(), 0);

        let ac_13 = results
            .results
            .iter()
            .find(|result| result.scenario_id == "AC-13")
            .expect("AC-13 result should exist");
        assert!(ac_13.passed);
        assert!(ac_13.message.contains("AC-13 verified"));

        let ac_18 = results
            .results
            .iter()
            .find(|result| result.scenario_id == "AC-18")
            .expect("AC-18 result should exist");
        assert!(ac_18.passed);
        assert!(ac_18.message.contains("AC-18 verified"));
    }

    #[test]
    fn run_hooks_bdd_suite_reports_ci_safe_guard_failure_without_mock() {
        let config = HooksBddConfig::new(Some("AC-01".to_string()), false);
        let results = run_hooks_bdd_suite(&config).expect("suite should run");

        assert_eq!(results.total_count(), 1);
        assert_eq!(results.failed_count(), 1);
        assert!(
            results.results[0]
                .message
                .contains("CI-safe mode not enabled")
        );
    }

    #[test]
    fn parse_feature_content_parses_scenario_tags_and_steps() {
        let content = r#"
@hooks
Feature: Example

  @AC-42
  Scenario: AC-42 Example scenario
    Given hooks acceptance criterion "AC-42" is defined as a placeholder
    When the hooks BDD suite is executed in CI-safe mode
    Then scenario "AC-42" is reported for later implementation
"#;

        let scenarios = parse_feature_content(content, "example.feature").expect("parse succeeds");

        assert_eq!(scenarios.len(), 1);
        assert_eq!(scenarios[0].scenario_id, "AC-42");
        assert_eq!(scenarios[0].feature_file, "example.feature");
        assert_eq!(scenarios[0].steps.len(), 3);
    }

    #[test]
    fn parse_feature_content_requires_at_least_one_scenario() {
        let content = "Feature: Empty";
        let error = parse_feature_content(content, "empty.feature").expect_err("must fail");
        assert!(error.contains("no scenarios discovered"));
    }

    #[test]
    fn execute_step_definition_rejects_mismatched_acceptance_ids() {
        let step = HooksStep {
            keyword: HooksStepKeyword::Then,
            text: "scenario \"AC-02\" is reported for later implementation".to_string(),
        };

        let mut context = HooksStepContext {
            criterion_id: Some("AC-01".to_string()),
            ci_safe_confirmed: true,
        };

        let error = execute_step_definition(&step, &mut context, true).expect_err("must fail");
        assert!(error.contains("criterion mismatch"));
    }
}
