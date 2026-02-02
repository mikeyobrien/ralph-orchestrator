//! Configuration types for the Ralph Orchestrator.
//!
//! This module supports both v1.x flat configuration format and v2.0 nested format.
//! Users can switch from Python v1.x to Rust v2.0 with zero config changes.

use ralph_proto::Topic;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tracing::debug;

/// Top-level configuration for Ralph Orchestrator.
///
/// Supports both v1.x flat format and v2.0 nested format:
/// - v1: `agent: claude`, `max_iterations: 100`
/// - v2: `cli: { backend: claude }`, `event_loop: { max_iterations: 100 }`
#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(clippy::struct_excessive_bools)] // Configuration struct with multiple feature flags
pub struct RalphConfig {
    /// Event loop configuration (v2 nested style).
    #[serde(default)]
    pub event_loop: EventLoopConfig,

    /// CLI backend configuration (v2 nested style).
    #[serde(default)]
    pub cli: CliConfig,

    /// Core paths and settings shared across all hats.
    #[serde(default)]
    pub core: CoreConfig,

    /// Custom hat definitions (optional).
    /// If empty, default planner and builder hats are used.
    #[serde(default)]
    pub hats: HashMap<String, HatConfig>,

    /// Event metadata definitions (optional).
    /// Defines what each event topic means, enabling auto-derived instructions.
    /// If a hat uses custom events, define them here for proper behavior injection.
    #[serde(default)]
    pub events: HashMap<String, EventMetadata>,

    // ─────────────────────────────────────────────────────────────────────────
    // V1 COMPATIBILITY FIELDS (flat format)
    // These map to nested v2 fields for backwards compatibility.
    // ─────────────────────────────────────────────────────────────────────────
    /// V1 field: Backend CLI (maps to cli.backend).
    /// Values: "claude", "kiro", "gemini", "codex", "amp", "auto", or "custom".
    #[serde(default)]
    pub agent: Option<String>,

    /// V1 field: Fallback order for auto-detection.
    #[serde(default)]
    pub agent_priority: Vec<String>,

    /// V1 field: Path to prompt file (maps to `event_loop.prompt_file`).
    #[serde(default)]
    pub prompt_file: Option<String>,

    /// V1 field: Completion detection string (maps to event_loop.completion_promise).
    #[serde(default)]
    pub completion_promise: Option<String>,

    /// V1 field: Maximum loop iterations (maps to event_loop.max_iterations).
    #[serde(default)]
    pub max_iterations: Option<u32>,

    /// V1 field: Maximum runtime in seconds (maps to event_loop.max_runtime_seconds).
    #[serde(default)]
    pub max_runtime: Option<u64>,

    /// V1 field: Maximum cost in USD (maps to event_loop.max_cost_usd).
    #[serde(default)]
    pub max_cost: Option<f64>,

    // ─────────────────────────────────────────────────────────────────────────
    // FEATURE FLAGS
    // ─────────────────────────────────────────────────────────────────────────
    /// Enable verbose output.
    #[serde(default)]
    pub verbose: bool,

    /// Archive prompts after completion (DEFERRED: warn if enabled).
    #[serde(default)]
    pub archive_prompts: bool,

    /// Enable metrics collection (DEFERRED: warn if enabled).
    #[serde(default)]
    pub enable_metrics: bool,

    // ─────────────────────────────────────────────────────────────────────────
    // DROPPED FIELDS (accepted but ignored with warning)
    // ─────────────────────────────────────────────────────────────────────────
    /// V1 field: Token limits (DROPPED: controlled by CLI tool).
    #[serde(default)]
    pub max_tokens: Option<u32>,

    /// V1 field: Retry delay (DROPPED: handled differently in v2).
    #[serde(default)]
    pub retry_delay: Option<u32>,

    /// V1 adapter settings (partially supported).
    #[serde(default)]
    pub adapters: AdaptersConfig,

    // ─────────────────────────────────────────────────────────────────────────
    // WARNING CONTROL
    // ─────────────────────────────────────────────────────────────────────────
    /// Suppress all warnings (for CI environments).
    #[serde(default, rename = "_suppress_warnings")]
    pub suppress_warnings: bool,

    /// TUI configuration.
    #[serde(default)]
    pub tui: TuiConfig,

    /// Memories configuration for persistent learning across sessions.
    #[serde(default)]
    pub memories: MemoriesConfig,

    /// Tasks configuration for runtime work tracking.
    #[serde(default)]
    pub tasks: TasksConfig,

    /// Skills configuration for the skill discovery and injection system.
    #[serde(default)]
    pub skills: SkillsConfig,

    /// Feature flags for optional capabilities.
    #[serde(default)]
    pub features: FeaturesConfig,

    /// RObot (Ralph-Orchestrator bot) configuration for Telegram-based interaction.
    #[serde(default, rename = "RObot")]
    pub robot: RobotConfig,
}

fn default_true() -> bool {
    true
}

#[allow(clippy::derivable_impls)] // Cannot derive due to serde default functions
impl Default for RalphConfig {
    fn default() -> Self {
        Self {
            event_loop: EventLoopConfig::default(),
            cli: CliConfig::default(),
            core: CoreConfig::default(),
            hats: HashMap::new(),
            events: HashMap::new(),
            // V1 compatibility fields
            agent: None,
            agent_priority: vec![],
            prompt_file: None,
            completion_promise: None,
            max_iterations: None,
            max_runtime: None,
            max_cost: None,
            // Feature flags
            verbose: false,
            archive_prompts: false,
            enable_metrics: false,
            // Dropped fields
            max_tokens: None,
            retry_delay: None,
            adapters: AdaptersConfig::default(),
            // Warning control
            suppress_warnings: false,
            // TUI
            tui: TuiConfig::default(),
            // Memories
            memories: MemoriesConfig::default(),
            // Tasks
            tasks: TasksConfig::default(),
            // Skills
            skills: SkillsConfig::default(),
            // Features
            features: FeaturesConfig::default(),
            // RObot (Ralph-Orchestrator bot)
            robot: RobotConfig::default(),
        }
    }
}

/// V1 adapter settings per backend.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AdaptersConfig {
    /// Claude adapter settings.
    #[serde(default)]
    pub claude: AdapterSettings,

    /// Gemini adapter settings.
    #[serde(default)]
    pub gemini: AdapterSettings,

    /// Kiro adapter settings.
    #[serde(default)]
    pub kiro: AdapterSettings,

    /// Codex adapter settings.
    #[serde(default)]
    pub codex: AdapterSettings,

    /// Amp adapter settings.
    #[serde(default)]
    pub amp: AdapterSettings,
}

/// Per-adapter settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdapterSettings {
    /// CLI execution timeout in seconds.
    #[serde(default = "default_timeout")]
    pub timeout: u64,

    /// Include in auto-detection.
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Tool permissions (DROPPED: CLI tool manages its own permissions).
    #[serde(default)]
    pub tool_permissions: Option<Vec<String>>,
}

fn default_timeout() -> u64 {
    300 // 5 minutes
}

impl Default for AdapterSettings {
    fn default() -> Self {
        Self {
            timeout: default_timeout(),
            enabled: true,
            tool_permissions: None,
        }
    }
}

impl RalphConfig {
    /// Loads configuration from a YAML file.
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, ConfigError> {
        let path_ref = path.as_ref();
        debug!(path = %path_ref.display(), "Loading configuration from file");
        let content = std::fs::read_to_string(path_ref)?;
        Self::parse_yaml(&content)
    }

    /// Parses configuration from a YAML string.
    pub fn parse_yaml(content: &str) -> Result<Self, ConfigError> {
        let config: Self = serde_yaml::from_str(content)?;
        debug!(
            backend = %config.cli.backend,
            has_v1_fields = config.agent.is_some(),
            custom_hats = config.hats.len(),
            "Configuration loaded"
        );
        Ok(config)
    }

    /// Normalizes v1 flat fields into v2 nested structure.
    ///
    /// V1 flat fields take precedence over v2 nested fields when both are present.
    /// This allows users to use either format or mix them.
    pub fn normalize(&mut self) {
        let mut normalized_count = 0;

        // Map v1 `agent` to v2 `cli.backend`
        if let Some(ref agent) = self.agent {
            debug!(from = "agent", to = "cli.backend", value = %agent, "Normalizing v1 field");
            self.cli.backend = agent.clone();
            normalized_count += 1;
        }

        // Map v1 `prompt_file` to v2 `event_loop.prompt_file`
        if let Some(ref pf) = self.prompt_file {
            debug!(from = "prompt_file", to = "event_loop.prompt_file", value = %pf, "Normalizing v1 field");
            self.event_loop.prompt_file = pf.clone();
            normalized_count += 1;
        }

        // Map v1 `completion_promise` to v2 `event_loop.completion_promise`
        if let Some(ref cp) = self.completion_promise {
            debug!(
                from = "completion_promise",
                to = "event_loop.completion_promise",
                "Normalizing v1 field"
            );
            self.event_loop.completion_promise = cp.clone();
            normalized_count += 1;
        }

        // Map v1 `max_iterations` to v2 `event_loop.max_iterations`
        if let Some(mi) = self.max_iterations {
            debug!(
                from = "max_iterations",
                to = "event_loop.max_iterations",
                value = mi,
                "Normalizing v1 field"
            );
            self.event_loop.max_iterations = mi;
            normalized_count += 1;
        }

        // Map v1 `max_runtime` to v2 `event_loop.max_runtime_seconds`
        if let Some(mr) = self.max_runtime {
            debug!(
                from = "max_runtime",
                to = "event_loop.max_runtime_seconds",
                value = mr,
                "Normalizing v1 field"
            );
            self.event_loop.max_runtime_seconds = mr;
            normalized_count += 1;
        }

        // Map v1 `max_cost` to v2 `event_loop.max_cost_usd`
        if self.max_cost.is_some() {
            debug!(
                from = "max_cost",
                to = "event_loop.max_cost_usd",
                "Normalizing v1 field"
            );
            self.event_loop.max_cost_usd = self.max_cost;
            normalized_count += 1;
        }

        // Merge extra_instructions into instructions for each hat
        for (hat_id, hat) in &mut self.hats {
            if !hat.extra_instructions.is_empty() {
                for fragment in hat.extra_instructions.drain(..) {
                    if !hat.instructions.ends_with('\n') {
                        hat.instructions.push('\n');
                    }
                    hat.instructions.push_str(&fragment);
                }
                debug!(hat = %hat_id, "Merged extra_instructions into hat instructions");
                normalized_count += 1;
            }
        }

        if normalized_count > 0 {
            debug!(
                fields_normalized = normalized_count,
                "V1 to V2 config normalization complete"
            );
        }
    }

