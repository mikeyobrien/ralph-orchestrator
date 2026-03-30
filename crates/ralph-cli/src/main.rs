//! # ralph-cli
//!
//! Binary entry point for the Ralph Orchestrator.
//!
//! This crate provides:
//! - CLI argument parsing using `clap`
//! - Application initialization and configuration
//! - Entry point to the headless orchestration loop
//! - Event history viewing via `ralph events`
//! - Project initialization via `ralph init`
//! - SOP-based planning via `ralph plan`
//! - Code task generation via `ralph code-task`
//! - Hook config validation via `ralph hooks validate`
//! - Work item tracking via `ralph task`

mod backend_support;
mod bot;
mod config_resolution;
mod display;
mod doctor;
mod hats;
mod hooks;
mod init;
mod interact;
mod loop_runner;
mod loops;
mod mcp;
mod memory;
mod preflight;
mod presets;
mod rpc_stdin;
mod skill_cli;
mod sop_runner;
mod task_cli;
#[cfg(test)]
mod test_support;
mod tools;
mod wave;
mod web;

use anyhow::{Context, Result};
use clap::{ArgAction, CommandFactory, Parser, Subcommand, ValueEnum};
use ralph_adapters::detect_backend;
use ralph_core::{
    CheckStatus, EventHistory, LockError, LoopContext, LoopEntry, LoopLock, LoopRegistry,
    PreflightReport, PreflightRunner, RalphConfig, TerminationReason, UrgentSteerStore,
    truncate_with_ellipsis,
    worktree::{WorktreeConfig, create_worktree, ensure_gitignore, remove_worktree},
};
use std::fs;
use std::io::{IsTerminal, Write, stdout};
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};

// Unix-specific process management for process group leadership
#[cfg(unix)]
mod process_management {
    use nix::unistd::{Pid, getpgrp, setpgid, tcgetpgrp};
    use std::io::{IsTerminal, stdin, stdout};
    use tracing::debug;

    /// Sets up process group leadership.
    ///
    /// Per spec: "The orchestrator must run as a process group leader. All spawned
    /// CLI processes (Claude, Kiro, etc.) belong to this group. On termination,
    /// the entire process group receives the signal, preventing orphans."
    pub fn setup_process_group() {
        // Make ourselves the process group leader when safe.
        // If we're launched by a wrapper (e.g., `npx`), moving to a new process
        // group can drop us out of the foreground TTY group and break TUI input.
        let pid = Pid::this();
        let pgrp = getpgrp();
        if pgrp == pid {
            debug!("Already process group leader: PID {}", pid);
            return;
        }

        if is_foreground_tty_group(pgrp) {
            debug!(
                "Skipping setpgid: keeping foreground process group {}",
                pgrp
            );
            return;
        }

        if let Err(e) = setpgid(pid, pid) {
            // EPERM is OK - we're already a process group leader (e.g., started from shell)
            if e != nix::errno::Errno::EPERM {
                debug!(
                    "Note: Could not set process group ({}), continuing anyway",
                    e
                );
            }
        }
        debug!("Process group initialized: PID {}", pid);
    }

    fn is_foreground_tty_group(current_pgrp: Pid) -> bool {
        // Prefer stdin for foreground checks, fall back to stdout.
        if stdin().is_terminal()
            && let Ok(fg) = tcgetpgrp(stdin())
        {
            return fg == current_pgrp;
        }

        if stdout().is_terminal()
            && let Ok(fg) = tcgetpgrp(stdout())
        {
            return fg == current_pgrp;
        }

        false
    }
}

#[cfg(not(unix))]
mod process_management {
    /// No-op on non-Unix platforms.
    pub fn setup_process_group() {}
}

/// Installs a panic hook that restores terminal state before printing panic info.
///
/// When a TUI application panics, the terminal can be left in a broken state:
/// - Raw mode enabled (input not line-buffered)
/// - Alternate screen buffer active (no scrollback)
/// - Cursor hidden
///
/// This hook ensures the terminal is restored so the panic message is visible
/// and the user can scroll/interact normally.
fn install_panic_hook() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        // Restore terminal state before printing panic info
        let _ = crossterm::terminal::disable_raw_mode();
        let _ = crossterm::execute!(
            std::io::stdout(),
            crossterm::terminal::LeaveAlternateScreen,
            crossterm::cursor::Show
        );
        // Call the default panic hook to print the panic message
        default_hook(panic_info);
    }));
}

/// Color output mode for terminal display.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, ValueEnum)]
pub enum ColorMode {
    /// Automatically detect if stdout is a TTY
    #[default]
    Auto,
    /// Always use colors
    Always,
    /// Never use colors
    Never,
}

impl ColorMode {
    /// Returns true if colors should be used based on mode and terminal detection.
    fn should_use_colors(self) -> bool {
        // NO_COLOR is a de-facto cross-tooling convention and should disable ANSI
        // colors by default, regardless of output mode.
        if std::env::var("NO_COLOR").is_ok() {
            return false;
        }

        match self {
            ColorMode::Always => true,
            ColorMode::Never => false,
            ColorMode::Auto => stdout().is_terminal(),
        }
    }
}

/// Returns the default config source path.
///
/// `RALPH_CONFIG` (if set) is used before the hardcoded fallback to `ralph.yml`.
pub(crate) fn default_config_path() -> PathBuf {
    if let Ok(value) = std::env::var("RALPH_CONFIG")
        && !value.trim().is_empty()
    {
        return PathBuf::from(value);
    }

    PathBuf::from("ralph.yml")
}

pub(crate) fn resolve_workspace_root(root: Option<&PathBuf>) -> PathBuf {
    if let Some(root) = root {
        return root.clone();
    }

    if let Ok(value) = std::env::var("RALPH_WORKSPACE_ROOT")
        && !value.trim().is_empty()
    {
        return PathBuf::from(value);
    }

    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    discover_workspace_root(&cwd).unwrap_or(cwd)
}

pub(crate) fn resolve_path_from_workspace(
    path: impl AsRef<Path>,
    root: Option<&PathBuf>,
) -> PathBuf {
    resolve_workspace_root(root).join(path)
}

fn urgent_steer_path_from_workspace(root: Option<&PathBuf>) -> PathBuf {
    resolve_workspace_root(root).join(".ralph/urgent-steer.json")
}

pub(crate) fn discover_workspace_root(start: &Path) -> Option<PathBuf> {
    start.ancestors().find_map(|dir| {
        let has_ralph = dir.join(".ralph").is_dir();
        let has_git = dir.join(".git").exists();
        if has_ralph || has_git {
            Some(dir.to_path_buf())
        } else {
            None
        }
    })
}

fn resolve_marker_target(workspace_root: &Path, marker_value: &str) -> PathBuf {
    let path = PathBuf::from(marker_value.trim());
    if path.is_absolute() {
        path
    } else {
        workspace_root.join(path)
    }
}

/// Verbosity level for streaming output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Verbosity {
    /// Suppress all streaming output (for CI/scripting)
    Quiet,
    /// Show assistant text and tool invocations (default)
    #[default]
    Normal,
    /// Show everything including tool results and session summary
    Verbose,
}

impl Verbosity {
    /// Resolves verbosity from CLI args, env vars, and config.
    ///
    /// Precedence (highest to lowest):
    /// 1. CLI flags: `--verbose`/`-v` or `--quiet`/`-q`
    /// 2. Environment variables: `RALPH_VERBOSE=1` or `RALPH_QUIET=1`
    /// 3. Config file: (if supported in future)
    /// 4. Default: Normal
    fn resolve(cli_verbose: bool, cli_quiet: bool) -> Self {
        let env_quiet = std::env::var("RALPH_QUIET").is_ok();
        let env_verbose = std::env::var("RALPH_VERBOSE").is_ok();
        Self::resolve_with_env(cli_verbose, cli_quiet, env_quiet, env_verbose)
    }

    #[allow(clippy::fn_params_excessive_bools)]
    fn resolve_with_env(
        cli_verbose: bool,
        cli_quiet: bool,
        env_quiet: bool,
        env_verbose: bool,
    ) -> Self {
        // CLI flags take precedence
        if cli_quiet {
            return Verbosity::Quiet;
        }
        if cli_verbose {
            return Verbosity::Verbose;
        }

        // Environment variables
        if env_quiet {
            return Verbosity::Quiet;
        }
        if env_verbose {
            return Verbosity::Verbose;
        }

        Verbosity::Normal
    }
}

/// Output format for events command.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, ValueEnum)]
pub enum OutputFormat {
    /// Human-readable table format
    #[default]
    Table,
    /// JSON format for programmatic access
    Json,
}

// Re-export colors and truncate from display module for use in this file
use display::colors;
use display::truncate;

/// Source for core configuration.
#[derive(Debug, Clone)]
pub enum ConfigSource {
    /// Local file path (default behavior)
    File(PathBuf),
    /// Legacy builtin preset source (no longer valid for core config).
    ///
    /// Kept so we can emit actionable migration errors.
    Builtin(String),
    /// Remote URL (e.g., "http://example.com/ralph.core.yml")
    Remote(String),
    /// Config override (e.g., "core.scratchpad=.ralph/feature/scratchpad.md")
    Override { key: String, value: String },
}

impl ConfigSource {
    /// Parse a core config source string into its variant.
    ///
    /// Format:
    /// - `core.field=value` → Override (for core.* fields)
    /// - `builtin:preset-name` → Legacy builtin preset (rejected with migration message)
    /// - `http://...` or `https://...` → Remote URL
    /// - Anything else → File path
    fn parse(s: &str) -> Self {
        // Check for core.* override pattern first (prevents false positives on paths with '=')
        // Only treat as override if it starts with "core." AND contains '='
        if s.starts_with("core.")
            && let Some((key, value)) = s.split_once('=')
        {
            return ConfigSource::Override {
                key: key.to_string(),
                value: value.to_string(),
            };
        }

        if let Some(name) = s.strip_prefix("builtin:") {
            ConfigSource::Builtin(name.to_string())
        } else if s.starts_with("http://") || s.starts_with("https://") {
            ConfigSource::Remote(s.to_string())
        } else {
            ConfigSource::File(PathBuf::from(s))
        }
    }

    /// Convert back to CLI string representation for forwarding to subprocess.
    fn to_cli_string(&self) -> String {
        match self {
            ConfigSource::File(path) => path.display().to_string(),
            ConfigSource::Builtin(name) => format!("builtin:{}", name),
            ConfigSource::Remote(url) => url.clone(),
            ConfigSource::Override { key, value } => format!("{}={}", key, value),
        }
    }
}

/// Source for hat collection configuration.
#[derive(Debug, Clone)]
pub enum HatsSource {
    /// Local file path
    File(PathBuf),
    /// Builtin hat collection name (e.g., "builtin:code-assist")
    Builtin(String),
    /// Remote URL (e.g., "http://example.com/hats.yml")
    Remote(String),
}

impl HatsSource {
    /// Parse a hats source string into its variant.
    fn parse(s: &str) -> Self {
        if let Some(name) = s.strip_prefix("builtin:") {
            HatsSource::Builtin(name.to_string())
        } else if s.starts_with("http://") || s.starts_with("https://") {
            HatsSource::Remote(s.to_string())
        } else {
            HatsSource::File(PathBuf::from(s))
        }
    }

    /// Human-readable source label.
    pub fn label(&self) -> String {
        match self {
            HatsSource::File(path) => path.display().to_string(),
            HatsSource::Builtin(name) => format!("builtin:{}", name),
            HatsSource::Remote(url) => url.clone(),
        }
    }
}

/// Known core fields that can be overridden via CLI.
const KNOWN_CORE_FIELDS: &[&str] = &["scratchpad", "specs_dir"];

/// Applies CLI config overrides to the loaded configuration.
///
/// Overrides are in the format `core.field=value` and take precedence
/// over values from the config file.
pub(crate) fn apply_config_overrides(
    config: &mut RalphConfig,
    sources: &[ConfigSource],
) -> anyhow::Result<()> {
    for source in sources {
        if let ConfigSource::Override { key, value } = source {
            match key.as_str() {
                "core.scratchpad" => {
                    config.core.scratchpad.path = value.clone();
                }
                "core.specs_dir" => {
                    config.core.specs_dir = value.clone();
                }
                other => {
                    // Note: with core.* prefix requirement in parse(), this branch
                    // only handles unknown core.* fields
                    let field = other.strip_prefix("core.").unwrap_or(other);
                    warn!(
                        "Unknown core field '{}'. Known fields: {}",
                        field,
                        KNOWN_CORE_FIELDS.join(", ")
                    );
                }
            }
        }
    }
    Ok(())
}

/// Ensures the scratchpad's parent directory exists, creating it if needed.
pub(crate) fn ensure_scratchpad_directory(config: &RalphConfig) -> anyhow::Result<()> {
    let scratchpad_path = config.core.resolve_path(&config.core.scratchpad.path);
    if let Some(parent) = scratchpad_path.parent()
        && !parent.exists()
    {
        info!("Creating scratchpad directory: {}", parent.display());
        std::fs::create_dir_all(parent)?;
    }
    Ok(())
}

/// Loads configuration from file sources with override support.
///
/// This is the common sync path used by resume_command and clean_command.
/// For the full async path (including Remote URLs), see run_command.
///
/// Returns the loaded config with overrides applied and workspace_root set.
pub(crate) fn load_config_with_overrides(
    config_sources: &[ConfigSource],
) -> anyhow::Result<RalphConfig> {
    let (primary_sources, overrides) = config_resolution::split_config_sources(config_sources);
    if primary_sources.len() > 1 {
        warn!("Multiple config sources specified, using first one. Others ignored.");
    }

    let (primary_value, primary_label, primary_uses_defaults) = match primary_sources.first() {
        Some(ConfigSource::File(path)) => {
            if path.exists() {
                let label = path.display().to_string();
                let content = std::fs::read_to_string(path)
                    .with_context(|| format!("Failed to load config from {}", label))?;
                let value = config_resolution::parse_yaml_value(&content, &label)?;
                (Some(value), label, false)
            } else {
                warn!("Config file {:?} not found, using defaults", path);
                (None, path.display().to_string(), false)
            }
        }
        Some(ConfigSource::Builtin(name)) => {
            anyhow::bail!(
                "`-c builtin:{name}` is no longer supported.\n\nBuiltin presets are now hat collections.\nUse:\n  ralph run -c ralph.yml -H builtin:{name}"
            );
        }
        Some(ConfigSource::Remote(url)) => {
            anyhow::bail!(
                "Remote core config sources are not supported for this command: {}",
                url
            );
        }
        Some(ConfigSource::Override { .. }) => unreachable!("Overrides are partitioned out"),
        None => {
            let default_path = default_config_path();
            if default_path.exists() {
                let label = default_path.display().to_string();
                let content = std::fs::read_to_string(&default_path)
                    .with_context(|| format!("Failed to load config from {}", label))?;
                let value = config_resolution::parse_yaml_value(&content, &label)?;
                (Some(value), label, false)
            } else {
                warn!(
                    "Config file {} not found, using defaults",
                    default_path.display()
                );
                (None, default_path.display().to_string(), true)
            }
        }
    };

    let user_layer = config_resolution::load_optional_user_config_value()?;
    let mut merged_value = config_resolution::default_core_value()?;
    if let Some((user_value, _)) = &user_layer {
        merged_value = config_resolution::merge_yaml_values(merged_value, user_value.clone())?;
    }
    if let Some(primary_value) = primary_value {
        merged_value = config_resolution::merge_yaml_values(merged_value, primary_value)?;
    }

    let merged_label = config_resolution::compose_core_label(
        user_layer.as_ref().map(|(_, label)| label.as_str()),
        &primary_label,
        primary_uses_defaults,
    );

    let mut config: RalphConfig = serde_yaml::from_value(merged_value)
        .with_context(|| format!("Failed to parse merged core config from {}", merged_label))?;

    config.normalize();

    // Set workspace_root to current directory
    config.core.workspace_root =
        std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));

    // Apply CLI config overrides
    apply_config_overrides(&mut config, &overrides)?;

    Ok(config)
}

