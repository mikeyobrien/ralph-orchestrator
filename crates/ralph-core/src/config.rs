//! Configuration types for the Ralph Orchestrator.
//!
//! This module supports both v1.x flat configuration format and v2.0 nested format.
//! Users can switch from Python v1.x to Rust v2.0 with zero config changes.

use ralph_proto::Topic;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;
use tracing::debug;

/// Top-level configuration for Ralph Orchestrator.
///
/// Supports both v1.x flat format and v2.0 nested format:
/// - v1: `agent: claude`, `max_iterations: 100`
/// - v2: `cli: { backend: claude }`, `event_loop: { max_iterations: 100 }`
#[derive(Debug, Clone, Serialize, Deserialize)]
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

    /// V1 field: Path to prompt file (maps to event_loop.prompt_file).
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

    /// V1 field: Iterations between git checkpoints (maps to event_loop.checkpoint_interval).
    #[serde(default)]
    pub checkpoint_interval: Option<u32>,

    // ─────────────────────────────────────────────────────────────────────────
    // FEATURE FLAGS
    // ─────────────────────────────────────────────────────────────────────────

    /// Enable git checkpointing.
    #[serde(default = "default_true")]
    pub git_checkpoint: bool,

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
}

fn default_true() -> bool {
    true
}

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
            checkpoint_interval: None,
            // Feature flags
            git_checkpoint: true,
            verbose: false,
            archive_prompts: false,
            enable_metrics: false,
            // Dropped fields
            max_tokens: None,
            retry_delay: None,
            adapters: AdaptersConfig::default(),
            // Warning control
            suppress_warnings: false,
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
        let config: Self = serde_yaml::from_str(&content)?;
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
            debug!(from = "completion_promise", to = "event_loop.completion_promise", "Normalizing v1 field");
            self.event_loop.completion_promise = cp.clone();
            normalized_count += 1;
        }

        // Map v1 `max_iterations` to v2 `event_loop.max_iterations`
        if let Some(mi) = self.max_iterations {
            debug!(from = "max_iterations", to = "event_loop.max_iterations", value = mi, "Normalizing v1 field");
            self.event_loop.max_iterations = mi;
            normalized_count += 1;
        }

        // Map v1 `max_runtime` to v2 `event_loop.max_runtime_seconds`
        if let Some(mr) = self.max_runtime {
            debug!(from = "max_runtime", to = "event_loop.max_runtime_seconds", value = mr, "Normalizing v1 field");
            self.event_loop.max_runtime_seconds = mr;
            normalized_count += 1;
        }

        // Map v1 `max_cost` to v2 `event_loop.max_cost_usd`
        if self.max_cost.is_some() {
            debug!(from = "max_cost", to = "event_loop.max_cost_usd", "Normalizing v1 field");
            self.event_loop.max_cost_usd = self.max_cost;
            normalized_count += 1;
        }

        // Map v1 `checkpoint_interval` to v2 `event_loop.checkpoint_interval`
        if let Some(ci) = self.checkpoint_interval {
            debug!(from = "checkpoint_interval", to = "event_loop.checkpoint_interval", value = ci, "Normalizing v1 field");
            self.event_loop.checkpoint_interval = ci;
            normalized_count += 1;
        }

        if normalized_count > 0 {
            debug!(fields_normalized = normalized_count, "V1 to V2 config normalization complete");
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
        if self.event_loop.prompt.is_some() && !self.event_loop.prompt_file.is_empty() && self.event_loop.prompt_file != default_prompt_file() {
            return Err(ConfigError::MutuallyExclusive {
                field1: "event_loop.prompt".to_string(),
                field2: "event_loop.prompt_file".to_string(),
            });
        }

        // Check custom backend has a command
        if self.cli.backend == "custom" && (self.cli.command.is_none() || self.cli.command.as_ref().map_or(true, |c| c.is_empty())) {
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

        // Check for ambiguous routing: each trigger topic must map to exactly one hat
        // Per spec: "Every trigger maps to exactly one hat | No ambiguous routing"
        if !self.hats.is_empty() {
            let mut trigger_to_hat: HashMap<&str, &str> = HashMap::new();
            for (hat_id, hat_config) in &self.hats {
                for trigger in &hat_config.triggers {
                    if let Some(existing_hat) = trigger_to_hat.get(trigger.as_str()) {
                        return Err(ConfigError::AmbiguousRouting {
                            trigger: trigger.clone(),
                            hat1: existing_hat.to_string(),
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

    /// Performs preflight validation to catch issues before the loop starts.
    ///
    /// This validates:
    /// - Event flow graph integrity (no dead-end events)
    /// - Path existence (scratchpad parent dir, specs dir)
    /// - Git availability (if checkpointing enabled)
    ///
    /// Returns a list of errors (fatal) and warnings (informational).
    pub fn preflight_check(&self) -> (Vec<PreflightError>, Vec<PreflightWarning>) {
        let mut errors = Vec::new();
        let mut warnings = Vec::new();

        // Build effective hat configs (use defaults if none configured)
        let effective_hats = self.get_effective_hats();

        // Check 1: At least one hat must exist
        if effective_hats.is_empty() {
            errors.push(PreflightError::NoHatsConfigured);
            return (errors, warnings);
        }

        // Build trigger and publish maps for event flow analysis
        let mut all_triggers: std::collections::HashSet<&str> = std::collections::HashSet::new();
        let mut all_publishes: std::collections::HashMap<&str, Vec<&str>> = std::collections::HashMap::new();

        for (hat_id, hat_config) in &effective_hats {
            for trigger in &hat_config.triggers {
                all_triggers.insert(trigger.as_str());
            }
            for publish in &hat_config.publishes {
                all_publishes.entry(publish.as_str()).or_default().push(hat_id.as_str());
            }
            // Check for hats that never publish (warning, not error)
            if hat_config.publishes.is_empty() {
                warnings.push(PreflightWarning::HatNeverPublishes {
                    hat: hat_id.clone(),
                });
            }
        }

        // Check 2: Initial events must have handlers
        for initial_event in &["task.start", "task.resume"] {
            if !all_triggers.contains(initial_event) {
                // Check if any hat would handle these (skip if not relevant to this config)
                let is_relevant = effective_hats.values().any(|h| {
                    h.triggers.iter().any(|t| t == "task.start" || t == "task.resume")
                });
                if !is_relevant && *initial_event == "task.start" {
                    errors.push(PreflightError::NoInitialHandler {
                        event: initial_event.to_string(),
                    });
                }
            }
        }

        // Check 3: Every published event must have a subscriber (except LOOP_COMPLETE)
        for (event, publishers) in &all_publishes {
            if *event == "LOOP_COMPLETE" || *event == self.event_loop.completion_promise.as_str() {
                continue; // Terminal event, no subscriber needed
            }
            if !all_triggers.contains(event) {
                for publisher in publishers {
                    errors.push(PreflightError::UnhandledEvent {
                        event: event.to_string(),
                        publisher: publisher.to_string(),
                    });
                }
            }
        }

        // Check 4: Git availability (if checkpointing enabled)
        if self.git_checkpoint && self.event_loop.checkpoint_interval > 0 {
            if !Self::is_git_available() {
                errors.push(PreflightError::GitNotAvailable);
            }
        }

        // Check 5: Path validation
        // Scratchpad parent directory should exist (or be creatable)
        let scratchpad_path = Path::new(&self.core.scratchpad);
        if let Some(parent) = scratchpad_path.parent() {
            if !parent.as_os_str().is_empty() && !parent.exists() {
                errors.push(PreflightError::PathNotFound {
                    path: parent.display().to_string(),
                    purpose: "scratchpad parent directory".to_string(),
                });
            }
        }

        // Specs directory should exist if referenced
        if !self.core.specs_dir.is_empty() {
            let specs_path = Path::new(&self.core.specs_dir);
            if !specs_path.exists() {
                // Warning, not error - planner may create it
                debug!("Specs directory '{}' does not exist yet", self.core.specs_dir);
            }
        }

        // Check 6: Custom events should have metadata defined
        // Well-known events have built-in defaults, custom events need metadata
        let well_known_events: std::collections::HashSet<&str> = [
            "task.start", "task.resume",
            "build.task", "build.done", "build.blocked",
            "review.request", "review.approved", "review.changes_requested",
            "LOOP_COMPLETE",
        ].into_iter().collect();

        for (hat_id, hat_config) in &effective_hats {
            // Skip if hat has explicit instructions
            if !hat_config.instructions.is_empty() {
                continue;
            }

            // Check triggers
            for trigger in &hat_config.triggers {
                if !well_known_events.contains(trigger.as_str())
                    && !self.events.contains_key(trigger)
                {
                    warnings.push(PreflightWarning::UndefinedEventMetadata {
                        event: trigger.clone(),
                        hat: hat_id.clone(),
                        usage: "trigger".to_string(),
                    });
                }
            }

            // Check publishes
            for publish in &hat_config.publishes {
                if !well_known_events.contains(publish.as_str())
                    && !self.events.contains_key(publish)
                    && publish != &self.event_loop.completion_promise
                {
                    warnings.push(PreflightWarning::UndefinedEventMetadata {
                        event: publish.clone(),
                        hat: hat_id.clone(),
                        usage: "publish".to_string(),
                    });
                }
            }
        }

        (errors, warnings)
    }

    /// Helper to check if git is available in PATH.
    fn is_git_available() -> bool {
        std::process::Command::new("git")
            .arg("--version")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false)
    }

    /// Returns effective hat configurations, using defaults if none configured.
    fn get_effective_hats(&self) -> HashMap<String, HatConfig> {
        if self.hats.is_empty() {
            // Return default planner + builder config
            let mut defaults = HashMap::new();
            defaults.insert("planner".to_string(), HatConfig {
                name: "Planner".to_string(),
                triggers: vec!["task.start".to_string(), "task.resume".to_string(), "build.done".to_string(), "build.blocked".to_string()],
                publishes: vec!["build.task".to_string()],
                instructions: String::new(),
            });
            defaults.insert("builder".to_string(), HatConfig {
                name: "Builder".to_string(),
                triggers: vec!["build.task".to_string()],
                publishes: vec!["build.done".to_string(), "build.blocked".to_string()],
                instructions: String::new(),
            });
            defaults
        } else {
            self.hats.clone()
        }
    }

    /// Returns the agent priority list for auto-detection.
    /// If empty, returns the default priority order.
    pub fn get_agent_priority(&self) -> Vec<&str> {
        if self.agent_priority.is_empty() {
            vec!["claude", "kiro", "gemini", "codex", "amp"]
        } else {
            self.agent_priority.iter().map(|s| s.as_str()).collect()
        }
    }

    /// Gets the adapter settings for a specific backend.
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
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ConfigWarning::DeferredFeature { field, message } => {
                write!(f, "Warning [{}]: {}", field, message)
            }
            ConfigWarning::DroppedField { field, reason } => {
                write!(f, "Warning [{}]: Field ignored - {}", field, reason)
            }
            ConfigWarning::InvalidValue { field, message } => {
                write!(f, "Warning [{}]: {}", field, message)
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

    /// String that signals loop completion.
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

    /// Create checkpoint commit every N iterations.
    #[serde(default = "default_checkpoint_interval")]
    pub checkpoint_interval: u32,

    /// Starting hat for multi-hat mode.
    pub starting_hat: Option<String>,
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

fn default_checkpoint_interval() -> u32 {
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
            checkpoint_interval: default_checkpoint_interval(),
            starting_hat: None,
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
}

fn default_scratchpad() -> String {
    ".agent/scratchpad.md".to_string()
}

fn default_specs_dir() -> String {
    "./specs/".to_string()
}

fn default_guardrails() -> Vec<String> {
    vec![
        "Fresh context each iteration - scratchpad is memory".to_string(),
        "Don't assume 'not implemented' - search first".to_string(),
        "Backpressure is law - tests/typecheck/lint must pass".to_string(),
    ]
}

impl Default for CoreConfig {
    fn default() -> Self {
        Self {
            scratchpad: default_scratchpad(),
            specs_dir: default_specs_dir(),
            guardrails: default_guardrails(),
        }
    }
}

/// CLI backend configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CliConfig {
    /// Backend to use: "claude", "kiro", "gemini", "codex", "amp", or "custom".
    #[serde(default = "default_backend")]
    pub backend: String,

    /// Custom command (for backend: "custom").
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

/// Configuration for a single hat.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HatConfig {
    /// Human-readable name for the hat.
    pub name: String,

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

/// Configuration errors.
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("YAML parse error: {0}")]
    Yaml(#[from] serde_yaml::Error),

    #[error("Ambiguous routing: trigger '{trigger}' is claimed by both '{hat1}' and '{hat2}'")]
    AmbiguousRouting {
        trigger: String,
        hat1: String,
        hat2: String,
    },

    #[error("Mutually exclusive fields: '{field1}' and '{field2}' cannot both be specified")]
    MutuallyExclusive {
        field1: String,
        field2: String,
    },

    #[error("Custom backend requires a command - set 'cli.command' in config")]
    CustomBackendRequiresCommand,
}

/// Errors that occur during preflight validation.
/// These are issues that would cause the loop to fail or terminate unexpectedly.
#[derive(Debug, Clone)]
pub enum PreflightError {
    /// No hats are configured - the loop would have nothing to execute.
    NoHatsConfigured,
    /// A published event has no subscribers (dead end in event flow).
    UnhandledEvent {
        event: String,
        publisher: String,
    },
    /// The initial event has no handler.
    NoInitialHandler {
        event: String,
    },
    /// Git is required for checkpointing but not available.
    GitNotAvailable,
    /// A required path doesn't exist.
    PathNotFound {
        path: String,
        purpose: String,
    },
    /// A path exists but is not writable.
    PathNotWritable {
        path: String,
        purpose: String,
    },
}

impl std::fmt::Display for PreflightError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PreflightError::NoHatsConfigured => {
                write!(f, "No hats configured. The event loop needs at least one hat to execute.")
            }
            PreflightError::UnhandledEvent { event, publisher } => {
                write!(f, "Event '{event}' published by '{publisher}' has no subscriber. This would cause the loop to terminate unexpectedly.")
            }
            PreflightError::NoInitialHandler { event } => {
                write!(f, "Initial event '{event}' has no handler. No hat is subscribed to receive it.")
            }
            PreflightError::GitNotAvailable => {
                write!(f, "Git checkpointing is enabled but 'git' command not found in PATH.")
            }
            PreflightError::PathNotFound { path, purpose } => {
                write!(f, "Required path not found: '{path}' ({purpose})")
            }
            PreflightError::PathNotWritable { path, purpose } => {
                write!(f, "Path not writable: '{path}' ({purpose})")
            }
        }
    }
}

impl std::error::Error for PreflightError {}

/// Warnings that occur during preflight validation.
/// These are potential issues that may cause problems but aren't fatal.
#[derive(Debug, Clone)]
pub enum PreflightWarning {
    /// A hat never publishes any events (may be intentional for terminal hats).
    HatNeverPublishes {
        hat: String,
    },
    /// An event is subscribed to but never published.
    EventNeverPublished {
        event: String,
        subscriber: String,
    },
    /// A custom event has no metadata defined (may cause missing instructions).
    UndefinedEventMetadata {
        event: String,
        hat: String,
        usage: String, // "trigger" or "publish"
    },
}

impl std::fmt::Display for PreflightWarning {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PreflightWarning::HatNeverPublishes { hat } => {
                write!(f, "Hat '{}' never publishes any events", hat)
            }
            PreflightWarning::EventNeverPublished { event, subscriber } => {
                write!(f, "Event '{}' is subscribed to by '{}' but never published", event, subscriber)
            }
            PreflightWarning::UndefinedEventMetadata { event, hat, usage } => {
                write!(
                    f,
                    "Custom event '{}' used as {} by '{}' has no metadata. Add to 'events:' config or provide explicit instructions.",
                    event, usage, hat
                )
            }
        }
    }
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
        assert!(config.git_checkpoint);
        assert!(!config.verbose);
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
    fn test_parse_yaml_v1_format() {
        // V1 flat format - identical to Python v1.x config
        let yaml = r#"
agent: gemini
prompt_file: "TASK.md"
completion_promise: "RALPH_DONE"
max_iterations: 75
max_runtime: 7200
max_cost: 10.0
checkpoint_interval: 10
git_checkpoint: true
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
        assert_eq!(config.event_loop.checkpoint_interval, 10);
        assert!(config.git_checkpoint);
        assert!(config.verbose);
    }

    #[test]
    fn test_agent_priority() {
        let yaml = r#"
agent: auto
agent_priority: [gemini, claude, codex]
"#;
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
        let yaml = r#"
archive_prompts: true
enable_metrics: true
"#;
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
        assert!(warnings
            .iter()
            .any(|w| matches!(w, ConfigWarning::DroppedField { field, .. } if field == "max_tokens")));
        assert!(warnings
            .iter()
            .any(|w| matches!(w, ConfigWarning::DroppedField { field, .. } if field == "retry_delay")));
        assert!(warnings
            .iter()
            .any(|w| matches!(w, ConfigWarning::DroppedField { field, .. } if field == "adapters.*.tool_permissions")));
    }

    #[test]
    fn test_suppress_warnings() {
        let yaml = r#"
_suppress_warnings: true
archive_prompts: true
max_tokens: 4096
"#;
        let config: RalphConfig = serde_yaml::from_str(yaml).unwrap();
        let warnings = config.validate().unwrap();

        // All warnings should be suppressed
        assert!(warnings.is_empty());
    }

    #[test]
    fn test_adapter_settings() {
        let yaml = r#"
adapters:
  claude:
    timeout: 600
    enabled: true
  gemini:
    timeout: 300
    enabled: false
"#;
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
        let yaml = r#"
hats:
  planner:
    name: "Planner"
    triggers: ["task.start", "build.done"]
  builder:
    name: "Builder"
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
        let yaml = r#"
hats:
  planner:
    name: "Planner"
    triggers: ["task.start", "build.done", "build.blocked"]
  builder:
    name: "Builder"
    triggers: ["build.task"]
"#;
        let config: RalphConfig = serde_yaml::from_str(yaml).unwrap();
        let result = config.validate();

        assert!(result.is_ok(), "Expected valid config, got: {:?}", result.unwrap_err());
    }

    #[test]
    fn test_core_config_defaults() {
        let config = RalphConfig::default();
        assert_eq!(config.core.scratchpad, ".agent/scratchpad.md");
        assert_eq!(config.core.specs_dir, "./specs/");
        // Default guardrails per spec
        assert_eq!(config.core.guardrails.len(), 3);
        assert!(config.core.guardrails[0].contains("Fresh context"));
        assert!(config.core.guardrails[1].contains("search first"));
        assert!(config.core.guardrails[2].contains("Backpressure"));
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
        assert_eq!(config.core.guardrails.len(), 3);
    }

    #[test]
    fn test_core_config_custom_guardrails() {
        let yaml = r#"
core:
  scratchpad: ".agent/scratchpad.md"
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
        
        assert!(result.is_ok(), "Should allow inline prompt with default prompt_file");
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
        
        assert!(result.is_ok(), "Should allow custom backend with command: {:?}", result.unwrap_err());
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
        
        assert!(result.is_ok(), "Should allow prompt_file without inline prompt");
        assert_eq!(config.event_loop.prompt, None);
        assert_eq!(config.event_loop.prompt_file, "custom.md");
    }

    #[test]
    fn test_default_prompt_file_value() {
        let config = RalphConfig::default();
        assert_eq!(config.event_loop.prompt_file, "PROMPT.md");
        assert_eq!(config.event_loop.prompt, None);
    }
}