    /// Validates the configuration and returns warnings.
    ///
    /// This method checks for:
    /// - Deferred features that are enabled (archive_prompts, enable_metrics)
    /// - Dropped fields that are present (max_tokens, retry_delay, tool_permissions)
    /// - Ambiguous trigger routing across custom hats
    /// - Mutual exclusivity of prompt and prompt_file
    ///
    /// Returns a list of warnings that should be displayed to the user.
    pub fn validate(&self) -> Result<Vec<ConfigWarning>, ConfigError> {
        let mut warnings = Vec::new();

        // Skip all warnings if suppressed
        if self.suppress_warnings {
            return Ok(warnings);
        }

        // Check for mutual exclusivity of prompt and prompt_file in config
        // Only error if both are explicitly set (not defaults)
        if self.event_loop.prompt.is_some()
            && !self.event_loop.prompt_file.is_empty()
            && self.event_loop.prompt_file != default_prompt_file()
        {
            return Err(ConfigError::MutuallyExclusive {
                field1: "event_loop.prompt".to_string(),
                field2: "event_loop.prompt_file".to_string(),
            });
        }
        if self.event_loop.completion_promise.trim().is_empty() {
            return Err(ConfigError::InvalidCompletionPromise);
        }

        // Check custom backend has a command
        if self.cli.backend == "custom" && self.cli.command.as_ref().is_none_or(String::is_empty) {
            return Err(ConfigError::CustomBackendRequiresCommand);
        }

        // Check for deferred features
        if self.archive_prompts {
            warnings.push(ConfigWarning::DeferredFeature {
                field: "archive_prompts".to_string(),
                message: "Feature not yet available in v2".to_string(),
            });
        }

        if self.enable_metrics {
            warnings.push(ConfigWarning::DeferredFeature {
                field: "enable_metrics".to_string(),
                message: "Feature not yet available in v2".to_string(),
            });
        }

        // Check for dropped fields
        if self.max_tokens.is_some() {
            warnings.push(ConfigWarning::DroppedField {
                field: "max_tokens".to_string(),
                reason: "Token limits are controlled by the CLI tool".to_string(),
            });
        }

        if self.retry_delay.is_some() {
            warnings.push(ConfigWarning::DroppedField {
                field: "retry_delay".to_string(),
                reason: "Retry logic handled differently in v2".to_string(),
            });
        }

        if let Some(threshold) = self.event_loop.mutation_score_warn_threshold
            && !(0.0..=100.0).contains(&threshold)
        {
            warnings.push(ConfigWarning::InvalidValue {
                field: "event_loop.mutation_score_warn_threshold".to_string(),
                message: "Value must be between 0 and 100".to_string(),
            });
        }

        // Check adapter tool_permissions (dropped field)
        if self.adapters.claude.tool_permissions.is_some()
            || self.adapters.gemini.tool_permissions.is_some()
            || self.adapters.codex.tool_permissions.is_some()
            || self.adapters.amp.tool_permissions.is_some()
        {
            warnings.push(ConfigWarning::DroppedField {
                field: "adapters.*.tool_permissions".to_string(),
                reason: "CLI tool manages its own permissions".to_string(),
            });
        }

        // Validate RObot config
        self.robot.validate()?;

        // Check for required description field on all hats
        for (hat_id, hat_config) in &self.hats {
            if hat_config
                .description
                .as_ref()
                .is_none_or(|d| d.trim().is_empty())
            {
                return Err(ConfigError::MissingDescription {
                    hat: hat_id.clone(),
                });
            }
        }

        // Check for reserved triggers: task.start and task.resume are reserved for Ralph
        // Per design: Ralph coordinates first, then delegates to custom hats via events
        const RESERVED_TRIGGERS: &[&str] = &["task.start", "task.resume"];
        for (hat_id, hat_config) in &self.hats {
            for trigger in &hat_config.triggers {
                if RESERVED_TRIGGERS.contains(&trigger.as_str()) {
                    return Err(ConfigError::ReservedTrigger {
                        trigger: trigger.clone(),
                        hat: hat_id.clone(),
                    });
                }
            }
        }

        // Check for ambiguous routing: each trigger topic must map to exactly one hat
        // Per spec: "Every trigger maps to exactly one hat | No ambiguous routing"
        if !self.hats.is_empty() {
            let mut trigger_to_hat: HashMap<&str, &str> = HashMap::new();
            for (hat_id, hat_config) in &self.hats {
                for trigger in &hat_config.triggers {
                    if let Some(existing_hat) = trigger_to_hat.get(trigger.as_str()) {
                        return Err(ConfigError::AmbiguousRouting {
                            trigger: trigger.clone(),
                            hat1: (*existing_hat).to_string(),
                            hat2: hat_id.clone(),
                        });
                    }
                    trigger_to_hat.insert(trigger.as_str(), hat_id.as_str());
                }
            }
        }

        Ok(warnings)
    }

    /// Gets the effective backend name, resolving "auto" using the priority list.
    pub fn effective_backend(&self) -> &str {
        &self.cli.backend
    }

    /// Returns the agent priority list for auto-detection.
    /// If empty, returns the default priority order.
    pub fn get_agent_priority(&self) -> Vec<&str> {
        if self.agent_priority.is_empty() {
            vec!["claude", "kiro", "gemini", "codex", "amp"]
        } else {
            self.agent_priority.iter().map(String::as_str).collect()
        }
    }

    /// Gets the adapter settings for a specific backend.
    #[allow(clippy::match_same_arms)] // Explicit match arms for each backend improves readability
    pub fn adapter_settings(&self, backend: &str) -> &AdapterSettings {
        match backend {
            "claude" => &self.adapters.claude,
            "gemini" => &self.adapters.gemini,
            "kiro" => &self.adapters.kiro,
            "codex" => &self.adapters.codex,
            "amp" => &self.adapters.amp,
            _ => &self.adapters.claude, // Default fallback
        }
    }
}

/// Configuration warnings emitted during validation.
#[derive(Debug, Clone)]
pub enum ConfigWarning {
    /// Feature is enabled but not yet available in v2.
    DeferredFeature { field: String, message: String },
    /// Field is present but ignored in v2.
    DroppedField { field: String, reason: String },
    /// Field has an invalid value.
    InvalidValue { field: String, message: String },
}

impl std::fmt::Display for ConfigWarning {
    #[allow(clippy::match_same_arms)] // Different arms have different messages despite similar structure
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigWarning::DeferredFeature { field, message }
            | ConfigWarning::InvalidValue { field, message } => {
                write!(f, "Warning [{field}]: {message}")
            }
            ConfigWarning::DroppedField { field, reason } => {
                write!(f, "Warning [{field}]: Field ignored - {reason}")
            }
        }
    }
}

/// Event loop configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventLoopConfig {
    /// Inline prompt text (mutually exclusive with prompt_file).
    pub prompt: Option<String>,

    /// Path to the prompt file.
    #[serde(default = "default_prompt_file")]
    pub prompt_file: String,

    /// Event topic that signals loop completion (must be emitted via `ralph emit`).
    #[serde(default = "default_completion_promise")]
    pub completion_promise: String,

    /// Maximum number of iterations before timeout.
    #[serde(default = "default_max_iterations")]
    pub max_iterations: u32,

    /// Maximum runtime in seconds.
    #[serde(default = "default_max_runtime")]
    pub max_runtime_seconds: u64,

    /// Maximum cost in USD before stopping.
    pub max_cost_usd: Option<f64>,

    /// Stop after this many consecutive failures.
    #[serde(default = "default_max_failures")]
    pub max_consecutive_failures: u32,

    /// Delay in seconds before starting the next iteration.
    /// Skipped when the next iteration is triggered by a human event.
    #[serde(default)]
    pub cooldown_delay_seconds: u64,

    /// Starting hat for multi-hat mode (deprecated, use starting_event instead).
    pub starting_hat: Option<String>,

    /// Event to publish after Ralph completes initial coordination.
    ///
    /// When custom hats are defined, Ralph handles `task.start` to do gap analysis
    /// and planning, then publishes this event to delegate to the first hat.
    ///
    /// Example: `starting_event: "tdd.start"` for TDD workflow.
    ///
    /// If not specified and hats are defined, Ralph will determine the appropriate
    /// event from the hat topology.
    pub starting_event: Option<String>,

    /// Warn when mutation testing score drops below this percentage (0-100).
    ///
    /// Warning-only: build.done is still accepted even if below threshold.
    #[serde(default)]
    pub mutation_score_warn_threshold: Option<f64>,

    /// When true, LOOP_COMPLETE does not terminate the loop.
    ///
    /// Instead of exiting, the loop injects a `task.resume` event and continues
    /// idling until new work arrives (human guidance, Telegram commands, etc.).
    /// The loop will only terminate on hard limits (max_iterations, max_runtime,
    /// max_cost), consecutive failures, or explicit interrupt/stop.
    #[serde(default)]
    pub persistent: bool,
}