/// Ralph Orchestrator - Multi-agent orchestration framework
#[derive(Parser, Debug)]
#[command(name = "ralph", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    // ─────────────────────────────────────────────────────────────────────────
    // Global options (available for all subcommands)
    // ─────────────────────────────────────────────────────────────────────────
    /// Core configuration source: file path, URL, or core.field=value override.
    /// Can be specified multiple times. Overrides are applied after core config loading.
    /// If not set, defaults to `ralph.yml` or `$RALPH_CONFIG`.
    #[arg(short, long, global = true, action = ArgAction::Append)]
    config: Vec<String>,

    /// Hat collection source: file path, builtin:name, or URL.
    ///
    /// Example: `-H builtin:code-assist` or `-H .ralph/hats/my-workflow.yml`
    #[arg(short = 'H', long, global = true)]
    hats: Option<String>,

    /// Verbose output
    #[arg(short, long, global = true)]
    verbose: bool,

    /// Color output mode (auto, always, never)
    #[arg(long, value_enum, default_value_t = ColorMode::Auto, global = true)]
    color: ColorMode,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Run the orchestration loop (default if no subcommand given)
    Run(RunArgs),

    /// Run preflight checks to validate configuration and environment
    Preflight(preflight::PreflightArgs),

    /// Validate hooks configuration and command wiring
    Hooks(hooks::HooksArgs),

    /// Run first-run diagnostics and environment checks
    Doctor(doctor::DoctorArgs),

    /// Interactive walkthrough of hats, hat collections, and workflow
    Tutorial(TutorialArgs),

    /// DEPRECATED: Use `ralph run --continue` instead.
    /// Resume a previously interrupted loop from existing scratchpad.
    #[command(hide = true)]
    Resume(ResumeArgs),

    /// View event history for debugging
    Events(EventsArgs),

    /// Initialize a new ralph.yml configuration file
    Init(InitArgs),

    /// Clean up Ralph artifacts from `.ralph/agent`.
    Clean(CleanArgs),

    /// Emit an event to the current run's events file with proper JSON formatting
    Emit(EmitArgs),

    /// Start a Prompt-Driven Development planning session
    Plan(PlanArgs),

    /// Generate code task files from descriptions or plans
    CodeTask(CodeTaskArgs),

    /// Legacy alias for `code-task` (runtime tasks are `ralph tools task`).
    #[command(hide = true)]
    Task(CodeTaskArgs),

    /// Ralph's runtime tools (agent-facing)
    Tools(tools::ToolsArgs),

    /// Dispatch wave events for parallel hat execution
    Wave(wave::WaveArgs),

    /// Manage parallel loops
    Loops(loops::LoopsArgs),

    /// Manage configured hats
    Hats(hats::HatsArgs),

    /// Attach a TUI to a running ralph-api server
    Tui(TuiArgs),

    /// Run the web dashboard
    Web(web::WebArgs),

    /// Run Ralph as an MCP server over stdio
    Mcp(mcp::McpArgs),

    /// Manage Telegram bot setup and testing
    Bot(bot::BotArgs),

    /// Generate shell completions
    Completions(CompletionsArgs),
}

/// Arguments for the init subcommand.
#[derive(Parser, Debug)]
struct InitArgs {
    /// Backend to use (claude, kiro, gemini, codex, amp, copilot, opencode, pi, custom).
    /// Generates core config only.
    #[arg(long, conflicts_with = "list_presets")]
    backend: Option<String>,

    /// REMOVED: monolithic presets are no longer supported.
    ///
    /// Use split config instead:
    ///   ralph init --backend <backend>
    ///   ralph run -c ralph.yml -H builtin:<collection>
    #[arg(long, conflicts_with = "list_presets", conflicts_with = "backend")]
    preset: Option<String>,

    /// List all available builtin hat collections
    #[arg(long, conflicts_with = "backend", conflicts_with = "preset")]
    list_presets: bool,

    /// Overwrite existing ralph.yml if present
    #[arg(long)]
    force: bool,
}

/// Arguments for the run subcommand.
#[derive(Parser, Debug)]
struct RunArgs {
    /// Inline prompt text (mutually exclusive with -P/--prompt-file)
    #[arg(short = 'p', long = "prompt", conflicts_with = "prompt_file")]
    prompt_text: Option<String>,

    /// Override backend from config (cli > config > auto-detect)
    #[arg(short = 'b', long = "backend", value_name = "BACKEND")]
    backend: Option<String>,

    /// Prompt file path (mutually exclusive with -p/--prompt)
    #[arg(short = 'P', long = "prompt-file", conflicts_with = "prompt_text")]
    prompt_file: Option<PathBuf>,

    /// Override max iterations
    #[arg(long)]
    max_iterations: Option<u32>,

    /// Override completion promise
    #[arg(long)]
    completion_promise: Option<String>,

    /// Dry run - show what would be executed without running
    #[arg(long)]
    dry_run: bool,

    /// Continue from existing scratchpad (resume interrupted loop).
    /// Use this when a previous run was interrupted and you want to
    /// continue from where it left off.
    #[arg(long = "continue")]
    continue_mode: bool,

    /// Explicit loop ID to use with --continue.
    /// Reuses tasks from the specified loop instead of generating a new ID.
    /// If omitted with --continue, reuses the existing current-loop-id marker.
    #[arg(long, requires = "continue_mode")]
    loop_id: Option<String>,

    // ─────────────────────────────────────────────────────────────────────────
    // Execution Mode Options
    // ─────────────────────────────────────────────────────────────────────────
    /// Disable TUI observation mode (TUI is enabled by default)
    #[arg(long, conflicts_with = "autonomous")]
    no_tui: bool,

    /// Force autonomous mode (headless, non-interactive).
    /// Overrides default_mode from config.
    #[arg(short, long, conflicts_with = "no_tui", conflicts_with = "rpc")]
    autonomous: bool,

    /// Run in RPC mode with JSON-lines protocol on stdin/stdout.
    /// All output is valid JSON; input accepts RpcCommand messages.
    /// Use this for IDE integrations and machine-readable interfaces.
    #[arg(long, conflicts_with = "no_tui", conflicts_with = "autonomous")]
    rpc: bool,

    /// Use legacy in-process TUI mode instead of subprocess RPC mode.
    /// This is an escape hatch during the migration to subprocess TUI.
    #[arg(long, hide = true, conflicts_with = "rpc", conflicts_with = "no_tui")]
    legacy_tui: bool,

    /// Idle timeout in seconds for interactive mode (default: 30).
    /// Process is terminated after this many seconds of inactivity.
    /// Set to 0 to disable idle timeout.
    #[arg(long)]
    idle_timeout: Option<u32>,

    // ─────────────────────────────────────────────────────────────────────────
    // Multi-Loop Concurrency Options
    // ─────────────────────────────────────────────────────────────────────────
    /// Wait for the primary loop slot instead of spawning into a worktree.
    /// Use this when you want to ensure only one loop runs at a time.
    #[arg(long)]
    exclusive: bool,

    /// Skip automatic merge after loop completes (keep worktree for manual handling).
    /// Only relevant for parallel loops running in worktrees.
    #[arg(long)]
    no_auto_merge: bool,

    // ─────────────────────────────────────────────────────────────────────────
    // Preflight Options
    // ─────────────────────────────────────────────────────────────────────────
    /// Skip preflight checks before loop start.
    /// Overrides features.preflight.enabled from config.
    #[arg(long)]
    skip_preflight: bool,

    // ─────────────────────────────────────────────────────────────────────────
    // Verbosity Options
    // ─────────────────────────────────────────────────────────────────────────
    /// Enable verbose output (show tool results and session summary)
    #[arg(short = 'v', long, conflicts_with = "quiet")]
    verbose: bool,

    /// Suppress streaming output (for CI/scripting)
    #[arg(short = 'q', long, conflicts_with = "verbose")]
    quiet: bool,

    /// Record session to JSONL file for replay testing
    #[arg(long, value_name = "FILE")]
    record_session: Option<PathBuf>,

    /// Custom backend command and arguments (use after --)
    #[arg(last = true)]
    custom_args: Vec<String>,
}

/// Arguments for the resume subcommand.
///
/// Per spec: "When loop terminates due to safeguard (not completion promise),
/// user can run `ralph resume` to restart reading existing scratchpad."
#[derive(Parser, Debug)]
struct ResumeArgs {
    /// Override max iterations (from current position)
    #[arg(long)]
    max_iterations: Option<u32>,

    /// Disable TUI observation mode (TUI is enabled by default)
    #[arg(long, conflicts_with = "autonomous")]
    no_tui: bool,

    /// Force autonomous mode
    #[arg(short, long, conflicts_with = "no_tui", conflicts_with = "rpc")]
    autonomous: bool,

    /// Run in RPC mode with JSON-lines protocol on stdin/stdout.
    #[arg(long, conflicts_with = "no_tui", conflicts_with = "autonomous")]
    rpc: bool,

    /// Idle timeout in seconds for TUI mode
    #[arg(long)]
    idle_timeout: Option<u32>,

    /// Enable verbose output (show tool results and session summary)
    #[arg(short = 'v', long, conflicts_with = "quiet")]
    verbose: bool,

    /// Suppress streaming output (for CI/scripting)
    #[arg(short = 'q', long, conflicts_with = "verbose")]
    quiet: bool,

    /// Record session to JSONL file for replay testing
    #[arg(long, value_name = "FILE")]
    record_session: Option<PathBuf>,
}

/// Arguments for the events subcommand.
#[derive(Parser, Debug)]
struct EventsArgs {
    /// Show only the last N events
    #[arg(long)]
    last: Option<usize>,

    /// Filter by topic (e.g., "build.blocked")
    #[arg(long)]
    topic: Option<String>,

    /// Filter by iteration number
    #[arg(long)]
    iteration: Option<u32>,

    /// Output format
    #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
    format: OutputFormat,

    /// Path to events file (default: auto-detects current run)
    #[arg(long)]
    file: Option<PathBuf>,

    /// Clear the event history
    #[arg(long)]
    clear: bool,
}

/// Arguments for the clean subcommand.
#[derive(Parser, Debug)]
struct CleanArgs {
    /// Preview what would be deleted without actually deleting
    #[arg(long)]
    dry_run: bool,

    /// Clean diagnostic logs instead of `.ralph/` directory
    #[arg(long)]
    diagnostics: bool,
}

/// Arguments for the emit subcommand.
#[derive(Parser, Debug)]
struct EmitArgs {
    /// Event topic (e.g., "build.done", "review.complete")
    pub topic: String,

    /// Event payload - string or JSON (optional, defaults to empty)
    #[arg(default_value = "")]
    pub payload: String,

    /// Parse payload as JSON object instead of string
    #[arg(long, short)]
    pub json: bool,

    /// Custom ISO 8601 timestamp (defaults to current time)
    #[arg(long)]
    pub ts: Option<String>,

    /// Path to events file (defaults to .ralph/events.jsonl)
    #[arg(long, default_value = ".ralph/events.jsonl")]
    pub file: PathBuf,
}

/// Arguments for the tutorial subcommand.
#[derive(Parser, Debug)]
struct TutorialArgs {
    /// Skip prompts and print the tutorial in one pass
    #[arg(long)]
    no_input: bool,
}

/// Arguments for the plan subcommand.
///
/// Starts an interactive PDD (Prompt-Driven Development) session.
/// This is a thin wrapper that spawns the AI backend with the bundled
/// PDD SOP, bypassing Ralph's event loop entirely.
#[derive(Parser, Debug)]
struct PlanArgs {
    /// The rough idea to develop (optional - SOP will prompt if not provided)
    #[arg(value_name = "IDEA")]
    idea: Option<String>,

    /// Backend to use (overrides config and auto-detection)
    #[arg(short, long, value_name = "BACKEND")]
    backend: Option<String>,

    /// Enable Claude Code's experimental Agent Teams feature
    #[arg(long)]
    teams: bool,

    /// Custom backend command and arguments (use after --)
    #[arg(last = true)]
    custom_args: Vec<String>,
}

/// Arguments for the task subcommand.
///
/// Starts an interactive code-task-generator session.
/// This is a thin wrapper that spawns the AI backend with the bundled
/// code-task-generator SOP, bypassing Ralph's event loop entirely.
#[derive(Parser, Debug)]
struct CodeTaskArgs {
    /// Input: description text or path to PDD plan file
    #[arg(value_name = "INPUT")]
    input: Option<String>,

    /// Backend to use (overrides config and auto-detection)
    #[arg(short, long, value_name = "BACKEND")]
    backend: Option<String>,

    /// Enable Claude Code's experimental Agent Teams feature
    #[arg(long)]
    teams: bool,

    /// Custom backend command and arguments (use after --)
    #[arg(last = true)]
    custom_args: Vec<String>,
}

/// Arguments for the `ralph tui` subcommand.
#[derive(Parser, Debug)]
struct TuiArgs {
    /// ralph-api server URL to connect to.
    /// Defaults to RALPH_API_URL env var, or http://127.0.0.1:3000.
    #[arg(short = 'u', long = "url")]
    url: Option<String>,
}

/// Arguments for the completions subcommand.
#[derive(Parser, Debug)]
struct CompletionsArgs {
    /// Shell to generate completions for
    #[arg(value_enum)]
    shell: clap_complete::Shell,
}

async fn tui_command(args: TuiArgs) -> Result<()> {
    use ralph_tui::Tui;

    let url = args
        .url
        .or_else(|| std::env::var("RALPH_API_URL").ok())
        .unwrap_or_else(|| "http://127.0.0.1:3000".to_string());

    info!(url = %url, "Attaching TUI to ralph-api server");

    let tui =
        Tui::connect(&url).with_context(|| format!("Failed to create TUI client for {url}"))?;

    tui.run().await.context("TUI exited with error")
}

