//! # ralph-cli
//!
//! Binary entry point for the Ralph Orchestrator.
//!
//! This crate provides:
//! - CLI argument parsing using `clap`
//! - Application initialization and configuration
//! - Entry point to the headless orchestration loop
//! - Event history viewing via `ralph events`

use anyhow::{Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use ralph_adapters::{detect_backend, CliBackend, CliExecutor, PtyConfig, PtyExecutor};
use ralph_core::{EventHistory, EventLogger, EventLoop, EventParser, EventRecord, RalphConfig, SummaryWriter, TerminationReason};
use ralph_proto::{Event, HatId};
use ralph_tui::Tui;
use std::io::{stdout, IsTerminal};
use std::path::PathBuf;
use std::process::Command;
use tracing::{debug, error, info, warn};

// Unix-specific process management for process group leadership
#[cfg(unix)]
mod process_management {
    use nix::unistd::{setpgid, Pid};
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
                debug!("Note: Could not set process group ({}), continuing anyway", e);
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
}

/// Arguments for the run subcommand.
#[derive(Parser, Debug)]
struct RunArgs {
    /// Inline prompt text (mutually exclusive with -P)
    #[arg(short = 'p', long = "prompt-text", conflicts_with = "prompt_file")]
    prompt_text: Option<String>,

    /// Prompt file path (mutually exclusive with -p)
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

    /// Enable interactive mode (PTY with user input forwarding).
    /// User can interact with the agent through Ralph.
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

    /// Enable terminal UI for real-time monitoring
    #[arg(long)]
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

    /// Enable interactive mode
    #[arg(short, long, conflicts_with = "autonomous")]
    interactive: bool,

    /// Force autonomous mode
    #[arg(short, long, conflicts_with = "interactive")]
    autonomous: bool,

    /// Idle timeout in seconds for interactive mode
    #[arg(long)]
    idle_timeout: Option<u32>,

    /// Enable terminal UI for real-time monitoring
    #[arg(long)]
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

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize logging
    let filter = if cli.verbose { "debug" } else { "info" };
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .init();

    match cli.command {
        Some(Commands::Run(args)) => run_command(cli.config, cli.verbose, cli.color, args).await,
        Some(Commands::Resume(args)) => resume_command(cli.config, cli.verbose, cli.color, args).await,
        Some(Commands::Events(args)) => events_command(cli.color, args),
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
                tui: false,
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
    let warnings = config.validate().context("Configuration validation failed")?;
    for warning in &warnings {
        eprintln!("{warning}");
    }

    // Run preflight validation to catch issues before the loop starts
    let (preflight_errors, preflight_warnings) = config.preflight_check();
    for warning in &preflight_warnings {
        warn!("Preflight: {}", warning);
    }
    if !preflight_errors.is_empty() {
        eprintln!("\n❌ Preflight check failed:");
        for error in &preflight_errors {
            eprintln!("   • {error}");
        }
        eprintln!("\nFix these issues before running the loop.\n");
        anyhow::bail!("Preflight validation failed with {} error(s)", preflight_errors.len());
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
        println!("  Hats: {}", if config.hats.is_empty() { "planner, builder (default)".to_string() } else { config.hats.keys().cloned().collect::<Vec<_>>().join(", ") });
        
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
        
        println!("  Completion promise: {}", config.event_loop.completion_promise);
        println!("  Max iterations: {}", config.event_loop.max_iterations);
        println!("  Max runtime: {}s", config.event_loop.max_runtime_seconds);
        println!("  Backend: {}", config.cli.backend);
        println!("  Git checkpoint: {}", config.git_checkpoint);
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
    let reason = run_loop(config, color_mode, args.tui).await?;
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
    let warnings = config.validate().context("Configuration validation failed")?;
    for warning in &warnings {
        eprintln!("{warning}");
    }

    // Run preflight validation to catch issues before the loop starts
    let (preflight_errors, preflight_warnings) = config.preflight_check();
    for warning in &preflight_warnings {
        warn!("Preflight: {}", warning);
    }
    if !preflight_errors.is_empty() {
        eprintln!("\n❌ Preflight check failed:");
        for error in &preflight_errors {
            eprintln!("   • {error}");
        }
        eprintln!("\nFix these issues before running the loop.\n");
        anyhow::bail!("Preflight validation failed with {} error(s)", preflight_errors.len());
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
    let reason = run_loop_impl(config, color_mode, true, args.tui).await?;
    let exit_code = reason.exit_code();

    if exit_code != 0 {
        std::process::exit(exit_code);
    }

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
    if let Some(n) = args.last {
        if records.len() > n {
            records = records.into_iter().rev().take(n).rev().collect();
        }
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
        println!(
            "\n{DIM}Total: {} events{RESET}",
            records.len()
        );
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
    // Check for inline prompt first (CLI -p or config prompt)
    if let Some(ref inline_text) = event_loop_config.prompt {
        debug!("Using inline prompt text");
        return Ok(inline_text.clone());
    }

    // Check for prompt file (CLI -P or config prompt_file or default)
    let prompt_file = &event_loop_config.prompt_file;
    if !prompt_file.is_empty() {
        let path = std::path::Path::new(prompt_file);
        if path.exists() {
            debug!(path = %prompt_file, "Reading prompt from file");
            return std::fs::read_to_string(path)
                .with_context(|| format!("Failed to read prompt file: {}", prompt_file));
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

async fn run_loop(config: RalphConfig, color_mode: ColorMode, enable_tui: bool) -> Result<TerminationReason> {
    run_loop_impl(config, color_mode, false, enable_tui).await
}

/// Core loop implementation supporting both fresh start and resume modes.
///
/// `resume`: If true, publishes `task.resume` instead of `task.start`,
/// signaling the planner to read existing scratchpad rather than doing fresh gap analysis.
async fn run_loop_impl(config: RalphConfig, color_mode: ColorMode, resume: bool, enable_tui: bool) -> Result<TerminationReason> {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    // Set up process group leadership per spec
    // "The orchestrator must run as a process group leader"
    process_management::setup_process_group();

    let use_colors = color_mode.should_use_colors();

    // Determine effective execution mode (with fallback logic)
    // Per spec: Claude backend requires PTY mode to avoid hangs
    let use_interactive = if config.cli.backend == "claude" {
        true
    } else if config.cli.default_mode == "interactive" {
        if stdout().is_terminal() {
            true
        } else {
            warn!("Interactive mode requested but stdout is not a TTY, falling back to autonomous");
            false
        }
    } else {
        false
    };

    // Set up signal handling for graceful shutdown
    // Per spec:
    // - SIGINT (Ctrl+C): Allow current iteration to finish gracefully, exit with code 130
    // - SIGTERM: Send SIGTERM to child process, wait up to 5s, then SIGKILL if needed
    // - SIGHUP: Same as SIGTERM—kill child process before exiting
    let interrupted = Arc::new(AtomicBool::new(false));

    // Spawn task to listen for SIGINT (Ctrl+C)
    let interrupted_sigint = Arc::clone(&interrupted);
    tokio::spawn(async move {
        if tokio::signal::ctrl_c().await.is_ok() {
            warn!("Interrupt received (SIGINT), finishing current iteration...");
            interrupted_sigint.store(true, Ordering::SeqCst);
        }
    });

    // Spawn task to listen for SIGTERM (Unix only)
    #[cfg(unix)]
    {
        let interrupted_sigterm = Arc::clone(&interrupted);
        tokio::spawn(async move {
            let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
                .expect("Failed to register SIGTERM handler");
            sigterm.recv().await;
            warn!("SIGTERM received, finishing current iteration...");
            interrupted_sigterm.store(true, Ordering::SeqCst);
        });
    }

    // Spawn task to listen for SIGHUP (Unix only)
    #[cfg(unix)]
    {
        let interrupted_sighup = Arc::clone(&interrupted);
        tokio::spawn(async move {
            let mut sighup = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::hangup())
                .expect("Failed to register SIGHUP handler");
            sighup.recv().await;
            warn!("SIGHUP received (terminal closed), finishing current iteration...");
            interrupted_sighup.store(true, Ordering::SeqCst);
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

    // Set up TUI if enabled
    let tui_handle = if enable_tui {
        let tui = Tui::new();
        let observer = tui.observer();
        event_loop.set_observer(observer);
        Some(tokio::spawn(async move { tui.run().await }))
    } else {
        None
    };

    // Per spec: Log startup message with registered hats
    let hat_names: Vec<String> = event_loop.registry().ids().map(|id| id.to_string()).collect();
    info!(
        hats = ?hat_names,
        "I'm Ralph. Got my hats ready: {}. Let's do this.",
        hat_names.join(", ")
    );

    // Initialize event logger for debugging
    let mut event_logger = EventLogger::default_path();

    // Log initial event (task.start or task.resume)
    let (start_topic, start_triggered) = if resume {
        ("task.resume", "planner")
    } else {
        ("task.start", "planner")
    };
    let start_event = Event::new(start_topic, &prompt_content);
    let start_record = EventRecord::new(0, "loop", &start_event, Some(&HatId::new(start_triggered)));
    if let Err(e) = event_logger.log(&start_record) {
        warn!("Failed to log start event: {}", e);
    }

    // Create backend
    let backend = CliBackend::from_config(&config.cli)
        .map_err(|e| anyhow::Error::new(e))?;

    // Log execution mode - hat info already logged by initialize()
    let exec_mode = if use_interactive { "interactive" } else { "autonomous" };
    debug!(execution_mode = %exec_mode, "Execution mode configured");

    // Track the last hat to detect hat changes for logging
    let mut last_hat: Option<HatId> = None;

    // Track consecutive fallback attempts to prevent infinite loops
    let mut consecutive_fallbacks: u32 = 0;
    const MAX_FALLBACK_ATTEMPTS: u32 = 3;

    // Helper closure to handle termination (writes summary, prints status, creates final checkpoint)
    let handle_termination = |reason: &TerminationReason, state: &ralph_core::LoopState, git_checkpoint: bool, scratchpad: &str| {
        // Per spec: Write summary file on termination
        let summary_writer = SummaryWriter::default();
        let scratchpad_path = std::path::Path::new(scratchpad);
        let scratchpad_opt = if scratchpad_path.exists() { Some(scratchpad_path) } else { None };

        // Get final commit SHA if available
        let final_commit = get_last_commit_info();

        if let Err(e) = summary_writer.write(reason, state, scratchpad_opt, final_commit.as_deref()) {
            warn!("Failed to write summary file: {}", e);
        }

        // Per spec: Create final checkpoint if pending changes exist
        if git_checkpoint {
            if let Ok(true) = create_final_checkpoint() {
                debug!("Final checkpoint created on termination");
            }
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
            log_terminate_event(&mut event_logger, event_loop.state().iteration, &terminate_event);
            handle_termination(&reason, event_loop.state(), config.git_checkpoint, &config.core.scratchpad);
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
                    log_terminate_event(&mut event_logger, event_loop.state().iteration, &terminate_event);
                    handle_termination(&reason, event_loop.state(), config.git_checkpoint, &config.core.scratchpad);
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
                log_terminate_event(&mut event_logger, event_loop.state().iteration, &terminate_event);
                handle_termination(&reason, event_loop.state(), config.git_checkpoint, &config.core.scratchpad);
                cleanup_tui(tui_handle);
                return Ok(reason);
            }
        };

        let iteration = event_loop.state().iteration + 1;

        // Per spec: Print iteration demarcation separator
        // "Each iteration must be clearly demarcated in the output so users can
        // visually distinguish where one iteration ends and another begins."
        print_iteration_separator(
            iteration,
            hat_id.as_str(),
            event_loop.state().elapsed(),
            config.event_loop.max_iterations,
            use_colors,
        );

        // Per spec: Log "Putting on my {hat} hat." when hat changes
        if last_hat.as_ref() != Some(&hat_id) {
            info!("Putting on my {} hat.", hat_id);
            last_hat = Some(hat_id.clone());
        }
        debug!("Iteration {}/{} — wearing {} hat", iteration, config.event_loop.max_iterations, hat_id);

        // Build prompt for this hat
        let prompt = match event_loop.build_prompt(&hat_id) {
            Some(p) => p,
            None => {
                error!("Failed to build prompt for hat '{}'", hat_id);
                continue;
            }
        };

        // Execute the prompt (interactive or autonomous mode)
        let (output, success) = if use_interactive {
            execute_pty(&backend, &config, &prompt).await?
        } else {
            let executor = CliExecutor::new(backend.clone());
            let result = executor.execute(&prompt, stdout()).await?;
            (result.output, result.success)
        };

        // Log events from output before processing
        log_events_from_output(&mut event_logger, iteration, &hat_id, &output, event_loop.registry());

        // Process output
        if let Some(reason) = event_loop.process_output(&hat_id, &output, success) {
            // Per spec: Log "All done! {promise} detected." when completion promise found
            if reason == TerminationReason::CompletionPromise {
                info!("All done! {} detected.", config.event_loop.completion_promise);
            }
            // Per spec: Publish loop.terminate event to observers
            let terminate_event = event_loop.publish_terminate_event(&reason);
            log_terminate_event(&mut event_logger, event_loop.state().iteration, &terminate_event);
            handle_termination(&reason, event_loop.state(), config.git_checkpoint, &config.core.scratchpad);
            cleanup_tui(tui_handle);
            return Ok(reason);
        }

        // Precheck validation: Warn if no pending events after processing output
        // Per EventLoop doc: "Use has_pending_events after process_output to detect
        // if the LLM failed to publish an event."
        if !event_loop.has_pending_events() {
            let expected = event_loop.get_hat_publishes(&hat_id);
            warn!(
                hat = %hat_id.as_str(),
                expected_topics = ?expected,
                "No pending events after iteration. Agent may have failed to publish a valid event. \
                 Expected one of: {:?}. Loop will terminate on next iteration.",
                expected
            );
        }

        // Handle checkpointing (only if git_checkpoint is enabled)
        if config.git_checkpoint && event_loop.should_checkpoint() {
            if create_checkpoint(event_loop.state().iteration)? {
                event_loop.record_checkpoint();
            }
        }

        // Per spec: Check for interrupt after each iteration completes
        // "SIGINT received during iteration → current iteration allowed to finish, then exit"
        if interrupted.load(Ordering::SeqCst) {
            let reason = TerminationReason::Interrupted;
            let terminate_event = event_loop.publish_terminate_event(&reason);
            log_terminate_event(&mut event_logger, event_loop.state().iteration, &terminate_event);
            handle_termination(&reason, event_loop.state(), config.git_checkpoint, &config.core.scratchpad);
            cleanup_tui(tui_handle);
            return Ok(reason);
        }
    }
}

/// Executes a prompt in PTY mode with raw terminal handling.
async fn execute_pty(
    backend: &CliBackend,
    config: &RalphConfig,
    prompt: &str,
) -> Result<(String, bool)> {
    use crossterm::terminal::{disable_raw_mode, enable_raw_mode};

    // Create PTY config from ralph config
    let is_interactive = config.cli.default_mode == "interactive";
    let pty_config = PtyConfig {
        interactive: is_interactive,
        idle_timeout_secs: config.cli.idle_timeout_secs,
        ..PtyConfig::from_env()
    };

    let executor = PtyExecutor::new(backend.clone(), pty_config);

    // Enter raw mode for interactive mode to capture keystrokes
    if is_interactive {
        enable_raw_mode().context("Failed to enable raw mode")?;
    }

    // Use scopeguard to ensure raw mode is restored on any exit path
    let _guard = scopeguard::guard(is_interactive, |interactive| {
        if interactive {
            let _ = disable_raw_mode();
        }
    });

    // Run PTY executor
    let result = if is_interactive {
        executor.run_interactive(prompt).await
    } else {
        executor.run_observe(prompt)
    };

    match result {
        Ok(pty_result) => {
            // Use stripped output for event parsing (ANSI sequences removed)
            Ok((pty_result.stripped_output, pty_result.success))
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
        println!("{BOLD}│{RESET}   Iterations:  {CYAN}{}{RESET}", state.iteration);
        println!(
            "{BOLD}│{RESET}   Elapsed:     {CYAN}{:.1}s{RESET}",
            state.elapsed().as_secs_f64()
        );
        if state.checkpoint_count > 0 {
            println!(
                "{BOLD}│{RESET}   Checkpoints: {CYAN}{}{RESET}",
                state.checkpoint_count
            );
        }
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
        if state.checkpoint_count > 0 {
            println!("|   Checkpoints: {}", state.checkpoint_count);
        }
        if state.cumulative_cost > 0.0 {
            println!("|   Cost:        ${:.2}", state.cumulative_cost);
        }
        println!("+{}+", "-".repeat(58));
    }
}

/// Creates a git checkpoint and returns true if the commit succeeded.
fn create_checkpoint(iteration: u32) -> Result<bool> {
    info!("Creating checkpoint at iteration {}", iteration);

    let status = Command::new("git")
        .args(["add", "-A"])
        .status()
        .context("Failed to run git add")?;

    if !status.success() {
        warn!("git add failed");
        return Ok(false);
    }

    let message = format!("ralph: checkpoint at iteration {iteration}");
    let status = Command::new("git")
        .args(["commit", "-m", &message, "--allow-empty"])
        .status()
        .context("Failed to run git commit")?;

    if !status.success() {
        warn!("git commit failed (may be nothing to commit)");
        return Ok(false);
    }

    Ok(true)
}

/// Creates a final git checkpoint on loop termination if there are pending changes.
///
/// Per spec: "Given loop terminates with pending changes, When termination flow executes,
/// Then final git checkpoint is created before exit."
fn create_final_checkpoint() -> Result<bool> {
    // Check if there are pending changes
    let output = Command::new("git")
        .args(["status", "--porcelain"])
        .output()
        .context("Failed to run git status")?;

    let has_changes = !output.stdout.is_empty();

    if !has_changes {
        debug!("No pending changes for final checkpoint");
        return Ok(false);
    }

    info!("Creating final checkpoint on termination");

    let status = Command::new("git")
        .args(["add", "-A"])
        .status()
        .context("Failed to run git add")?;

    if !status.success() {
        warn!("git add failed for final checkpoint");
        return Ok(false);
    }

    let status = Command::new("git")
        .args(["commit", "-m", "ralph: final checkpoint on termination"])
        .status()
        .context("Failed to run git commit")?;

    if !status.success() {
        warn!("git commit failed for final checkpoint");
        return Ok(false);
    }

    Ok(true)
}

/// Gets the last commit info (short SHA and subject) for the summary file.
fn get_last_commit_info() -> Option<String> {
    let output = Command::new("git")
        .args(["log", "-1", "--format=%h: %s"])
        .output()
        .ok()?;

    if output.status.success() {
        let info = String::from_utf8_lossy(&output.stdout).trim().to_string();
        if info.is_empty() {
            None
        } else {
            Some(info)
        }
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use ralph_core::RalphConfig;

    #[test]
    fn test_claude_backend_forces_pty_mode() {
        // Given: backend is "claude" and default_mode is "autonomous"
        let mut config = RalphConfig::default();
        config.cli.backend = "claude".to_string();
        config.cli.default_mode = "autonomous".to_string();

        // When: determining use_interactive
        let use_interactive = if config.cli.backend == "claude" {
            true
        } else if config.cli.default_mode == "interactive" {
            true
        } else {
            false
        };

        // Then: PTY mode should be enabled
        assert!(use_interactive, "Claude backend should force PTY mode");
    }

    #[test]
    fn test_gemini_backend_respects_default_mode() {
        // Given: backend is "gemini" and default_mode is "autonomous"
        let mut config = RalphConfig::default();
        config.cli.backend = "gemini".to_string();
        config.cli.default_mode = "autonomous".to_string();

        // When: determining use_interactive
        let use_interactive = if config.cli.backend == "claude" {
            true
        } else if config.cli.default_mode == "interactive" {
            true
        } else {
            false
        };

        // Then: PTY mode should NOT be enabled (respects autonomous mode)
        assert!(!use_interactive, "Gemini backend should respect autonomous mode");
    }

    #[test]
    fn test_claude_backend_overrides_interactive_mode_setting() {
        // Given: backend is "claude" and default_mode is "interactive"
        let mut config = RalphConfig::default();
        config.cli.backend = "claude".to_string();
        config.cli.default_mode = "interactive".to_string();

        // When: determining use_interactive
        let use_interactive = if config.cli.backend == "claude" {
            true
        } else if config.cli.default_mode == "interactive" {
            true
        } else {
            false
        };

        // Then: PTY mode should be enabled (would be true anyway, but Claude forces it)
        assert!(use_interactive, "Claude backend should enable PTY mode");
    }

    #[test]
    fn test_other_backends_respect_autonomous_mode() {
        let backends = vec!["kiro", "gemini", "codex", "amp"];

        for backend in backends {
            // Given: backend is not "claude" and default_mode is "autonomous"
            let mut config = RalphConfig::default();
            config.cli.backend = backend.to_string();
            config.cli.default_mode = "autonomous".to_string();

            // When: determining use_interactive
            let use_interactive = if config.cli.backend == "claude" {
                true
            } else if config.cli.default_mode == "interactive" {
                true
            } else {
                false
            };

            // Then: PTY mode should NOT be enabled
            assert!(
                !use_interactive,
                "{} backend should respect autonomous mode",
                backend
            );
        }
    }
}