fn default_prompt_file() -> String {
    "PROMPT.md".to_string()
}

fn default_completion_promise() -> String {
    "LOOP_COMPLETE".to_string()
}

fn default_max_iterations() -> u32 {
    100
}

fn default_max_runtime() -> u64 {
    14400 // 4 hours
}

fn default_max_failures() -> u32 {
    5
}

impl Default for EventLoopConfig {
    fn default() -> Self {
        Self {
            prompt: None,
            prompt_file: default_prompt_file(),
            completion_promise: default_completion_promise(),
            max_iterations: default_max_iterations(),
            max_runtime_seconds: default_max_runtime(),
            max_cost_usd: None,
            max_consecutive_failures: default_max_failures(),
            cooldown_delay_seconds: 0,
            starting_hat: None,
            starting_event: None,
            mutation_score_warn_threshold: None,
            persistent: false,
        }
    }
}

/// Core paths and settings shared across all hats.
///
/// Per spec: "Core behaviors (always injected, can customize paths)"
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoreConfig {
    /// Path to the scratchpad file (shared state between hats).
    #[serde(default = "default_scratchpad")]
    pub scratchpad: String,

    /// Path to the specs directory (source of truth for requirements).
    #[serde(default = "default_specs_dir")]
    pub specs_dir: String,

    /// Guardrails injected into every prompt (core behaviors).
    ///
    /// Per spec: These are always present regardless of hat.
    #[serde(default = "default_guardrails")]
    pub guardrails: Vec<String>,

    /// Root directory for workspace-relative paths (.ralph/, specs, etc.).
    ///
    /// All relative paths (scratchpad, specs_dir, memories) are resolved relative
    /// to this directory. Defaults to the current working directory.
    ///
    /// This is especially important for E2E tests that run in isolated workspaces.
    #[serde(skip)]
    pub workspace_root: std::path::PathBuf,
}

fn default_scratchpad() -> String {
    ".ralph/agent/scratchpad.md".to_string()
}

fn default_specs_dir() -> String {
    ".ralph/specs/".to_string()
}

fn default_guardrails() -> Vec<String> {
    vec![
        "Fresh context each iteration - scratchpad is memory".to_string(),
        "Don't assume 'not implemented' - search first".to_string(),
        "Backpressure is law - tests/typecheck/lint/audit must pass".to_string(),
        "Confidence protocol: score decisions 0-100. >80 proceed autonomously; 50-80 proceed + document in .ralph/agent/decisions.md; <50 choose safe default + document".to_string(),
        "Commit atomically - one logical change per commit, capture the why".to_string(),
    ]
}

impl Default for CoreConfig {
    fn default() -> Self {
        Self {
            scratchpad: default_scratchpad(),
            specs_dir: default_specs_dir(),
            guardrails: default_guardrails(),
            workspace_root: std::env::var("RALPH_WORKSPACE_ROOT")
                .map(std::path::PathBuf::from)
                .unwrap_or_else(|_| {
                    std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."))
                }),
        }
    }
}

impl CoreConfig {
    /// Sets the workspace root for resolving relative paths.
    ///
    /// This is used by E2E tests to point to their isolated test workspace.
    pub fn with_workspace_root(mut self, root: impl Into<std::path::PathBuf>) -> Self {
        self.workspace_root = root.into();
        self
    }

    /// Resolves a relative path against the workspace root.
    ///
    /// If the path is already absolute, it is returned as-is.
    /// Otherwise, it is joined with the workspace root.
    pub fn resolve_path(&self, relative: &str) -> std::path::PathBuf {
        let path = std::path::Path::new(relative);
        if path.is_absolute() {
            path.to_path_buf()
        } else {
            self.workspace_root.join(path)
        }
    }
}

/// CLI backend configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CliConfig {
    /// Backend to use: "claude", "kiro", "gemini", "codex", "amp", or "custom".
    #[serde(default = "default_backend")]
    pub backend: String,

    /// Command override. Required for "custom" backend.
    /// For named backends, overrides the default binary path.
    pub command: Option<String>,

    /// How to pass prompts: "arg" or "stdin".
    #[serde(default = "default_prompt_mode")]
    pub prompt_mode: String,

    /// Execution mode when --interactive not specified.
    /// Values: "autonomous" (default), "interactive"
    #[serde(default = "default_mode")]
    pub default_mode: String,

    /// Idle timeout in seconds for interactive mode.
    /// Process is terminated after this many seconds of inactivity (no output AND no user input).
    /// Set to 0 to disable idle timeout.
    #[serde(default = "default_idle_timeout")]
    pub idle_timeout_secs: u32,

    /// Custom arguments to pass to the CLI command (for backend: "custom").
    /// These are inserted before the prompt argument.
    #[serde(default)]
    pub args: Vec<String>,

    /// Custom prompt flag for arg mode (for backend: "custom").
    /// If None, defaults to "-p" for arg mode.
    #[serde(default)]
    pub prompt_flag: Option<String>,
}

fn default_backend() -> String {
    "claude".to_string()
}

fn default_prompt_mode() -> String {
    "arg".to_string()
}

fn default_mode() -> String {
    "autonomous".to_string()
}

fn default_idle_timeout() -> u32 {
    30 // 30 seconds per spec
}

impl Default for CliConfig {
    fn default() -> Self {
        Self {
            backend: default_backend(),
            command: None,
            prompt_mode: default_prompt_mode(),
            default_mode: default_mode(),
            idle_timeout_secs: default_idle_timeout(),
            args: Vec::new(),
            prompt_flag: None,
        }
    }
}

/// TUI configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TuiConfig {
    /// Prefix key combination (e.g., "ctrl-a", "ctrl-b").
    #[serde(default = "default_prefix_key")]
    pub prefix_key: String,
}

/// Memory injection mode.
///
/// Controls how memories are injected into agent context.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InjectMode {
    /// Ralph automatically injects memories at the start of each iteration.
    #[default]
    Auto,
    /// Agent must explicitly run `ralph memory search` to access memories.
    Manual,
    /// Memories feature is disabled.
    None,
}

impl std::fmt::Display for InjectMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Auto => write!(f, "auto"),
            Self::Manual => write!(f, "manual"),
            Self::None => write!(f, "none"),
        }
    }
}

/// Memories configuration.
///
/// Controls the persistent learning system that allows Ralph to accumulate
/// wisdom across sessions. Memories are stored in `.ralph/agent/memories.md`.
///
/// When enabled, the memories skill is automatically injected to teach
/// agents how to create and search memories (skill injection is implicit).
///
/// Example configuration:
/// ```yaml
/// memories:
///   enabled: true
///   inject: auto
///   budget: 2000
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoriesConfig {
    /// Whether the memories feature is enabled.
    ///
    /// When true, memories are injected and the skill is taught to the agent.
    #[serde(default)]
    pub enabled: bool,

    /// How memories are injected into agent context.
    #[serde(default)]
    pub inject: InjectMode,

    /// Maximum tokens to inject (0 = unlimited).
    ///
    /// When set, memories are truncated to fit within this budget.
    #[serde(default)]
    pub budget: usize,

    /// Filter configuration for memory injection.
    #[serde(default)]
    pub filter: MemoriesFilter,
}

impl Default for MemoriesConfig {
    fn default() -> Self {
        Self {
            enabled: true, // Memories enabled by default
            inject: InjectMode::Auto,
            budget: 0,
            filter: MemoriesFilter::default(),
        }
    }
}

/// Filter configuration for memory injection.
///
/// Controls which memories are included when priming context.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MemoriesFilter {
    /// Filter by memory types (empty = all types).
    #[serde(default)]
    pub types: Vec<String>,

    /// Filter by tags (empty = all tags).
    #[serde(default)]
    pub tags: Vec<String>,

    /// Only include memories from the last N days (0 = no time limit).
    #[serde(default)]
    pub recent: u32,
}

/// Tasks configuration.
///
/// Controls the runtime task tracking system that allows Ralph to manage
/// work items across iterations. Tasks are stored in `.ralph/agent/tasks.jsonl`.
///
/// When enabled, tasks replace scratchpad for loop completion verification.
///
/// Example configuration:
/// ```yaml
/// tasks:
///   enabled: true
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TasksConfig {
    /// Whether the tasks feature is enabled.
    ///
    /// When true, tasks are used for loop completion verification.
    #[serde(default = "default_true")]
    pub enabled: bool,
}

impl Default for TasksConfig {
    fn default() -> Self {
        Self {
            enabled: true, // Tasks enabled by default
        }
    }
}

/// Skills configuration.
///
/// Controls the skill discovery and injection system that makes tool
/// knowledge and domain expertise available to agents during loops.
///
/// Skills use a two-tier injection model: a compact skill index is always
/// present in every prompt, and the agent loads full skill content on demand
/// via `ralph tools skill load <name>`.
///
/// Example configuration:
/// ```yaml
/// skills:
///   enabled: true
///   dirs:
///     - ".claude/skills"
///   overrides:
///     pdd:
///       enabled: false
///     memories:
///       auto_inject: true
///       hats: ["ralph"]
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillsConfig {
    /// Whether the skills system is enabled.
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// Directories to scan for skill files.
    /// Relative paths resolved against workspace root.
    #[serde(default)]
    pub dirs: Vec<PathBuf>,

    /// Per-skill overrides keyed by skill name.
    #[serde(default)]
    pub overrides: HashMap<String, SkillOverride>,
}