fn completions_command(args: CompletionsArgs) -> Result<()> {
    use clap_complete::generate;
    use std::io::ErrorKind;

    let mut cli = Cli::command();

    // Generate into a buffer first so we can handle broken pipe errors
    // from shell consumers like `| head` without surfacing a panic.
    let mut output = Vec::new();
    generate(args.shell, &mut cli, "ralph", &mut output);

    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    handle.write_all(&output).or_else(|e| {
        if e.kind() == ErrorKind::BrokenPipe {
            Ok(())
        } else {
            Err(e)
        }
    })?;

    Ok(())
}

/// Returns true if the given command is eligible for diagnostics session creation.
/// Only `run` and `resume` commands (and the default no-subcommand case) should
/// create diagnostics session directories. Other subcommands like `emit` or `tools`
/// would otherwise create empty session dirs.
fn is_diagnostics_eligible_command(command: Option<&Commands>) -> bool {
    matches!(command, Some(Commands::Run(_) | Commands::Resume(_)) | None)
}

#[tokio::main]
async fn main() -> Result<()> {
    // Install panic hook to restore terminal state on crash
    // This prevents the terminal from being left in raw mode or alternate screen
    install_panic_hook();

    let cli = Cli::parse();

    // Detect if TUI mode is requested - TUI owns the terminal, so logs must not go to stdout
    // TUI is enabled by default unless --no-tui, --autonomous, or --rpc is specified
    // RPC mode also suppresses stdout logging (JSON-only output)
    let tui_enabled = match &cli.command {
        Some(Commands::Run(args)) => !args.no_tui && !args.autonomous && !args.rpc,
        Some(Commands::Resume(args)) => !args.no_tui && !args.autonomous && !args.rpc,
        None => true,
        _ => false,
    };
    let rpc_enabled = match &cli.command {
        Some(Commands::Run(args)) => args.rpc,
        Some(Commands::Resume(args)) => args.rpc,
        _ => false,
    };
    let mcp_enabled = matches!(&cli.command, Some(Commands::Mcp(_)));

    // Initialize logging - suppress in TUI mode to avoid corrupting the display
    let filter = if cli.verbose { "debug" } else { "info" };

    // Check if diagnostics are enabled
    let diagnostics_enabled = is_diagnostics_eligible_command(cli.command.as_ref())
        && std::env::var("RALPH_DIAGNOSTICS")
            .map(|v| v == "1")
            .unwrap_or(false);

    if tui_enabled {
        // TUI mode: logs would corrupt the display, so write to a rotating log file
        if let Ok((file, _log_path)) =
            ralph_core::diagnostics::create_log_file(std::path::Path::new("."))
        {
            if diagnostics_enabled {
                use ralph_core::diagnostics::DiagnosticTraceLayer;
                use tracing_subscriber::prelude::*;

                if let Ok(collector) =
                    ralph_core::diagnostics::DiagnosticsCollector::new(std::path::Path::new("."))
                    && let Some(session_dir) = collector.session_dir()
                {
                    if let Ok(trace_layer) = DiagnosticTraceLayer::new(session_dir) {
                        tracing_subscriber::registry()
                            .with(
                                tracing_subscriber::fmt::layer()
                                    .with_writer(std::sync::Mutex::new(file))
                                    .with_ansi(false),
                            )
                            .with(tracing_subscriber::EnvFilter::new(filter))
                            .with(trace_layer)
                            .init();
                    } else {
                        tracing_subscriber::fmt()
                            .with_env_filter(filter)
                            .with_writer(std::sync::Mutex::new(file))
                            .with_ansi(false)
                            .init();
                    }
                }
            } else {
                tracing_subscriber::fmt()
                    .with_env_filter(filter)
                    .with_writer(std::sync::Mutex::new(file))
                    .with_ansi(false)
                    .init();
            }
        }
        // If log file creation fails, silently continue without logging
    } else if rpc_enabled || mcp_enabled {
        // RPC/MCP mode: logs must go to stderr to keep stdout clean for protocol messages
        tracing_subscriber::fmt()
            .with_env_filter(filter)
            .with_writer(std::io::stderr)
            .init();
    } else {
        // Normal mode: logs go to stdout
        if diagnostics_enabled {
            // Normal mode + diagnostics: stdout + trace layer
            use ralph_core::diagnostics::DiagnosticTraceLayer;
            use tracing_subscriber::prelude::*;

            if let Ok(collector) =
                ralph_core::diagnostics::DiagnosticsCollector::new(std::path::Path::new("."))
                && let Some(session_dir) = collector.session_dir()
            {
                if let Ok(trace_layer) = DiagnosticTraceLayer::new(session_dir) {
                    tracing_subscriber::registry()
                        .with(tracing_subscriber::fmt::layer())
                        .with(tracing_subscriber::EnvFilter::new(filter))
                        .with(trace_layer)
                        .init();
                } else {
                    // Fallback: just stdout
                    tracing_subscriber::fmt().with_env_filter(filter).init();
                }
            } else {
                // Fallback: just stdout
                tracing_subscriber::fmt().with_env_filter(filter).init();
            }
        } else {
            // Normal mode without diagnostics: just stdout
            tracing_subscriber::fmt().with_env_filter(filter).init();
        }
    }

    // Parse all config sources from CLI
    let config_values: Vec<String> = if cli.config.is_empty() {
        vec![default_config_path().to_string_lossy().to_string()]
    } else {
        cli.config.clone()
    };

    let config_sources: Vec<ConfigSource> = config_values
        .iter()
        .map(|s| ConfigSource::parse(s))
        .collect();
    let hats_source = cli.hats.as_deref().map(HatsSource::parse);

    match cli.command {
        Some(Commands::Run(args)) => {
            run_command(
                &config_sources,
                hats_source.as_ref(),
                cli.verbose,
                cli.color,
                args,
            )
            .await
        }
        Some(Commands::Preflight(args)) => {
            preflight::execute(
                &config_sources,
                hats_source.as_ref(),
                args,
                cli.color.should_use_colors(),
            )
            .await
        }
        Some(Commands::Hooks(args)) => {
            hooks::execute(
                &config_sources,
                hats_source.as_ref(),
                args,
                cli.color.should_use_colors(),
            )
            .await
        }
        Some(Commands::Doctor(args)) => {
            doctor::execute(
                &config_sources,
                hats_source.as_ref(),
                args,
                cli.color.should_use_colors(),
            )
            .await
        }
        Some(Commands::Tutorial(args)) => tutorial_command(cli.color, args),
        Some(Commands::Resume(args)) => {
            resume_command(
                &config_sources,
                hats_source.as_ref(),
                cli.verbose,
                cli.color,
                args,
            )
            .await
        }
        Some(Commands::Events(args)) => events_command(cli.color, args),
        Some(Commands::Init(args)) => init_command(cli.color, args),
        Some(Commands::Clean(args)) => clean_command(&config_sources, cli.color, args),
        Some(Commands::Emit(args)) => emit_command(cli.color, args),
        Some(Commands::Plan(args)) => {
            plan_command(&config_sources, hats_source.as_ref(), cli.color, args).await
        }
        Some(Commands::CodeTask(args)) => {
            code_task_command(&config_sources, hats_source.as_ref(), cli.color, args).await
        }
        Some(Commands::Task(args)) => {
            code_task_command(&config_sources, hats_source.as_ref(), cli.color, args).await
        }
        Some(Commands::Tools(args)) => tools::execute(args, cli.color.should_use_colors()).await,
        Some(Commands::Wave(args)) => wave::execute(args, cli.color.should_use_colors()),
        Some(Commands::Loops(args)) => loops::execute(args, cli.color.should_use_colors()),
        Some(Commands::Hats(args)) => {
            hats::execute(
                &config_sources,
                hats_source.as_ref(),
                args,
                cli.color.should_use_colors(),
            )
            .await
        }
        Some(Commands::Tui(args)) => tui_command(args).await,
        Some(Commands::Web(args)) => web::execute(args).await,
        Some(Commands::Mcp(args)) => mcp::execute(args).await,
        Some(Commands::Bot(args)) => {
            bot::execute(
                args,
                &config_sources,
                hats_source.as_ref(),
                cli.color.should_use_colors(),
            )
            .await
        }
        Some(Commands::Completions(args)) => completions_command(args),
        None => {
            // Default to run with TUI enabled (new default behavior)
            let args = RunArgs {
                prompt_text: None,
                prompt_file: None,
                backend: None,
                max_iterations: None,
                completion_promise: None,
                dry_run: false,
                continue_mode: false,
                loop_id: None,
                no_tui: false, // TUI enabled by default
                autonomous: false,
                rpc: false,
                legacy_tui: false,
                idle_timeout: None,
                exclusive: false,
                no_auto_merge: false,
                skip_preflight: false,
                verbose: false,
                quiet: false,
                record_session: None,
                custom_args: Vec::new(),
            };
            run_command(
                &config_sources,
                hats_source.as_ref(),
                cli.verbose,
                cli.color,
                args,
            )
            .await
        }
    }
}

fn format_preflight_summary(report: &PreflightReport) -> String {
    let icons: Vec<String> = report
        .checks
        .iter()
        .map(|check| {
            let icon = match check.status {
                CheckStatus::Pass => "✓",
                CheckStatus::Warn => "⚠",
                CheckStatus::Fail => "✗",
            };
            format!("{icon} {}", check.name)
        })
        .collect();

    let summary = if icons.is_empty() {
        "no checks".to_string()
    } else {
        icons.join(" ")
    };

    let suffix = if report.failures > 0 {
        format!(
            " ({} failure{})",
            report.failures,
            if report.failures == 1 { "" } else { "s" }
        )
    } else if report.warnings > 0 {
        format!(
            " ({} warning{})",
            report.warnings,
            if report.warnings == 1 { "" } else { "s" }
        )
    } else {
        String::new()
    };

    format!("{summary}{suffix}")
}

enum AutoPreflightMode {
    DryRun,
    Run,
}

fn preflight_failure_detail(report: &PreflightReport, strict: bool) -> String {
    if strict && report.warnings > 0 {
        format!(
            "{} failure{}, {} warning{}",
            report.failures,
            if report.failures == 1 { "" } else { "s" },
            report.warnings,
            if report.warnings == 1 { "" } else { "s" }
        )
    } else {
        format!(
            "{} failure{}",
            report.failures,
            if report.failures == 1 { "" } else { "s" }
        )
    }
}

async fn run_auto_preflight(
    config: &RalphConfig,
    skip_preflight: bool,
    verbose: bool,
    mode: AutoPreflightMode,
) -> Result<Option<PreflightReport>> {
    if skip_preflight || !config.features.preflight.enabled {
        return Ok(None);
    }

    let runner = PreflightRunner::default_checks();
    let mut report = if config.features.preflight.skip.is_empty() {
        runner.run_all(config).await
    } else {
        let skip_lower: std::collections::HashSet<String> = config
            .features
            .preflight
            .skip
            .iter()
            .map(|name| name.to_lowercase())
            .collect();
        let selected: Vec<String> = runner
            .check_names()
            .into_iter()
            .filter(|name| !skip_lower.contains(&name.to_lowercase()))
            .map(|name| name.to_string())
            .collect();
        runner.run_selected(config, &selected).await
    };

    let effective_passed = if config.features.preflight.strict {
        report.failures == 0 && report.warnings == 0
    } else {
        report.failures == 0
    };
    report.passed = effective_passed;

    match mode {
        AutoPreflightMode::DryRun => Ok(Some(report)),
        AutoPreflightMode::Run => {
            print_preflight_summary(&report, verbose, "Preflight: ", false);
            if !effective_passed {
                let detail = preflight_failure_detail(&report, config.features.preflight.strict);
                anyhow::bail!(
                    "Preflight checks failed ({}). Fix the issues above or use --skip-preflight to bypass.",
                    detail
                );
            }
            Ok(None)
        }
    }
}

fn print_preflight_summary(
    report: &PreflightReport,
    verbose: bool,
    prefix: &str,
    use_stdout: bool,
) {
    let summary = format_preflight_summary(report);
    if use_stdout {
        println!("{prefix}{summary}");
    } else {
        eprintln!("{prefix}{summary}");
    }

    let emit = |line: String| {
        if use_stdout {
            println!("{line}");
        } else {
            eprintln!("{line}");
        }
    };

    for check in &report.checks {
        if check.status == CheckStatus::Fail
            && let Some(message) = &check.message
        {
            emit(format!("  ✗ {}: {}", check.name, message));
        }
    }

    if verbose {
        for check in &report.checks {
            if check.status == CheckStatus::Warn
                && let Some(message) = &check.message
            {
                emit(format!("  ⚠ {}: {}", check.name, message));
            }
        }
    }
}

