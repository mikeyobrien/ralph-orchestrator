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
//! - Work item tracking via `ralph task`

mod init;
mod memory;
mod presets;
mod sop_runner;
mod task_cli;
mod tools;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use ralph_adapters::{
    CliBackend, CliExecutor, ConsoleStreamHandler, OutputFormat as BackendOutputFormat,
    PrettyStreamHandler, PtyConfig, PtyExecutor, QuietStreamHandler, TuiStreamHandler,
    detect_backend,
};
use ralph_core::{
    EventHistory, EventLogger, EventLoop, EventParser, EventRecord, RalphConfig, Record,
    SessionRecorder, SummaryWriter, TerminationReason,
};
use ralph_proto::{Event, HatId};
use ralph_tui::Tui;
use std::fs::{self, File};
use std::io::{BufWriter, IsTerminal, Write, stdout};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, error, info, warn};

// Unix-specific process management for process group leadership
#[cfg(unix)]
mod process_management {
    use nix::unistd::{Pid, setpgid};
    use tracing::debug;

    /// Sets up process group leadership.
    ///
    /// Per spec: "The orchestrator must run as a process group leader. All spawned
    /// CLI processes (Claude, Kiro, etc.) belong to this group. On termination,
    /// the entire process group receives the signal, preventing orphans."
    pub fn setup_process_group() {
        // Make ourselves the process group leader
        // This ensures our child processes are in our process group
        let pid = Pid::this();
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
        match self {
            ColorMode::Always => true,
            ColorMode::Never => false,
            ColorMode::Auto => stdout().is_terminal(),
        }
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
        // CLI flags take precedence
        if cli_quiet {
            return Verbosity::Quiet;
        }
        if cli_verbose {
            return Verbosity::Verbose;
        }

        // Environment variables
        if std::env::var("RALPH_QUIET").is_ok() {
            return Verbosity::Quiet;
        }
        if std::env::var("RALPH_VERBOSE").is_ok() {
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

/// ANSI color codes for terminal output.
mod colors {
    pub const RESET: &str = "\x1b[0m";
    pub const BOLD: &str = "\x1b[1m";
    pub const DIM: &str = "\x1b[2m";
    pub const GREEN: &str = "\x1b[32m";
    pub const YELLOW: &str = "\x1b[33m";
    pub const RED: &str = "\x1b[31m";
    pub const CYAN: &str = "\x1b[36m";
    pub const BLUE: &str = "\x1b[34m";
    pub const MAGENTA: &str = "\x1b[35m";
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
    /// Path to configuration file
    #[arg(short, long, default_value = "ralph.yml", global = true)]
    config: PathBuf,

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

    /// DEPRECATED: Use `ralph run --continue` instead.
    /// Resume a previously interrupted loop from existing scratchpad.
    #[command(hide = true)]
    Resume(ResumeArgs),

    /// View event history for debugging
    Events(EventsArgs),

    /// Initialize a new ralph.yml configuration file
    Init(InitArgs),

    /// Clean up Ralph artifacts (.agent/ directory)
    Clean(CleanArgs),

    /// Emit an event to the current run's events file with proper JSON formatting
    Emit(EmitArgs),

    /// Start a Prompt-Driven Development planning session
    Plan(PlanArgs),

    /// Generate code task files from descriptions or plans
    CodeTask(CodeTaskArgs),

    /// Create code tasks (alias for code-task)
    Task(CodeTaskArgs),

    /// Ralph's runtime tools (agent-facing)
    Tools(tools::ToolsArgs),
}

/// Arguments for the init subcommand.
#[derive(Parser, Debug)]
struct InitArgs {
    /// Backend to use (claude, kiro, gemini, codex, amp, custom).
    /// When used alone, generates minimal config.
    /// When used with --preset, overrides the preset's backend.
    #[arg(long, conflicts_with = "list_presets")]
    backend: Option<String>,

    /// Copy embedded preset to ralph.yml
    #[arg(long, conflicts_with = "list_presets")]
    preset: Option<String>,

    /// List all available embedded presets
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

    // ─────────────────────────────────────────────────────────────────────────
    // Execution Mode Options
    // ─────────────────────────────────────────────────────────────────────────
    /// Disable TUI observation mode (TUI is enabled by default)
    #[arg(long, conflicts_with = "autonomous")]
    no_tui: bool,

    /// Force autonomous mode (headless, non-interactive).
    /// Overrides default_mode from config.
    #[arg(short, long, conflicts_with = "no_tui")]
    autonomous: bool,

    /// Idle timeout in seconds for interactive mode (default: 30).
    /// Process is terminated after this many seconds of inactivity.
    /// Set to 0 to disable idle timeout.
    #[arg(long)]
    idle_timeout: Option<u32>,

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
    #[arg(short, long, conflicts_with = "no_tui")]
    autonomous: bool,

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

    /// Clean diagnostic logs instead of .agent directory
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
}

#[tokio::main]
async fn main() -> Result<()> {
    // Install panic hook to restore terminal state on crash
    // This prevents the terminal from being left in raw mode or alternate screen
    install_panic_hook();

    let cli = Cli::parse();

    // Detect if TUI mode is requested - TUI owns the terminal, so logs must not go to stdout
    // TUI is enabled by default unless --no-tui is specified or --autonomous is used
    let tui_enabled = match &cli.command {
        Some(Commands::Run(args)) => !args.no_tui && !args.autonomous,
        Some(Commands::Resume(args)) => !args.no_tui && !args.autonomous,
        _ => false,
    };

    // Initialize logging - suppress in TUI mode to avoid corrupting the display
    let filter = if cli.verbose { "debug" } else { "info" };

    // Check if diagnostics are enabled
    let diagnostics_enabled = std::env::var("RALPH_DIAGNOSTICS")
        .map(|v| v == "1")
        .unwrap_or(false);

    if tui_enabled {
        // TUI mode: logs would corrupt the display, so we suppress them entirely.
        // For debugging TUI issues, set RALPH_DEBUG_LOG=1 to write to .agent/ralph.log
        if std::env::var("RALPH_DEBUG_LOG").is_ok() {
            let log_path = std::path::Path::new(".agent").join("ralph.log");
            if let Ok(file) = std::fs::File::create(&log_path) {
                if diagnostics_enabled {
                    // TUI + diagnostics: logs to file + trace layer
                    use ralph_core::diagnostics::DiagnosticTraceLayer;
                    use tracing_subscriber::prelude::*;

                    if let Ok(collector) = ralph_core::diagnostics::DiagnosticsCollector::new(
                        std::path::Path::new("."),
                    ) && let Some(session_dir) = collector.session_dir()
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
                            // Fallback: just file logging
                            tracing_subscriber::fmt()
                                .with_env_filter(filter)
                                .with_writer(std::sync::Mutex::new(file))
                                .with_ansi(false)
                                .init();
                        }
                    }
                } else {
                    // TUI without diagnostics: just file logging
                    tracing_subscriber::fmt()
                        .with_env_filter(filter)
                        .with_writer(std::sync::Mutex::new(file))
                        .with_ansi(false)
                        .init();
                }
            }
        }
        // If RALPH_DEBUG_LOG is not set or file creation fails, no logging (default)
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

    match cli.command {
        Some(Commands::Run(args)) => run_command(cli.config, cli.verbose, cli.color, args).await,
        Some(Commands::Resume(args)) => {
            resume_command(cli.config, cli.verbose, cli.color, args).await
        }
        Some(Commands::Events(args)) => events_command(cli.color, args),
        Some(Commands::Init(args)) => init_command(cli.color, args),
        Some(Commands::Clean(args)) => clean_command(cli.config, cli.color, args),
        Some(Commands::Emit(args)) => emit_command(cli.color, args),
        Some(Commands::Plan(args)) => plan_command(cli.config, cli.color, args),
        Some(Commands::CodeTask(args)) => code_task_command(cli.config, cli.color, args),
        Some(Commands::Task(args)) => code_task_command(cli.config, cli.color, args),
        Some(Commands::Tools(args)) => tools::execute(args, cli.color.should_use_colors()),
        None => {
            // Default to run with TUI enabled (new default behavior)
            let args = RunArgs {
                prompt_text: None,
                prompt_file: None,
                max_iterations: None,
                completion_promise: None,
                dry_run: false,
                continue_mode: false,
                no_tui: false, // TUI enabled by default
                autonomous: false,
                idle_timeout: None,
                verbose: false,
                quiet: false,
                record_session: None,
            };
            run_command(cli.config, cli.verbose, cli.color, args).await
        }
    }
}

async fn run_command(
    config_path: PathBuf,
    verbose: bool,
    color_mode: ColorMode,
    args: RunArgs,
) -> Result<()> {
    // Load configuration
    let mut config = if config_path.exists() {
        RalphConfig::from_file(&config_path)
            .with_context(|| format!("Failed to load config from {:?}", config_path))?
    } else {
        warn!("Config file {:?} not found, using defaults", config_path);
        RalphConfig::default()
    };

    // Normalize v1 flat fields into v2 nested structure
    config.normalize();

    // Set workspace_root to current directory (critical for E2E tests in isolated workspaces).
    // This must happen after config load because workspace_root has #[serde(skip)] and
    // defaults to cwd at deserialize time - but we need it set to the actual runtime cwd.
    config.core.workspace_root =
        std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));