impl Default for SkillsConfig {
    fn default() -> Self {
        Self {
            enabled: true, // Skills enabled by default
            dirs: vec![],
            overrides: HashMap::new(),
        }
    }
}

/// Per-skill configuration override.
///
/// Allows enabling/disabling individual skills and overriding their
/// frontmatter fields (hats, backends, tags, auto_inject).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SkillOverride {
    /// Disable a discovered skill.
    #[serde(default)]
    pub enabled: Option<bool>,

    /// Restrict skill to specific hats.
    #[serde(default)]
    pub hats: Vec<String>,

    /// Restrict skill to specific backends.
    #[serde(default)]
    pub backends: Vec<String>,

    /// Tags for categorization.
    #[serde(default)]
    pub tags: Vec<String>,

    /// Inject full content into prompt (not just index entry).
    #[serde(default)]
    pub auto_inject: Option<bool>,
}

/// Preflight check configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PreflightConfig {
    /// Whether to run preflight checks before `ralph run`.
    #[serde(default)]
    pub enabled: bool,

    /// Whether to treat warnings as failures.
    #[serde(default)]
    pub strict: bool,

    /// Specific checks to skip (by name). Empty = run all checks.
    #[serde(default)]
    pub skip: Vec<String>,
}

/// Feature flags for optional Ralph capabilities.
///
/// Example configuration:
/// ```yaml
/// features:
///   parallel: true  # Enable parallel loops via git worktrees
///   auto_merge: false  # Auto-merge worktree branches on completion
///   preflight:
///     enabled: false      # Opt-in: run preflight checks before `ralph run`
///     strict: false       # Treat warnings as failures
///     skip: ["telegram"]  # Skip specific checks by name
///   loop_naming:
///     format: human-readable  # or "timestamp" for legacy format
///     max_length: 50
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeaturesConfig {
    /// Whether parallel loops are enabled.
    ///
    /// When true (default), if another loop holds the lock, Ralph spawns
    /// a parallel loop in a git worktree. When false, Ralph errors instead.
    #[serde(default = "default_true")]
    pub parallel: bool,

    /// Whether to automatically merge worktree branches on completion.
    ///
    /// When false (default), completed worktree loops queue for manual merge.
    /// When true, Ralph automatically merges the worktree branch into the
    /// main branch after a parallel loop completes.
    #[serde(default)]
    pub auto_merge: bool,

    /// Loop naming configuration for worktree branches.
    ///
    /// Controls how loop IDs are generated for parallel loops.
    /// Default uses human-readable format: `fix-header-swift-peacock`
    /// Legacy timestamp format: `ralph-YYYYMMDD-HHMMSS-XXXX`
    #[serde(default)]
    pub loop_naming: crate::loop_name::LoopNamingConfig,

    /// Preflight check configuration.
    #[serde(default)]
    pub preflight: PreflightConfig,
}

impl Default for FeaturesConfig {
    fn default() -> Self {
        Self {
            parallel: true,    // Parallel loops enabled by default
            auto_merge: false, // Auto-merge disabled by default for safety
            loop_naming: crate::loop_name::LoopNamingConfig::default(),
            preflight: PreflightConfig::default(),
        }
    }
}

fn default_prefix_key() -> String {
    "ctrl-a".to_string()
}

impl Default for TuiConfig {
    fn default() -> Self {
        Self {
            prefix_key: default_prefix_key(),
        }
    }
}

impl TuiConfig {
    /// Parses the prefix_key string into KeyCode and KeyModifiers.
    /// Returns an error if the format is invalid.
    pub fn parse_prefix(
        &self,
    ) -> Result<(crossterm::event::KeyCode, crossterm::event::KeyModifiers), String> {
        use crossterm::event::{KeyCode, KeyModifiers};

        let parts: Vec<&str> = self.prefix_key.split('-').collect();
        if parts.len() != 2 {
            return Err(format!(
                "Invalid prefix_key format: '{}'. Expected format: 'ctrl-<key>' (e.g., 'ctrl-a', 'ctrl-b')",
                self.prefix_key
            ));
        }

        let modifier = match parts[0].to_lowercase().as_str() {
            "ctrl" => KeyModifiers::CONTROL,
            _ => {
                return Err(format!(
                    "Invalid modifier: '{}'. Only 'ctrl' is supported (e.g., 'ctrl-a')",
                    parts[0]
                ));
            }
        };

        let key_str = parts[1];
        if key_str.len() != 1 {
            return Err(format!(
                "Invalid key: '{}'. Expected a single character (e.g., 'a', 'b')",
                key_str
            ));
        }

        let key_char = key_str.chars().next().unwrap();
        let key_code = KeyCode::Char(key_char);

        Ok((key_code, modifier))
    }
}

/// Metadata for an event topic.
///
/// Defines what an event means, enabling auto-derived instructions for hats.
/// When a hat triggers on or publishes an event, this metadata is used to
/// generate appropriate behavior instructions.
///
/// Example:
/// ```yaml
/// events:
///   deploy.start:
///     description: "Deployment has been requested"
///     on_trigger: "Prepare artifacts, validate config, check dependencies"
///     on_publish: "Signal that deployment should begin"
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EventMetadata {
    /// Brief description of what this event represents.
    #[serde(default)]
    pub description: String,

    /// Instructions for a hat that triggers on (receives) this event.
    /// Describes what the hat should do when it receives this event.
    #[serde(default)]
    pub on_trigger: String,

    /// Instructions for a hat that publishes (emits) this event.
    /// Describes when/how the hat should emit this event.
    #[serde(default)]
    pub on_publish: String,
}

/// Backend configuration for a hat.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum HatBackend {
    // Order matters for serde untagged - most specific first
    /// Kiro agent with custom agent name and optional args.
    KiroAgent {
        #[serde(rename = "type")]
        backend_type: String,
        agent: String,
        #[serde(default)]
        args: Vec<String>,
    },
    /// Named backend with args (has `type` but no `agent`).
    NamedWithArgs {
        #[serde(rename = "type")]
        backend_type: String,
        #[serde(default)]
        args: Vec<String>,
    },
    /// Simple named backend (string form).
    Named(String),
    /// Custom backend with command and args.
    Custom {
        command: String,
        #[serde(default)]
        args: Vec<String>,
    },
}

impl HatBackend {
    /// Converts to CLI backend string for execution.
    pub fn to_cli_backend(&self) -> String {
        match self {
            HatBackend::Named(name) => name.clone(),
            HatBackend::NamedWithArgs { backend_type, .. } => backend_type.clone(),
            HatBackend::KiroAgent { .. } => "kiro".to_string(),
            HatBackend::Custom { .. } => "custom".to_string(),
        }
    }
}

/// Configuration for a single hat.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HatConfig {
    /// Human-readable name for the hat.
    pub name: String,

    /// Short description of the hat's purpose (required).
    /// Used in the HATS table to help Ralph understand when to delegate to this hat.
    pub description: Option<String>,

    /// Events that trigger this hat to be worn.
    /// Per spec: "Hats define triggers — which events cause Ralph to wear this hat."
    #[serde(default)]
    pub triggers: Vec<String>,

    /// Topics this hat publishes.
    #[serde(default)]
    pub publishes: Vec<String>,

    /// Instructions prepended to prompts.
    #[serde(default)]
    pub instructions: String,

    /// Additional instruction fragments appended to `instructions`.
    ///
    /// Use with YAML anchors to share common instruction blocks across hats:
    /// ```yaml
    /// _confidence_protocol: &confidence_protocol |
    ///   ### Confidence-Based Decision Protocol
    ///   ...
    ///
    /// hats:
    ///   architect:
    ///     instructions: |
    ///       ## ARCHITECT MODE
    ///       ...
    ///     extra_instructions:
    ///       - *confidence_protocol
    /// ```
    #[serde(default)]
    pub extra_instructions: Vec<String>,

    /// Backend to use for this hat (inherits from cli.backend if not specified).
    #[serde(default)]
    pub backend: Option<HatBackend>,

    /// Default event to publish if hat forgets to write an event.
    #[serde(default)]
    pub default_publishes: Option<String>,

    /// Maximum number of times this hat may be activated in a single loop run.
    ///
    /// When the limit is exceeded, the orchestrator publishes `<hat_id>.exhausted`
    /// instead of activating the hat again.
    pub max_activations: Option<u32>,
}

impl HatConfig {
    /// Converts trigger strings to Topic objects.
    pub fn trigger_topics(&self) -> Vec<Topic> {
        self.triggers.iter().map(|s| Topic::new(s)).collect()
    }

    /// Converts publish strings to Topic objects.
    pub fn publish_topics(&self) -> Vec<Topic> {
        self.publishes.iter().map(|s| Topic::new(s)).collect()
    }
}

/// RObot (Ralph-Orchestrator bot) configuration.
///
/// Enables bidirectional communication between AI agents and humans
/// during orchestration loops. When enabled, agents can emit `human.interact`
/// events to request clarification (blocking the loop), and humans can
/// send proactive guidance via Telegram.
///
/// Example configuration:
/// ```yaml
/// RObot:
///   enabled: true
///   timeout_seconds: 300
///   checkin_interval_seconds: 120  # Optional: send status every 2 min
///   telegram:
///     bot_token: "..."  # Or set RALPH_TELEGRAM_BOT_TOKEN env var
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RobotConfig {
    /// Whether the RObot is enabled.
    #[serde(default)]
    pub enabled: bool,

    /// Timeout in seconds for waiting on human responses.
    /// Required when enabled (no default — must be explicit).
    pub timeout_seconds: Option<u64>,

    /// Interval in seconds between periodic check-in messages sent via Telegram.
    /// When set, Ralph sends a status message every N seconds so the human
    /// knows it's still working. If `None`, no check-ins are sent.
    pub checkin_interval_seconds: Option<u64>,

    /// Telegram bot configuration.
    #[serde(default)]
    pub telegram: Option<TelegramBotConfig>,
}