async fn run_command(
    config_sources: &[ConfigSource],
    hats_source: Option<&HatsSource>,
    verbose: bool,
    color_mode: ColorMode,
    args: RunArgs,
) -> Result<()> {
    let mut config = preflight::load_config_for_preflight(config_sources, hats_source).await?;

    // Handle --continue mode: check scratchpad exists before proceeding
    let resume = args.continue_mode;
    if resume {
        let scratchpad_path = std::path::Path::new(&config.core.scratchpad.path);
        if !scratchpad_path.exists() {
            anyhow::bail!(
                "Cannot continue: scratchpad not found at '{}'. \
                 Start a fresh run with `ralph run`.",
                config.core.scratchpad.path
            );
        }
        info!(
            "Found existing scratchpad at '{}', continuing from previous state",
            config.core.scratchpad.path
        );
    }

    // Capture args for subprocess TUI mode BEFORE fields are consumed below
    let subprocess_tui_args = SubprocessTuiArgs::new(&args, config_sources, hats_source);

    // Apply CLI overrides (after normalization so they take final precedence)
    // Per spec: CLI -p and -P are mutually exclusive (enforced by clap)
    if let Some(text) = args.prompt_text {
        config.event_loop.prompt = Some(text);
        config.event_loop.prompt_file = String::new(); // Clear file path
    } else if let Some(path) = args.prompt_file {
        config.event_loop.prompt_file = path.to_string_lossy().to_string();
        config.event_loop.prompt = None; // Clear inline
    }
    if let Some(max_iter) = args.max_iterations {
        config.event_loop.max_iterations = max_iter;
    }
    if let Some(promise) = args.completion_promise {
        config.event_loop.completion_promise = promise;
    }
    if verbose {
        config.verbose = true;
    }

    // Apply execution mode overrides per spec
    // TUI is enabled by default (unless --no-tui is specified)
    if args.autonomous {
        config.cli.default_mode = "autonomous".to_string();
    } else if !args.no_tui {
        config.cli.default_mode = "interactive".to_string();
    }

    // Override idle timeout if specified
    if let Some(timeout) = args.idle_timeout {
        config.cli.idle_timeout_secs = timeout;
    }

    // Apply backend override from CLI (takes precedence over config)
    if let Some(backend) = args.backend {
        config.cli.backend = backend;
    }

    // Validate configuration and emit warnings
    let warnings = config
        .validate()
        .context("Configuration validation failed")?;
    for warning in &warnings {
        eprintln!("{warning}");
    }

    // Handle auto-detection if backend is "auto"
    if config.cli.backend == "auto" {
        let priority = config.get_agent_priority();
        let detected = detect_backend(&priority, |backend| {
            config.adapter_settings(backend).enabled
        });

        match detected {
            Ok(backend) => {
                info!("Auto-detected backend: {}", backend);
                config.cli.backend = backend;
            }
            Err(e) => {
                eprintln!("{e}");
                return Err(anyhow::Error::new(e));
            }
        }
    }

    let preflight_verbose = verbose || args.verbose;

    if args.dry_run {
        let preflight_report = run_auto_preflight(
            &config,
            args.skip_preflight,
            preflight_verbose,
            AutoPreflightMode::DryRun,
        )
        .await?;
        println!("Dry run mode - configuration:");
        println!(
            "  Hats: {}",
            if config.hats.is_empty() {
                "planner, builder (default)".to_string()
            } else {
                config.hats.keys().cloned().collect::<Vec<_>>().join(", ")
            }
        );

        // Show prompt source
        if let Some(ref inline) = config.event_loop.prompt {
            let preview = truncate_with_ellipsis(&inline.replace('\n', " "), 60);
            println!("  Prompt: inline text ({})", preview);
        } else {
            println!("  Prompt file: {}", config.event_loop.prompt_file);
        }

        println!(
            "  Completion promise: {}",
            config.event_loop.completion_promise
        );
        println!("  Max iterations: {}", config.event_loop.max_iterations);
        println!("  Max runtime: {}s", config.event_loop.max_runtime_seconds);
        println!(
            "  Scratchpad: {} (enabled: {})",
            config.core.scratchpad.path, config.core.scratchpad.enabled
        );
        println!("  Specs dir: {}", config.core.specs_dir);
        println!("  Backend: {}", config.cli.backend);
        println!("  Verbose: {}", config.verbose);
        // Execution mode info
        println!("  Default mode: {}", config.cli.default_mode);
        if config.cli.default_mode == "interactive" {
            println!("  Idle timeout: {}s", config.cli.idle_timeout_secs);
        }
        if !warnings.is_empty() {
            println!("  Warnings: {}", warnings.len());
        }
        if let Some(report) = preflight_report.as_ref() {
            print_preflight_summary(report, preflight_verbose, "  Preflight: ", true);
        }
        return Ok(());
    }

    // Ensure scratchpad directory exists (auto-create with depth limit)
    // This is done after dry-run check to avoid creating directories during dry-run
    ensure_scratchpad_directory(&config)?;

    // Get the prompt for lock metadata (short version for display)
    // When prompt_file is used, read its content for the summary instead of showing the file path
    let prompt_summary = config
        .event_loop
        .prompt
        .clone()
        .or_else(|| {
            let prompt_file = &config.event_loop.prompt_file;
            if prompt_file.is_empty() {
                None
            } else {
                let path = std::path::Path::new(prompt_file);
                if path.exists() {
                    std::fs::read_to_string(path).ok()
                } else {
                    None
                }
            }
        })
        .map(|p| truncate(&p, 100))
        .unwrap_or_else(|| "[no prompt]".to_string());

    let mut pending_worktree_registration: Option<LoopEntry> = None;

    // Determine TUI mode early (before lock acquisition) to avoid self-lock contention
    // in subprocess TUI mode. The child RPC process will acquire the lock itself.
    let is_tty = std::io::stdin().is_terminal() && std::io::stdout().is_terminal();
    let use_subprocess_tui =
        !args.no_tui && !args.autonomous && !args.rpc && !args.legacy_tui && is_tty;

    // Try to acquire the loop lock for multi-loop concurrency support
    // This implements the lock detection flow from the multi-loop spec
    // Skip lock acquisition in subprocess TUI mode - let the child acquire it
    let workspace_root = &config.core.workspace_root;
    let (loop_context, _lock_guard) = if use_subprocess_tui {
        // In subprocess TUI mode, don't acquire lock here - the child RPC process will do it
        // This avoids the self-lock contention where parent holds lock and child sees it,
        // then incorrectly spawns a worktree thinking there's another concurrent loop
        debug!("Skipping lock acquisition in subprocess TUI mode (child will acquire)");
        let context = LoopContext::primary(workspace_root.clone());
        (context, None)
    } else {
        match LoopLock::try_acquire(workspace_root, &prompt_summary) {
            Ok(guard) => {
                // We're the primary loop - run in place
                debug!("Acquired loop lock, running as primary loop");
                let context = LoopContext::primary(workspace_root.clone());
                (context, Some(guard))
            }
            Err(LockError::AlreadyLocked(existing)) => {
                // Another loop is running
                if args.exclusive {
                    // --exclusive: wait for the lock instead of spawning worktree
                    info!(
                        "Loop lock held by PID {} (started {}), waiting for lock (--exclusive mode)...",
                        existing.pid, existing.started
                    );
                    let guard = LoopLock::acquire_blocking(workspace_root, &prompt_summary)
                        .context("Failed to acquire loop lock in exclusive mode")?;
                    debug!("Acquired loop lock after waiting");
                    let context = LoopContext::primary(workspace_root.clone());
                    (context, Some(guard))
                } else if !config.features.parallel {
                    // Parallel loops disabled via config - error out
                    anyhow::bail!(
                        "Another loop is already running (PID {}, prompt: \"{}\"). \
                    Parallel loops are disabled in config (features.parallel: false). \
                    Use --exclusive to wait for the lock, or enable parallel loops.",
                        existing.pid,
                        existing.prompt.chars().take(50).collect::<String>()
                    );
                } else {
                    // Auto-spawn into worktree
                    info!(
                        "Loop lock held by PID {} ({}), spawning parallel loop in worktree",
                        existing.pid,
                        existing.prompt.chars().take(50).collect::<String>()
                    );

                    let worktree_config = WorktreeConfig::default();

                    // Generate memorable loop ID (adjective-noun only, no prompt keywords)
                    // This ID will be used consistently for: registry ID, worktree path, and branch name
                    let name_generator =
                        ralph_core::LoopNameGenerator::from_config(&config.features.loop_naming);
                    let loop_id = name_generator.generate_memorable_unique(|name| {
                        ralph_core::worktree_exists(workspace_root, name, &worktree_config)
                    });

                    // Ensure worktree directory is in .gitignore
                    ensure_gitignore(workspace_root, ".worktrees")
                        .context("Failed to update .gitignore for worktrees")?;

                    // Create the worktree
                    let worktree = create_worktree(workspace_root, &loop_id, &worktree_config)
                        .context("Failed to create worktree for parallel loop")?;

                    info!(
                        "Created worktree at {} on branch {}",
                        worktree.path.display(),
                        worktree.branch
                    );

                    // Create loop context for the worktree
                    let context = LoopContext::worktree(
                        loop_id.clone(),
                        worktree.path.clone(),
                        workspace_root.clone(),
                    );

                    // Set up all worktree symlinks (memories, specs, code tasks)
                    context
                        .setup_worktree_symlinks()
                        .context("Failed to create symlinks in worktree")?;

                    // Generate context file with worktree metadata
                    context
                        .generate_context_file(&worktree.branch, &prompt_summary)
                        .context("Failed to generate context file in worktree")?;

                    // Register this loop after preflight succeeds so failed runs
                    // don't leave stale registry entries behind.
                    let entry = LoopEntry::with_id(
                        &loop_id,
                        &prompt_summary,
                        Some(worktree.path.to_string_lossy().to_string()),
                        worktree.path.to_string_lossy().to_string(),
                    );
                    pending_worktree_registration = Some(entry);

                    // Update config to use worktree paths
                    // The scratchpad and other paths should resolve to the worktree
                    // Note: We keep the lock guard as None since worktree loops don't hold the primary lock

                    (context, None)
                }
            }
            Err(LockError::UnsupportedPlatform) => {
                // Non-Unix: just run without locking (single-loop fallback)
                warn!("Loop locking not supported on this platform, running without lock");
                let context = LoopContext::primary(workspace_root.clone());
                (context, None)
            }
            Err(e) => {
                return Err(anyhow::Error::new(e).context("Failed to acquire loop lock"));
            }
        }
    };

    // Update workspace_root in config if running in worktree
    if !loop_context.is_primary() {
        config.core.workspace_root = loop_context.workspace().to_path_buf();
        // Also update scratchpad path to use worktree location
        config.core.scratchpad.path = loop_context.scratchpad_path().to_string_lossy().to_string();
        debug!(
            "Running in worktree: workspace={}, scratchpad={}",
            config.core.workspace_root.display(),
            config.core.scratchpad.path
        );
    }

    // Ensure directories exist in the loop context
    loop_context
        .ensure_directories()
        .context("Failed to create loop directories")?;

    if let Err(err) = run_auto_preflight(
        &config,
        args.skip_preflight,
        preflight_verbose,
        AutoPreflightMode::Run,
    )
    .await
    {
        if !loop_context.is_primary()
            && let Err(clean_err) =
                remove_worktree(loop_context.repo_root(), loop_context.workspace())
        {
            warn!(
                "Preflight failed; unable to remove worktree {}: {}",
                loop_context.workspace().display(),
                clean_err
            );
        }
        return Err(err);
    }

    if let Some(entry) = pending_worktree_registration {
        let registry = LoopRegistry::new(loop_context.repo_root());
        registry
            .register(entry)
            .context("Failed to register loop in registry")?;
    }

    // Run the orchestration loop and exit with proper exit code
    // TUI is enabled by default (unless --no-tui, --autonomous, or --rpc is specified)
    let wants_tui = !args.no_tui && !args.autonomous && !args.rpc;
    let use_legacy_tui = args.legacy_tui;
    let enable_rpc = args.rpc;
    let verbosity = Verbosity::resolve(verbose || args.verbose, args.quiet);
    let custom_args = args.custom_args.clone();
    // --no-auto-merge CLI flag overrides config.features.auto_merge
    let auto_merge_override = if args.no_auto_merge {
        Some(false)
    } else {
        None
    };
    let workspace_root = config.core.workspace_root.clone();

    // Determine TUI mode:
    // 1. Subprocess TUI (default): TUI spawns `ralph run --rpc` as child, reads JSON events
    // 2. Legacy TUI: In-process TUI (--legacy-tui escape hatch)
    // 3. RPC mode: Headless JSON-lines output (--rpc)
    // 4. CLI mode: No TUI (--no-tui or --autonomous)
    // Note: use_subprocess_tui is now determined earlier (before lock acquisition)
    let reason = if use_subprocess_tui {
        // Subprocess TUI mode: spawn child with --rpc and attach TUI
        run_subprocess_tui(subprocess_tui_args, resume, custom_args).await?
    } else {
        // In-process mode: run_loop_impl handles everything
        let enable_tui = wants_tui && use_legacy_tui;
        loop_runner::run_loop_impl(
            config,
            color_mode,
            resume,
            enable_tui,
            enable_rpc,
            verbosity,
            args.record_session,
            Some(loop_context),
            custom_args,
            auto_merge_override,
            args.loop_id,
        )
        .await?
    };

    // Handle restart: run required single-command restart sequence.
    if matches!(reason, TerminationReason::RestartRequested) {
        clear_restart_request_signal(&workspace_root);

        #[cfg(unix)]
        {
            let restart_cmd = required_restart_command(std::process::id());
            info!(
                "Restart requested — launching single-command restart: {}",
                restart_cmd
            );

            std::process::Command::new("sh")
                .arg("-lc")
                .arg(&restart_cmd)
                .spawn()
                .with_context(|| format!("Failed to spawn restart command: {}", restart_cmd))?;

            // Shell command takes over restarting this loop after kill.
            return Ok(());
        }

        #[cfg(not(unix))]
        {
            anyhow::bail!("Restart via single-command shell restart is only supported on Unix");
        }
    }

    let exit_code = reason.exit_code();

    // Use explicit exit for non-zero codes to ensure proper exit status
    if exit_code != 0 {
        std::process::exit(exit_code);
    }

    Ok(())
}

fn required_restart_command(pid: u32) -> String {
    format!("kill {pid} && RALPH_DIAGNOSTICS=1 cargo run --bin ralph -- resume -c ralph.test.yml")
}

fn clear_restart_request_signal(workspace_root: &std::path::Path) {
    let restart_path = workspace_root.join(".ralph/restart-requested");
    let _ = std::fs::remove_file(&restart_path);
}

/// Arguments needed for subprocess TUI mode.
/// We clone these early before RunArgs fields are consumed.
#[derive(Clone)]
struct SubprocessTuiArgs {
    prompt_text: Option<String>,
    prompt_file: Option<PathBuf>,
    backend: Option<String>,
    max_iterations: Option<u32>,
    completion_promise: Option<String>,
    continue_mode: bool,
    loop_id: Option<String>,
    idle_timeout: Option<u32>,
    verbose: bool,
    quiet: bool,
    record_session: Option<PathBuf>,
    exclusive: bool,
    no_auto_merge: bool,
    skip_preflight: bool,
    /// Config sources to forward to child process (-c args)
    config_sources: Vec<String>,
    /// Hats source to forward to child process (-H arg)
    hats_source: Option<String>,
}

impl SubprocessTuiArgs {
    /// Create from RunArgs with config/hats sources from Cli.
    fn new(
        args: &RunArgs,
        config_sources: &[ConfigSource],
        hats_source: Option<&HatsSource>,
    ) -> Self {
        Self {
            prompt_text: args.prompt_text.clone(),
            prompt_file: args.prompt_file.clone(),
            backend: args.backend.clone(),
            max_iterations: args.max_iterations,
            completion_promise: args.completion_promise.clone(),
            continue_mode: args.continue_mode,
            loop_id: args.loop_id.clone(),
            idle_timeout: args.idle_timeout,
            verbose: args.verbose,
            quiet: args.quiet,
            record_session: args.record_session.clone(),
            exclusive: args.exclusive,
            no_auto_merge: args.no_auto_merge,
            skip_preflight: args.skip_preflight,
            config_sources: config_sources.iter().map(|s| s.to_cli_string()).collect(),
            hats_source: hats_source.map(|h| h.label()),
        }
    }
}

