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
    /// Values: "claude", "kiro", "gemini", "codex", "amp", "pi", "auto", or "custom".
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

    /// Lifecycle hooks configuration.
    #[serde(default)]
    pub hooks: HooksConfig,

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
            // Hooks
            hooks: HooksConfig::default(),
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
        // Pre-flight check for deprecated/invalid keys to improve UX.
        let value: serde_yaml::Value = serde_yaml::from_str(content)?;
        if let Some(map) = value.as_mapping()
            && map.contains_key(serde_yaml::Value::String("project".to_string()))
        {
            return Err(ConfigError::DeprecatedProjectKey);
        }

        validate_hooks_phase_event_keys(&value)?;

        let config: Self = serde_yaml::from_value(value)?;
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
    pub fn validate(&self) -> Result<Vec<ConfigWarning>, ConfigError> {
        let mut warnings = Vec::new();

        if self.suppress_warnings {
            return Ok(warnings);
        }

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

        if self.cli.backend == "custom" && self.cli.command.as_ref().is_none_or(String::is_empty) {
            return Err(ConfigError::CustomBackendRequiresCommand);
        }

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

        self.robot.validate()?;
        self.validate_hooks()?;

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

        for (hat_id, hat_config) in &self.hats {
            if hat_config.concurrency == 0 {
                return Err(ConfigError::InvalidConcurrency {
                    hat: hat_id.clone(),
                    value: 0,
                });
            }
            if hat_config.aggregate.is_some() && hat_config.concurrency > 1 {
                return Err(ConfigError::AggregateOnConcurrentHat {
                    hat: hat_id.clone(),
                });
            }
        }

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

    fn validate_hooks(&self) -> Result<(), ConfigError> {
        Self::validate_non_v1_hook_fields("hooks", &self.hooks.extra)?;

        if self.hooks.defaults.timeout_seconds == 0 {
            return Err(ConfigError::HookValidation {
                field: "hooks.defaults.timeout_seconds".to_string(),
                message: "must be greater than 0".to_string(),
            });
        }

        if self.hooks.defaults.max_output_bytes == 0 {
            return Err(ConfigError::HookValidation {
                field: "hooks.defaults.max_output_bytes".to_string(),
                message: "must be greater than 0".to_string(),
            });
        }

        for (phase_event, hook_specs) in &self.hooks.events {
            for (index, hook) in hook_specs.iter().enumerate() {
                let hook_field_base = format!("hooks.events.{phase_event}[{index}]");

                if hook.name.trim().is_empty() {
                    return Err(ConfigError::HookValidation {
                        field: format!("{hook_field_base}.name"),
                        message: "is required and must be non-empty".to_string(),
                    });
                }

                if hook.command.first().is_none_or(|command| command.trim().is_empty()) {
                    return Err(ConfigError::HookValidation {
                        field: format!("{hook_field_base}.command"),
                        message: "is required and must include an executable at command[0]".to_string(),
                    });
                }

                if hook.on_error.is_none() {
                    return Err(ConfigError::HookValidation {
                        field: format!("{hook_field_base}.on_error"),
                        message: "is required in v1 (warn | block | suspend)".to_string(),
                    });
                }

                if let Some(timeout_seconds) = hook.timeout_seconds
                    && timeout_seconds == 0
                {
                    return Err(ConfigError::HookValidation {
                        field: format!("{hook_field_base}.timeout_seconds"),
                        message: "must be greater than 0 when specified".to_string(),
                    });
                }

                if let Some(max_output_bytes) = hook.max_output_bytes
                    && max_output_bytes == 0
                {
                    return Err(ConfigError::HookValidation {
                        field: format!("{hook_field_base}.max_output_bytes"),
                        message: "must be greater than 0 when specified".to_string(),
                    });
                }

                if hook.suspend_mode.is_some() && hook.on_error != Some(HookOnError::Suspend) {
                    return Err(ConfigError::HookValidation {
                        field: format!("{hook_field_base}.suspend_mode"),
                        message: "requires on_error: suspend".to_string(),
                    });
                }

                Self::validate_non_v1_hook_fields(&hook_field_base, &hook.extra)?;
                Self::validate_mutation_contract(&hook_field_base, &hook.mutate)?;
            }
        }

        Ok(())
    }

    fn validate_non_v1_hook_fields(
        path_prefix: &str,
        fields: &HashMap<String, serde_yaml::Value>,
    ) -> Result<(), ConfigError> {
        for key in fields.keys() {
            let field = format!("{path_prefix}.{key}");
            match key.as_str() {
                "global" | "globals" | "global_defaults" | "global_hooks" | "scope" => {
                    return Err(ConfigError::UnsupportedHookField {
                        field,
                        reason: "Global hooks are out of scope for v1; use per-project hooks only".to_string(),
                    });
                }
                "parallel" | "parallelism" | "max_parallel" | "concurrency" | "run_in_parallel" => {
                    return Err(ConfigError::UnsupportedHookField {
                        field,
                        reason: "Parallel hook execution is out of scope for v1; hooks must run sequentially".to_string(),
                    });
                }
                _ => {}
            }
        }
        Ok(())
    }

    fn validate_mutation_contract(
        hook_field_base: &str,
        mutate: &HookMutationConfig,
    ) -> Result<(), ConfigError> {
        let mutate_field_base = format!("{hook_field_base}.mutate");

        if !mutate.enabled {
            if mutate.format.is_some() || !mutate.extra.is_empty() {
                return Err(ConfigError::HookValidation {
                    field: mutate_field_base,
                    message: "mutation settings require mutate.enabled: true".to_string(),
                });
            }
            return Ok(());
        }

        if let Some(format) = mutate.format.as_deref()
            && !format.eq_ignore_ascii_case("json")
        {
            return Err(ConfigError::HookValidation {
                field: format!("{mutate_field_base}.format"),
                message: "only 'json' is supported for v1 mutation payloads".to_string(),
            });
        }

        if let Some(key) = mutate.extra.keys().next() {
            let field = format!("{mutate_field_base}.{key}");
            let reason = match key.as_str() {
                "prompt" | "prompt_mutation" | "events" | "event" | "config" | "full_context" => {
                    "v1 allows metadata-only mutation; prompt/event/config mutation is unsupported".to_string()
                }
                "xml" => "v1 mutation payloads are JSON-only".to_string(),
                _ => "unsupported mutate field in v1 (supported keys: enabled, format)".to_string(),
            };
            return Err(ConfigError::UnsupportedHookField { field, reason });
        }

        Ok(())
    }

    /// Gets the effective backend name, resolving "auto" using the priority list.
    pub fn effective_backend(&self) -> &str {
        &self.cli.backend
    }

    /// Returns the agent priority list for auto-detection.
    pub fn get_agent_priority(&self) -> Vec<&str> {
        if self.agent_priority.is_empty() {
            vec!["claude", "kiro", "gemini", "codex", "amp"]
        } else {
            self.agent_priority.iter().map(String::as_str).collect()
        }
    }

    /// Gets the adapter settings for a specific backend.
    #[allow(clippy::match_same_arms)]
    pub fn adapter_settings(&self, backend: &str) -> &AdapterSettings {
        match backend {
            "claude" => &self.adapters.claude,
            "gemini" => &self.adapters.gemini,
            "kiro" => &self.adapters.kiro,
            "codex" => &self.adapters.codex,
            "amp" => &self.adapters.amp,
            _ => &self.adapters.claude,
        }
    }
}