impl RobotConfig {
    /// Validates the RObot config. Returns an error if enabled but misconfigured.
    pub fn validate(&self) -> Result<(), ConfigError> {
        if !self.enabled {
            return Ok(());
        }

        if self.timeout_seconds.is_none() {
            return Err(ConfigError::RobotMissingField {
                field: "RObot.timeout_seconds".to_string(),
                hint: "timeout_seconds is required when RObot is enabled".to_string(),
            });
        }

        // Bot token must be available from config, keychain, or env var
        if self.resolve_bot_token().is_none() {
            return Err(ConfigError::RobotMissingField {
                field: "RObot.telegram.bot_token".to_string(),
                hint: "Run `ralph bot onboard --telegram`, set RALPH_TELEGRAM_BOT_TOKEN env var, or set RObot.telegram.bot_token in config"
                    .to_string(),
            });
        }

        Ok(())
    }

    /// Resolves the bot token from multiple sources.
    ///
    /// Resolution order (highest to lowest priority):
    /// 1. `RALPH_TELEGRAM_BOT_TOKEN` environment variable
    /// 2. `RObot.telegram.bot_token` in config file (explicit project override)
    /// 3. OS keychain (service: "ralph", user: "telegram-bot-token")
    pub fn resolve_bot_token(&self) -> Option<String> {
        // 1. Env var (highest priority)
        let env_token = std::env::var("RALPH_TELEGRAM_BOT_TOKEN").ok();
        let config_token = self
            .telegram
            .as_ref()
            .and_then(|telegram| telegram.bot_token.clone());

        if cfg!(test) {
            return env_token.or(config_token);
        }

        env_token
            // 2. Config file (explicit override)
            .or(config_token)
            // 3. OS keychain (best effort)
            .or_else(|| {
                std::panic::catch_unwind(|| {
                    keyring::Entry::new("ralph", "telegram-bot-token")
                        .ok()
                        .and_then(|e| e.get_password().ok())
                })
                .ok()
                .flatten()
            })
    }
}

/// Telegram bot configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelegramBotConfig {
    /// Bot token. Optional if `RALPH_TELEGRAM_BOT_TOKEN` env var is set.
    pub bot_token: Option<String>,
}