/// Run the orchestration loop as a subprocess with TUI attached.
///
/// This spawns `ralph run --rpc` as a child process and attaches the TUI
/// as a client that reads JSON events from stdout and sends commands to stdin.
/// This two-process model allows the TUI to be decoupled from the orchestration loop.
async fn run_subprocess_tui(
    args: SubprocessTuiArgs,
    resume: bool,
    custom_args: Vec<String>,
) -> Result<TerminationReason> {
    use std::process::Stdio;
    use tokio::process::Command;

    // Build child command: ralph [-c ...] [-H ...] run --rpc <forwarded args>
    // Note: -c and -H are global options that must come BEFORE the subcommand
    let mut child_args = Vec::new();

    // Forward config sources (global option, before subcommand)
    for config_source in &args.config_sources {
        child_args.push("-c".to_string());
        child_args.push(config_source.clone());
    }

    // Forward hats source (global option, before subcommand)
    if let Some(ref hats) = args.hats_source {
        child_args.push("-H".to_string());
        child_args.push(hats.clone());
    }

    // Add subcommand and mode
    child_args.push("run".to_string());
    child_args.push("--rpc".to_string());

    // Forward prompt
    if let Some(ref prompt) = args.prompt_text {
        child_args.push("-p".to_string());
        child_args.push(prompt.clone());
    }
    if let Some(ref prompt_file) = args.prompt_file {
        child_args.push("-P".to_string());
        child_args.push(prompt_file.to_string_lossy().to_string());
    }

    // Forward backend
    if let Some(ref backend) = args.backend {
        child_args.push("-b".to_string());
        child_args.push(backend.clone());
    }

    // Forward max iterations
    if let Some(max_iters) = args.max_iterations {
        child_args.push("--max-iterations".to_string());
        child_args.push(max_iters.to_string());
    }

    // Forward completion promise
    if let Some(ref promise) = args.completion_promise {
        child_args.push("--completion-promise".to_string());
        child_args.push(promise.clone());
    }

    // Forward continue mode and loop ID
    if resume || args.continue_mode {
        child_args.push("--continue".to_string());
    }
    if let Some(ref loop_id) = args.loop_id {
        child_args.push("--loop-id".to_string());
        child_args.push(loop_id.clone());
    }

    // Forward idle timeout
    if let Some(timeout) = args.idle_timeout {
        child_args.push("--idle-timeout".to_string());
        child_args.push(timeout.to_string());
    }

    // Forward verbosity
    if args.verbose {
        child_args.push("-v".to_string());
    }
    if args.quiet {
        child_args.push("-q".to_string());
    }

    // Forward record session
    if let Some(ref path) = args.record_session {
        child_args.push("--record-session".to_string());
        child_args.push(path.to_string_lossy().to_string());
    }

    // Forward multi-loop options
    if args.exclusive {
        child_args.push("--exclusive".to_string());
    }
    if args.no_auto_merge {
        child_args.push("--no-auto-merge".to_string());
    }

    // Forward preflight options
    if args.skip_preflight {
        child_args.push("--skip-preflight".to_string());
    }

    // Forward custom args (after --)
    if !custom_args.is_empty() {
        child_args.push("--".to_string());
        child_args.extend(custom_args);
    }

    info!(child_args = ?child_args, "Spawning subprocess for TUI mode");

    // Spawn child process.
    // Redirect stderr to a log file to prevent child tracing output from
    // corrupting the TUI display (ratatui runs in raw terminal mode).
    let stderr_stdio = match ralph_core::diagnostics::create_log_file(
        &std::env::current_dir().unwrap_or_default(),
    ) {
        Ok((file, path)) => {
            info!(log_file = %path.display(), "TUI subprocess stderr redirected to log file");
            Stdio::from(file)
        }
        Err(_) => Stdio::null(),
    };

    let mut child = Command::new(std::env::current_exe()?)
        .args(&child_args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(stderr_stdio)
        .spawn()
        .context("Failed to spawn ralph subprocess for TUI")?;

    let stdin = child
        .stdin
        .take()
        .context("Failed to capture subprocess stdin")?;
    let stdout = child
        .stdout
        .take()
        .context("Failed to capture subprocess stdout")?;

    // Create TUI state and start event reader
    let state = std::sync::Arc::new(std::sync::Mutex::new(ralph_tui::TuiState::new()));
    let (terminated_tx, terminated_rx) = tokio::sync::watch::channel(false);

    // Create RPC writer for sending commands
    let rpc_writer = ralph_tui::RpcWriter::new(stdin);

    // Spawn the event reader as a background task
    let reader_state = std::sync::Arc::clone(&state);
    let cancel_rx = terminated_rx.clone();
    let reader_handle = tokio::spawn(async move {
        ralph_tui::run_rpc_event_reader(stdout, reader_state, cancel_rx).await;
    });

    info!("TUI running in subprocess RPC mode");

    // Run the TUI render/input loop with subprocess support
    let app = ralph_tui::App::new_subprocess(
        std::sync::Arc::clone(&state),
        terminated_rx,
        rpc_writer.clone(),
    );
    let tui_result = app.run().await;

    // Signal cancellation
    let _ = terminated_tx.send(true);

    // Send abort to subprocess and close stdin
    let _ = rpc_writer.send_abort().await;
    let _ = rpc_writer.close().await;

    // Wait for reader to finish
    let _ = reader_handle.await;

    // Wait for subprocess to exit and get exit status
    let exit_status = child.wait().await?;

    // Map exit status to termination reason
    // Exit codes: 0=success, 1=max_iterations, 130=interrupted (SIGINT)
    let reason = if exit_status.success() {
        TerminationReason::CompletionPromise
    } else {
        match exit_status.code() {
            Some(1) => TerminationReason::MaxIterations,
            Some(130) => TerminationReason::Interrupted,
            _ => TerminationReason::Stopped,
        }
    };

    // Return TUI result if it failed, otherwise the termination reason
    tui_result.map(|_| reason)
}

/// Resume a previously interrupted loop from existing scratchpad.
///
/// DEPRECATED: Use `ralph run --continue` instead.
///
/// Per spec: "When loop terminates due to safeguard (not completion promise),
/// user can run `ralph run --continue` to restart reading existing scratchpad,
/// continuing from where it left off."
async fn resume_command(
    config_sources: &[ConfigSource],
    hats_source: Option<&HatsSource>,
    verbose: bool,
    color_mode: ColorMode,
    args: ResumeArgs,
) -> Result<()> {
    // Show deprecation warning
    eprintln!(
        "{}warning:{} `ralph resume` is deprecated. Use `ralph run --continue` instead.",
        colors::YELLOW,
        colors::RESET
    );

    // Load split core + hats config
    let mut config = preflight::load_config_for_preflight(config_sources, hats_source).await?;

    // Check that scratchpad exists (required for resume)
    let scratchpad_path = std::path::Path::new(&config.core.scratchpad.path);
    if !scratchpad_path.exists() {
        anyhow::bail!(
            "Cannot continue: scratchpad not found at '{}'. \
             Start a fresh run with `ralph run`.",
            config.core.scratchpad.path
        );
    }

    info!(
        "Found existing scratchpad at '{}', continuing from previous state",
        config.core.scratchpad.path
    );

    // Apply CLI overrides
    if let Some(max_iter) = args.max_iterations {
        config.event_loop.max_iterations = max_iter;
    }
    if verbose {
        config.verbose = true;
    }

    // Apply execution mode overrides
    // TUI is enabled by default (unless --no-tui is specified)
    if args.autonomous {
        config.cli.default_mode = "autonomous".to_string();
    } else if !args.no_tui {
        config.cli.default_mode = "interactive".to_string();
    }

    // Override idle timeout if specified
    if let Some(timeout) = args.idle_timeout {
        config.cli.idle_timeout_secs = timeout;
    }

    // Validate configuration
    let warnings = config
        .validate()
        .context("Configuration validation failed")?;
    for warning in &warnings {
        eprintln!("{warning}");
    }

    // Handle auto-detection if backend is "auto"
    if config.cli.backend == "auto" {
        let priority = config.get_agent_priority();
        let detected = detect_backend(&priority, |backend| {
            config.adapter_settings(backend).enabled
        });

        match detected {
            Ok(backend) => {
                info!("Auto-detected backend: {}", backend);
                config.cli.backend = backend;
            }
            Err(e) => {
                eprintln!("{e}");
                return Err(anyhow::Error::new(e));
            }
        }
    }

    // Run the orchestration loop in resume mode
    // The key difference: we publish task.resume instead of task.start,
    // signaling the planner to read the existing scratchpad
    // TUI is enabled by default (unless --no-tui, --autonomous, or --rpc is specified)
    let enable_tui = !args.no_tui && !args.autonomous && !args.rpc;
    let enable_rpc = args.rpc;
    let verbosity = Verbosity::resolve(verbose || args.verbose, args.quiet);
    let reason = loop_runner::run_loop_impl(
        config,
        color_mode,
        true,
        enable_tui,
        enable_rpc,
        verbosity,
        args.record_session,
        None,       // Deprecated resume command doesn't have loop_context
        Vec::new(), // Resume command doesn't support custom args
        None,       // Use config.features.auto_merge (deprecated command)
        None,       // Deprecated resume command doesn't support --loop-id
    )
    .await?;
    let exit_code = reason.exit_code();

    if exit_code != 0 {
        std::process::exit(exit_code);
    }

    Ok(())
}

fn init_command(color_mode: ColorMode, args: InitArgs) -> Result<()> {
    let use_colors = color_mode.should_use_colors();

    // Handle --list-presets (lists builtin hat collections)
    if args.list_presets {
        println!("{}", init::format_preset_list());
        return Ok(());
    }

    // Hard cutover: --preset no longer writes monolithic config.
    if let Some(preset) = args.preset {
        anyhow::bail!(
            "`ralph init --preset {preset}` was removed.\n\nUse split config:\n  1) Create core config: ralph init --backend <backend>\n  2) Run with hats:     ralph run -c ralph.yml -H builtin:{preset}"
        );
    }

    // Handle --backend alone (minimal config)
    if let Some(backend) = args.backend {
        match init::init_from_backend(&backend, args.force) {
            Ok(()) => {
                if use_colors {
                    println!(
                        "{}✓{} Created ralph.yml with {} backend",
                        colors::GREEN,
                        colors::RESET,
                        backend
                    );
                    println!(
                        "\n{}Next steps:{}\n  1. Create PROMPT.md with your task\n  2. Run core-only: ralph run -c ralph.yml\n  3. Or with hats:  ralph run -c ralph.yml -H builtin:code-assist",
                        colors::DIM,
                        colors::RESET
                    );
                } else {
                    println!("Created ralph.yml with {} backend", backend);
                    println!(
                        "\nNext steps:\n  1. Create PROMPT.md with your task\n  2. Run core-only: ralph run -c ralph.yml\n  3. Or with hats:  ralph run -c ralph.yml -H builtin:code-assist"
                    );
                }
                return Ok(());
            }
            Err(e) => {
                anyhow::bail!("{}", e);
            }
        }
    }

    // No flag specified - show help
    println!("Initialize a new ralph.yml configuration file.\n");
    println!("Usage:");
    println!("  ralph init --backend <backend>   Generate core config (ralph.yml)");
    println!("  ralph init --list-presets        Show builtin hat collections\n");
    println!("Backends: {}", backend_support::VALID_BACKENDS_LABEL);
    println!("\nThen run with hats, e.g.: ralph run -c ralph.yml -H builtin:code-assist");

    Ok(())
}

fn events_command(color_mode: ColorMode, args: EventsArgs) -> Result<()> {
    let use_colors = color_mode.should_use_colors();
    let workspace_root = resolve_workspace_root(None);
    let current_events_marker = workspace_root.join(".ralph/current-events");

    // Read events path from marker file, fall back to default if marker doesn't exist
    // This ensures `ralph events` reads from the same events file as the active run
    let history = match args.file {
        Some(path) => EventHistory::new(path),
        None => fs::read_to_string(&current_events_marker)
            .map(|s| EventHistory::new(resolve_marker_target(&workspace_root, &s)))
            .unwrap_or_else(|_| EventHistory::new(workspace_root.join(".ralph/events.jsonl"))),
    };

    // Handle clear command
    if args.clear {
        history.clear()?;
        if use_colors {
            println!("{}✓{} Event history cleared", colors::GREEN, colors::RESET);
        } else {
            println!("Event history cleared");
        }
        return Ok(());
    }

    if !history.exists() {
        if use_colors {
            println!(
                "{}No event history found.{} Run `ralph` to generate events.",
                colors::DIM,
                colors::RESET
            );
        } else {
            println!("No event history found. Run `ralph` to generate events.");
        }
        return Ok(());
    }

    // Read and filter events
    let mut records = history.read_all()?;

    // Apply filters in sequence
    if let Some(ref topic) = args.topic {
        records.retain(|r| r.topic == *topic);
    }

    if let Some(iteration) = args.iteration {
        records.retain(|r| r.iteration == iteration);
    }

    // Apply 'last' filter after other filters (to get last N of filtered results)
    if let Some(n) = args.last
        && records.len() > n
    {
        records = records.into_iter().rev().take(n).rev().collect();
    }

    if records.is_empty() {
        if use_colors {
            println!("{}No matching events found.{}", colors::DIM, colors::RESET);
        } else {
            println!("No matching events found.");
        }
        return Ok(());
    }

    match args.format {
        OutputFormat::Json => {
            let json = serde_json::to_string_pretty(&records)?;
            println!("{json}");
        }
        OutputFormat::Table => {
            display::print_events_table(&records, use_colors);
        }
    }

    Ok(())
}

fn clean_command(
    config_sources: &[ConfigSource],
    color_mode: ColorMode,
    args: CleanArgs,
) -> Result<()> {
    let use_colors = color_mode.should_use_colors();

    // If --diagnostics flag is set, clean diagnostics directory
    if args.diagnostics {
        let workspace_root = std::env::current_dir().context("Failed to get current directory")?;
        return ralph_cli::clean_diagnostics(&workspace_root, use_colors, args.dry_run);
    }

    // Load config with overrides applied
    let config = load_config_with_overrides(config_sources)?;

    // Extract the .agent directory path from scratchpad path
    let scratchpad_path = Path::new(&config.core.scratchpad.path);
    let agent_dir = scratchpad_path.parent().ok_or_else(|| {
        anyhow::anyhow!(
            "Could not determine parent directory from scratchpad path: {}",
            config.core.scratchpad.path
        )
    })?;

    // Check if directory exists
    if !agent_dir.exists() {
        // Not an error - just inform user
        if use_colors {
            println!(
                "{}Nothing to clean:{} Directory '{}' does not exist",
                colors::DIM,
                colors::RESET,
                agent_dir.display()
            );
        } else {
            println!(
                "Nothing to clean: Directory '{}' does not exist",
                agent_dir.display()
            );
        }
        return Ok(());
    }

    // Dry run mode - list what would be deleted
    if args.dry_run {
        if use_colors {
            println!(
                "{}Dry run mode:{} Would delete directory and all contents:",
                colors::CYAN,
                colors::RESET
            );
        } else {
            println!("Dry run mode: Would delete directory and all contents:");
        }
        println!("  {}", agent_dir.display());

        // List directory contents
        list_directory_contents(agent_dir, use_colors, 1)?;

        return Ok(());
    }

    // Perform actual deletion
    fs::remove_dir_all(agent_dir).with_context(|| {
        format!(
            "Failed to delete directory '{}'. Check permissions and try again.",
            agent_dir.display()
        )
    })?;

    // Success message
    if use_colors {
        println!(
            "{}✓{} Cleaned: Deleted '{}' and all contents",
            colors::GREEN,
            colors::RESET,
            agent_dir.display()
        );
    } else {
        println!(
            "Cleaned: Deleted '{}' and all contents",
            agent_dir.display()
        );
    }

    Ok(())
}

/// Emit an event to the current run's events file with proper JSON formatting.
///
/// This command provides a deterministic way for agents to emit events without
/// risking malformed JSONL from manual echo commands. All JSON serialization
/// is handled via serde_json, ensuring proper escaping of payloads.
///
/// Events are written to the path specified in `.ralph/current-events` marker file
/// (created by `ralph run`), or falls back to `.ralph/events.jsonl` if no marker exists.
fn emit_command(color_mode: ColorMode, args: EmitArgs) -> Result<()> {
    emit_command_with_root(color_mode, args, None)
}

fn emit_command_with_root(
    color_mode: ColorMode,
    args: EmitArgs,
    root: Option<&PathBuf>,
) -> Result<()> {
    let use_colors = color_mode.should_use_colors();
    let workspace_root = resolve_workspace_root(root);
    let current_events_marker = workspace_root.join(".ralph/current-events");

    if std::env::var("RALPH_WAVE_ID").is_err() {
        let urgent_steer_store = UrgentSteerStore::new(urgent_steer_path_from_workspace(root));
        if let Some(record) = urgent_steer_store
            .take()
            .context("Failed to read urgent-steer marker")?
        {
            let guidance = record
                .messages
                .iter()
                .enumerate()
                .map(|(idx, message)| format!("{}. {}", idx + 1, message))
                .collect::<Vec<_>>()
                .join("\n");

            anyhow::bail!(
                "Urgent steer is pending. Do not hand off yet.\n\n\
                 Human feedback:\n{guidance}\n\n\
                 You have now seen the steer. Address it in this turn, then rerun `ralph emit` \
                 once you are ready to hand off."
            );
        }
    }

    // Generate timestamp if not provided
    let ts = args.ts.unwrap_or_else(|| chrono::Utc::now().to_rfc3339());

    // Validate JSON payload if --json flag is set
    let payload = if args.json && !args.payload.is_empty() {
        // Validate it's valid JSON
        serde_json::from_str::<serde_json::Value>(&args.payload).context("Invalid JSON payload")?;
        args.payload
    } else {
        args.payload
    };

    // Build the event record
    // We use serde_json directly to ensure proper escaping
    let payload_value = if args.json && !payload.is_empty() {
        // Parse and embed as object
        serde_json::from_str::<serde_json::Value>(&payload)?
    } else if payload.is_empty() {
        serde_json::Value::Null
    } else {
        serde_json::Value::String(payload)
    };

    let mut record = serde_json::json!({
        "topic": args.topic,
        "payload": payload_value,
        "ts": ts
    });

    // Auto-tag with wave metadata from env vars (set by loop runner on wave workers)
    if let (Ok(wave_id), Ok(wave_index_str)) = (
        std::env::var("RALPH_WAVE_ID"),
        std::env::var("RALPH_WAVE_INDEX"),
    ) && let Ok(wave_index) = wave_index_str.parse::<u32>()
    {
        record["wave_id"] = serde_json::Value::String(wave_id);
        record["wave_index"] = serde_json::Value::Number(wave_index.into());
    }

    // Resolve events file: RALPH_EVENTS_FILE env > marker file > CLI arg
    // This ensures `ralph emit` writes to the same events file as the active run
    let events_file = if let Ok(path) = std::env::var("RALPH_EVENTS_FILE") {
        PathBuf::from(path)
    } else {
        fs::read_to_string(&current_events_marker)
            .map(|s| resolve_marker_target(&workspace_root, &s))
            .unwrap_or_else(|_| args.file.clone())
    };

    // Ensure parent directory exists
    if let Some(parent) = events_file.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
    }

    // Append to file
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&events_file)
        .with_context(|| format!("Failed to open events file: {}", events_file.display()))?;

    // Write as single-line JSON (JSONL format)
    let json_line = serde_json::to_string(&record)?;
    writeln!(file, "{}", json_line)?;

    // Success message
    if use_colors {
        println!(
            "{}✓{} Event emitted: {}",
            colors::GREEN,
            colors::RESET,
            args.topic
        );
    } else {
        println!("Event emitted: {}", args.topic);
    }

    Ok(())
}