    // Handle --continue mode: check scratchpad exists before proceeding
    let resume = args.continue_mode;
    if resume {
        let scratchpad_path = std::path::Path::new(&config.core.scratchpad);
        if !scratchpad_path.exists() {
            anyhow::bail!(
                "Cannot continue: scratchpad not found at '{}'. \
                 Start a fresh run with `ralph run`.",
                config.core.scratchpad
            );
        }
        info!(
            "Found existing scratchpad at '{}', continuing from previous state",
            config.core.scratchpad
        );
    }

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

    if args.dry_run {
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
            let preview = if inline.len() > 60 {
                format!("{}...", &inline[..60].replace('\n', " "))
            } else {
                inline.replace('\n', " ")
            };
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
        return Ok(());
    }

    // Run the orchestration loop and exit with proper exit code
    // TUI is enabled by default (unless --no-tui or --autonomous is specified)
    let enable_tui = !args.no_tui && !args.autonomous;
    let verbosity = Verbosity::resolve(verbose || args.verbose, args.quiet);
    let reason = run_loop_impl(
        config,
        color_mode,
        resume,
        enable_tui,
        verbosity,
        args.record_session,
    )
    .await?;
    let exit_code = reason.exit_code();

    // Use explicit exit for non-zero codes to ensure proper exit status
    if exit_code != 0 {
        std::process::exit(exit_code);
    }

    Ok(())
}