/// Configuration warnings emitted during validation.
#[derive(Debug, Clone)]
pub enum ConfigWarning {
    DeferredFeature { field: String, message: String },
    DroppedField { field: String, reason: String },
    InvalidValue { field: String, message: String },
}

impl std::fmt::Display for ConfigWarning {
    #[allow(clippy::match_same_arms)]
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
    pub prompt: Option<String>,
    #[serde(default = "default_prompt_file")]
    pub prompt_file: String,
    #[serde(default = "default_completion_promise")]
    pub completion_promise: String,
    #[serde(default = "default_max_iterations")]
    pub max_iterations: u32,
    #[serde(default = "default_max_runtime")]
    pub max_runtime_seconds: u64,
    pub max_cost_usd: Option<f64>,
    #[serde(default = "default_max_failures")]
    pub max_consecutive_failures: u32,
    #[serde(default)]
    pub cooldown_delay_seconds: u64,
    pub starting_hat: Option<String>,
    pub starting_event: Option<String>,
    #[serde(default)]
    pub mutation_score_warn_threshold: Option<f64>,
    #[serde(default)]
    pub persistent: bool,
    #[serde(default)]
    pub required_events: Vec<String>,
    #[serde(default)]
    pub cancellation_promise: String,
    #[serde(default)]
    pub enforce_hat_scope: bool,
}

fn default_prompt_file() -> String { "PROMPT.md".to_string() }
fn default_completion_promise() -> String { "LOOP_COMPLETE".to_string() }
fn default_max_iterations() -> u32 { 100 }
fn default_max_runtime() -> u64 { 14400 }
fn default_max_failures() -> u32 { 5 }

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
            required_events: Vec::new(),
            cancellation_promise: String::new(),
            enforce_hat_scope: false,
        }
    }
}

/// Core paths and settings shared across all hats.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoreConfig {
    #[serde(default = "default_scratchpad")]
    pub scratchpad: String,
    #[serde(default = "default_specs_dir")]
    pub specs_dir: String,
    #[serde(default = "default_guardrails")]
    pub guardrails: Vec<String>,
    #[serde(skip)]
    pub workspace_root: std::path::PathBuf,
}