#[derive(Debug, Clone, Copy)]
struct TutorialStep {
    title: &'static str,
    body: &'static [&'static str],
}

const TUTORIAL_STEPS: &[TutorialStep] = &[
    TutorialStep {
        title: "Hats: Event-driven personas",
        body: &[
            "Hats are named personas that subscribe to events and publish new events.",
            "Each hat lists triggers (ex: task.start) and outputs (ex: build.task).",
            "Inspect hats with: ralph hats list",
            "Visualize the flow with: ralph hats graph --format ascii",
        ],
    },
    TutorialStep {
        title: "Hat collections: Swappable workflows",
        body: &[
            "Core config and hat collections are split.",
            "List built-in hat collections: ralph init --list-presets",
            "Create core config: ralph init --backend <name>",
            "Run with hats: ralph run -c ralph.yml -H builtin:code-assist",
        ],
    },
    TutorialStep {
        title: "Workflow: The loop lifecycle",
        body: &[
            "Write a prompt file (ex: PROMPT.md) or pass --prompt/--prompt-file.",
            "Run: ralph run -P PROMPT.md or ralph run -p \"...\"",
            "Ralph emits task.start, hats process events, and the loop ends on done events.",
            "Artifacts live in .ralph/agent (scratchpad, tasks, memories).",
            "Check open tasks with: ralph tools task ready",
        ],
    },
];

fn tutorial_steps() -> &'static [TutorialStep] {
    TUTORIAL_STEPS
}

/// Runs the interactive tutorial walkthrough.
fn tutorial_command(color_mode: ColorMode, args: TutorialArgs) -> Result<()> {
    let use_colors = color_mode.should_use_colors();
    let interactive = !args.no_input && std::io::stdin().is_terminal();
    let steps = tutorial_steps();

    print_tutorial_intro(use_colors, interactive);

    for (index, step) in steps.iter().enumerate() {
        print_tutorial_step(index + 1, steps.len(), step, use_colors);
        if interactive && index + 1 < steps.len() {
            prompt_to_continue(use_colors)?;
        } else {
            println!();
        }
    }

    print_tutorial_outro(use_colors);
    Ok(())
}

fn print_tutorial_intro(use_colors: bool, interactive: bool) {
    if use_colors {
        println!(
            "{}{}Ralph Tutorial{}",
            colors::BOLD,
            colors::CYAN,
            colors::RESET
        );
        println!(
            "{}Interactive walkthrough of hats, hat collections, and workflow.{}",
            colors::DIM,
            colors::RESET
        );
    } else {
        println!("Ralph Tutorial");
        println!("Interactive walkthrough of hats, hat collections, and workflow.");
    }

    if !interactive {
        println!("Non-interactive mode: printing all steps.");
    }

    println!();
}

fn print_tutorial_step(index: usize, total: usize, step: &TutorialStep, use_colors: bool) {
    if use_colors {
        println!(
            "{}{}Step {}/{}: {}{}",
            colors::BOLD,
            colors::CYAN,
            index,
            total,
            step.title,
            colors::RESET
        );
    } else {
        println!("Step {}/{}: {}", index, total, step.title);
    }

    for line in step.body {
        println!("  - {}", line);
    }
}

fn prompt_to_continue(use_colors: bool) -> Result<()> {
    if use_colors {
        print!("{}Press Enter to continue...{}", colors::DIM, colors::RESET);
    } else {
        print!("Press Enter to continue...");
    }

    stdout().flush()?;
    let mut input = String::new();
    std::io::stdin()
        .read_line(&mut input)
        .context("Failed to read input")?;
    println!();
    Ok(())
}

fn print_tutorial_outro(use_colors: bool) {
    if use_colors {
        println!(
            "{}Tutorial complete. Next: ralph init --backend <name>, then ralph run -c ralph.yml -H builtin:code-assist.{}",
            colors::GREEN,
            colors::RESET
        );
    } else {
        println!(
            "Tutorial complete. Next: ralph init --backend <name>, then ralph run -c ralph.yml -H builtin:code-assist."
        );
    }
}

/// Starts a Prompt-Driven Development planning session.
///
/// This is a thin wrapper that bypasses Ralph's event loop entirely.
/// It spawns the AI backend with the bundled PDD SOP for interactive planning.
async fn plan_command(
    config_sources: &[ConfigSource],
    hats_source: Option<&HatsSource>,
    color_mode: ColorMode,
    args: PlanArgs,
) -> Result<()> {
    use sop_runner::{Sop, SopRunConfig, SopRunError};

    let use_colors = color_mode.should_use_colors();

    // Show what we're starting
    if use_colors {
        println!(
            "{}🎯{} Starting {} session...",
            colors::CYAN,
            colors::RESET,
            Sop::Pdd.name()
        );
    } else {
        println!("Starting {} session...", Sop::Pdd.name());
    }

    let config = preflight::load_config_for_preflight(config_sources, hats_source).await?;

    let config = SopRunConfig {
        sop: Sop::Pdd,
        user_input: args.idea,
        backend_override: args.backend,
        config: Some(config),
        config_path: None,
        custom_args: if args.custom_args.is_empty() {
            None
        } else {
            Some(args.custom_args)
        },
        agent_teams: args.teams,
    };

    sop_runner::run_sop(config).map_err(|e| match e {
        SopRunError::NoBackend(no_backend) => anyhow::Error::new(no_backend),
        SopRunError::UnknownBackend(msg) => anyhow::anyhow!("{}", msg),
        SopRunError::SpawnError(io_err) => anyhow::anyhow!("Failed to spawn backend: {}", io_err),
    })
}

/// Starts a code-task-generator session.
///
/// This is a thin wrapper that bypasses Ralph's event loop entirely.
/// It spawns the AI backend with the bundled code-task-generator SOP.
async fn code_task_command(
    config_sources: &[ConfigSource],
    hats_source: Option<&HatsSource>,
    color_mode: ColorMode,
    args: CodeTaskArgs,
) -> Result<()> {
    use sop_runner::{Sop, SopRunConfig, SopRunError};

    let use_colors = color_mode.should_use_colors();

    // Show what we're starting
    if use_colors {
        println!(
            "{}📋{} Starting {} session...",
            colors::CYAN,
            colors::RESET,
            Sop::CodeTaskGenerator.name()
        );
    } else {
        println!("Starting {} session...", Sop::CodeTaskGenerator.name());
    }

    let config = preflight::load_config_for_preflight(config_sources, hats_source).await?;

    let config = SopRunConfig {
        sop: Sop::CodeTaskGenerator,
        user_input: args.input,
        backend_override: args.backend,
        config: Some(config),
        config_path: None,
        custom_args: if args.custom_args.is_empty() {
            None
        } else {
            Some(args.custom_args)
        },
        agent_teams: args.teams,
    };

    sop_runner::run_sop(config).map_err(|e| match e {
        SopRunError::NoBackend(no_backend) => anyhow::Error::new(no_backend),
        SopRunError::UnknownBackend(msg) => anyhow::anyhow!("{}", msg),
        SopRunError::SpawnError(io_err) => anyhow::anyhow!("Failed to spawn backend: {}", io_err),
    })
}