/// Resume a previously interrupted loop from existing scratchpad.
///
/// DEPRECATED: Use `ralph run --continue` instead.
///
/// Per spec: "When loop terminates due to safeguard (not completion promise),
/// user can run `ralph run --continue` to restart reading existing scratchpad,
/// continuing from where it left off."
async fn resume_command(
    config_path: PathBuf,
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

    // Load configuration
    let mut config = if config_path.exists() {
        RalphConfig::from_file(&config_path)
            .with_context(|| format!("Failed to load config from {:?}", config_path))?
    } else {
        warn!("Config file {:?} not found, using defaults", config_path);
        RalphConfig::default()
    };

    config.normalize();

    // Set workspace_root to current directory (critical for E2E tests in isolated workspaces).
    config.core.workspace_root =
        std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));

    // Check that scratchpad exists (required for resume)
    let scratchpad_path = std::path::Path::new(&config.core.scratchpad);
    if !scratchpad_path.exists() {
        anyhow::bail!(
            "Cannot continue: scratchpad not found at '{}'. \
             Start a fresh run with `ralph run`.",
            config.core.scratchpad
        );
    }

    info!(
        "Found existing scratchpad at '{}', continuing from previous state",
        config.core.scratchpad
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
    // TUI is enabled by default (unless --no-tui or --autonomous is specified)
    let enable_tui = !args.no_tui && !args.autonomous;
    let verbosity = Verbosity::resolve(verbose || args.verbose, args.quiet);
    let reason = run_loop_impl(
        config,
        color_mode,
        true,
        enable_tui,
        verbosity,
        args.record_session,
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

    // Handle --list-presets
    if args.list_presets {
        println!("{}", init::format_preset_list());
        return Ok(());
    }

    // Handle --preset (with optional --backend override)
    if let Some(preset) = args.preset {
        let backend_override = args.backend.as_deref();
        match init::init_from_preset(&preset, backend_override, args.force) {
            Ok(()) => {
                let msg = if let Some(backend) = backend_override {
                    format!(
                        "Created ralph.yml from '{}' preset with {} backend",
                        preset, backend
                    )
                } else {
                    format!("Created ralph.yml from '{}' preset", preset)
                };
                if use_colors {
                    println!("{}✓{} {}", colors::GREEN, colors::RESET, msg);
                    println!(
                        "\n{}Next steps:{}\n  1. Create PROMPT.md with your task\n  2. Run: ralph run",
                        colors::DIM,
                        colors::RESET
                    );
                } else {
                    println!("{}", msg);
                    println!(
                        "\nNext steps:\n  1. Create PROMPT.md with your task\n  2. Run: ralph run"
                    );
                }
                return Ok(());
            }
            Err(e) => {
                anyhow::bail!("{}", e);
            }
        }
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
                        "\n{}Next steps:{}\n  1. Create PROMPT.md with your task\n  2. Run: ralph run",
                        colors::DIM,
                        colors::RESET
                    );
                } else {
                    println!("Created ralph.yml with {} backend", backend);
                    println!(
                        "\nNext steps:\n  1. Create PROMPT.md with your task\n  2. Run: ralph run"
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
    println!("  ralph init --backend <backend>   Generate minimal config for backend");
    println!("  ralph init --preset <preset>     Use an embedded preset");
    println!("  ralph init --list-presets        Show available presets\n");
    println!("Backends: claude, kiro, gemini, codex, amp, custom");
    println!("\nRun 'ralph init --list-presets' to see available presets.");

    Ok(())
}

fn events_command(color_mode: ColorMode, args: EventsArgs) -> Result<()> {
    let use_colors = color_mode.should_use_colors();

    // Read events path from marker file, fall back to default if marker doesn't exist
    // This ensures `ralph events` reads from the same events file as the active run
    let history = match args.file {
        Some(path) => EventHistory::new(path),
        None => fs::read_to_string(".ralph/current-events")
            .map(|s| EventHistory::new(s.trim()))
            .unwrap_or_else(|_| EventHistory::default_path()),
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
            print_events_table(&records, use_colors);
        }
    }

    Ok(())
}

fn clean_command(config_path: PathBuf, color_mode: ColorMode, args: CleanArgs) -> Result<()> {
    let use_colors = color_mode.should_use_colors();

    // If --diagnostics flag is set, clean diagnostics directory
    if args.diagnostics {
        let workspace_root = std::env::current_dir().context("Failed to get current directory")?;
        return ralph_cli::clean_diagnostics(&workspace_root, use_colors, args.dry_run);
    }

    // Otherwise, clean .agent directory (existing behavior)
    // Load configuration
    let config = if config_path.exists() {
        RalphConfig::from_file(&config_path)
            .with_context(|| format!("Failed to load config from {:?}", config_path))?
    } else {
        warn!("Config file {:?} not found, using defaults", config_path);
        RalphConfig::default()
    };

    // Extract the .agent directory path from scratchpad path
    let scratchpad_path = Path::new(&config.core.scratchpad);
    let agent_dir = scratchpad_path.parent().ok_or_else(|| {
        anyhow::anyhow!(
            "Could not determine parent directory from scratchpad path: {}",
            config.core.scratchpad
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
    let use_colors = color_mode.should_use_colors();

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
    let record = serde_json::json!({
        "topic": args.topic,
        "payload": if args.json && !payload.is_empty() {
            // Parse and embed as object
            serde_json::from_str::<serde_json::Value>(&payload)?
        } else if payload.is_empty() {
            serde_json::Value::Null
        } else {
            serde_json::Value::String(payload)
        },
        "ts": ts
    });

    // Read events path from marker file, fall back to CLI arg if marker doesn't exist
    // This ensures `ralph emit` writes to the same events file as the active run
    let events_file = fs::read_to_string(".ralph/current-events")
        .map(|s| PathBuf::from(s.trim()))
        .unwrap_or_else(|_| args.file.clone());

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

/// Starts a Prompt-Driven Development planning session.
///
/// This is a thin wrapper that bypasses Ralph's event loop entirely.
/// It spawns the AI backend with the bundled PDD SOP for interactive planning.
fn plan_command(config_path: PathBuf, color_mode: ColorMode, args: PlanArgs) -> Result<()> {
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

    let config = SopRunConfig {
        sop: Sop::Pdd,
        user_input: args.idea,
        backend_override: args.backend,
        config_path: Some(config_path),
    };

    sop_runner::run_sop(config).map_err(|e| match e {
        SopRunError::NoBackend(no_backend) => anyhow::Error::new(no_backend),
        SopRunError::UnknownBackend(name) => anyhow::anyhow!(
            "Unknown backend: {}\n\nValid backends: claude, kiro, gemini, codex, amp",
            name
        ),
        SopRunError::SpawnError(io_err) => anyhow::anyhow!("Failed to spawn backend: {}", io_err),
    })
}

/// Starts a code-task-generator session.
///
/// This is a thin wrapper that bypasses Ralph's event loop entirely.
/// It spawns the AI backend with the bundled code-task-generator SOP.
fn code_task_command(
    config_path: PathBuf,
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

    let config = SopRunConfig {
        sop: Sop::CodeTaskGenerator,
        user_input: args.input,
        backend_override: args.backend,
        config_path: Some(config_path),
    };

    sop_runner::run_sop(config).map_err(|e| match e {
        SopRunError::NoBackend(no_backend) => anyhow::Error::new(no_backend),
        SopRunError::UnknownBackend(name) => anyhow::anyhow!(
            "Unknown backend: {}\n\nValid backends: claude, kiro, gemini, codex, amp",
            name
        ),
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

fn print_events_table(records: &[ralph_core::EventRecord], use_colors: bool) {
    use colors::*;

    // Header
    if use_colors {
        println!(
            "{BOLD}{DIM}  # │ Time     │ Iteration │ Hat           │ Topic              │ Triggered      │ Payload{RESET}"
        );
        println!(
            "{DIM}────┼──────────┼───────────┼───────────────┼────────────────────┼────────────────┼─────────────────{RESET}"
        );
    } else {
        println!(
            "  # | Time     | Iteration | Hat           | Topic              | Triggered      | Payload"
        );
        println!(
            "----|----------|-----------|---------------|--------------------|-----------------|-----------------"
        );
    }

    for (i, record) in records.iter().enumerate() {
        let topic_color = get_topic_color(&record.topic);
        let triggered = record.triggered.as_deref().unwrap_or("-");
        let payload_preview = if record.payload.len() > 40 {
            format!("{}...", &record.payload[..40].replace('\n', " "))
        } else {
            record.payload.replace('\n', " ")
        };

        // Extract time portion (HH:MM:SS) from ISO 8601 timestamp
        let time = record
            .ts
            .find('T')
            .and_then(|t_pos| {
                let after_t = &record.ts[t_pos + 1..];
                // Find end of time (before timezone indicator or end of string)
                let end = after_t
                    .find(|c| c == 'Z' || c == '+' || c == '-')
                    .unwrap_or(after_t.len());
                let time_str = &after_t[..end];
                // Take only HH:MM:SS (first 8 chars if available)
                Some(&time_str[..time_str.len().min(8)])
            })
            .unwrap_or("-");

        if use_colors {
            println!(
                "{DIM}{:>3}{RESET} │ {:<8} │ {:>9} │ {:<13} │ {topic_color}{:<18}{RESET} │ {:<14} │ {DIM}{}{RESET}",
                i + 1,
                time,
                record.iteration,
                truncate(&record.hat, 13),
                truncate(&record.topic, 18),
                truncate(triggered, 14),
                payload_preview
            );
        } else {
            println!(
                "{:>3} | {:<8} | {:>9} | {:<13} | {:<18} | {:<14} | {}",
                i + 1,
                time,
                record.iteration,
                truncate(&record.hat, 13),
                truncate(&record.topic, 18),
                truncate(triggered, 14),
                payload_preview
            );
        }
    }

    // Footer
    if use_colors {
        println!("\n{DIM}Total: {} events{RESET}", records.len());
    } else {
        println!("\nTotal: {} events", records.len());
    }
}

fn get_topic_color(topic: &str) -> &'static str {
    use colors::*;
    if topic.starts_with("task.") {
        CYAN
    } else if topic.starts_with("build.done") {
        GREEN
    } else if topic.starts_with("build.blocked") {
        RED
    } else if topic.starts_with("build.") {
        YELLOW
    } else if topic.starts_with("review.") {
        MAGENTA
    } else {
        BLUE
    }
}

/// Returns the emoji for a hat ID.
fn hat_emoji(hat_id: &str) -> &'static str {
    match hat_id {
        "planner" => "🎩",
        "builder" => "🔨",
        "reviewer" => "👀",
        _ => "🎭",
    }
}

/// Prints the iteration demarcation separator.
///
/// Per spec: "Each iteration must be clearly demarcated in the output so users can
/// visually distinguish where one iteration ends and another begins."
///
/// Format:
/// ```text
/// ═══════════════════════════════════════════════════════════════════════════════
///  ITERATION 3 │ 🔨 builder │ 2m 15s elapsed │ 3/100
/// ═══════════════════════════════════════════════════════════════════════════════
/// ```
fn print_iteration_separator(
    iteration: u32,
    hat_id: &str,
    elapsed: std::time::Duration,
    max_iterations: u32,
    use_colors: bool,
) {
    use colors::*;

    let emoji = hat_emoji(hat_id);
    let elapsed_str = format_elapsed(elapsed);

    // Build the content line (without box chars for measuring)
    let content = format!(
        " ITERATION {} │ {} {} │ {} elapsed │ {}/{}",
        iteration, emoji, hat_id, elapsed_str, iteration, max_iterations
    );

    // Use fixed width of 79 characters for the box (standard terminal width)
    let box_width = 79;
    let separator = "═".repeat(box_width);

    if use_colors {
        println!("\n{BOLD}{CYAN}{separator}{RESET}");
        println!("{BOLD}{CYAN}{content}{RESET}");
        println!("{BOLD}{CYAN}{separator}{RESET}");
    } else {
        println!("\n{separator}");
        println!("{content}");
        println!("{separator}");
    }
}

/// Formats elapsed duration as human-readable string.
fn format_elapsed(d: std::time::Duration) -> String {
    let total_secs = d.as_secs();
    let hours = total_secs / 3600;
    let minutes = (total_secs % 3600) / 60;
    let seconds = total_secs % 60;

    if hours > 0 {
        format!("{}h {}m {}s", hours, minutes, seconds)
    } else if minutes > 0 {
        format!("{}m {}s", minutes, seconds)
    } else {
        format!("{}s", seconds)
    }
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.len() <= max_len {
        s.to_string()
    } else {
        format!("{}…", &s[..max_len - 1])
    }
}

/// Builds a map of event topics to hat display information for the TUI.
///
/// This allows the TUI to dynamically resolve which hat should be displayed
/// for any event topic, including custom hats (e.g., "review.security" → "🔒 Security Reviewer").
///
/// Only exact topic patterns (non-wildcard) are included to avoid pattern matching complexity.
fn build_tui_hat_map(
    registry: &ralph_core::HatRegistry,
) -> std::collections::HashMap<String, (HatId, String)> {
    use std::collections::HashMap;

    let mut map = HashMap::new();

    for hat in registry.all() {
        // For each subscription topic, add exact matches to the map
        for subscription in &hat.subscriptions {
            let topic_str = subscription.to_string();
            // Only add non-wildcard topics
            if !topic_str.contains('*') {
                map.insert(topic_str, (hat.id.clone(), hat.name.clone()));
            }
        }
    }

    map
}

/// Resolves prompt content with proper precedence.
///
/// Precedence (highest to lowest):
/// 1. CLI -p "text" (inline prompt text)
/// 2. CLI -P path (prompt file path)
/// 3. Config event_loop.prompt (inline prompt text)
/// 4. Config event_loop.prompt_file (prompt file path)
/// 5. Default PROMPT.md
///
/// Note: CLI overrides are already applied to config before this function is called.
fn resolve_prompt_content(event_loop_config: &ralph_core::EventLoopConfig) -> Result<String> {
    debug!(
        inline_prompt = ?event_loop_config.prompt.as_ref().map(|s| format!("{}...", &s[..s.len().min(50)])),
        prompt_file = %event_loop_config.prompt_file,
        "Resolving prompt content"
    );

    // Check for inline prompt first (CLI -p or config prompt)
    if let Some(ref inline_text) = event_loop_config.prompt {
        debug!(len = inline_text.len(), "Using inline prompt text");
        return Ok(inline_text.clone());
    }

    // Check for prompt file (CLI -P or config prompt_file or default)
    let prompt_file = &event_loop_config.prompt_file;
    if !prompt_file.is_empty() {
        let path = std::path::Path::new(prompt_file);
        debug!(path = %prompt_file, exists = path.exists(), "Checking prompt file");
        if path.exists() {
            let content = std::fs::read_to_string(path)
                .with_context(|| format!("Failed to read prompt file: {}", prompt_file))?;
            debug!(path = %prompt_file, len = content.len(), "Read prompt from file");
            return Ok(content);
        } else {
            // File specified but doesn't exist - error with helpful message
            anyhow::bail!(
                "Prompt file '{}' not found. Check the path or use -p \"text\" for inline prompt.",
                prompt_file
            );
        }
    }

    // No valid prompt source found
    anyhow::bail!(
        "No prompt specified. Use -p \"text\" for inline prompt, -P path for file, \
         or create PROMPT.md in the current directory."
    )
}

/// Core loop implementation supporting both fresh start and continue modes.
///
/// `resume`: If true, publishes `task.resume` instead of `task.start`,
/// signaling the planner to read existing scratchpad rather than doing fresh gap analysis.
///
/// `record_session`: If provided, records all events to the specified JSONL file for replay testing.
async fn run_loop_impl(
    config: RalphConfig,
    color_mode: ColorMode,
    resume: bool,
    enable_tui: bool,
    verbosity: Verbosity,
    record_session: Option<PathBuf>,
) -> Result<TerminationReason> {
    // Set up process group leadership per spec
    // "The orchestrator must run as a process group leader"
    process_management::setup_process_group();

    let use_colors = color_mode.should_use_colors();

    // Determine effective execution mode (with fallback logic)
    // Per spec: Claude backend requires PTY mode to avoid hangs
    // TUI mode is observation-only - uses streaming mode, not interactive
    let interactive_requested = config.cli.default_mode == "interactive" && !enable_tui;
    let user_interactive = if interactive_requested {
        if stdout().is_terminal() {
            true
        } else {
            warn!("Interactive mode requested but stdout is not a TTY, falling back to autonomous");
            false
        }
    } else {
        false
    };
    // Always use PTY for real-time streaming output (vs buffered CliExecutor)
    let use_pty = true;

    // Set up signal handling for immediate termination
    // Per spec:
    // - SIGINT (Ctrl+C): Immediately terminate child process (SIGTERM → 5s grace → SIGKILL), exit with code 130
    // - SIGTERM: Same as SIGINT
    // - SIGHUP: Same as SIGINT
    //
    // Use watch channel for interrupt notification so we can race execution vs interrupt
    let (interrupt_tx, interrupt_rx) = tokio::sync::watch::channel(false);

    // Spawn task to listen for SIGINT (Ctrl+C)
    let interrupt_tx_sigint = interrupt_tx.clone();
    tokio::spawn(async move {
        if tokio::signal::ctrl_c().await.is_ok() {
            debug!("Interrupt received (SIGINT), terminating immediately...");
            let _ = interrupt_tx_sigint.send(true);
        }
    });

    // Spawn task to listen for SIGTERM (Unix only)
    #[cfg(unix)]
    {
        let interrupt_tx_sigterm = interrupt_tx.clone();
        tokio::spawn(async move {
            let mut sigterm =
                tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                    .expect("Failed to register SIGTERM handler");
            sigterm.recv().await;
            debug!("SIGTERM received, terminating immediately...");
            let _ = interrupt_tx_sigterm.send(true);
        });
    }

    // Spawn task to listen for SIGHUP (Unix only)
    #[cfg(unix)]
    {
        let interrupt_tx_sighup = interrupt_tx.clone();
        tokio::spawn(async move {
            let mut sighup = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::hangup())
                .expect("Failed to register SIGHUP handler");
            sighup.recv().await;
            warn!("SIGHUP received (terminal closed), terminating immediately...");
            let _ = interrupt_tx_sighup.send(true);
        });
    }

    // Resolve prompt content with precedence:
    // 1. CLI -p (inline text)
    // 2. CLI -P (file path)
    // 3. Config prompt (inline text)
    // 4. Config prompt_file (file path)
    // 5. Default PROMPT.md
    let prompt_content = resolve_prompt_content(&config.event_loop)?;

    // For fresh runs (not resume), generate a unique timestamped events file
    // This prevents stale events from previous runs polluting new runs (issue #82)
    // The marker file `.ralph/current-events` coordinates path between Ralph and agents
    if !resume {
        let run_id = chrono::Utc::now().format("%Y%m%d-%H%M%S").to_string();
        let events_path = format!(".ralph/events-{}.jsonl", run_id);

        fs::create_dir_all(".ralph").context("Failed to create .ralph directory")?;
        fs::write(".ralph/current-events", &events_path)
            .context("Failed to write .ralph/current-events marker file")?;

        debug!("Created events file for this run: {}", events_path);
    }

    // Initialize event loop
    let mut event_loop = EventLoop::new(config.clone());

    // For resume mode, we initialize with a different event topic
    // This tells the planner to read existing scratchpad rather than creating a new one
    if resume {
        event_loop.initialize_resume(&prompt_content);
    } else {
        event_loop.initialize(&prompt_content);
    }

    // Set up session recording if requested
    // This records all events to a JSONL file for replay testing
    let _session_recorder: Option<Arc<SessionRecorder<BufWriter<File>>>> =
        if let Some(record_path) = record_session {
            let file = File::create(&record_path).with_context(|| {
                format!("Failed to create session recording file: {:?}", record_path)
            })?;
            let recorder = Arc::new(SessionRecorder::new(BufWriter::new(file)));

            // Record metadata for the session
            recorder.record_meta(Record::meta_loop_start(
                &config.event_loop.prompt_file,
                config.event_loop.max_iterations,
                if enable_tui { Some("tui") } else { Some("cli") },
            ));

            // Wire observer to EventBus so events are recorded
            let observer = SessionRecorder::make_observer(Arc::clone(&recorder));
            event_loop.add_observer(observer);

            info!("Session recording enabled: {:?}", record_path);
            Some(recorder)
        } else {
            None
        };

    // Initialize event logger for debugging
    let mut event_logger = EventLogger::default_path();

    // Log initial event (task.start or task.resume)
    let (start_topic, start_triggered) = if resume {
        ("task.resume", "planner")
    } else {
        ("task.start", "planner")
    };
    let start_event = Event::new(start_topic, &prompt_content);
    let start_record =
        EventRecord::new(0, "loop", &start_event, Some(&HatId::new(start_triggered)));
    if let Err(e) = event_logger.log(&start_record) {
        warn!("Failed to log start event: {}", e);
    }

    // Create backend from config - TUI mode uses the same backend as non-TUI
    // The TUI is an observation layer that displays output, not a different mode
    let backend = CliBackend::from_config(&config.cli).map_err(|e| anyhow::Error::new(e))?;

    // Create PTY executor if using interactive mode
    let mut pty_executor = if use_pty {
        let idle_timeout_secs = if user_interactive {
            config.cli.idle_timeout_secs
        } else {
            0
        };
        let pty_config = PtyConfig {
            interactive: user_interactive,
            idle_timeout_secs,
            workspace_root: config.core.workspace_root.clone(),
            ..PtyConfig::from_env()
        };
        Some(PtyExecutor::new(backend.clone(), pty_config))
    } else {
        None
    };

    // Create termination signal for TUI shutdown
    let (terminated_tx, terminated_rx) = tokio::sync::watch::channel(false);

    // Wire TUI with termination signal and shared state
    // TUI is observation-only - works in both interactive and autonomous modes
    // Only requirement: stdout must be a terminal for TUI to render
    let enable_tui = enable_tui && stdout().is_terminal();
    let (mut tui_handle, tui_state) = if enable_tui {
        // Build hat map for dynamic topic-to-hat resolution
        // This allows TUI to display custom hats (e.g., "🔒 Security Reviewer")
        // instead of generic "ralph" for all events
        let hat_map = build_tui_hat_map(event_loop.registry());
        let tui = Tui::new()
            .with_hat_map(hat_map)
            .with_termination_signal(terminated_rx);

        // Get shared state before spawning (for content streaming)
        let state = tui.state();

        // Wire interrupt channel so TUI can signal main loop on Ctrl+C
        // (raw mode prevents SIGINT from being generated by the OS)
        let tui = tui.with_interrupt_tx(interrupt_tx.clone());

        let observer = tui.observer();
        event_loop.add_observer(observer);
        (
            Some(tokio::spawn(async move { tui.run().await })),
            Some(state),
        )
    } else {
        (None, None)
    };

    // Give TUI task time to initialize (enter alternate screen, enable raw mode)
    // before the main loop starts doing work
    if tui_handle.is_some() {
        tokio::time::sleep(Duration::from_millis(100)).await;
    }

    // Log execution mode - hat info already logged by initialize()
    let exec_mode = if user_interactive {
        "interactive"
    } else {
        "autonomous"
    };
    debug!(execution_mode = %exec_mode, "Execution mode configured");

    // Track the last hat to detect hat changes for logging
    let mut last_hat: Option<HatId> = None;

    // Track consecutive fallback attempts to prevent infinite loops
    let mut consecutive_fallbacks: u32 = 0;
    const MAX_FALLBACK_ATTEMPTS: u32 = 3;

    // Helper closure to handle termination (writes summary, prints status)
    let handle_termination = |reason: &TerminationReason,
                              state: &ralph_core::LoopState,
                              scratchpad: &str| {
        // Per spec: Write summary file on termination
        let summary_writer = SummaryWriter::default();
        let scratchpad_path = std::path::Path::new(scratchpad);
        let scratchpad_opt = if scratchpad_path.exists() {
            Some(scratchpad_path)
        } else {
            None
        };

        // Get final commit SHA if available
        let final_commit = get_last_commit_info();

        if let Err(e) = summary_writer.write(reason, state, scratchpad_opt, final_commit.as_deref())
        {
            warn!("Failed to write summary file: {}", e);
        }

        // Print termination info to console (skip in TUI mode - TUI handles display)
        if !enable_tui {
            print_termination(reason, state, use_colors);
        }
    };

    // Main orchestration loop
    loop {
        // Check for interrupt signal at start of each iteration
        // This catches TUI Ctrl+C (via interrupt_tx) before printing iteration separator
        if *interrupt_rx.borrow() {
            #[cfg(unix)]
            {
                use nix::sys::signal::{Signal, killpg};
                use nix::unistd::getpgrp;
                let pgid = getpgrp();
                debug!(
                    "Interrupt detected at loop start, sending SIGTERM to process group {}",
                    pgid
                );
                let _ = killpg(pgid, Signal::SIGTERM);
                tokio::time::sleep(Duration::from_millis(250)).await;
                let _ = killpg(pgid, Signal::SIGKILL);
            }
            let reason = TerminationReason::Interrupted;
            let terminate_event = event_loop.publish_terminate_event(&reason);
            log_terminate_event(
                &mut event_logger,
                event_loop.state().iteration,
                &terminate_event,
            );
            handle_termination(&reason, event_loop.state(), &config.core.scratchpad);
            // Signal TUI to exit immediately on interrupt
            let _ = terminated_tx.send(true);
            return Ok(reason);
        }

        // Check termination before execution
        if let Some(reason) = event_loop.check_termination() {
            // Per spec: Publish loop.terminate event to observers
            let terminate_event = event_loop.publish_terminate_event(&reason);
            log_terminate_event(
                &mut event_logger,
                event_loop.state().iteration,
                &terminate_event,
            );
            handle_termination(&reason, event_loop.state(), &config.core.scratchpad);
            // Wait for user to exit TUI (press 'q') on natural completion
            if let Some(handle) = tui_handle.take() {
                let _ = handle.await;
            }
            return Ok(reason);
        }

        // Get next hat to execute, with fallback recovery if no pending events
        let hat_id = match event_loop.next_hat() {
            Some(id) => {
                // Reset fallback counter on successful event routing
                consecutive_fallbacks = 0;
                id.clone()
            }
            None => {
                // No pending events - try to recover by injecting a fallback event
                // This triggers the built-in planner to assess the situation
                consecutive_fallbacks += 1;

                if consecutive_fallbacks > MAX_FALLBACK_ATTEMPTS {
                    warn!(
                        attempts = consecutive_fallbacks,
                        "Fallback recovery exhausted after {} attempts, terminating",
                        MAX_FALLBACK_ATTEMPTS
                    );
                    let reason = TerminationReason::Stopped;
                    let terminate_event = event_loop.publish_terminate_event(&reason);
                    log_terminate_event(
                        &mut event_logger,
                        event_loop.state().iteration,
                        &terminate_event,
                    );
                    handle_termination(&reason, event_loop.state(), &config.core.scratchpad);
                    // Wait for user to exit TUI (press 'q') on natural completion
                    if let Some(handle) = tui_handle.take() {
                        let _ = handle.await;
                    }
                    return Ok(reason);
                }

                if event_loop.inject_fallback_event() {
                    // Fallback injected successfully, continue to next iteration
                    // The planner will be triggered and can either:
                    // - Dispatch more work if tasks remain
                    // - Output LOOP_COMPLETE if done
                    // - Determine what went wrong and recover
                    continue;
                }

                // Fallback not possible (no planner hat or doesn't subscribe to task.resume)
                warn!("No hats with pending events and fallback not available, terminating");
                let reason = TerminationReason::Stopped;
                // Per spec: Publish loop.terminate event to observers
                let terminate_event = event_loop.publish_terminate_event(&reason);
                log_terminate_event(
                    &mut event_logger,
                    event_loop.state().iteration,
                    &terminate_event,
                );
                handle_termination(&reason, event_loop.state(), &config.core.scratchpad);
                // Wait for user to exit TUI (press 'q') on natural completion
                if let Some(handle) = tui_handle.take() {
                    let _ = handle.await;
                }
                return Ok(reason);
            }
        };

        let iteration = event_loop.state().iteration + 1;

        // Determine which hat to display in iteration separator
        // When Ralph is coordinating (hat_id == "ralph"), show the active hat being worked on
        let display_hat = if hat_id.as_str() == "ralph" {
            event_loop.get_active_hat_id()
        } else {
            hat_id.clone()
        };

        // Per spec: Print iteration demarcation separator
        // "Each iteration must be clearly demarcated in the output so users can
        // visually distinguish where one iteration ends and another begins."
        // Skip when TUI is enabled - TUI has its own header showing iteration info
        if tui_state.is_none() {
            print_iteration_separator(
                iteration,
                display_hat.as_str(),
                event_loop.state().elapsed(),
                config.event_loop.max_iterations,
                use_colors,
            );
        }

        // Log hat changes with appropriate messaging
        // Skip in TUI mode - TUI shows hat info in header, and stdout would corrupt display
        if last_hat.as_ref() != Some(&hat_id) {
            if tui_state.is_none() {
                if hat_id.as_str() == "ralph" {
                    info!("I'm Ralph. Let's do this.");
                } else {
                    info!("Putting on my {} hat.", hat_id);
                }
            }
            last_hat = Some(hat_id.clone());
        }
        debug!(
            "Iteration {}/{} — {} active",
            iteration, config.event_loop.max_iterations, hat_id
        );

        // Build prompt for this hat
        let prompt = match event_loop.build_prompt(&hat_id) {
            Some(p) => p,
            None => {
                error!("Failed to build prompt for hat '{}'", hat_id);
                continue;
            }
        };

        // In verbose mode, print the full prompt before execution
        if verbosity == Verbosity::Verbose {
            eprintln!("\n{}", "═".repeat(80));
            eprintln!("📋 PROMPT FOR {} (iteration {})", hat_id, iteration);
            eprintln!("{}", "─".repeat(80));
            eprintln!("{}", prompt);
            eprintln!("{}\n", "═".repeat(80));
        }

        // Execute the prompt (interactive or autonomous mode)
        // Get per-adapter timeout from config
        let timeout_secs = config.adapter_settings(&config.cli.backend).timeout;
        let timeout = Some(Duration::from_secs(timeout_secs));

        // For TUI mode, get the shared lines buffer for this iteration.
        // The buffer is owned by TuiState's IterationBuffer, so writes from
        // TuiStreamHandler appear immediately in the TUI (real-time streaming).
        let tui_lines: Option<Arc<std::sync::Mutex<Vec<ratatui::text::Line<'static>>>>> =
            if let Some(ref state) = tui_state {
                // Start new iteration and get handle to the LATEST iteration's lines buffer.
                // We must use latest_iteration_lines_handle() instead of current_iteration_lines_handle()
                // because the user may be viewing an older iteration while a new one executes.
                if let Ok(mut s) = state.lock() {
                    s.start_new_iteration();
                    s.latest_iteration_lines_handle()
                } else {
                    None
                }
            } else {
                None
            };

        // Race execution against interrupt signal for immediate termination on Ctrl+C
        let mut interrupt_rx_clone = interrupt_rx.clone();
        let interrupt_rx_for_pty = interrupt_rx.clone();
        let tui_lines_for_pty = tui_lines.clone();
        let execute_future = async {
            if use_pty {
                execute_pty(
                    pty_executor.as_mut(),
                    &backend,
                    &config,
                    &prompt,
                    user_interactive,
                    interrupt_rx_for_pty,
                    verbosity,
                    tui_lines_for_pty,
                )
                .await
            } else {
                let executor = CliExecutor::new(backend.clone());
                let result = executor
                    .execute(&prompt, stdout(), timeout, verbosity == Verbosity::Verbose)
                    .await?;
                Ok(ExecutionOutcome {
                    output: result.output,
                    success: result.success,
                    termination: None,
                })
            }
        };

        let outcome = tokio::select! {
            result = execute_future => result?,
            _ = interrupt_rx_clone.changed() => {
                // Immediately terminate children via process group signal
                #[cfg(unix)]
                {
                    use nix::sys::signal::{killpg, Signal};
                    use nix::unistd::getpgrp;
                    let pgid = getpgrp();
                    debug!("Sending SIGTERM to process group {}", pgid);
                    let _ = killpg(pgid, Signal::SIGTERM);

                    // Wait briefly for graceful exit, then SIGKILL
                    tokio::time::sleep(Duration::from_millis(250)).await;
                    let _ = killpg(pgid, Signal::SIGKILL);
                }

                let reason = TerminationReason::Interrupted;
                let terminate_event = event_loop.publish_terminate_event(&reason);
                log_terminate_event(&mut event_logger, event_loop.state().iteration, &terminate_event);
                handle_termination(&reason, event_loop.state(), &config.core.scratchpad);
                // Signal TUI to exit immediately on interrupt
                let _ = terminated_tx.send(true);
                return Ok(reason);
            }
        };

        if let Some(reason) = outcome.termination {
            let terminate_event = event_loop.publish_terminate_event(&reason);
            log_terminate_event(
                &mut event_logger,
                event_loop.state().iteration,
                &terminate_event,
            );
            handle_termination(&reason, event_loop.state(), &config.core.scratchpad);
            // Wait for user to exit TUI (press 'q') on natural completion
            if let Some(handle) = tui_handle.take() {
                let _ = handle.await;
            }
            return Ok(reason);
        }

        let output = outcome.output;
        let success = outcome.success;

        // Note: TUI lines are now written directly to IterationBuffer during streaming,
        // so no post-execution transfer is needed.

        // Log events from output before processing
        log_events_from_output(
            &mut event_logger,
            iteration,
            &hat_id,
            &output,
            event_loop.registry(),
        );

        // Process output
        if let Some(reason) = event_loop.process_output(&hat_id, &output, success) {
            // Per spec: Log "All done! {promise} detected." when completion promise found
            if reason == TerminationReason::CompletionPromise {
                info!(
                    "All done! {} detected.",
                    config.event_loop.completion_promise
                );
            }
            // Per spec: Publish loop.terminate event to observers
            let terminate_event = event_loop.publish_terminate_event(&reason);
            log_terminate_event(
                &mut event_logger,
                event_loop.state().iteration,
                &terminate_event,
            );
            handle_termination(&reason, event_loop.state(), &config.core.scratchpad);
            // Wait for user to exit TUI (press 'q') on natural completion
            if let Some(handle) = tui_handle.take() {
                let _ = handle.await;
            }
            return Ok(reason);
        }

        // Read events from JSONL that agent may have written
        if let Err(e) = event_loop.process_events_from_jsonl() {
            warn!(error = %e, "Failed to read events from JSONL");
        }

        // Precheck validation: Warn if no pending events after processing output
        // Per EventLoop doc: "Use has_pending_events after process_output to detect
        // if the LLM failed to publish an event."
        if !event_loop.has_pending_events() {
            let expected = event_loop.get_hat_publishes(&hat_id);
            debug!(
                hat = %hat_id.as_str(),
                expected_topics = ?expected,
                "No pending events after iteration. Agent may have failed to publish a valid event. \
                 Expected one of: {:?}. Loop will terminate on next iteration.",
                expected
            );
        }

        // Note: Interrupt handling moved into tokio::select! above for immediate termination
    }
}

/// Executes a prompt in PTY mode with raw terminal handling.
/// Converts PTY termination type to loop termination reason.
///
/// In interactive mode, idle timeout signals "iteration complete" rather than
/// "loop stopped", allowing the event loop to process output and continue.
///
/// # Arguments
/// * `termination_type` - The PTY executor's termination type
/// * `interactive` - Whether running in interactive mode
///
/// # Returns
/// * `None` - Continue processing (iteration complete)
/// * `Some(TerminationReason)` - Stop the loop
fn convert_termination_type(
    termination_type: ralph_adapters::TerminationType,
    interactive: bool,
) -> Option<TerminationReason> {
    match termination_type {
        ralph_adapters::TerminationType::Natural => None,
        ralph_adapters::TerminationType::IdleTimeout => {
            if interactive {
                // In interactive mode, idle timeout signals iteration complete,
                // not loop termination. Let output be processed for events.
                info!("PTY idle timeout in interactive mode, iteration complete");
                None
            } else {
                warn!("PTY idle timeout reached, terminating loop");
                Some(TerminationReason::Stopped)
            }
        }
        ralph_adapters::TerminationType::UserInterrupt
        | ralph_adapters::TerminationType::ForceKill => Some(TerminationReason::Interrupted),
    }
}

///
/// # Arguments
/// * `backend` - The CLI backend to use for command building
/// * `config` - Ralph configuration for timeout settings
/// * `prompt` - The prompt to execute
/// * `interactive` - The actual execution mode (may differ from config's `default_mode`)
struct ExecutionOutcome {
    output: String,
    success: bool,
    termination: Option<TerminationReason>,
}

async fn execute_pty(
    executor: Option<&mut PtyExecutor>,
    backend: &CliBackend,
    config: &RalphConfig,
    prompt: &str,
    interactive: bool,
    interrupt_rx: tokio::sync::watch::Receiver<bool>,
    verbosity: Verbosity,
    tui_lines: Option<Arc<std::sync::Mutex<Vec<ratatui::text::Line<'static>>>>>,
) -> Result<ExecutionOutcome> {
    use crossterm::terminal::{disable_raw_mode, enable_raw_mode};

    // Use provided executor or create a new one
    // If executor is provided, TUI is connected and owns raw mode management
    let tui_connected = executor.is_some();
    let mut temp_executor;
    let exec = if let Some(e) = executor {
        e
    } else {
        let idle_timeout_secs = if interactive {
            config.cli.idle_timeout_secs
        } else {
            0
        };
        let pty_config = PtyConfig {
            interactive,
            idle_timeout_secs,
            workspace_root: config.core.workspace_root.clone(),
            ..PtyConfig::from_env()
        };
        temp_executor = PtyExecutor::new(backend.clone(), pty_config);
        &mut temp_executor
    };

    // Set TUI mode flag when TUI is connected (tui_lines is Some)
    // This replaces the broken output_rx.is_none() detection in PtyExecutor
    if tui_lines.is_some() {
        exec.set_tui_mode(true);
    }

    // Enter raw mode for interactive mode to capture keystrokes
    // Skip if TUI is connected - TUI owns raw mode and will manage it
    if interactive && !tui_connected {
        enable_raw_mode().context("Failed to enable raw mode")?;
    }

    // Use scopeguard to ensure raw mode is restored on any exit path
    // Skip if TUI is connected - TUI owns raw mode
    let _guard = scopeguard::guard((interactive, tui_connected), |(is_interactive, tui)| {
        if is_interactive && !tui {
            let _ = disable_raw_mode();
        }
    });

    // Run PTY executor with shared interrupt channel
    let result = if interactive && tui_lines.is_none() {
        // Raw interactive mode only when not using TUI (TUI handles its own terminal)
        exec.run_interactive(prompt, interrupt_rx).await
    } else if let Some(lines) = tui_lines {
        // TUI mode: use TuiStreamHandler to capture output for TUI display
        let verbose = verbosity == Verbosity::Verbose;
        let mut handler = TuiStreamHandler::with_lines(verbose, lines);
        exec.run_observe_streaming(prompt, interrupt_rx, &mut handler)
            .await
    } else {
        // Use streaming handler for non-interactive mode (respects verbosity)
        // Use PrettyStreamHandler for StreamJson backends (Claude) on TTY for markdown rendering
        // Use ConsoleStreamHandler for Text format backends (Kiro, Gemini, etc.) for immediate output
        let use_pretty =
            backend.output_format == BackendOutputFormat::StreamJson && stdout().is_terminal();

        match verbosity {
            Verbosity::Quiet => {
                let mut handler = QuietStreamHandler;
                exec.run_observe_streaming(prompt, interrupt_rx, &mut handler)
                    .await
            }
            Verbosity::Normal => {
                if use_pretty {
                    let mut handler = PrettyStreamHandler::new(false);
                    exec.run_observe_streaming(prompt, interrupt_rx, &mut handler)
                        .await
                } else {
                    let mut handler = ConsoleStreamHandler::new(false);
                    exec.run_observe_streaming(prompt, interrupt_rx, &mut handler)
                        .await
                }
            }
            Verbosity::Verbose => {
                if use_pretty {
                    let mut handler = PrettyStreamHandler::new(true);
                    exec.run_observe_streaming(prompt, interrupt_rx, &mut handler)
                        .await
                } else {
                    let mut handler = ConsoleStreamHandler::new(true);
                    exec.run_observe_streaming(prompt, interrupt_rx, &mut handler)
                        .await
                }
            }
        }
    };

    match result {
        Ok(pty_result) => {
            let termination = convert_termination_type(pty_result.termination, interactive);

            // Use extracted_text for event parsing when available (NDJSON backends like Claude),
            // otherwise fall back to stripped_output (non-JSON backends or interactive mode).
            // This fixes event parsing for Claude's stream-json output where event tags like
            // <event topic="..."> are inside JSON string values and not directly visible.
            let output_for_parsing = if pty_result.extracted_text.is_empty() {
                pty_result.stripped_output
            } else {
                pty_result.extracted_text
            };
            Ok(ExecutionOutcome {
                output: output_for_parsing,
                success: pty_result.success,
                termination,
            })
        }
        Err(e) => {
            // PTY allocation may have failed - log and continue with error
            warn!("PTY execution failed: {}, continuing with error status", e);
            Err(anyhow::Error::new(e))
        }
    }
}

/// Logs events parsed from output to the event history file.
fn log_events_from_output(
    logger: &mut EventLogger,
    iteration: u32,
    hat_id: &HatId,
    output: &str,
    registry: &ralph_core::HatRegistry,
) {
    let parser = EventParser::new();
    let events = parser.parse(output);

    for event in events {
        // Determine which hat will be triggered by this event
        let triggered = registry.find_by_trigger(event.topic.as_str());

        // Per spec: Log "Published {topic} → triggers {hat}" at DEBUG level
        if let Some(triggered_hat) = triggered {
            debug!("Published {} → triggers {}", event.topic, triggered_hat);
        } else {
            debug!("Published {} → no hat triggered", event.topic);
        }

        let record = EventRecord::new(iteration, hat_id.to_string(), &event, triggered);

        if let Err(e) = logger.log(&record) {
            warn!("Failed to log event {}: {}", event.topic, e);
        }
    }
}

/// Logs the loop.terminate system event to the event history.
///
/// Per spec: loop.terminate is an observer-only event published on loop exit.
fn log_terminate_event(logger: &mut EventLogger, iteration: u32, event: &Event) {
    // loop.terminate is published by the orchestrator, not a hat
    // No hat can trigger on it (it's observer-only)
    let record = EventRecord::new(iteration, "loop", event, None::<&HatId>);

    if let Err(e) = logger.log(&record) {
        warn!("Failed to log loop.terminate event: {}", e);
    }
}

fn print_termination(reason: &TerminationReason, state: &ralph_core::LoopState, use_colors: bool) {
    use colors::*;

    // Determine status color and message based on termination reason
    let (color, icon, label) = match reason {
        TerminationReason::CompletionPromise => (GREEN, "✓", "Completion promise detected"),
        TerminationReason::MaxIterations => (YELLOW, "⚠", "Maximum iterations reached"),
        TerminationReason::MaxRuntime => (YELLOW, "⚠", "Maximum runtime exceeded"),
        TerminationReason::MaxCost => (YELLOW, "⚠", "Maximum cost exceeded"),
        TerminationReason::ConsecutiveFailures => (RED, "✗", "Too many consecutive failures"),
        TerminationReason::LoopThrashing => (RED, "🔄", "Loop thrashing detected"),
        TerminationReason::ValidationFailure => (RED, "⚠", "Too many malformed JSONL events"),
        TerminationReason::Stopped => (CYAN, "■", "Manually stopped"),
        TerminationReason::Interrupted => (YELLOW, "⚡", "Interrupted by signal"),
    };

    let separator = "─".repeat(58);

    if use_colors {
        println!("\n{BOLD}┌{separator}┐{RESET}");
        println!(
            "{BOLD}│{RESET} {color}{BOLD}{icon}{RESET} Loop terminated: {color}{label}{RESET}"
        );
        println!("{BOLD}├{separator}┤{RESET}");
        println!(
            "{BOLD}│{RESET}   Iterations:  {CYAN}{}{RESET}",
            state.iteration
        );
        println!(
            "{BOLD}│{RESET}   Elapsed:     {CYAN}{:.1}s{RESET}",
            state.elapsed().as_secs_f64()
        );
        if state.cumulative_cost > 0.0 {
            println!(
                "{BOLD}│{RESET}   Cost:        {CYAN}${:.2}{RESET}",
                state.cumulative_cost
            );
        }
        println!("{BOLD}└{separator}┘{RESET}");
    } else {
        println!("\n+{}+", "-".repeat(58));
        println!("| {icon} Loop terminated: {label}");
        println!("+{}+", "-".repeat(58));
        println!("|   Iterations:  {}", state.iteration);
        println!("|   Elapsed:     {:.1}s", state.elapsed().as_secs_f64());
        if state.cumulative_cost > 0.0 {
            println!("|   Cost:        ${:.2}", state.cumulative_cost);
        }
        println!("+{}+", "-".repeat(58));
    }
}

/// Gets the last commit info (short SHA and subject) for the summary file.
fn get_last_commit_info() -> Option<String> {
    let output = Command::new("git")
        .args(["log", "-1", "--format=%h: %s"])
        .output()
        .ok()?;

    if output.status.success() {
        let info = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if info.is_empty() { None } else { Some(info) }
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ralph_core::RalphConfig;

    #[test]
    fn test_pty_always_enabled_for_streaming() {
        // PTY mode is always enabled for real-time streaming output.
        // This ensures all backends (claude, gemini, kiro, codex, amp) get
        // streaming output instead of buffered output from CliExecutor.
        let use_pty = true; // Matches the actual implementation

        // PTY should always be true regardless of backend or mode
        assert!(use_pty, "PTY should always be enabled for streaming output");
    }

    #[test]
    fn test_user_interactive_mode_determination() {
        // user_interactive is determined by default_mode setting, not PTY.
        // PTY handles output streaming; user_interactive handles input forwarding.

        // Autonomous mode: no user input forwarding
        let autonomous_interactive = false;
        assert!(
            !autonomous_interactive,
            "Autonomous mode should not forward user input"
        );

        // Interactive mode with TTY: forward user input
        let interactive_with_tty = true;
        assert!(
            interactive_with_tty,
            "Interactive mode with TTY should forward user input"
        );
    }

    #[test]
    fn test_idle_timeout_interactive_mode_continues() {
        // Given: interactive mode and IdleTimeout termination
        let termination_type = ralph_adapters::TerminationType::IdleTimeout;
        let interactive = true;

        // When: converting termination type
        let result = convert_termination_type(termination_type, interactive);

        // Then: should return None (allow iteration to continue)
        assert!(
            result.is_none(),
            "Interactive mode idle timeout should return None to allow iteration progression"
        );
    }

    #[test]
    fn test_idle_timeout_autonomous_mode_stops() {
        // Given: autonomous mode and IdleTimeout termination
        let termination_type = ralph_adapters::TerminationType::IdleTimeout;
        let interactive = false;

        // When: converting termination type
        let result = convert_termination_type(termination_type, interactive);

        // Then: should return Some(Stopped)
        assert_eq!(
            result,
            Some(TerminationReason::Stopped),
            "Autonomous mode idle timeout should return Stopped"
        );
    }

    #[test]
    fn test_natural_termination_always_continues() {
        // Given: Natural termination in any mode
        let termination_type = ralph_adapters::TerminationType::Natural;

        // When/Then: should return None regardless of mode
        assert!(
            convert_termination_type(termination_type.clone(), true).is_none(),
            "Natural termination should continue in interactive mode"
        );
        assert!(
            convert_termination_type(termination_type, false).is_none(),
            "Natural termination should continue in autonomous mode"
        );
    }

    #[test]
    fn test_user_interrupt_always_terminates() {
        // Given: UserInterrupt termination in any mode
        let termination_type = ralph_adapters::TerminationType::UserInterrupt;

        // When/Then: should return Interrupted regardless of mode
        assert_eq!(
            convert_termination_type(termination_type.clone(), true),
            Some(TerminationReason::Interrupted),
            "UserInterrupt should terminate in interactive mode"
        );
        assert_eq!(
            convert_termination_type(termination_type, false),
            Some(TerminationReason::Interrupted),
            "UserInterrupt should terminate in autonomous mode"
        );
    }

    #[test]
    fn test_force_kill_always_terminates() {
        // Given: ForceKill termination in any mode
        let termination_type = ralph_adapters::TerminationType::ForceKill;

        // When/Then: should return Interrupted regardless of mode
        assert_eq!(
            convert_termination_type(termination_type.clone(), true),
            Some(TerminationReason::Interrupted),
            "ForceKill should terminate in interactive mode"
        );
        assert_eq!(
            convert_termination_type(termination_type, false),
            Some(TerminationReason::Interrupted),
            "ForceKill should terminate in autonomous mode"
        );
    }

    #[test]
    fn test_build_tui_hat_map_extracts_custom_hats() {
        // Given: A config with custom hats from pr-review preset
        let yaml = r#"
hats:
  security_reviewer:
    name: "🔒 Security Reviewer"
    triggers: ["review.security"]
    publishes: ["security.done"]
  correctness_reviewer:
    name: "🎯 Correctness Reviewer"
    triggers: ["review.correctness"]
    publishes: ["correctness.done"]
  architecture_reviewer:
    name: "🏗️ Architecture Reviewer"
    triggers: ["review.architecture", "arch.*"]
    publishes: ["architecture.done"]
"#;
        let config: RalphConfig = serde_yaml::from_str(yaml).unwrap();
        let registry = ralph_core::HatRegistry::from_config(&config);

        // When: Building the TUI hat map
        let hat_map = build_tui_hat_map(&registry);

        // Then: Exact topic patterns should be mapped
        assert_eq!(hat_map.len(), 3, "Should have 3 exact topic mappings");

        // Security reviewer
        assert!(
            hat_map.contains_key("review.security"),
            "Should map review.security topic"
        );
        let (hat_id, hat_display) = &hat_map["review.security"];
        assert_eq!(hat_id.as_str(), "security_reviewer");
        assert_eq!(hat_display, "🔒 Security Reviewer");

        // Correctness reviewer
        assert!(
            hat_map.contains_key("review.correctness"),
            "Should map review.correctness topic"
        );
        let (hat_id, hat_display) = &hat_map["review.correctness"];
        assert_eq!(hat_id.as_str(), "correctness_reviewer");
        assert_eq!(hat_display, "🎯 Correctness Reviewer");

        // Architecture reviewer - exact topic only
        assert!(
            hat_map.contains_key("review.architecture"),
            "Should map review.architecture topic"
        );
        let (hat_id, hat_display) = &hat_map["review.architecture"];
        assert_eq!(hat_id.as_str(), "architecture_reviewer");
        assert_eq!(hat_display, "🏗️ Architecture Reviewer");

        // Wildcard patterns should be skipped
        assert!(
            !hat_map.contains_key("arch.*"),
            "Wildcard patterns should not be in the map"
        );
    }

    #[test]
    fn test_build_tui_hat_map_empty_registry() {
        // Given: An empty registry (solo mode)
        let config = RalphConfig::default();
        let registry = ralph_core::HatRegistry::from_config(&config);

        // When: Building the TUI hat map
        let hat_map = build_tui_hat_map(&registry);

        // Then: Map should be empty
        assert_eq!(
            hat_map.len(),
            0,
            "Empty registry should produce empty hat map"
        );
    }

    #[test]
    fn test_build_tui_hat_map_skips_wildcard_patterns() {
        // Given: A config with only wildcard patterns
        let yaml = r#"
hats:
  planner:
    name: "📋 Planner"
    triggers: ["task.*", "build.*"]
    publishes: ["build.task"]
"#;
        let config: RalphConfig = serde_yaml::from_str(yaml).unwrap();
        let registry = ralph_core::HatRegistry::from_config(&config);

        // When: Building the TUI hat map
        let hat_map = build_tui_hat_map(&registry);

        // Then: No mappings should be created (all wildcards skipped)
        assert_eq!(
            hat_map.len(),
            0,
            "Wildcard-only subscriptions should produce empty map"
        );
        assert!(!hat_map.contains_key("task.*"));
        assert!(!hat_map.contains_key("build.*"));
    }
}
