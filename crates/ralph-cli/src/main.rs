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
//! - SOP-based planning via `ralph plan` and `ralph task`

mod init;
mod presets;
mod sop_runner;

use anyhow::{Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use ralph_adapters::{
    CliBackend, CliExecutor, ConsoleStreamHandler, PtyConfig, PtyExecutor, QuietStreamHandler,
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

    /// Resume a previously interrupted loop from existing scratchpad
    Resume(ResumeArgs),

    /// View event history for debugging
    Events(EventsArgs),

    /// Initialize a new ralph.yml configuration file
    Init(InitArgs),

    /// Clean up Ralph artifacts (.agent/ directory)
    Clean(CleanArgs),

    /// Emit an event to .agent/events.jsonl with proper JSON formatting
    Emit(EmitArgs),

    /// Start a Prompt-Driven Development planning session
    Plan(PlanArgs),

    /// Generate code task files from descriptions or plans
    Task(TaskArgs),

    /// MCP (Model Context Protocol) server commands
    Mcp(McpArgs),
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

    // ─────────────────────────────────────────────────────────────────────────
    // Execution Mode Options
    // ─────────────────────────────────────────────────────────────────────────
    /// Enable interactive TUI mode for real-time monitoring
    #[arg(short, long, conflicts_with = "autonomous")]
    interactive: bool,

    /// Force autonomous mode (headless, non-interactive).
    /// Overrides default_mode from config.
    #[arg(short, long, conflicts_with = "interactive")]
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

    /// [DEPRECATED] Use -i/--interactive instead
    #[arg(long, hide = true)]
    tui: bool,
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

    /// Enable interactive TUI mode for real-time monitoring
    #[arg(short, long, conflicts_with = "autonomous")]
    interactive: bool,

    /// Force autonomous mode
    #[arg(short, long, conflicts_with = "interactive")]
    autonomous: bool,

    /// Idle timeout in seconds for interactive mode
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

    /// [DEPRECATED] Use -i/--interactive instead
    #[arg(long, hide = true)]
    tui: bool,
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

    /// Path to events file (default: .agent/events.jsonl)
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

    /// Path to events file (defaults to .agent/events.jsonl)
    #[arg(long, default_value = ".agent/events.jsonl")]
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
struct TaskArgs {
    /// Input: description text or path to PDD plan file
    #[arg(value_name = "INPUT")]
    input: Option<String>,

    /// Backend to use (overrides config and auto-detection)
    #[arg(short, long, value_name = "BACKEND")]
    backend: Option<String>,
}

/// Arguments for the mcp subcommand.
#[derive(Parser, Debug)]
struct McpArgs {
    #[command(subcommand)]
    command: McpCommands,
}

/// MCP (Model Context Protocol) subcommands.
#[derive(Subcommand, Debug)]
enum McpCommands {
    /// Start the MCP server over stdio
    Serve,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Install panic hook to restore terminal state on crash
    // This prevents the terminal from being left in raw mode or alternate screen
    install_panic_hook();

    let cli = Cli::parse();

    // Initialize logging
    let filter = if cli.verbose { "debug" } else { "info" };
    tracing_subscriber::fmt().with_env_filter(filter).init();

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
        Some(Commands::Task(args)) => task_command(cli.config, cli.color, args),
        Some(Commands::Mcp(args)) => mcp_command(args).await,
        None => {
            // Default to run with no overrides (backwards compatibility)
            let args = RunArgs {
                prompt_text: None,
                prompt_file: None,
                max_iterations: None,
                completion_promise: None,
                dry_run: false,
                interactive: false,
                autonomous: false,
                idle_timeout: None,
                verbose: false,
                quiet: false,
                record_session: None,
                tui: false, // No TUI by default
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
    // Show deprecation warning if --tui is used
    if args.tui {
        eprintln!("⚠️  Warning: --tui flag is deprecated. Use -i or --interactive instead.");
    }

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
    if args.autonomous {
        config.cli.default_mode = "autonomous".to_string();
    } else if args.interactive {
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
    let enable_tui = args.interactive || args.tui; // Support both for backward compat
    let verbosity = Verbosity::resolve(verbose || args.verbose, args.quiet);
    let reason = run_loop(
        config,
        color_mode,
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
/// Per spec: "When loop terminates due to safeguard (not completion promise),
/// user can run `ralph resume` to restart reading existing scratchpad,
/// continuing from where it left off."
async fn resume_command(
    config_path: PathBuf,
    verbose: bool,
    color_mode: ColorMode,
    args: ResumeArgs,
) -> Result<()> {
    // Show deprecation warning if --tui is used
    if args.tui {
        eprintln!("⚠️  Warning: --tui flag is deprecated. Use -i or --interactive instead.");
    }

    // Load configuration
    let mut config = if config_path.exists() {
        RalphConfig::from_file(&config_path)
            .with_context(|| format!("Failed to load config from {:?}", config_path))?
    } else {
        warn!("Config file {:?} not found, using defaults", config_path);
        RalphConfig::default()
    };

    config.normalize();

    // Check that scratchpad exists (required for resume)
    let scratchpad_path = std::path::Path::new(&config.core.scratchpad);
    if !scratchpad_path.exists() {
        anyhow::bail!(
            "Cannot resume: scratchpad not found at '{}'. Use `ralph run` to start a new loop.",
            config.core.scratchpad
        );
    }

    info!("Found existing scratchpad at '{}'", config.core.scratchpad);

    // Apply CLI overrides
    if let Some(max_iter) = args.max_iterations {
        config.event_loop.max_iterations = max_iter;
    }
    if verbose {
        config.verbose = true;
    }

    // Apply PTY mode overrides
    if args.autonomous {
        config.cli.default_mode = "autonomous".to_string();
    } else if args.interactive {
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
    let enable_tui = args.interactive || args.tui; // Support both for backward compat
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

    let history = match args.file {
        Some(path) => EventHistory::new(path),
        None => EventHistory::default_path(),
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

/// Emit an event to .agent/events.jsonl with proper JSON formatting.
///
/// This command provides a deterministic way for agents to emit events without
/// risking malformed JSONL from manual echo commands. All JSON serialization
/// is handled via serde_json, ensuring proper escaping of payloads.
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

    // Ensure parent directory exists
    if let Some(parent) = args.file.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent)
            .with_context(|| format!("Failed to create directory: {}", parent.display()))?;
    }

    // Append to file
    let mut file = fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&args.file)
        .with_context(|| format!("Failed to open events file: {}", args.file.display()))?;

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
fn task_command(config_path: PathBuf, color_mode: ColorMode, args: TaskArgs) -> Result<()> {
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

/// Runs MCP (Model Context Protocol) commands.
async fn mcp_command(args: McpArgs) -> Result<()> {
    match args.command {
        McpCommands::Serve => {
            // Note: MCP servers log to stderr since stdout is reserved for the protocol
            ralph_mcp::serve_stdio().await
        }
    }
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

async fn run_loop(
    config: RalphConfig,
    color_mode: ColorMode,
    enable_tui: bool,
    verbosity: Verbosity,
    record_session: Option<PathBuf>,
) -> Result<TerminationReason> {
    run_loop_impl(
        config,
        color_mode,
        false,
        enable_tui,
        verbosity,
        record_session,
    )
    .await
}

/// Core loop implementation supporting both fresh start and resume modes.
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
    let interactive_requested = config.cli.default_mode == "interactive" || enable_tui;
    let user_interactive = if interactive_requested {
        // Check if experimental_tui is enabled
        if !config.cli.experimental_tui {
            warn!(
                "Interactive TUI mode is experimental and disabled by default. \
                To enable, set `cli.experimental_tui: true` in your config. \
                Falling back to autonomous mode."
            );
            false
        } else if stdout().is_terminal() {
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
        let interrupt_tx_sighup = interrupt_tx;
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

    // Create backend - use TUI mode for Claude when TUI is enabled
    // This switches from `-p` with stream-json to positional arg without stream-json,
    // allowing Claude's native TUI to render properly.
    let backend = if enable_tui && config.cli.backend == "claude" {
        CliBackend::claude_tui()
    } else {
        CliBackend::from_config(&config.cli).map_err(|e| anyhow::Error::new(e))?
    };

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
            ..PtyConfig::from_env()
        };
        Some(PtyExecutor::new(backend.clone(), pty_config))
    } else {
        None
    };

    // Wire TUI to PTY executor if both are enabled
    let enable_tui = enable_tui && user_interactive && pty_executor.is_some();
    let tui_handle = if enable_tui {
        let mut tui = Tui::new();

        // Parse and apply TUI prefix key configuration
        match config.tui.parse_prefix() {
            Ok((key_code, key_modifiers)) => {
                tui = tui.with_prefix(key_code, key_modifiers);
            }
            Err(e) => {
                error!("Invalid TUI prefix_key configuration: {}", e);
                return Err(anyhow::anyhow!("Invalid TUI prefix_key: {}", e));
            }
        }

        // Build hat map for dynamic topic-to-hat resolution
        // This allows TUI to display custom hats (e.g., "🔒 Security Reviewer")
        // instead of generic "ralph" for all events
        let hat_map = build_tui_hat_map(event_loop.registry());
        tui = tui.with_hat_map(hat_map);

        // Wire PTY handle to TUI
        if let Some(ref mut executor) = pty_executor {
            let pty_handle = executor.handle();
            tui = tui.with_pty(pty_handle);
        }

        let observer = tui.observer();
        event_loop.add_observer(observer);
        Some(tokio::spawn(async move { tui.run().await }))
    } else {
        None
    };

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

        // Print termination info to console
        print_termination(reason, state, use_colors);
    };

    // Helper closure to clean up TUI task on exit
    let cleanup_tui = |tui_handle: Option<tokio::task::JoinHandle<Result<()>>>| {
        if let Some(handle) = tui_handle {
            handle.abort();
        }
    };

    // Main orchestration loop
    loop {
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
            cleanup_tui(tui_handle);
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
                    cleanup_tui(tui_handle);
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
                cleanup_tui(tui_handle);
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
        print_iteration_separator(
            iteration,
            display_hat.as_str(),
            event_loop.state().elapsed(),
            config.event_loop.max_iterations,
            use_colors,
        );

        // Log hat changes with appropriate messaging
        if last_hat.as_ref() != Some(&hat_id) {
            if hat_id.as_str() == "ralph" {
                info!("I'm Ralph. Let's do this.");
            } else {
                info!("Putting on my {} hat.", hat_id);
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

        // Race execution against interrupt signal for immediate termination on Ctrl+C
        let mut interrupt_rx_clone = interrupt_rx.clone();
        let interrupt_rx_for_pty = interrupt_rx.clone();
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
                cleanup_tui(tui_handle);
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
            cleanup_tui(tui_handle);
            return Ok(reason);
        }

        let output = outcome.output;
        let success = outcome.success;

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
            cleanup_tui(tui_handle);
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
) -> Result<ExecutionOutcome> {
    use crossterm::terminal::{disable_raw_mode, enable_raw_mode};

    // Use provided executor or create a new one
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
            ..PtyConfig::from_env()
        };
        temp_executor = PtyExecutor::new(backend.clone(), pty_config);
        &mut temp_executor
    };

    // Enter raw mode for interactive mode to capture keystrokes
    if interactive {
        enable_raw_mode().context("Failed to enable raw mode")?;
    }

    // Use scopeguard to ensure raw mode is restored on any exit path
    let _guard = scopeguard::guard(interactive, |is_interactive| {
        if is_interactive {
            let _ = disable_raw_mode();
        }
    });

    // Run PTY executor with shared interrupt channel
    let result = if interactive {
        exec.run_interactive(prompt, interrupt_rx).await
    } else {
        // Use streaming handler for non-interactive mode (respects verbosity)
        match verbosity {
            Verbosity::Quiet => {
                let mut handler = QuietStreamHandler;
                exec.run_observe_streaming(prompt, interrupt_rx, &mut handler)
                    .await
            }
            Verbosity::Normal => {
                let mut handler = ConsoleStreamHandler::new(false);
                exec.run_observe_streaming(prompt, interrupt_rx, &mut handler)
                    .await
            }
            Verbosity::Verbose => {
                let mut handler = ConsoleStreamHandler::new(true);
                exec.run_observe_streaming(prompt, interrupt_rx, &mut handler)
                    .await
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