/// Lists directory contents recursively for dry-run mode.
fn list_directory_contents(path: &Path, use_colors: bool, indent: usize) -> Result<()> {
    let entries = fs::read_dir(path)?;
    let indent_str = "  ".repeat(indent);

    for entry in entries {
        let entry = entry?;
        let entry_path = entry.path();
        let file_name = entry.file_name();

        if entry_path.is_dir() {
            if use_colors {
                println!(
                    "{}{}{}/{}",
                    indent_str,
                    colors::BLUE,
                    file_name.to_string_lossy(),
                    colors::RESET
                );
            } else {
                println!("{}{}/", indent_str, file_name.to_string_lossy());
            }
            list_directory_contents(&entry_path, use_colors, indent + 1)?;
        } else if use_colors {
            println!(
                "{}{}{}{}",
                indent_str,
                colors::DIM,
                file_name.to_string_lossy(),
                colors::RESET
            );
        } else {
            println!("{}{}", indent_str, file_name.to_string_lossy());
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::CwdGuard;
    use ralph_core::{HookMutationConfig, HookOnError, HookPhaseEvent, HookSpec};
    use std::path::PathBuf;
    use tempfile::TempDir;
    #[test]
    fn test_required_restart_command_matches_contract() {
        let command = required_restart_command(4242);
        assert_eq!(
            command,
            "kill 4242 && RALPH_DIAGNOSTICS=1 cargo run --bin ralph -- resume -c ralph.test.yml"
        );
    }

    #[test]
    fn test_clear_restart_request_signal_removes_sentinel_file() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let restart_dir = temp_dir.path().join(".ralph");
        std::fs::create_dir_all(&restart_dir).expect("create .ralph dir");
        let restart_path = restart_dir.join("restart-requested");
        std::fs::write(&restart_path, "requested").expect("write sentinel");

        clear_restart_request_signal(temp_dir.path());

        assert!(
            !restart_path.exists(),
            "restart sentinel should be removed before restart command dispatch"
        );
    }

    #[test]
    fn test_verbosity_cli_quiet() {
        assert_eq!(Verbosity::resolve(false, true), Verbosity::Quiet);
    }

    #[test]
    fn test_verbosity_cli_verbose() {
        assert_eq!(Verbosity::resolve(true, false), Verbosity::Verbose);
    }

    #[test]
    fn test_verbosity_default() {
        assert_eq!(Verbosity::resolve(false, false), Verbosity::Normal);
    }

    #[test]
    fn test_verbosity_env_quiet() {
        assert_eq!(
            Verbosity::resolve_with_env(false, false, true, false),
            Verbosity::Quiet
        );
    }

    #[test]
    fn test_verbosity_env_verbose() {
        assert_eq!(
            Verbosity::resolve_with_env(false, false, false, true),
            Verbosity::Verbose
        );
    }

    #[test]
    fn test_color_mode_should_use_colors() {
        // `NO_COLOR` disables ANSI globally, including `--color always`.
        let expected_always = std::env::var("NO_COLOR").is_err();
        assert_eq!(ColorMode::Always.should_use_colors(), expected_always);
        assert!(!ColorMode::Never.should_use_colors());
    }

    #[test]
    fn test_config_source_parse_builtin() {
        let source = ConfigSource::parse("builtin:code-assist");
        match source {
            ConfigSource::Builtin(name) => assert_eq!(name, "code-assist"),
            _ => panic!("Expected Builtin variant"),
        }
    }

    #[test]
    fn test_hats_source_parse_builtin() {
        let source = HatsSource::parse("builtin:code-assist");
        match source {
            HatsSource::Builtin(name) => assert_eq!(name, "code-assist"),
            _ => panic!("Expected Builtin variant"),
        }
    }

    #[test]
    fn test_hats_source_parse_file() {
        let source = HatsSource::parse("hats/feature.yml");
        match source {
            HatsSource::File(path) => {
                assert_eq!(path, std::path::PathBuf::from("hats/feature.yml"))
            }
            _ => panic!("Expected File variant"),
        }
    }

    #[test]
    fn test_cli_parses_global_hats_flag() {
        let cli = Cli::try_parse_from(["ralph", "run", "-H", "builtin:code-assist"])
            .expect("CLI parse failed");
        assert_eq!(cli.hats.as_deref(), Some("builtin:code-assist"));
    }

    #[test]
    fn test_config_source_parse_remote_https() {
        let source = ConfigSource::parse("https://example.com/preset.yml");
        match source {
            ConfigSource::Remote(url) => assert_eq!(url, "https://example.com/preset.yml"),
            _ => panic!("Expected Remote variant"),
        }
    }

    #[test]
    fn test_config_source_parse_remote_http() {
        let source = ConfigSource::parse("http://example.com/preset.yml");
        match source {
            ConfigSource::Remote(url) => assert_eq!(url, "http://example.com/preset.yml"),
            _ => panic!("Expected Remote variant"),
        }
    }

    #[test]
    fn test_config_source_parse_file() {
        let source = ConfigSource::parse("ralph.yml");
        match source {
            ConfigSource::File(path) => assert_eq!(path, std::path::PathBuf::from("ralph.yml")),
            _ => panic!("Expected File variant"),
        }
    }

    #[test]
    fn test_config_source_parse_override_scratchpad() {
        let source = ConfigSource::parse("core.scratchpad=.ralph/feature/scratchpad.md");
        match source {
            ConfigSource::Override { key, value } => {
                assert_eq!(key, "core.scratchpad");
                assert_eq!(value, ".ralph/feature/scratchpad.md");
            }
            _ => panic!("Expected Override variant"),
        }
    }

    #[test]
    fn test_config_source_parse_override_specs_dir() {
        let source = ConfigSource::parse("core.specs_dir=./my-specs/");
        match source {
            ConfigSource::Override { key, value } => {
                assert_eq!(key, "core.specs_dir");
                assert_eq!(value, "./my-specs/");
            }
            _ => panic!("Expected Override variant"),
        }
    }

    #[test]
    fn test_config_source_to_cli_string_roundtrips() {
        // File path
        let source = ConfigSource::File(PathBuf::from("ralph.yml"));
        assert_eq!(source.to_cli_string(), "ralph.yml");

        // Builtin (legacy)
        let source = ConfigSource::Builtin("code-assist".to_string());
        assert_eq!(source.to_cli_string(), "builtin:code-assist");

        // Remote URL
        let source = ConfigSource::Remote("https://example.com/ralph.yml".to_string());
        assert_eq!(source.to_cli_string(), "https://example.com/ralph.yml");

        // Override
        let source = ConfigSource::Override {
            key: "core.scratchpad".to_string(),
            value: ".ralph/feature/scratchpad.md".to_string(),
        };
        assert_eq!(
            source.to_cli_string(),
            "core.scratchpad=.ralph/feature/scratchpad.md"
        );
    }

    #[test]
    fn test_bot_daemon_parses_global_config_flag() {
        let cli = Cli::try_parse_from(["ralph", "bot", "daemon", "-c", "ralph.bot.yml"])
            .expect("CLI parse failed");

        assert!(cli.config.iter().any(|value| value == "ralph.bot.yml"));
        assert!(matches!(
            cli.command,
            Some(Commands::Bot(crate::bot::BotArgs {
                command: crate::bot::BotCommands::Daemon(_),
            }))
        ));
    }

    #[test]
    fn test_doctor_parses_command() {
        let cli = Cli::try_parse_from(["ralph", "doctor"]).expect("CLI parse failed");

        assert!(matches!(cli.command, Some(Commands::Doctor(_))));
    }

    #[test]
    fn test_tutorial_parses_command() {
        let cli = Cli::try_parse_from(["ralph", "tutorial"]).expect("CLI parse failed");

        assert!(matches!(cli.command, Some(Commands::Tutorial(_))));
    }

    #[test]
    fn test_mcp_serve_parses_command() {
        let cli = Cli::try_parse_from(["ralph", "mcp", "serve"]).expect("CLI parse failed");
        assert!(matches!(cli.command, Some(Commands::Mcp(_))));
    }

    #[test]
    fn test_mcp_serve_parses_workspace_root_flag() {
        let cli = Cli::try_parse_from([
            "ralph",
            "mcp",
            "serve",
            "--workspace-root",
            "/tmp/ralph-workspace",
        ])
        .expect("CLI parse failed");

        match cli.command {
            Some(Commands::Mcp(crate::mcp::McpArgs {
                command: crate::mcp::McpCommands::Serve(crate::mcp::ServeArgs { workspace_root }),
            })) => {
                assert_eq!(
                    workspace_root,
                    Some(std::path::PathBuf::from("/tmp/ralph-workspace"))
                );
            }
            other => panic!("unexpected CLI parse result: {other:?}"),
        }
    }

    #[test]
    fn test_resolve_workspace_root_discovers_ancestor_ralph_dir() {
        let temp_dir = TempDir::new().expect("temp dir");
        std::fs::create_dir_all(temp_dir.path().join(".ralph")).expect("ralph dir");
        let nested = temp_dir.path().join("a/b/c");
        std::fs::create_dir_all(&nested).expect("nested dir");

        assert_eq!(
            discover_workspace_root(&nested),
            Some(temp_dir.path().to_path_buf())
        );
    }

    #[test]
    fn test_emit_command_resolves_marker_relative_to_workspace_root_from_nested_dir() {
        let temp_dir = TempDir::new().expect("temp dir");
        let workspace = temp_dir.path().to_path_buf();
        std::fs::create_dir_all(workspace.join(".ralph")).expect("ralph dir");
        std::fs::write(
            workspace.join(".ralph/current-events"),
            ".ralph/events-20260309-test.jsonl\n",
        )
        .expect("write marker");

        emit_command_with_root(
            ColorMode::Never,
            EmitArgs {
                topic: "debug.step".to_string(),
                payload: "task_id=demo".to_string(),
                json: false,
                ts: Some("2026-03-09T00:00:00Z".to_string()),
                file: PathBuf::from(".ralph/events.jsonl"),
            },
            Some(&workspace),
        )
        .expect("emit command");

        let events = std::fs::read_to_string(workspace.join(".ralph/events-20260309-test.jsonl"))
            .expect("read events");
        assert!(events.contains("\"topic\":\"debug.step\""));
        assert!(events.contains("task_id=demo"));
    }

    #[test]
    fn test_emit_command_blocks_once_when_urgent_steer_pending() {
        let temp_dir = TempDir::new().expect("temp dir");
        let workspace = temp_dir.path().to_path_buf();
        std::fs::create_dir_all(workspace.join(".ralph")).expect("ralph dir");
        UrgentSteerStore::new(urgent_steer_path_from_workspace(Some(&workspace)))
            .append_message("stop and fix the failing tests")
            .expect("write urgent steer");

        let err = emit_command_with_root(
            ColorMode::Never,
            EmitArgs {
                topic: "debug.step".to_string(),
                payload: "task_id=demo".to_string(),
                json: false,
                ts: Some("2026-03-09T00:00:00Z".to_string()),
                file: PathBuf::from(".ralph/events.jsonl"),
            },
            Some(&workspace),
        )
        .expect_err("urgent steer should block first emit");

        let message = format!("{err:#}");
        assert!(message.contains("Urgent steer is pending"));
        assert!(message.contains("stop and fix the failing tests"));

        assert!(
            UrgentSteerStore::new(urgent_steer_path_from_workspace(Some(&workspace)))
                .load()
                .expect("load marker")
                .is_none(),
            "first blocked emit should clear urgent steer marker"
        );
    }

    #[test]
    fn test_tutorial_steps_cover_core_topics() {
        let steps = tutorial_steps();
        assert_eq!(steps.len(), 3);
        assert!(steps.iter().any(|step| step.title.contains("Hats")));
        assert!(
            steps
                .iter()
                .any(|step| step.title.contains("Hat collections"))
        );
        assert!(steps.iter().any(|step| step.title.contains("Workflow")));
    }

    #[test]
    fn test_config_source_parse_file_with_equals() {
        // Paths containing '=' but not starting with 'core.' should be treated as files
        let source = ConfigSource::parse("path/with=equals.yml");
        match source {
            ConfigSource::File(path) => {
                assert_eq!(path, std::path::PathBuf::from("path/with=equals.yml"))
            }
            _ => panic!("Expected File variant for path with equals sign"),
        }
    }

    #[test]
    fn test_config_source_parse_core_without_equals() {
        // "core.field" without '=' should be treated as a file path (will fail to load)
        let source = ConfigSource::parse("core.field");
        match source {
            ConfigSource::File(path) => assert_eq!(path, std::path::PathBuf::from("core.field")),
            _ => panic!("Expected File variant for core.field without ="),
        }
    }

    #[test]
    fn test_apply_config_overrides_scratchpad() {
        let mut config = RalphConfig::default();
        let sources = vec![ConfigSource::Override {
            key: "core.scratchpad".to_string(),
            value: ".custom/scratch.md".to_string(),
        }];
        apply_config_overrides(&mut config, &sources).unwrap();
        assert_eq!(config.core.scratchpad.path, ".custom/scratch.md");
    }

    #[test]
    fn test_apply_config_overrides_specs_dir() {
        let mut config = RalphConfig::default();
        let sources = vec![ConfigSource::Override {
            key: "core.specs_dir".to_string(),
            value: "./specifications/".to_string(),
        }];
        apply_config_overrides(&mut config, &sources).unwrap();
        assert_eq!(config.core.specs_dir, "./specifications/");
    }

    #[test]
    fn test_apply_config_overrides_multiple() {
        let mut config = RalphConfig::default();
        let sources = vec![
            ConfigSource::Override {
                key: "core.scratchpad".to_string(),
                value: ".custom/scratch.md".to_string(),
            },
            ConfigSource::Override {
                key: "core.specs_dir".to_string(),
                value: "./my-specs/".to_string(),
            },
        ];
        apply_config_overrides(&mut config, &sources).unwrap();
        assert_eq!(config.core.scratchpad.path, ".custom/scratch.md");
        assert_eq!(config.core.specs_dir, "./my-specs/");
    }

    #[test]
    fn test_apply_config_overrides_unknown_field() {
        // Unknown core.* fields should warn but not error
        let mut config = RalphConfig::default();
        let original_scratchpad = config.core.scratchpad.path.clone();
        let sources = vec![ConfigSource::Override {
            key: "core.unknown_field".to_string(),
            value: "some_value".to_string(),
        }];
        // Should not error
        apply_config_overrides(&mut config, &sources).unwrap();
        // Original values should be unchanged
        assert_eq!(config.core.scratchpad.path, original_scratchpad);
    }

    #[test]
    fn test_config_source_parse_non_core_with_equals_is_file() {
        // Non-core.* prefix with '=' should be treated as file path per spec
        let source = ConfigSource::parse("event_loop.max_iterations=5");
        match source {
            ConfigSource::File(path) => {
                assert_eq!(
                    path,
                    std::path::PathBuf::from("event_loop.max_iterations=5")
                )
            }
            _ => panic!("Expected File variant, not Override"),
        }
    }

    #[test]
    fn test_ensure_scratchpad_directory_creates_nested() {
        let temp_dir = tempfile::tempdir().unwrap();
        let mut config = RalphConfig::default();
        config.core.workspace_root = temp_dir.path().to_path_buf();

        config.core.scratchpad.path = "a/b/c/scratchpad.md".to_string();

        let result = ensure_scratchpad_directory(&config);
        assert!(result.is_ok());

        // Verify directory was created
        let expected_dir = temp_dir.path().join("a/b/c");
        assert!(expected_dir.exists());
    }

    #[test]
    fn test_ensure_scratchpad_directory_noop_when_exists() {
        let temp_dir = tempfile::tempdir().unwrap();
        let mut config = RalphConfig::default();
        config.core.workspace_root = temp_dir.path().to_path_buf();

        // Pre-create the directory
        let subdir = temp_dir.path().join("existing");
        std::fs::create_dir_all(&subdir).unwrap();
        config.core.scratchpad.path = "existing/scratchpad.md".to_string();

        // Should succeed without error (no-op)
        let result = ensure_scratchpad_directory(&config);
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_auto_preflight_dry_run_returns_report() {
        let temp_dir = tempfile::tempdir().unwrap();
        let mut config = RalphConfig::default();
        config.core.workspace_root = temp_dir.path().to_path_buf();
        config.features.preflight.enabled = true;
        config.features.preflight.skip = vec!["git".to_string(), "tools".to_string()];
        config.cli.backend = "custom".to_string();
        config.cli.command = Some("definitely-missing-12345".to_string());

        let report = run_auto_preflight(&config, false, false, AutoPreflightMode::DryRun)
            .await
            .unwrap();

        let report = report.expect("expected preflight report in dry-run mode");
        assert!(!report.passed);
        assert!(report.failures >= 1);
    }

    #[tokio::test]
    async fn test_auto_preflight_skip_list_can_omit_hooks_check_failures() {
        let temp_dir = tempfile::tempdir().unwrap();
        let mut config = RalphConfig::default();
        config.core.workspace_root = temp_dir.path().to_path_buf();
        config.features.preflight.enabled = true;
        config.cli.backend = "custom".to_string();

        let backend_cmd = temp_dir.path().join("backend-ok");
        std::fs::write(&backend_cmd, "ok").unwrap();
        config.cli.command = Some(backend_cmd.to_string_lossy().to_string());

        config.hooks.enabled = true;
        config.hooks.events.insert(
            HookPhaseEvent::PreLoopStart,
            vec![HookSpec {
                name: "broken-hook".to_string(),
                command: vec!["./scripts/hooks/missing.sh".to_string()],
                cwd: None,
                env: std::collections::HashMap::new(),
                timeout_seconds: None,
                max_output_bytes: None,
                on_error: Some(HookOnError::Block),
                suspend_mode: None,
                mutate: HookMutationConfig::default(),
                extra: std::collections::HashMap::new(),
            }],
        );

        let unskipped = run_auto_preflight(&config, false, false, AutoPreflightMode::DryRun)
            .await
            .unwrap()
            .expect("dry-run preflight report");

        assert!(!unskipped.passed);
        let hooks_check = unskipped
            .checks
            .iter()
            .find(|check| check.name == "hooks")
            .expect("hooks check should be present without skip");
        assert_eq!(hooks_check.status, CheckStatus::Fail);

        config.features.preflight.skip = vec!["hooks".to_string()];
        let skipped = run_auto_preflight(&config, false, false, AutoPreflightMode::DryRun)
            .await
            .unwrap()
            .expect("dry-run preflight report");

        assert!(skipped.passed);
        assert!(skipped.checks.iter().all(|check| check.name != "hooks"));
    }

    #[tokio::test]
    async fn test_auto_preflight_run_fails_on_check_failure() {
        let temp_dir = tempfile::tempdir().unwrap();
        let mut config = RalphConfig::default();
        config.core.workspace_root = temp_dir.path().to_path_buf();
        config.features.preflight.enabled = true;
        config.features.preflight.skip = vec!["git".to_string(), "tools".to_string()];
        config.cli.backend = "custom".to_string();
        config.cli.command = Some("definitely-missing-12345".to_string());

        let err = run_auto_preflight(&config, false, false, AutoPreflightMode::Run)
            .await
            .expect_err("expected preflight failure in run mode");

        assert!(err.to_string().contains("Preflight checks failed"));
    }

    #[test]
    fn test_partition_config_sources_separates_overrides() {
        let sources = [
            ConfigSource::File(PathBuf::from("ralph.yml")),
            ConfigSource::Override {
                key: "core.scratchpad".to_string(),
                value: ".custom/scratchpad.md".to_string(),
            },
            ConfigSource::Builtin("tdd".to_string()),
            ConfigSource::Override {
                key: "core.specs_dir".to_string(),
                value: "./specs/".to_string(),
            },
        ];

        let (primary, overrides): (Vec<_>, Vec<_>) = sources
            .iter()
            .partition(|s| !matches!(s, ConfigSource::Override { .. }));

        assert_eq!(primary.len(), 2); // File + Builtin
        assert_eq!(overrides.len(), 2); // Two overrides
        assert!(matches!(primary[0], ConfigSource::File(_)));
        assert!(matches!(primary[1], ConfigSource::Builtin(_)));
    }

    #[test]
    fn test_partition_config_sources_only_overrides() {
        let sources = [ConfigSource::Override {
            key: "core.scratchpad".to_string(),
            value: ".custom/scratchpad.md".to_string(),
        }];

        let (primary, overrides): (Vec<_>, Vec<_>) = sources
            .iter()
            .partition(|s| !matches!(s, ConfigSource::Override { .. }));

        assert_eq!(primary.len(), 0); // No primary sources
        assert_eq!(overrides.len(), 1); // One override
    }

    #[test]
    fn test_load_config_from_file_with_overrides() {
        // Integration test: load a real config file and apply overrides
        let temp_dir = tempfile::tempdir().unwrap();
        let config_path = temp_dir.path().join("test.yml");
        std::fs::write(
            &config_path,
            r"
cli:
  backend: claude
core:
  scratchpad: .agent/scratchpad.md
  specs_dir: ./specs/
",
        )
        .unwrap();

        let mut config = RalphConfig::from_file(&config_path).unwrap();
        assert_eq!(config.core.scratchpad.path, ".agent/scratchpad.md");

        // Apply override
        let overrides = vec![ConfigSource::Override {
            key: "core.scratchpad".to_string(),
            value: ".custom/scratch.md".to_string(),
        }];
        apply_config_overrides(&mut config, &overrides).unwrap();

        assert_eq!(config.core.scratchpad.path, ".custom/scratch.md");
        assert_eq!(config.core.specs_dir, "./specs/"); // Unchanged
    }

    /// Regression test for prompt_summary reading file content instead of path.
    ///
    /// Previously, when prompt_file was used, the prompt_summary would just
    /// return the file path string. This caused confusing error messages like
    /// "Configuration file not found at con..." when the path was displayed.
    ///
    /// The fix ensures prompt_summary reads the actual file content.
    #[test]
    fn test_prompt_summary_reads_file_content_not_path() {
        let temp_dir = tempfile::tempdir().unwrap();
        let prompt_path = temp_dir.path().join("PROMPT.md");
        let prompt_content = "Build a feature that does amazing things";

        // Write the prompt file
        std::fs::write(&prompt_path, prompt_content).unwrap();

        // Create config with prompt_file set
        let mut config = RalphConfig::default();
        config.event_loop.prompt_file = prompt_path.to_string_lossy().to_string();
        config.event_loop.prompt = None;

        // Simulate the prompt_summary logic from run_command
        let prompt_summary = config
            .event_loop
            .prompt
            .clone()
            .or_else(|| {
                let prompt_file = &config.event_loop.prompt_file;
                if prompt_file.is_empty() {
                    None
                } else {
                    let path = std::path::Path::new(prompt_file);
                    if path.exists() {
                        std::fs::read_to_string(path).ok()
                    } else {
                        None
                    }
                }
            })
            .map(|p| truncate_with_ellipsis(&p, 100))
            .unwrap_or_else(|| "[no prompt]".to_string());

        // Assert: summary contains file content, NOT the file path
        assert_eq!(prompt_summary, prompt_content);
        assert!(!prompt_summary.contains("PROMPT.md"));
        assert!(!prompt_summary.contains(&temp_dir.path().to_string_lossy().to_string()));
    }

    #[test]
    fn test_prompt_summary_truncates_long_content() {
        let temp_dir = tempfile::tempdir().unwrap();
        let prompt_path = temp_dir.path().join("LONG_PROMPT.md");
        let long_content = "X".repeat(150); // 150 chars, exceeds 100 limit

        std::fs::write(&prompt_path, &long_content).unwrap();

        let mut config = RalphConfig::default();
        config.event_loop.prompt_file = prompt_path.to_string_lossy().to_string();
        config.event_loop.prompt = None;

        // Simulate the prompt_summary logic
        let prompt_summary = config
            .event_loop
            .prompt
            .clone()
            .or_else(|| {
                let prompt_file = &config.event_loop.prompt_file;
                if prompt_file.is_empty() {
                    None
                } else {
                    let path = std::path::Path::new(prompt_file);
                    if path.exists() {
                        std::fs::read_to_string(path).ok()
                    } else {
                        None
                    }
                }
            })
            .map(|p| truncate_with_ellipsis(&p, 100))
            .unwrap_or_else(|| "[no prompt]".to_string());

        // Assert: truncated to 100 chars total
        assert_eq!(prompt_summary.len(), 100);
        assert!(prompt_summary.ends_with("..."));
    }

    #[test]
    fn test_prompt_summary_returns_no_prompt_for_missing_file() {
        let mut config = RalphConfig::default();
        config.event_loop.prompt_file = "/nonexistent/path/PROMPT.md".to_string();
        config.event_loop.prompt = None;

        // Simulate the prompt_summary logic
        let prompt_summary = config
            .event_loop
            .prompt
            .clone()
            .or_else(|| {
                let prompt_file = &config.event_loop.prompt_file;
                if prompt_file.is_empty() {
                    None
                } else {
                    let path = std::path::Path::new(prompt_file);
                    if path.exists() {
                        std::fs::read_to_string(path).ok()
                    } else {
                        None
                    }
                }
            })
            .map(|p| truncate_with_ellipsis(&p, 100))
            .unwrap_or_else(|| "[no prompt]".to_string());

        // Assert: returns "[no prompt]" for missing file
        assert_eq!(prompt_summary, "[no prompt]");
    }

    #[test]
    fn test_format_preflight_summary_with_failures() {
        let report = PreflightReport {
            passed: false,
            warnings: 1,
            failures: 1,
            checks: vec![
                ralph_core::CheckResult::pass("config", "Config"),
                ralph_core::CheckResult::warn("backend", "Backend", "Missing"),
                ralph_core::CheckResult::fail("paths", "Paths", "Missing path"),
            ],
        };

        let summary = format_preflight_summary(&report);

        assert!(summary.contains("✓"));
        assert!(summary.contains("⚠"));
        assert!(summary.contains("✗"));
        assert!(summary.contains("(1 failure)"));
    }

    #[test]
    fn test_format_preflight_summary_no_checks() {
        let report = PreflightReport {
            passed: true,
            warnings: 0,
            failures: 0,
            checks: Vec::new(),
        };

        let summary = format_preflight_summary(&report);

        assert_eq!(summary, "no checks");
    }

    #[test]
    fn test_preflight_failure_detail_strict_includes_warnings() {
        let report = PreflightReport {
            passed: false,
            warnings: 2,
            failures: 1,
            checks: Vec::new(),
        };

        assert_eq!(preflight_failure_detail(&report, false), "1 failure");
        assert_eq!(
            preflight_failure_detail(&report, true),
            "1 failure, 2 warnings"
        );
    }

    #[test]
    fn test_load_config_with_overrides_applies_override_sources() {
        let temp_dir = tempfile::tempdir().unwrap();
        let _cwd = CwdGuard::set(temp_dir.path());
        let config_path = temp_dir.path().join("ralph.yml");
        std::fs::write(&config_path, "core:\n  scratchpad: .agent/scratchpad.md\n").unwrap();

        let sources = vec![
            ConfigSource::File(config_path),
            ConfigSource::Override {
                key: "core.scratchpad".to_string(),
                value: ".custom/scratch.md".to_string(),
            },
        ];

        let config = load_config_with_overrides(&sources).unwrap();

        assert_eq!(config.core.scratchpad.path, ".custom/scratch.md");
        let expected_root = std::fs::canonicalize(temp_dir.path())
            .unwrap_or_else(|_| temp_dir.path().to_path_buf());
        let actual_root = std::fs::canonicalize(&config.core.workspace_root)
            .unwrap_or_else(|_| config.core.workspace_root.clone());
        assert_eq!(actual_root, expected_root);
    }

    #[test]
    fn test_load_config_with_overrides_only_overrides_uses_defaults() {
        let temp_dir = tempfile::tempdir().unwrap();
        let _cwd = CwdGuard::set(temp_dir.path());

        let sources = vec![ConfigSource::Override {
            key: "core.specs_dir".to_string(),
            value: "custom-specs".to_string(),
        }];

        let config = load_config_with_overrides(&sources).unwrap();

        assert_eq!(config.core.specs_dir, "custom-specs");
        let expected_root = std::fs::canonicalize(temp_dir.path())
            .unwrap_or_else(|_| temp_dir.path().to_path_buf());
        let actual_root = std::fs::canonicalize(&config.core.workspace_root)
            .unwrap_or_else(|_| config.core.workspace_root.clone());
        assert_eq!(actual_root, expected_root);
    }

    #[test]
    fn test_load_config_with_overrides_missing_file_falls_back_to_defaults() {
        let temp_dir = tempfile::tempdir().unwrap();
        let _cwd = CwdGuard::set(temp_dir.path());

        let sources = vec![ConfigSource::File(PathBuf::from("missing.yml"))];

        let config = load_config_with_overrides(&sources).unwrap();

        assert!(!config.core.scratchpad.path.is_empty());
        let expected_root = std::fs::canonicalize(temp_dir.path())
            .unwrap_or_else(|_| temp_dir.path().to_path_buf());
        let actual_root = std::fs::canonicalize(&config.core.workspace_root)
            .unwrap_or_else(|_| config.core.workspace_root.clone());
        assert_eq!(actual_root, expected_root);
    }

    #[test]
    fn test_list_directory_contents_handles_nested_paths() {
        let temp_dir = tempfile::tempdir().unwrap();
        let nested_dir = temp_dir.path().join("one/two");
        std::fs::create_dir_all(&nested_dir).unwrap();
        std::fs::write(temp_dir.path().join("one/file.txt"), "hello").unwrap();

        assert!(list_directory_contents(temp_dir.path(), false, 0).is_ok());
        assert!(list_directory_contents(temp_dir.path(), true, 0).is_ok());
    }

    #[test]
    fn test_list_directory_contents_missing_path_returns_error() {
        let temp_dir = tempfile::tempdir().unwrap();
        let missing = temp_dir.path().join("missing");

        assert!(list_directory_contents(&missing, false, 0).is_err());
    }

    #[test]
    fn test_print_preflight_summary_handles_failures_and_warnings() {
        let report = PreflightReport {
            passed: false,
            warnings: 1,
            failures: 1,
            checks: vec![
                ralph_core::CheckResult::pass("config", "Config"),
                ralph_core::CheckResult::warn("backend", "Backend", "Missing"),
                ralph_core::CheckResult::fail("paths", "Paths", "Missing path"),
            ],
        };

        print_preflight_summary(&report, true, "Preflight: ", true);
        print_preflight_summary(&report, false, "Preflight: ", false);
    }

    fn default_run_args() -> RunArgs {
        RunArgs {
            prompt_text: None,
            backend: Some("claude".to_string()),
            prompt_file: None,
            max_iterations: None,
            completion_promise: None,
            dry_run: false,
            continue_mode: false,
            loop_id: None,
            no_tui: true,
            autonomous: false,
            rpc: false,
            legacy_tui: false,
            idle_timeout: None,
            exclusive: false,
            no_auto_merge: false,
            skip_preflight: true,
            verbose: false,
            quiet: false,
            record_session: None,
            custom_args: Vec::new(),
        }
    }

    #[tokio::test]
    async fn test_run_command_continue_missing_scratchpad_returns_error() {
        let temp_dir = tempfile::tempdir().unwrap();
        let _cwd = CwdGuard::set(temp_dir.path());

        let mut args = default_run_args();
        args.continue_mode = true;

        let err = run_command(&[], None, false, ColorMode::Never, args)
            .await
            .expect_err("expected missing scratchpad error");
        assert!(err.to_string().contains("scratchpad not found"));
    }

    #[tokio::test]
    async fn test_run_command_dry_run_inline_prompt_skips_execution() {
        let temp_dir = tempfile::tempdir().unwrap();
        let _cwd = CwdGuard::set(temp_dir.path());

        let mut args = default_run_args();
        args.dry_run = true;
        args.prompt_text = Some("Test inline prompt".to_string());

        run_command(&[], None, false, ColorMode::Never, args)
            .await
            .expect("dry run should succeed");
    }

    #[tokio::test]
    async fn test_run_command_allows_single_file_combined_config() {
        let temp_dir = tempfile::tempdir().unwrap();
        let _cwd = CwdGuard::set(temp_dir.path());

        std::fs::write(
            temp_dir.path().join("ralph.yml"),
            r#"
cli:
  backend: claude
hats:
  builder:
    name: Builder
    description: Test builder
    triggers: ["build.task"]
    publishes: ["build.done"]
"#,
        )
        .unwrap();

        let mut args = default_run_args();
        args.dry_run = true;
        args.prompt_text = Some("Test inline prompt".to_string());

        run_command(
            &[ConfigSource::File(std::path::PathBuf::from("ralph.yml"))],
            None,
            false,
            ColorMode::Never,
            args,
        )
        .await
        .expect("combined config should be accepted");
    }

    #[test]
    fn test_diagnostics_eligible_for_run_command() {
        let command = Some(Commands::Run(default_run_args()));
        assert!(is_diagnostics_eligible_command(command.as_ref()));
    }

    #[test]
    fn test_diagnostics_eligible_for_no_subcommand() {
        assert!(is_diagnostics_eligible_command(None));
    }

    #[test]
    fn test_diagnostics_not_eligible_for_emit_command() {
        let command = Some(Commands::Emit(EmitArgs {
            topic: "test.event".to_string(),
            payload: String::new(),
            json: false,
            ts: None,
            file: PathBuf::from(".ralph/events.jsonl"),
        }));
        assert!(!is_diagnostics_eligible_command(command.as_ref()));
    }

    #[test]
    fn test_diagnostics_not_eligible_for_events_command() {
        let command = Some(Commands::Events(EventsArgs {
            last: None,
            topic: None,
            iteration: None,
            format: OutputFormat::Table,
            file: None,
            clear: false,
        }));
        assert!(!is_diagnostics_eligible_command(command.as_ref()));
    }
}