/// Configuration errors.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("YAML parse error: {0}")]
    Yaml(#[from] serde_yaml::Error),

    #[error(
        "Ambiguous routing: trigger '{trigger}' is claimed by both '{hat1}' and '{hat2}'.\nFix: ensure only one hat claims this trigger or delegate with a new event.\nSee: docs/reference/troubleshooting.md#ambiguous-routing"
    )]
    AmbiguousRouting {
        trigger: String,
        hat1: String,
        hat2: String,
    },

    #[error(
        "Mutually exclusive fields: '{field1}' and '{field2}' cannot both be specified.\nFix: remove one field or split into separate configs.\nSee: docs/reference/troubleshooting.md#mutually-exclusive-fields"
    )]
    MutuallyExclusive { field1: String, field2: String },

    #[error("Invalid completion_promise: must be non-empty and non-whitespace")]
    InvalidCompletionPromise,

    #[error(
        "Custom backend requires a command.\nFix: set 'cli.command' in your config (or run `ralph init --backend custom`).\nSee: docs/reference/troubleshooting.md#custom-backend-command"
    )]
    CustomBackendRequiresCommand,

    #[error(
        "Reserved trigger '{trigger}' used by hat '{hat}' - task.start and task.resume are reserved for Ralph (the coordinator). Use a delegated event like 'work.start' instead.\nSee: docs/reference/troubleshooting.md#reserved-trigger"
    )]
    ReservedTrigger { trigger: String, hat: String },

    #[error(
        "Hat '{hat}' is missing required 'description' field - add a short description of the hat's purpose.\nSee: docs/reference/troubleshooting.md#missing-hat-description"
    )]
    MissingDescription { hat: String },

    #[error(
        "RObot config error: {field} - {hint}\nSee: docs/reference/troubleshooting.md#robot-config"
    )]
    RobotMissingField { field: String, hint: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = RalphConfig::default();
        // Default config has no custom hats (uses default planner+builder)
        assert!(config.hats.is_empty());
        assert_eq!(config.event_loop.max_iterations, 100);
        assert!(!config.verbose);
        assert!(!config.features.preflight.enabled);
        assert!(!config.features.preflight.strict);
        assert!(config.features.preflight.skip.is_empty());
    }

    #[test]
    fn test_parse_yaml_with_custom_hats() {
        let yaml = r#"
event_loop:
  prompt_file: "TASK.md"
  completion_promise: "DONE"
  max_iterations: 50
cli:
  backend: "claude"
hats:
  implementer:
    name: "Implementer"
    triggers: ["task.*", "review.done"]
    publishes: ["impl.done"]
    instructions: "You are the implementation agent."
"#;
        let config: RalphConfig = serde_yaml::from_str(yaml).unwrap();
        // Custom hats are defined
        assert_eq!(config.hats.len(), 1);
        assert_eq!(config.event_loop.prompt_file, "TASK.md");

        let hat = config.hats.get("implementer").unwrap();
        assert_eq!(hat.triggers.len(), 2);
    }

    #[test]
    fn test_preflight_config_deserialize() {
        let yaml = r#"
features:
  preflight:
    enabled: true
    strict: true
    skip: ["telegram", "git"]
"#;
        let config: RalphConfig = serde_yaml::from_str(yaml).unwrap();
        assert!(config.features.preflight.enabled);
        assert!(config.features.preflight.strict);
        assert_eq!(
            config.features.preflight.skip,
            vec!["telegram".to_string(), "git".to_string()]
        );
    }

    #[test]
    fn test_parse_yaml_v1_format() {
        // V1 flat format - identical to Python v1.x config
        let yaml = r#"
agent: gemini
prompt_file: "TASK.md"
completion_promise: "RALPH_DONE"
max_iterations: 75
max_runtime: 7200
max_cost: 10.0
verbose: true
"#;
        let mut config: RalphConfig = serde_yaml::from_str(yaml).unwrap();

        // Before normalization, v2 fields have defaults
        assert_eq!(config.cli.backend, "claude"); // default
        assert_eq!(config.event_loop.max_iterations, 100); // default

        // Normalize v1 -> v2
        config.normalize();

        // After normalization, v2 fields have v1 values
        assert_eq!(config.cli.backend, "gemini");
        assert_eq!(config.event_loop.prompt_file, "TASK.md");
        assert_eq!(config.event_loop.completion_promise, "RALPH_DONE");
        assert_eq!(config.event_loop.max_iterations, 75);
        assert_eq!(config.event_loop.max_runtime_seconds, 7200);
        assert_eq!(config.event_loop.max_cost_usd, Some(10.0));
        assert!(config.verbose);
    }

    #[test]
    fn test_agent_priority() {
        let yaml = r"
agent: auto
agent_priority: [gemini, claude, codex]
";
        let config: RalphConfig = serde_yaml::from_str(yaml).unwrap();
        let priority = config.get_agent_priority();
        assert_eq!(priority, vec!["gemini", "claude", "codex"]);
    }

    #[test]
    fn test_default_agent_priority() {
        let config = RalphConfig::default();
        let priority = config.get_agent_priority();
        assert_eq!(priority, vec!["claude", "kiro", "gemini", "codex", "amp"]);
    }

    #[test]
    fn test_validate_deferred_features() {
        let yaml = r"
archive_prompts: true
enable_metrics: true
";
        let config: RalphConfig = serde_yaml::from_str(yaml).unwrap();
        let warnings = config.validate().unwrap();

        assert_eq!(warnings.len(), 2);
        assert!(warnings
            .iter()
            .any(|w| matches!(w, ConfigWarning::DeferredFeature { field, .. } if field == "archive_prompts")));
        assert!(warnings
            .iter()
            .any(|w| matches!(w, ConfigWarning::DeferredFeature { field, .. } if field == "enable_metrics")));
    }

    #[test]
    fn test_validate_dropped_fields() {
        let yaml = r#"
max_tokens: 4096
retry_delay: 5
adapters:
  claude:
    tool_permissions: ["read", "write"]
"#;
        let config: RalphConfig = serde_yaml::from_str(yaml).unwrap();
        let warnings = config.validate().unwrap();

        assert_eq!(warnings.len(), 3);
        assert!(warnings.iter().any(
            |w| matches!(w, ConfigWarning::DroppedField { field, .. } if field == "max_tokens")
        ));
        assert!(warnings.iter().any(
            |w| matches!(w, ConfigWarning::DroppedField { field, .. } if field == "retry_delay")
        ));
        assert!(warnings
            .iter()
            .any(|w| matches!(w, ConfigWarning::DroppedField { field, .. } if field == "adapters.*.tool_permissions")));
    }

    #[test]
    fn test_suppress_warnings() {
        let yaml = r"
_suppress_warnings: true
archive_prompts: true
max_tokens: 4096
";
        let config: RalphConfig = serde_yaml::from_str(yaml).unwrap();
        let warnings = config.validate().unwrap();

        // All warnings should be suppressed
        assert!(warnings.is_empty());
    }

    #[test]
    fn test_adapter_settings() {
        let yaml = r"
adapters:
  claude:
    timeout: 600
    enabled: true
  gemini:
    timeout: 300
    enabled: false
";
        let config: RalphConfig = serde_yaml::from_str(yaml).unwrap();

        let claude = config.adapter_settings("claude");
        assert_eq!(claude.timeout, 600);
        assert!(claude.enabled);

        let gemini = config.adapter_settings("gemini");
        assert_eq!(gemini.timeout, 300);
        assert!(!gemini.enabled);
    }

    #[test]
    fn test_unknown_fields_ignored() {
        // Unknown fields should be silently ignored (forward compatibility)
        let yaml = r#"
agent: claude
unknown_field: "some value"
future_feature: true
"#;
        let result: Result<RalphConfig, _> = serde_yaml::from_str(yaml);
        // Should parse successfully, ignoring unknown fields
        assert!(result.is_ok());
    }

    #[test]
    fn test_ambiguous_routing_rejected() {
        // Per spec: "Every trigger maps to exactly one hat | No ambiguous routing"
        // Note: using semantic events since task.start is reserved
        let yaml = r#"
hats:
  planner:
    name: "Planner"
    description: "Plans tasks"
    triggers: ["planning.start", "build.done"]
  builder:
    name: "Builder"
    description: "Builds code"
    triggers: ["build.task", "build.done"]
"#;
        let config: RalphConfig = serde_yaml::from_str(yaml).unwrap();
        let result = config.validate();

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            matches!(&err, ConfigError::AmbiguousRouting { trigger, .. } if trigger == "build.done"),
            "Expected AmbiguousRouting error for 'build.done', got: {:?}",
            err
        );
    }

    #[test]
    fn test_unique_triggers_accepted() {
        // Valid config: each trigger maps to exactly one hat
        // Note: task.start is reserved for Ralph, so use semantic events
        let yaml = r#"
hats:
  planner:
    name: "Planner"
    description: "Plans tasks"
    triggers: ["planning.start", "build.done", "build.blocked"]
  builder:
    name: "Builder"
    description: "Builds code"
    triggers: ["build.task"]
"#;
        let config: RalphConfig = serde_yaml::from_str(yaml).unwrap();
        let result = config.validate();

        assert!(
            result.is_ok(),
            "Expected valid config, got: {:?}",
            result.unwrap_err()
        );
    }

    #[test]
    fn test_reserved_trigger_task_start_rejected() {
        // Per design: task.start is reserved for Ralph (the coordinator)
        let yaml = r#"
hats:
  my_hat:
    name: "My Hat"
    description: "Test hat"
    triggers: ["task.start"]
"#;
        let config: RalphConfig = serde_yaml::from_str(yaml).unwrap();
        let result = config.validate();

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            matches!(&err, ConfigError::ReservedTrigger { trigger, hat }
                if trigger == "task.start" && hat == "my_hat"),
            "Expected ReservedTrigger error for 'task.start', got: {:?}",
            err
        );
    }

    #[test]
    fn test_reserved_trigger_task_resume_rejected() {
        // Per design: task.resume is reserved for Ralph (the coordinator)
        let yaml = r#"
hats:
  my_hat:
    name: "My Hat"
    description: "Test hat"
    triggers: ["task.resume", "other.event"]
"#;
        let config: RalphConfig = serde_yaml::from_str(yaml).unwrap();
        let result = config.validate();

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            matches!(&err, ConfigError::ReservedTrigger { trigger, hat }
                if trigger == "task.resume" && hat == "my_hat"),
            "Expected ReservedTrigger error for 'task.resume', got: {:?}",
            err
        );
    }

    #[test]
    fn test_missing_description_rejected() {
        // Description is required for all hats
        let yaml = r#"
hats:
  my_hat:
    name: "My Hat"
    triggers: ["build.task"]
"#;
        let config: RalphConfig = serde_yaml::from_str(yaml).unwrap();
        let result = config.validate();

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            matches!(&err, ConfigError::MissingDescription { hat } if hat == "my_hat"),
            "Expected MissingDescription error, got: {:?}",
            err
        );
    }

    #[test]
    fn test_empty_description_rejected() {
        // Empty description should also be rejected
        let yaml = r#"
hats:
  my_hat:
    name: "My Hat"
    description: "   "
    triggers: ["build.task"]
"#;
        let config: RalphConfig = serde_yaml::from_str(yaml).unwrap();
        let result = config.validate();

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            matches!(&err, ConfigError::MissingDescription { hat } if hat == "my_hat"),
            "Expected MissingDescription error for empty description, got: {:?}",
            err
        );
    }

    #[test]
    fn test_core_config_defaults() {
        let config = RalphConfig::default();
        assert_eq!(config.core.scratchpad, ".ralph/agent/scratchpad.md");
        assert_eq!(config.core.specs_dir, ".ralph/specs/");
        // Default guardrails per spec
        assert_eq!(config.core.guardrails.len(), 5);
        assert!(config.core.guardrails[0].contains("Fresh context"));
        assert!(config.core.guardrails[1].contains("search first"));
        assert!(config.core.guardrails[2].contains("Backpressure"));
        assert!(config.core.guardrails[3].contains("Confidence protocol"));
        assert!(config.core.guardrails[4].contains("Commit atomically"));
    }

    #[test]
    fn test_core_config_customizable() {
        let yaml = r#"
core:
  scratchpad: ".workspace/plan.md"
  specs_dir: "./specifications/"
"#;
        let config: RalphConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.core.scratchpad, ".workspace/plan.md");
        assert_eq!(config.core.specs_dir, "./specifications/");
        // Guardrails should use defaults when not specified
        assert_eq!(config.core.guardrails.len(), 5);
    }

    #[test]
    fn test_core_config_custom_guardrails() {
        let yaml = r#"
core:
  scratchpad: ".ralph/agent/scratchpad.md"
  specs_dir: "./specs/"
  guardrails:
    - "Custom rule one"
    - "Custom rule two"
"#;
        let config: RalphConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.core.guardrails.len(), 2);
        assert_eq!(config.core.guardrails[0], "Custom rule one");
        assert_eq!(config.core.guardrails[1], "Custom rule two");
    }

    #[test]
    fn test_prompt_and_prompt_file_mutually_exclusive() {
        // Both prompt and prompt_file specified in config should error
        let yaml = r#"
event_loop:
  prompt: "inline text"
  prompt_file: "custom.md"
"#;
        let config: RalphConfig = serde_yaml::from_str(yaml).unwrap();
        let result = config.validate();

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            matches!(&err, ConfigError::MutuallyExclusive { field1, field2 }
                if field1 == "event_loop.prompt" && field2 == "event_loop.prompt_file"),
            "Expected MutuallyExclusive error, got: {:?}",
            err
        );
    }

    #[test]
    fn test_prompt_with_default_prompt_file_allowed() {
        // Having inline prompt with default prompt_file value should be OK
        let yaml = r#"
event_loop:
  prompt: "inline text"
"#;
        let config: RalphConfig = serde_yaml::from_str(yaml).unwrap();
        let result = config.validate();

        assert!(
            result.is_ok(),
            "Should allow inline prompt with default prompt_file"
        );
        assert_eq!(config.event_loop.prompt, Some("inline text".to_string()));
        assert_eq!(config.event_loop.prompt_file, "PROMPT.md");
    }

    #[test]
    fn test_custom_backend_requires_command() {
        // Custom backend without command should error
        let yaml = r#"
cli:
  backend: "custom"
"#;
        let config: RalphConfig = serde_yaml::from_str(yaml).unwrap();
        let result = config.validate();

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            matches!(&err, ConfigError::CustomBackendRequiresCommand),
            "Expected CustomBackendRequiresCommand error, got: {:?}",
            err
        );
    }

    #[test]
    fn test_empty_completion_promise_rejected() {
        let yaml = r#"
event_loop:
  completion_promise: "   "
"#;
        let config: RalphConfig = serde_yaml::from_str(yaml).unwrap();
        let result = config.validate();

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            matches!(&err, ConfigError::InvalidCompletionPromise),
            "Expected InvalidCompletionPromise error, got: {:?}",
            err
        );
    }

    #[test]
    fn test_custom_backend_with_empty_command_errors() {
        // Custom backend with empty command should error
        let yaml = r#"
cli:
  backend: "custom"
  command: ""
"#;
        let config: RalphConfig = serde_yaml::from_str(yaml).unwrap();
        let result = config.validate();

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            matches!(&err, ConfigError::CustomBackendRequiresCommand),
            "Expected CustomBackendRequiresCommand error, got: {:?}",
            err
        );
    }

    #[test]
    fn test_custom_backend_with_command_succeeds() {
        // Custom backend with valid command should pass validation
        let yaml = r#"
cli:
  backend: "custom"
  command: "my-agent"
"#;
        let config: RalphConfig = serde_yaml::from_str(yaml).unwrap();
        let result = config.validate();

        assert!(
            result.is_ok(),
            "Should allow custom backend with command: {:?}",
            result.unwrap_err()
        );
    }

    #[test]
    fn test_custom_backend_requires_command_message_actionable() {
        let err = ConfigError::CustomBackendRequiresCommand;
        let msg = err.to_string();
        assert!(msg.contains("cli.command"));
        assert!(msg.contains("ralph init --backend custom"));
        assert!(msg.contains("docs/reference/troubleshooting.md#custom-backend-command"));
    }

    #[test]
    fn test_reserved_trigger_message_actionable() {
        let err = ConfigError::ReservedTrigger {
            trigger: "task.start".to_string(),
            hat: "builder".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("Reserved trigger"));
        assert!(msg.contains("docs/reference/troubleshooting.md#reserved-trigger"));
    }

    #[test]
    fn test_prompt_file_with_no_inline_allowed() {
        // Having only prompt_file specified should be OK
        let yaml = r#"
event_loop:
  prompt_file: "custom.md"
"#;
        let config: RalphConfig = serde_yaml::from_str(yaml).unwrap();
        let result = config.validate();

        assert!(
            result.is_ok(),
            "Should allow prompt_file without inline prompt"
        );
        assert_eq!(config.event_loop.prompt, None);
        assert_eq!(config.event_loop.prompt_file, "custom.md");
    }

    #[test]
    fn test_default_prompt_file_value() {
        let config = RalphConfig::default();
        assert_eq!(config.event_loop.prompt_file, "PROMPT.md");
        assert_eq!(config.event_loop.prompt, None);
    }

    #[test]
    fn test_tui_config_default() {
        let config = RalphConfig::default();
        assert_eq!(config.tui.prefix_key, "ctrl-a");
    }

    #[test]
    fn test_tui_config_parse_ctrl_b() {
        let yaml = r#"
tui:
  prefix_key: "ctrl-b"
"#;
        let config: RalphConfig = serde_yaml::from_str(yaml).unwrap();
        let (key_code, key_modifiers) = config.tui.parse_prefix().unwrap();

        use crossterm::event::{KeyCode, KeyModifiers};
        assert_eq!(key_code, KeyCode::Char('b'));
        assert_eq!(key_modifiers, KeyModifiers::CONTROL);
    }

    #[test]
    fn test_tui_config_parse_invalid_format() {
        let tui_config = TuiConfig {
            prefix_key: "invalid".to_string(),
        };
        let result = tui_config.parse_prefix();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid prefix_key format"));
    }

    #[test]
    fn test_tui_config_parse_invalid_modifier() {
        let tui_config = TuiConfig {
            prefix_key: "alt-a".to_string(),
        };
        let result = tui_config.parse_prefix();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid modifier"));
    }

    #[test]
    fn test_tui_config_parse_invalid_key() {
        let tui_config = TuiConfig {
            prefix_key: "ctrl-abc".to_string(),
        };
        let result = tui_config.parse_prefix();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid key"));
    }

    #[test]
    fn test_hat_backend_named() {
        let yaml = r#""claude""#;
        let backend: HatBackend = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(backend.to_cli_backend(), "claude");
        match backend {
            HatBackend::Named(name) => assert_eq!(name, "claude"),
            _ => panic!("Expected Named variant"),
        }
    }

    #[test]
    fn test_hat_backend_kiro_agent() {
        let yaml = r#"
type: "kiro"
agent: "builder"
"#;
        let backend: HatBackend = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(backend.to_cli_backend(), "kiro");
        match backend {
            HatBackend::KiroAgent {
                backend_type,
                agent,
                args,
            } => {
                assert_eq!(backend_type, "kiro");
                assert_eq!(agent, "builder");
                assert!(args.is_empty());
            }
            _ => panic!("Expected KiroAgent variant"),
        }
    }

    #[test]
    fn test_hat_backend_kiro_agent_with_args() {
        let yaml = r#"
type: "kiro"
agent: "builder"
args: ["--verbose", "--debug"]
"#;
        let backend: HatBackend = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(backend.to_cli_backend(), "kiro");
        match backend {
            HatBackend::KiroAgent {
                backend_type,
                agent,
                args,
            } => {
                assert_eq!(backend_type, "kiro");
                assert_eq!(agent, "builder");
                assert_eq!(args, vec!["--verbose", "--debug"]);
            }
            _ => panic!("Expected KiroAgent variant"),
        }
    }

    #[test]
    fn test_hat_backend_named_with_args() {
        let yaml = r#"
type: "claude"
args: ["--model", "claude-sonnet-4"]
"#;
        let backend: HatBackend = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(backend.to_cli_backend(), "claude");
        match backend {
            HatBackend::NamedWithArgs { backend_type, args } => {
                assert_eq!(backend_type, "claude");
                assert_eq!(args, vec!["--model", "claude-sonnet-4"]);
            }
            _ => panic!("Expected NamedWithArgs variant"),
        }
    }

    #[test]
    fn test_hat_backend_named_with_args_empty() {
        // type: claude without args should still work (NamedWithArgs with empty args)
        let yaml = r#"
type: "gemini"
"#;
        let backend: HatBackend = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(backend.to_cli_backend(), "gemini");
        match backend {
            HatBackend::NamedWithArgs { backend_type, args } => {
                assert_eq!(backend_type, "gemini");
                assert!(args.is_empty());
            }
            _ => panic!("Expected NamedWithArgs variant"),
        }
    }

    #[test]
    fn test_hat_backend_custom() {
        let yaml = r#"
command: "/usr/bin/my-agent"
args: ["--flag", "value"]
"#;
        let backend: HatBackend = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(backend.to_cli_backend(), "custom");
        match backend {
            HatBackend::Custom { command, args } => {
                assert_eq!(command, "/usr/bin/my-agent");
                assert_eq!(args, vec!["--flag", "value"]);
            }
            _ => panic!("Expected Custom variant"),
        }
    }

    #[test]
    fn test_hat_config_with_backend() {
        let yaml = r#"
name: "Custom Builder"
triggers: ["build.task"]
publishes: ["build.done"]
instructions: "Build stuff"
backend: "gemini"
default_publishes: "task.done"
"#;
        let hat: HatConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(hat.name, "Custom Builder");
        assert!(hat.backend.is_some());
        match hat.backend.unwrap() {
            HatBackend::Named(name) => assert_eq!(name, "gemini"),
            _ => panic!("Expected Named backend"),
        }
        assert_eq!(hat.default_publishes, Some("task.done".to_string()));
    }

    #[test]
    fn test_hat_config_without_backend() {
        let yaml = r#"
name: "Default Hat"
triggers: ["task.start"]
publishes: ["task.done"]
instructions: "Do work"
"#;
        let hat: HatConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(hat.name, "Default Hat");
        assert!(hat.backend.is_none());
        assert!(hat.default_publishes.is_none());
    }

    #[test]
    fn test_mixed_backends_config() {
        let yaml = r#"
event_loop:
  prompt_file: "TASK.md"
  max_iterations: 50

cli:
  backend: "claude"

hats:
  planner:
    name: "Planner"
    triggers: ["task.start"]
    publishes: ["build.task"]
    instructions: "Plan the work"
    backend: "claude"
    
  builder:
    name: "Builder"
    triggers: ["build.task"]
    publishes: ["build.done"]
    instructions: "Build the thing"
    backend:
      type: "kiro"
      agent: "builder"
      
  reviewer:
    name: "Reviewer"
    triggers: ["build.done"]
    publishes: ["review.complete"]
    instructions: "Review the work"
    backend:
      command: "/usr/local/bin/custom-agent"
      args: ["--mode", "review"]
    default_publishes: "review.complete"
"#;
        let config: RalphConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.hats.len(), 3);

        // Check planner (Named backend)
        let planner = config.hats.get("planner").unwrap();
        assert!(planner.backend.is_some());
        match planner.backend.as_ref().unwrap() {
            HatBackend::Named(name) => assert_eq!(name, "claude"),
            _ => panic!("Expected Named backend for planner"),
        }

        // Check builder (KiroAgent backend)
        let builder = config.hats.get("builder").unwrap();
        assert!(builder.backend.is_some());
        match builder.backend.as_ref().unwrap() {
            HatBackend::KiroAgent {
                backend_type,
                agent,
                args,
            } => {
                assert_eq!(backend_type, "kiro");
                assert_eq!(agent, "builder");
                assert!(args.is_empty());
            }
            _ => panic!("Expected KiroAgent backend for builder"),
        }

        // Check reviewer (Custom backend)
        let reviewer = config.hats.get("reviewer").unwrap();
        assert!(reviewer.backend.is_some());
        match reviewer.backend.as_ref().unwrap() {
            HatBackend::Custom { command, args } => {
                assert_eq!(command, "/usr/local/bin/custom-agent");
                assert_eq!(args, &vec!["--mode".to_string(), "review".to_string()]);
            }
            _ => panic!("Expected Custom backend for reviewer"),
        }
        assert_eq!(
            reviewer.default_publishes,
            Some("review.complete".to_string())
        );
    }

    #[test]
    fn test_features_config_auto_merge_defaults_to_false() {
        // Per spec: auto_merge should default to false for safety
        // This prevents automatic merging of parallel loop branches
        let config = RalphConfig::default();
        assert!(
            !config.features.auto_merge,
            "auto_merge should default to false"
        );
    }

    #[test]
    fn test_features_config_auto_merge_from_yaml() {
        // Users can opt into auto_merge via config
        let yaml = r"
features:
  auto_merge: true
";
        let config: RalphConfig = serde_yaml::from_str(yaml).unwrap();
        assert!(
            config.features.auto_merge,
            "auto_merge should be true when configured"
        );
    }

    #[test]
    fn test_features_config_auto_merge_false_from_yaml() {
        // Explicit false should work too
        let yaml = r"
features:
  auto_merge: false
";
        let config: RalphConfig = serde_yaml::from_str(yaml).unwrap();
        assert!(
            !config.features.auto_merge,
            "auto_merge should be false when explicitly configured"
        );
    }

    #[test]
    fn test_features_config_preserves_parallel_when_adding_auto_merge() {
        // Ensure adding auto_merge doesn't break existing parallel feature
        let yaml = r"
features:
  parallel: false
  auto_merge: true
";
        let config: RalphConfig = serde_yaml::from_str(yaml).unwrap();
        assert!(!config.features.parallel, "parallel should be false");
        assert!(config.features.auto_merge, "auto_merge should be true");
    }

    #[test]
    fn test_skills_config_defaults_when_absent() {
        // Configs without a skills: section should still parse (backwards compat)
        let yaml = r"
agent: claude
";
        let config: RalphConfig = serde_yaml::from_str(yaml).unwrap();
        assert!(config.skills.enabled);
        assert!(config.skills.dirs.is_empty());
        assert!(config.skills.overrides.is_empty());
    }

    #[test]
    fn test_skills_config_deserializes_all_fields() {
        let yaml = r#"
skills:
  enabled: true
  dirs:
    - ".claude/skills"
    - "/shared/skills"
  overrides:
    pdd:
      enabled: false
    memories:
      auto_inject: true
      hats: ["ralph"]
      backends: ["claude"]
      tags: ["core"]
"#;
        let config: RalphConfig = serde_yaml::from_str(yaml).unwrap();
        assert!(config.skills.enabled);
        assert_eq!(config.skills.dirs.len(), 2);
        assert_eq!(
            config.skills.dirs[0],
            std::path::PathBuf::from(".claude/skills")
        );
        assert_eq!(config.skills.overrides.len(), 2);

        let pdd = config.skills.overrides.get("pdd").unwrap();
        assert_eq!(pdd.enabled, Some(false));

        let memories = config.skills.overrides.get("memories").unwrap();
        assert_eq!(memories.auto_inject, Some(true));
        assert_eq!(memories.hats, vec!["ralph"]);
        assert_eq!(memories.backends, vec!["claude"]);
        assert_eq!(memories.tags, vec!["core"]);
    }

    #[test]
    fn test_skills_config_disabled() {
        let yaml = r"
skills:
  enabled: false
";
        let config: RalphConfig = serde_yaml::from_str(yaml).unwrap();
        assert!(!config.skills.enabled);
        assert!(config.skills.dirs.is_empty());
    }

    #[test]
    fn test_skill_override_partial_fields() {
        let yaml = r#"
skills:
  overrides:
    my-skill:
      hats: ["builder", "reviewer"]
"#;
        let config: RalphConfig = serde_yaml::from_str(yaml).unwrap();
        let override_ = config.skills.overrides.get("my-skill").unwrap();
        assert_eq!(override_.enabled, None);
        assert_eq!(override_.auto_inject, None);
        assert_eq!(override_.hats, vec!["builder", "reviewer"]);
        assert!(override_.backends.is_empty());
        assert!(override_.tags.is_empty());
    }

    // ─────────────────────────────────────────────────────────────────────────
    // ROBOT CONFIG TESTS
    // ─────────────────────────────────────────────────────────────────────────

    #[test]
    fn test_robot_config_defaults_disabled() {
        let config = RalphConfig::default();
        assert!(!config.robot.enabled);
        assert!(config.robot.timeout_seconds.is_none());
        assert!(config.robot.telegram.is_none());
    }

    #[test]
    fn test_robot_config_absent_parses_as_default() {
        // Existing configs without RObot: section should still parse
        let yaml = r"
agent: claude
";
        let config: RalphConfig = serde_yaml::from_str(yaml).unwrap();
        assert!(!config.robot.enabled);
        assert!(config.robot.timeout_seconds.is_none());
    }

    #[test]
    fn test_robot_config_valid_full() {
        let yaml = r#"
RObot:
  enabled: true
  timeout_seconds: 300
  telegram:
    bot_token: "123456:ABC-DEF"
"#;
        let config: RalphConfig = serde_yaml::from_str(yaml).unwrap();
        assert!(config.robot.enabled);
        assert_eq!(config.robot.timeout_seconds, Some(300));
        let telegram = config.robot.telegram.as_ref().unwrap();
        assert_eq!(telegram.bot_token, Some("123456:ABC-DEF".to_string()));

        // Validation should pass
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_robot_config_disabled_skips_validation() {
        // Disabled RObot config should pass validation even with missing fields
        let yaml = r"
RObot:
  enabled: false
";
        let config: RalphConfig = serde_yaml::from_str(yaml).unwrap();
        assert!(!config.robot.enabled);
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_robot_config_enabled_missing_timeout_fails() {
        let yaml = r#"
RObot:
  enabled: true
  telegram:
    bot_token: "123456:ABC-DEF"
"#;
        let config: RalphConfig = serde_yaml::from_str(yaml).unwrap();
        let result = config.validate();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            matches!(&err, ConfigError::RobotMissingField { field, .. }
                if field == "RObot.timeout_seconds"),
            "Expected RobotMissingField for timeout_seconds, got: {:?}",
            err
        );
    }

    #[test]
    fn test_robot_config_enabled_missing_timeout_and_token_fails_on_timeout_first() {
        // Both timeout and token are missing, but timeout is checked first
        let robot = RobotConfig {
            enabled: true,
            timeout_seconds: None,
            checkin_interval_seconds: None,
            telegram: None,
        };
        let result = robot.validate();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            matches!(&err, ConfigError::RobotMissingField { field, .. }
                if field == "RObot.timeout_seconds"),
            "Expected timeout validation failure first, got: {:?}",
            err
        );
    }

    #[test]
    fn test_robot_config_resolve_bot_token_from_config() {
        // Config has a token — resolve_bot_token returns it
        // (env var behavior is tested separately via integration tests since
        // forbid(unsafe_code) prevents env var manipulation in unit tests)
        let config = RobotConfig {
            enabled: true,
            timeout_seconds: Some(300),
            checkin_interval_seconds: None,
            telegram: Some(TelegramBotConfig {
                bot_token: Some("config-token".to_string()),
            }),
        };

        // When RALPH_TELEGRAM_BOT_TOKEN is not set, config token is returned
        // (Can't set/unset env vars in tests due to forbid(unsafe_code))
        let resolved = config.resolve_bot_token();
        // The result depends on whether RALPH_TELEGRAM_BOT_TOKEN is set in the
        // test environment. We can at least assert it's Some.
        assert!(resolved.is_some());
    }

    #[test]
    fn test_robot_config_resolve_bot_token_none_without_config() {
        // No config token and no telegram section
        let config = RobotConfig {
            enabled: true,
            timeout_seconds: Some(300),
            checkin_interval_seconds: None,
            telegram: None,
        };

        // Without env var AND without config token, resolve returns None
        // (unless RALPH_TELEGRAM_BOT_TOKEN happens to be set in test env)
        let resolved = config.resolve_bot_token();
        if std::env::var("RALPH_TELEGRAM_BOT_TOKEN").is_err() {
            assert!(resolved.is_none());
        }
    }

    #[test]
    fn test_robot_config_validate_with_config_token() {
        // Validation passes when bot_token is in config
        let robot = RobotConfig {
            enabled: true,
            timeout_seconds: Some(300),
            checkin_interval_seconds: None,
            telegram: Some(TelegramBotConfig {
                bot_token: Some("test-token".to_string()),
            }),
        };
        assert!(robot.validate().is_ok());
    }

    #[test]
    fn test_robot_config_validate_missing_telegram_section() {
        // No telegram section at all and no env var → fails
        // (Skip if env var happens to be set)
        if std::env::var("RALPH_TELEGRAM_BOT_TOKEN").is_ok() {
            return;
        }

        let robot = RobotConfig {
            enabled: true,
            timeout_seconds: Some(300),
            checkin_interval_seconds: None,
            telegram: None,
        };
        let result = robot.validate();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            matches!(&err, ConfigError::RobotMissingField { field, .. }
                if field == "RObot.telegram.bot_token"),
            "Expected bot_token validation failure, got: {:?}",
            err
        );
    }

    #[test]
    fn test_robot_config_validate_empty_bot_token() {
        // telegram section present but bot_token is None
        // (Skip if env var happens to be set)
        if std::env::var("RALPH_TELEGRAM_BOT_TOKEN").is_ok() {
            return;
        }

        let robot = RobotConfig {
            enabled: true,
            timeout_seconds: Some(300),
            checkin_interval_seconds: None,
            telegram: Some(TelegramBotConfig { bot_token: None }),
        };
        let result = robot.validate();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            matches!(&err, ConfigError::RobotMissingField { field, .. }
                if field == "RObot.telegram.bot_token"),
            "Expected bot_token validation failure, got: {:?}",
            err
        );
    }

    #[test]
    fn test_extra_instructions_merged_during_normalize() {
        let yaml = r#"
_fragments:
  shared_protocol: &shared_protocol |
    ### Shared Protocol
    Follow this protocol.

hats:
  builder:
    name: "Builder"
    triggers: ["build.start"]
    instructions: |
      ## BUILDER MODE
      Build things.
    extra_instructions:
      - *shared_protocol
"#;
        let mut config: RalphConfig = serde_yaml::from_str(yaml).unwrap();
        let hat = config.hats.get("builder").unwrap();

        // Before normalize: extra_instructions has content, instructions does not include it
        assert_eq!(hat.extra_instructions.len(), 1);
        assert!(!hat.instructions.contains("Shared Protocol"));

        config.normalize();

        let hat = config.hats.get("builder").unwrap();
        // After normalize: extra_instructions drained, instructions includes the fragment
        assert!(hat.extra_instructions.is_empty());
        assert!(hat.instructions.contains("## BUILDER MODE"));
        assert!(hat.instructions.contains("### Shared Protocol"));
        assert!(hat.instructions.contains("Follow this protocol."));
    }

    #[test]
    fn test_extra_instructions_empty_by_default() {
        let yaml = r#"
hats:
  simple:
    name: "Simple"
    triggers: ["start"]
    instructions: "Do the thing."
"#;
        let config: RalphConfig = serde_yaml::from_str(yaml).unwrap();
        let hat = config.hats.get("simple").unwrap();
        assert!(hat.extra_instructions.is_empty());
    }
}