fn default_scratchpad() -> String { ".ralph/agent/scratchpad.md".to_string() }
fn default_specs_dir() -> String { ".ralph/specs/".to_string() }
fn default_guardrails() -> Vec<String> {
    vec![
        "Fresh context each iteration - scratchpad is memory".to_string(),
        "Don't assume 'not implemented' - search first".to_string(),
        "Backpressure is law - tests/typecheck/lint/audit must pass".to_string(),
        "When behavior is runnable or user-facing, exercise the real app with the strongest available harness (Playwright, tmux, real CLI/API) and try at least one adversarial path before reporting done".to_string(),
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
    pub fn with_workspace_root(mut self, root: impl Into<std::path::PathBuf>) -> Self {
        self.workspace_root = root.into();
        self
    }

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
    #[serde(default = "default_backend")]
    pub backend: String,
    pub command: Option<String>,
    #[serde(default = "default_prompt_mode")]
    pub prompt_mode: String,
    #[serde(default = "default_mode")]
    pub default_mode: String,
    #[serde(default = "default_idle_timeout")]
    pub idle_timeout_secs: u32,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub prompt_flag: Option<String>,
}

fn default_backend() -> String { "claude".to_string() }
fn default_prompt_mode() -> String { "arg".to_string() }
fn default_mode() -> String { "autonomous".to_string() }
fn default_idle_timeout() -> u32 { 30 }

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
    #[serde(default = "default_prefix_key")]
    pub prefix_key: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InjectMode {
    #[default]
    Auto,
    Manual,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemoriesConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub inject: InjectMode,
    #[serde(default)]
    pub budget: usize,
    #[serde(default)]
    pub filter: MemoriesFilter,
}

impl Default for MemoriesConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            inject: InjectMode::Auto,
            budget: 0,
            filter: MemoriesFilter::default(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MemoriesFilter {
    #[serde(default)]
    pub types: Vec<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub recent: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TasksConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
}

impl Default for TasksConfig {
    fn default() -> Self { Self { enabled: true } }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HooksConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub defaults: HookDefaults,
    #[serde(default)]
    pub events: HashMap<HookPhaseEvent, Vec<HookSpec>>,
    #[serde(default, flatten)]
    pub extra: HashMap<String, serde_yaml::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookDefaults {
    #[serde(default = "default_hook_timeout_seconds")]
    pub timeout_seconds: u64,
    #[serde(default = "default_hook_max_output_bytes")]
    pub max_output_bytes: u64,
    #[serde(default)]
    pub suspend_mode: HookSuspendMode,
}

fn default_hook_timeout_seconds() -> u64 { 30 }
fn default_hook_max_output_bytes() -> u64 { 8192 }

impl Default for HookDefaults {
    fn default() -> Self {
        Self {
            timeout_seconds: default_hook_timeout_seconds(),
            max_output_bytes: default_hook_max_output_bytes(),
            suspend_mode: HookSuspendMode::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HookPhaseEvent {
    #[serde(rename = "pre.loop.start")] PreLoopStart,
    #[serde(rename = "post.loop.start")] PostLoopStart,
    #[serde(rename = "pre.iteration.start")] PreIterationStart,
    #[serde(rename = "post.iteration.start")] PostIterationStart,
    #[serde(rename = "pre.plan.created")] PrePlanCreated,
    #[serde(rename = "post.plan.created")] PostPlanCreated,
    #[serde(rename = "pre.human.interact")] PreHumanInteract,
    #[serde(rename = "post.human.interact")] PostHumanInteract,
    #[serde(rename = "pre.loop.complete")] PreLoopComplete,
    #[serde(rename = "post.loop.complete")] PostLoopComplete,
    #[serde(rename = "pre.loop.error")] PreLoopError,
    #[serde(rename = "post.loop.error")] PostLoopError,
}

impl HookPhaseEvent {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::PreLoopStart => "pre.loop.start",
            Self::PostLoopStart => "post.loop.start",
            Self::PreIterationStart => "pre.iteration.start",
            Self::PostIterationStart => "post.iteration.start",
            Self::PrePlanCreated => "pre.plan.created",
            Self::PostPlanCreated => "post.plan.created",
            Self::PreHumanInteract => "pre.human.interact",
            Self::PostHumanInteract => "post.human.interact",
            Self::PreLoopComplete => "pre.loop.complete",
            Self::PostLoopComplete => "post.loop.complete",
            Self::PreLoopError => "pre.loop.error",
            Self::PostLoopError => "post.loop.error",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "pre.loop.start" => Some(Self::PreLoopStart),
            "post.loop.start" => Some(Self::PostLoopStart),
            "pre.iteration.start" => Some(Self::PreIterationStart),
            "post.iteration.start" => Some(Self::PostIterationStart),
            "pre.plan.created" => Some(Self::PrePlanCreated),
            "post.plan.created" => Some(Self::PostPlanCreated),
            "pre.human.interact" => Some(Self::PreHumanInteract),
            "post.human.interact" => Some(Self::PostHumanInteract),
            "pre.loop.complete" => Some(Self::PreLoopComplete),
            "post.loop.complete" => Some(Self::PostLoopComplete),
            "pre.loop.error" => Some(Self::PreLoopError),
            "post.loop.error" => Some(Self::PostLoopError),
            _ => None,
        }
    }
}

impl std::fmt::Display for HookPhaseEvent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str((*self).as_str())
    }
}

fn validate_hooks_phase_event_keys(value: &serde_yaml::Value) -> Result<(), ConfigError> {
    let Some(root) = value.as_mapping() else { return Ok(()); };
    let Some(hooks) = root.get(serde_yaml::Value::String("hooks".to_string())) else { return Ok(()); };
    let Some(hooks_map) = hooks.as_mapping() else { return Ok(()); };
    let Some(events) = hooks_map.get(serde_yaml::Value::String("events".to_string())) else { return Ok(()); };
    let Some(events_map) = events.as_mapping() else { return Ok(()); };
    for key in events_map.keys() {
        if let Some(phase_event) = key.as_str()
            && HookPhaseEvent::parse(phase_event).is_none()
        {
            return Err(ConfigError::InvalidHookPhaseEvent {
                phase_event: phase_event.to_string(),
            });
        }
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HookOnError { Warn, Block, Suspend }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HookSuspendMode {
    #[default]
    WaitForResume,
    RetryBackoff,
    WaitThenRetry,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HookMutationConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub format: Option<String>,
    #[serde(default, flatten)]
    pub extra: HashMap<String, serde_yaml::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookSpec {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub command: Vec<String>,
    #[serde(default)]
    pub cwd: Option<PathBuf>,
    #[serde(default)]
    pub env: HashMap<String, String>,
    #[serde(default)]
    pub timeout_seconds: Option<u64>,
    #[serde(default)]
    pub max_output_bytes: Option<u64>,
    #[serde(default)]
    pub on_error: Option<HookOnError>,
    #[serde(default)]
    pub suspend_mode: Option<HookSuspendMode>,
    #[serde(default)]
    pub mutate: HookMutationConfig,
    #[serde(default, flatten)]
    pub extra: HashMap<String, serde_yaml::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillsConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default)]
    pub dirs: Vec<PathBuf>,
    #[serde(default)]
    pub overrides: HashMap<String, SkillOverride>,
}

impl Default for SkillsConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            dirs: vec![],
            overrides: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SkillOverride {
    #[serde(default)]
    pub enabled: Option<bool>,
    #[serde(default)]
    pub hats: Vec<String>,
    #[serde(default)]
    pub backends: Vec<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub auto_inject: Option<bool>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PreflightConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default)]
    pub strict: bool,
    #[serde(default)]
    pub skip: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeaturesConfig {
    #[serde(default = "default_true")]
    pub parallel: bool,
    #[serde(default)]
    pub auto_merge: bool,
    #[serde(default)]
    pub loop_naming: crate::loop_name::LoopNamingConfig,
    #[serde(default)]
    pub preflight: PreflightConfig,
}

impl Default for FeaturesConfig {
    fn default() -> Self {
        Self {
            parallel: true,
            auto_merge: false,
            loop_naming: crate::loop_name::LoopNamingConfig::default(),
            preflight: PreflightConfig::default(),
        }
    }
}

fn default_prefix_key() -> String { "ctrl-a".to_string() }

impl Default for TuiConfig {
    fn default() -> Self { Self { prefix_key: default_prefix_key() } }
}

impl TuiConfig {
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
            _ => return Err(format!("Invalid modifier: '{}'. Only 'ctrl' is supported (e.g., 'ctrl-a')", parts[0])),
        };
        let key_str = parts[1];
        if key_str.len() != 1 {
            return Err(format!("Invalid key: '{}'. Expected a single character (e.g., 'a', 'b')", key_str));
        }
        let key_char = key_str.chars().next().unwrap();
        Ok((KeyCode::Char(key_char), modifier))
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EventMetadata {
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub on_trigger: String,
    #[serde(default)]
    pub on_publish: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum HatBackend {
    KiroAgent {
        #[serde(rename = "type")]
        backend_type: String,
        agent: String,
        #[serde(default)]
        args: Vec<String>,
    },
    NamedWithArgs {
        #[serde(rename = "type")]
        backend_type: String,
        #[serde(default)]
        args: Vec<String>,
    },
    Named(String),
    Custom {
        command: String,
        #[serde(default)]
        args: Vec<String>,
    },
}

impl HatBackend {
    pub fn to_cli_backend(&self) -> String {
        match self {
            HatBackend::Named(name) => name.clone(),
            HatBackend::NamedWithArgs { backend_type, .. } => backend_type.clone(),
            HatBackend::KiroAgent { backend_type, .. } => backend_type.clone(),
            HatBackend::Custom { .. } => "custom".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HatConfig {
    pub name: String,
    pub description: Option<String>,
    #[serde(default)]
    pub triggers: Vec<String>,
    #[serde(default)]
    pub publishes: Vec<String>,
    #[serde(default)]
    pub instructions: String,
    #[serde(default)]
    pub extra_instructions: Vec<String>,
    #[serde(default)]
    pub backend: Option<HatBackend>,
    #[serde(default, alias = "args")]
    pub backend_args: Option<Vec<String>>,
    #[serde(default)]
    pub default_publishes: Option<String>,
    pub max_activations: Option<u32>,
    #[serde(default)]
    pub disallowed_tools: Vec<String>,
    #[serde(default)]
    pub timeout: Option<u32>,
    #[serde(default = "default_concurrency")]
    pub concurrency: u32,
    #[serde(default)]
    pub aggregate: Option<AggregateConfig>,
}

fn default_concurrency() -> u32 { 1 }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregateConfig {
    pub mode: AggregateMode,
    pub timeout: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AggregateMode {
    WaitForAll,
}

impl HatConfig {
    pub fn trigger_topics(&self) -> Vec<Topic> {
        self.triggers.iter().map(|s| Topic::new(s)).collect()
    }
    pub fn publish_topics(&self) -> Vec<Topic> {
        self.publishes.iter().map(|s| Topic::new(s)).collect()
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RobotConfig {
    #[serde(default)]
    pub enabled: bool,
    pub timeout_seconds: Option<u64>,
    pub checkin_interval_seconds: Option<u64>,
    #[serde(default)]
    pub telegram: Option<TelegramBotConfig>,
}

impl RobotConfig {
    pub fn validate(&self) -> Result<(), ConfigError> {
        if !self.enabled { return Ok(()); }
        if self.timeout_seconds.is_none() {
            return Err(ConfigError::RobotMissingField {
                field: "RObot.timeout_seconds".to_string(),
                hint: "timeout_seconds is required when RObot is enabled".to_string(),
            });
        }
        if self.resolve_bot_token().is_none() {
            return Err(ConfigError::RobotMissingField {
                field: "RObot.telegram.bot_token".to_string(),
                hint: "Run `ralph bot onboard --telegram`, set RALPH_TELEGRAM_BOT_TOKEN env var, or set RObot.telegram.bot_token in config".to_string(),
            });
        }
        Ok(())
    }

    pub fn resolve_bot_token(&self) -> Option<String> {
        let env_token = std::env::var("RALPH_TELEGRAM_BOT_TOKEN").ok();
        let config_token = self.telegram.as_ref().and_then(|t| t.bot_token.clone());
        if cfg!(test) { return env_token.or(config_token); }
        env_token.or(config_token).or_else(|| {
            std::panic::catch_unwind(|| {
                keyring::Entry::new("ralph", "telegram-bot-token")
                    .ok()
                    .and_then(|e| e.get_password().ok())
            }).ok().flatten()
        })
    }

    pub fn resolve_api_url(&self) -> Option<String> {
        std::env::var("RALPH_TELEGRAM_API_URL").ok().or_else(|| {
            self.telegram.as_ref().and_then(|t| t.api_url.clone())
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelegramBotConfig {
    pub bot_token: Option<String>,
    pub api_url: Option<String>,
}

#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("IO error: {0}")] Io(#[from] std::io::Error),
    #[error("YAML parse error: {0}")] Yaml(#[from] serde_yaml::Error),
    #[error("Ambiguous routing: trigger '{trigger}' is claimed by both '{hat1}' and '{hat2}'.\nFix: ensure only one hat claims this trigger or delegate with a new event.\nSee: docs/reference/troubleshooting.md#ambiguous-routing")]
    AmbiguousRouting { trigger: String, hat1: String, hat2: String },
    #[error("Mutually exclusive fields: '{field1}' and '{field2}' cannot both be specified.\nFix: remove one field or split into separate configs.\nSee: docs/reference/troubleshooting.md#mutually-exclusive-fields")]
    MutuallyExclusive { field1: String, field2: String },
    #[error("Invalid completion_promise: must be non-empty and non-whitespace")]
    InvalidCompletionPromise,
    #[error("Custom backend requires a command.\nFix: set 'cli.command' in your config (or run `ralph init --backend custom`).\nSee: docs/reference/troubleshooting.md#custom-backend-command")]
    CustomBackendRequiresCommand,
    #[error("Reserved trigger '{trigger}' used by hat '{hat}' - task.start and task.resume are reserved for Ralph (the coordinator). Use a delegated event like 'work.start' instead.\nSee: docs/reference/troubleshooting.md#reserved-trigger")]
    ReservedTrigger { trigger: String, hat: String },
    #[error("Hat '{hat}' is missing required 'description' field - add a short description of the hat's purpose.\nSee: docs/reference/troubleshooting.md#missing-hat-description")]
    MissingDescription { hat: String },
    #[error("RObot config error: {field} - {hint}\nSee: docs/reference/troubleshooting.md#robot-config")]
    RobotMissingField { field: String, hint: String },
    #[error("Invalid hooks phase-event '{phase_event}'. Supported v1 phase-events: pre.loop.start, post.loop.start, pre.iteration.start, post.iteration.start, pre.plan.created, post.plan.created, pre.human.interact, post.human.interact, pre.loop.complete, post.loop.complete, pre.loop.error, post.loop.error.\nFix: use one of the supported keys under hooks.events.")]
    InvalidHookPhaseEvent { phase_event: String },
    #[error("Hook config validation error at '{field}': {message}\nSee: specs/add-hooks-to-ralph-orchestrator-lifecycle/design.md#hookspec-fields-v1")]
    HookValidation { field: String, message: String },
    #[error("Unsupported hooks field '{field}' for v1. {reason}\nSee: specs/add-hooks-to-ralph-orchestrator-lifecycle/design.md#out-of-scope-v1-non-goals")]
    UnsupportedHookField { field: String, reason: String },
    #[error("Invalid config key 'project'. Use 'core' instead (e.g. 'core.specs_dir' instead of 'project.specs_dir').\nSee: docs/guide/configuration.md")]
    DeprecatedProjectKey,
    #[error("Hat '{hat}' has invalid concurrency: {value}. Must be >= 1.\nFix: set 'concurrency' to 1 or higher.")]
    InvalidConcurrency { hat: String, value: u32 },
    #[error("Hat '{hat}' has both 'aggregate' and 'concurrency > 1'. An aggregator hat cannot also be a concurrent worker.\nFix: remove 'aggregate' or set 'concurrency' to 1.")]
    AggregateOnConcurrentHat { hat: String },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = RalphConfig::default();
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
        assert_eq!(config.features.preflight.skip, vec!["telegram".to_string(), "git".to_string()]);
    }

    #[test]
    fn test_parse_yaml_v1_format() {
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
        assert_eq!(config.cli.backend, "claude");
        assert_eq!(config.event_loop.max_iterations, 100);
        config.normalize();
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
        assert!(warnings.iter().any(|w| matches!(w, ConfigWarning::DeferredFeature { field, .. } if field == "archive_prompts")));
        assert!(warnings.iter().any(|w| matches!(w, ConfigWarning::DeferredFeature { field, .. } if field == "enable_metrics")));
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
        assert!(warnings.iter().any(|w| matches!(w, ConfigWarning::DroppedField { field, .. } if field == "max_tokens")));
        assert!(warnings.iter().any(|w| matches!(w, ConfigWarning::DroppedField { field, .. } if field == "retry_delay")));
        assert!(warnings.iter().any(|w| matches!(w, ConfigWarning::DroppedField { field, .. } if field == "adapters.*.tool_permissions")));
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
        let yaml = r#"
agent: claude
unknown_field: "some value"
future_feature: true
"#;
        let result: Result<RalphConfig, _> = serde_yaml::from_str(yaml);
        assert!(result.is_ok());
    }

    #[test]
    fn test_custom_backend_args_shorthand() {
        let yaml = r#"
hats:
  opencode_builder:
    name: "Opencode"
    description: "Opencode hat"
    backend: "opencode"
    args: ["-m", "model"]
"#;
        let config = RalphConfig::parse_yaml(yaml).unwrap();
        let hat = config.hats.get("opencode_builder").unwrap();
        assert!(hat.backend_args.is_some());
        assert_eq!(hat.backend_args.as_ref().unwrap(), &vec!["-m".to_string(), "model".to_string()]);
    }

    #[test]
    fn test_custom_backend_args_explicit_key() {
        let yaml = r#"
hats:
  opencode_builder:
    name: "Opencode"
    description: "Opencode hat"
    backend: "opencode"
    backend_args: ["-m", "model"]
"#;
        let config = RalphConfig::parse_yaml(yaml).unwrap();
        let hat = config.hats.get("opencode_builder").unwrap();
        assert!(hat.backend_args.is_some());
        assert_eq!(hat.backend_args.as_ref().unwrap(), &vec!["-m".to_string(), "model".to_string()]);
    }

    #[test]
    fn test_project_key_rejected() {
        let yaml = r#"
project:
  specs_dir: "my_specs"
"#;
        let result = RalphConfig::parse_yaml(yaml);
        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ConfigError::DeprecatedProjectKey));
    }

    #[test]
    fn test_ambiguous_routing_rejected() {
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
        assert!(matches!(&err, ConfigError::AmbiguousRouting { trigger, .. } if trigger == "build.done"), "Expected AmbiguousRouting error for 'build.done', got: {:?}", err);
    }

    #[test]
    fn test_unique_triggers_accepted() {
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
        assert!(result.is_ok(), "Expected valid config, got: {:?}", result.unwrap_err());
    }

    #[test]
    fn test_reserved_trigger_task_start_rejected() {
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
        assert!(matches!(&err, ConfigError::ReservedTrigger { trigger, hat } if trigger == "task.start" && hat == "my_hat"), "Expected ReservedTrigger error for 'task.start', got: {:?}", err);
    }

    #[test]
    fn test_reserved_trigger_task_resume_rejected() {
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
        assert!(matches!(&err, ConfigError::ReservedTrigger { trigger, hat } if trigger == "task.resume" && hat == "my_hat"), "Expected ReservedTrigger error for 'task.resume', got: {:?}", err);
    }

    #[test]
    fn test_missing_description_rejected() {
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
        assert!(matches!(&err, ConfigError::MissingDescription { hat } if hat == "my_hat"), "Expected MissingDescription error, got: {:?}", err);
    }

    #[test]
    fn test_empty_description_rejected() {
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
        assert!(matches!(&err, ConfigError::MissingDescription { hat } if hat == "my_hat"), "Expected MissingDescription error for empty description, got: {:?}", err);
    }

    #[test]
    fn test_core_config_defaults() {
        let config = RalphConfig::default();
        assert_eq!(config.core.scratchpad, ".ralph/agent/scratchpad.md");
        assert_eq!(config.core.specs_dir, ".ralph/specs/");
        assert_eq!(config.core.guardrails.len(), 6);
        assert!(config.core.guardrails[0].contains("Fresh context"));
        assert!(config.core.guardrails[1].contains("search first"));
        assert!(config.core.guardrails[2].contains("Backpressure"));
        assert!(config.core.guardrails[3].contains("strongest available harness"));
        assert!(config.core.guardrails[4].contains("Confidence protocol"));
        assert!(config.core.guardrails[5].contains("Commit atomically"));
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
        assert_eq!(config.core.guardrails.len(), 6);
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
        let yaml = r#"
event_loop:
  prompt: "inline text"
  prompt_file: "custom.md"
"#;
        let config: RalphConfig = serde_yaml::from_str(yaml).unwrap();
        let result = config.validate();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(&err, ConfigError::MutuallyExclusive { field1, field2 } if field1 == "event_loop.prompt" && field2 == "event_loop.prompt_file"), "Expected MutuallyExclusive error, got: {:?}", err);
    }

    #[test]
    fn test_prompt_with_default_prompt_file_allowed() {
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
        let yaml = r#"
cli:
  backend: "custom"
"#;
        let config: RalphConfig = serde_yaml::from_str(yaml).unwrap();
        let result = config.validate();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(&err, ConfigError::CustomBackendRequiresCommand), "Expected CustomBackendRequiresCommand error, got: {:?}", err);
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
        assert!(matches!(&err, ConfigError::InvalidCompletionPromise), "Expected InvalidCompletionPromise error, got: {:?}", err);
    }

    #[test]
    fn test_custom_backend_with_empty_command_errors() {
        let yaml = r#"
cli:
  backend: "custom"
  command: ""
"#;
        let config: RalphConfig = serde_yaml::from_str(yaml).unwrap();
        let result = config.validate();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(&err, ConfigError::CustomBackendRequiresCommand), "Expected CustomBackendRequiresCommand error, got: {:?}", err);
    }

    #[test]
    fn test_custom_backend_with_command_succeeds() {
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
    fn test_custom_backend_requires_command_message_actionable() {
        let err = ConfigError::CustomBackendRequiresCommand;
        let msg = err.to_string();
        assert!(msg.contains("cli.command"));
        assert!(msg.contains("ralph init --backend custom"));
        assert!(msg.contains("docs/reference/troubleshooting.md#custom-backend-command"));
    }

    #[test]
    fn test_reserved_trigger_message_actionable() {
        let err = ConfigError::ReservedTrigger { trigger: "task.start".to_string(), hat: "builder".to_string() };
        let msg = err.to_string();
        assert!(msg.contains("Reserved trigger"));
        assert!(msg.contains("docs/reference/troubleshooting.md#reserved-trigger"));
    }

    #[test]
    fn test_prompt_file_with_no_inline_allowed() {
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
        let tui_config = TuiConfig { prefix_key: "invalid".to_string() };
        let result = tui_config.parse_prefix();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid prefix_key format"));
    }

    #[test]
    fn test_tui_config_parse_invalid_modifier() {
        let tui_config = TuiConfig { prefix_key: "alt-a".to_string() };
        let result = tui_config.parse_prefix();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid modifier"));
    }

    #[test]
    fn test_tui_config_parse_invalid_key() {
        let tui_config = TuiConfig { prefix_key: "ctrl-abc".to_string() };
        let result = tui_config.parse_prefix();
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid key"));
    }

    #[test]
    fn test_hat_backend_named() {
        let yaml = r#""claude""#;
        let backend: HatBackend = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(backend.to_cli_backend(), "claude");
        match backend { HatBackend::Named(name) => assert_eq!(name, "claude"), _ => panic!("Expected Named variant") }
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
            HatBackend::KiroAgent { backend_type, agent, args } => {
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
            HatBackend::KiroAgent { backend_type, agent, args } => {
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
        match hat.backend.unwrap() { HatBackend::Named(name) => assert_eq!(name, "gemini"), _ => panic!("Expected Named backend") }
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
        let planner = config.hats.get("planner").unwrap();
        assert!(planner.backend.is_some());
        match planner.backend.as_ref().unwrap() { HatBackend::Named(name) => assert_eq!(name, "claude"), _ => panic!("Expected Named backend for planner") }
        let builder = config.hats.get("builder").unwrap();
        assert!(builder.backend.is_some());
        match builder.backend.as_ref().unwrap() {
            HatBackend::KiroAgent { backend_type, agent, args } => {
                assert_eq!(backend_type, "kiro");
                assert_eq!(agent, "builder");
                assert!(args.is_empty());
            }
            _ => panic!("Expected KiroAgent backend for builder"),
        }
        let reviewer = config.hats.get("reviewer").unwrap();
        assert!(reviewer.backend.is_some());
        match reviewer.backend.as_ref().unwrap() {
            HatBackend::Custom { command, args } => {
                assert_eq!(command, "/usr/local/bin/custom-agent");
                assert_eq!(args, &vec!["--mode".to_string(), "review".to_string()]);
            }
            _ => panic!("Expected Custom backend for reviewer"),
        }
        assert_eq!(reviewer.default_publishes, Some("review.complete".to_string()));
    }

    #[test]
    fn test_features_config_auto_merge_defaults_to_false() {
        let config = RalphConfig::default();
        assert!(!config.features.auto_merge, "auto_merge should default to false");
    }

    #[test]
    fn test_features_config_auto_merge_from_yaml() {
        let yaml = r"
features:
  auto_merge: true
";
        let config: RalphConfig = serde_yaml::from_str(yaml).unwrap();
        assert!(config.features.auto_merge, "auto_merge should be true when configured");
    }

    #[test]
    fn test_features_config_auto_merge_false_from_yaml() {
        let yaml = r"
features:
  auto_merge: false
";
        let config: RalphConfig = serde_yaml::from_str(yaml).unwrap();
        assert!(!config.features.auto_merge, "auto_merge should be false when explicitly configured");
    }

    #[test]
    fn test_features_config_preserves_parallel_when_adding_auto_merge() {
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
        assert_eq!(config.skills.dirs[0], std::path::PathBuf::from(".claude/skills"));
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

    #[test]
    fn test_hooks_config_valid_yaml_parses_and_validates() {
        let yaml = r#"
hooks:
  enabled: true
  defaults:
    timeout_seconds: 45
    max_output_bytes: 16384
    suspend_mode: wait_for_resume
  events:
    pre.loop.start:
      - name: env-guard
        command: ["./scripts/hooks/env-guard.sh", "--check"]
        on_error: block
    post.loop.complete:
      - name: notify
        command: ["./scripts/hooks/notify.sh"]
        on_error: warn
        mutate:
          enabled: true
          format: json
"#;
        let config = RalphConfig::parse_yaml(yaml).unwrap();
        assert!(config.hooks.enabled);
        assert_eq!(config.hooks.defaults.timeout_seconds, 45);
        assert_eq!(config.hooks.defaults.max_output_bytes, 16384);
        assert_eq!(config.hooks.events.len(), 2);
        let warnings = config.validate().unwrap();
        assert!(warnings.is_empty());
    }

    #[test]
    fn test_hooks_parse_rejects_invalid_phase_event_key() {
        let yaml = r#"
hooks:
  enabled: true
  events:
    pre.loop.launch:
      - name: bad-phase
        command: ["./scripts/hooks/bad-phase.sh"]
        on_error: warn
"#;
        let result = RalphConfig::parse_yaml(yaml);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(&err, ConfigError::InvalidHookPhaseEvent { phase_event } if phase_event == "pre.loop.launch"));
    }

    #[test]
    fn test_hooks_parse_rejects_backpressure_phase_event_keys_in_v1() {
        let yaml = r#"
hooks:
  enabled: true
  events:
    pre.backpressure.triggered:
      - name: unsupported-backpressure
        command: ["./scripts/hooks/backpressure.sh"]
        on_error: warn
"#;
        let result = RalphConfig::parse_yaml(yaml);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(&err, ConfigError::InvalidHookPhaseEvent { phase_event } if phase_event == "pre.backpressure.triggered"));
        let message = err.to_string();
        assert!(message.contains("Supported v1 phase-events"));
        assert!(message.contains("pre.plan.created"));
        assert!(message.contains("post.loop.error"));
    }

    #[test]
    fn test_hooks_parse_rejects_invalid_on_error_enum_value() {
        let yaml = r#"
hooks:
  enabled: true
  events:
    pre.loop.start:
      - name: bad-on-error
        command: ["./scripts/hooks/bad-on-error.sh"]
        on_error: explode
"#;
        let result = RalphConfig::parse_yaml(yaml);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(&err, ConfigError::Yaml(_)));
        let message = err.to_string();
        assert!(message.contains("unknown variant `explode`"));
        assert!(message.contains("warn"));
        assert!(message.contains("block"));
        assert!(message.contains("suspend"));
    }

    #[test]
    fn test_hooks_validate_rejects_missing_name() {
        let yaml = r#"
hooks:
  enabled: true
  events:
    pre.loop.start:
      - command: ["./scripts/hooks/no-name.sh"]
        on_error: block
"#;
        let config = RalphConfig::parse_yaml(yaml).unwrap();
        let result = config.validate();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(&err, ConfigError::HookValidation { field, .. } if field == "hooks.events.pre.loop.start[0].name"));
    }

    #[test]
    fn test_hooks_validate_rejects_missing_command() {
        let yaml = r"
hooks:
  enabled: true
  events:
    pre.loop.start:
      - name: missing-command
        on_error: block
";
        let config = RalphConfig::parse_yaml(yaml).unwrap();
        let result = config.validate();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(&err, ConfigError::HookValidation { field, .. } if field == "hooks.events.pre.loop.start[0].command"));
    }

    #[test]
    fn test_hooks_validate_rejects_missing_on_error() {
        let yaml = r#"
hooks:
  enabled: true
  events:
    pre.loop.start:
      - name: missing-on-error
        command: ["./scripts/hooks/no-on-error.sh"]
"#;
        let config = RalphConfig::parse_yaml(yaml).unwrap();
        let result = config.validate();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(&err, ConfigError::HookValidation { field, .. } if field == "hooks.events.pre.loop.start[0].on_error"));
    }

    #[test]
    fn test_hooks_validate_rejects_zero_timeout_seconds() {
        let yaml = r"
hooks:
  enabled: true
  defaults:
    timeout_seconds: 0
";
        let config = RalphConfig::parse_yaml(yaml).unwrap();
        let result = config.validate();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(&err, ConfigError::HookValidation { field, .. } if field == "hooks.defaults.timeout_seconds"));
    }

    #[test]
    fn test_hooks_validate_rejects_zero_max_output_bytes() {
        let yaml = r"
hooks:
  enabled: true
  defaults:
    max_output_bytes: 0
";
        let config = RalphConfig::parse_yaml(yaml).unwrap();
        let result = config.validate();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(&err, ConfigError::HookValidation { field, .. } if field == "hooks.defaults.max_output_bytes"));
    }

    #[test]
    fn test_hooks_validate_rejects_parallel_non_v1_field() {
        let yaml = r"
hooks:
  enabled: true
  parallel: true
";
        let config = RalphConfig::parse_yaml(yaml).unwrap();
        let result = config.validate();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(&err, ConfigError::UnsupportedHookField { field, .. } if field == "hooks.parallel"));
    }

    #[test]
    fn test_hooks_validate_rejects_global_scope_non_v1_field() {
        let yaml = r#"
hooks:
  enabled: true
  events:
    pre.loop.start:
      - name: global-scope
        command: ["./scripts/hooks/global.sh"]
        on_error: warn
        scope: global
"#;
        let config = RalphConfig::parse_yaml(yaml).unwrap();
        let result = config.validate();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(&err, ConfigError::UnsupportedHookField { field, .. } if field == "hooks.events.pre.loop.start[0].scope"));
    }

    #[test]
    fn test_robot_config_defaults_disabled() {
        let config = RalphConfig::default();
        assert!(!config.robot.enabled);
        assert!(config.robot.timeout_seconds.is_none());
        assert!(config.robot.telegram.is_none());
    }

    #[test]
    fn test_robot_config_absent_parses_as_default() {
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
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_robot_config_disabled_skips_validation() {
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
        assert!(matches!(&err, ConfigError::RobotMissingField { field, .. } if field == "RObot.timeout_seconds"), "Expected RobotMissingField for timeout_seconds, got: {:?}", err);
    }

    #[test]
    fn test_robot_config_enabled_missing_timeout_and_token_fails_on_timeout_first() {
        let robot = RobotConfig { enabled: true, timeout_seconds: None, checkin_interval_seconds: None, telegram: None };
        let result = robot.validate();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(&err, ConfigError::RobotMissingField { field, .. } if field == "RObot.timeout_seconds"), "Expected timeout validation failure first, got: {:?}", err);
    }

    #[test]
    fn test_robot_config_resolve_bot_token_from_config() {
        let config = RobotConfig {
            enabled: true,
            timeout_seconds: Some(300),
            checkin_interval_seconds: None,
            telegram: Some(TelegramBotConfig { bot_token: Some("config-token".to_string()), api_url: None }),
        };
        let resolved = config.resolve_bot_token();
        assert!(resolved.is_some());
    }

    #[test]
    fn test_robot_config_resolve_bot_token_none_without_config() {
        let config = RobotConfig { enabled: true, timeout_seconds: Some(300), checkin_interval_seconds: None, telegram: None };
        let resolved = config.resolve_bot_token();
        if std::env::var("RALPH_TELEGRAM_BOT_TOKEN").is_err() {
            assert!(resolved.is_none());
        }
    }

    #[test]
    fn test_robot_config_validate_with_config_token() {
        let robot = RobotConfig {
            enabled: true,
            timeout_seconds: Some(300),
            checkin_interval_seconds: None,
            telegram: Some(TelegramBotConfig { bot_token: Some("test-token".to_string()), api_url: None }),
        };
        assert!(robot.validate().is_ok());
    }

    #[test]
    fn test_robot_config_validate_missing_telegram_section() {
        if std::env::var("RALPH_TELEGRAM_BOT_TOKEN").is_ok() { return; }
        let robot = RobotConfig { enabled: true, timeout_seconds: Some(300), checkin_interval_seconds: None, telegram: None };
        let result = robot.validate();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(&err, ConfigError::RobotMissingField { field, .. } if field == "RObot.telegram.bot_token"), "Expected bot_token validation failure, got: {:?}", err);
    }

    #[test]
    fn test_robot_config_validate_empty_bot_token() {
        if std::env::var("RALPH_TELEGRAM_BOT_TOKEN").is_ok() { return; }
        let robot = RobotConfig {
            enabled: true,
            timeout_seconds: Some(300),
            checkin_interval_seconds: None,
            telegram: Some(TelegramBotConfig { bot_token: None, api_url: None }),
        };
        let result = robot.validate();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(&err, ConfigError::RobotMissingField { field, .. } if field == "RObot.telegram.bot_token"), "Expected bot_token validation failure, got: {:?}", err);
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
        assert_eq!(hat.extra_instructions.len(), 1);
        assert!(!hat.instructions.contains("Shared Protocol"));
        config.normalize();
        let hat = config.hats.get("builder").unwrap();
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

    #[test]
    fn test_wave_config_concurrency_and_aggregate_parse() {
        let yaml = r#"
hats:
  reviewer:
    name: "Reviewer"
    description: "Reviews files in parallel"
    triggers: ["review.file"]
    publishes: ["review.done"]
    instructions: "Review the file."
    concurrency: 3
  aggregator:
    name: "Aggregator"
    description: "Aggregates review results"
    triggers: ["review.done"]
    publishes: ["review.complete"]
    instructions: "Aggregate results."
    aggregate:
      mode: wait_for_all
      timeout: 600
"#;
        let config: RalphConfig = serde_yaml::from_str(yaml).unwrap();
        let reviewer = config.hats.get("reviewer").unwrap();
        assert_eq!(reviewer.concurrency, 3);
        assert!(reviewer.aggregate.is_none());
        let aggregator = config.hats.get("aggregator").unwrap();
        assert_eq!(aggregator.concurrency, 1);
        let agg = aggregator.aggregate.as_ref().unwrap();
        assert!(matches!(agg.mode, AggregateMode::WaitForAll));
        assert_eq!(agg.timeout, 600);
    }

    #[test]
    fn test_wave_config_defaults_without_new_fields() {
        let yaml = r#"
hats:
  builder:
    name: "Builder"
    description: "Builds code"
    triggers: ["build.task"]
    publishes: ["build.done"]
    instructions: "Build stuff."
"#;
        let config: RalphConfig = serde_yaml::from_str(yaml).unwrap();
        let hat = config.hats.get("builder").unwrap();
        assert_eq!(hat.concurrency, 1);
        assert!(hat.aggregate.is_none());
    }

    #[test]
    fn test_wave_config_concurrency_zero_rejected() {
        let yaml = r#"
hats:
  worker:
    name: "Worker"
    description: "Parallel worker"
    triggers: ["work.item"]
    publishes: ["work.done"]
    instructions: "Do work."
    concurrency: 0
"#;
        let config: RalphConfig = serde_yaml::from_str(yaml).unwrap();
        let result = config.validate();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(&err, ConfigError::InvalidConcurrency { hat, .. } if hat == "worker"), "Expected InvalidConcurrency error, got: {:?}", err);
    }

    #[test]
    fn test_wave_config_aggregate_on_concurrent_hat_rejected() {
        let yaml = r#"
hats:
  hybrid:
    name: "Hybrid"
    description: "Invalid: both concurrent and aggregator"
    triggers: ["work.item"]
    publishes: ["work.done"]
    instructions: "Invalid config."
    concurrency: 3
    aggregate:
      mode: wait_for_all
      timeout: 300
"#;
        let config: RalphConfig = serde_yaml::from_str(yaml).unwrap();
        let result = config.validate();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(matches!(&err, ConfigError::AggregateOnConcurrentHat { hat, .. } if hat == "hybrid"), "Expected AggregateOnConcurrentHat error, got: {:?}", err);
    }

    #[test]
    fn test_wave_config_aggregate_on_non_concurrent_hat_valid() {
        let yaml = r#"
hats:
  aggregator:
    name: "Aggregator"
    description: "Collects results"
    triggers: ["work.done"]
    publishes: ["work.complete"]
    instructions: "Aggregate."
    aggregate:
      mode: wait_for_all
      timeout: 300
"#;
        let config: RalphConfig = serde_yaml::from_str(yaml).unwrap();
        let result = config.validate();
        assert!(result.is_ok(), "Aggregate on non-concurrent hat should be valid: {:?}", result.unwrap_err());
    }
}
