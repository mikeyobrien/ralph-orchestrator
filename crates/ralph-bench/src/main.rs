//! # ralph-bench
//!
//! Benchmark harness for the Ralph Orchestrator.
//!
//! This crate provides:
//! - Recording sessions by observing EventBus events
//! - Replaying sessions with timing and UX output control
//! - Batch benchmarking with isolated workspaces
//! - Metrics collection for benchmark comparison

use anyhow::{Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use ralph_core::{
    CleanupPolicy, PlayerConfig, ReplayMode, SessionPlayer, TaskSuite, WorkspaceManager,
};
use std::fs::{self, File};
use std::io::{self, BufReader, BufWriter};
use std::path::PathBuf;
use tracing::info;

mod autoloop_task;

/// Ralph Benchmark Harness - Record, replay, and benchmark orchestration loops
#[derive(Parser, Debug)]
#[command(name = "ralph-bench", version, about)]
struct Args {
    /// Verbose output
    #[arg(short, long)]
    verbose: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Run benchmark tasks
    Run {
        /// Path to tasks.json file
        tasks: PathBuf,

        /// Record session to JSONL file (single task mode)
        #[arg(long)]
        record: Option<PathBuf>,

        /// Record each task to separate file in directory
        #[arg(long)]
        record_dir: Option<PathBuf>,

        /// Enable UX (terminal output) recording
        #[arg(long)]
        record_ux: bool,

        /// Write metrics summary to JSON file
        #[arg(long, short)]
        output: Option<PathBuf>,

        /// Filter to specific task by name
        #[arg(long)]
        task: Option<String>,

        /// Cleanup policy: rotate, on_success, always, never
        #[arg(long, default_value = "on_success")]
        cleanup: String,

        /// Number of workspaces to keep when using rotate policy
        #[arg(long, default_value = "5")]
        keep_last_n: usize,
    },

    /// Replay a recorded session
    Replay {
        /// Path to session JSONL file
        session: PathBuf,

        /// Output mode: terminal (with timing/colors), text (ANSI stripped)
        #[arg(long, value_enum, default_value = "terminal")]
        ux_mode: UxMode,

        /// Playback speed multiplier (e.g., 2.0 for 2x speed)
        #[arg(long, default_value = "1.0")]
        speed: f32,

        /// Step through events manually (press Enter after each)
        #[arg(long)]
        step: bool,

        /// Filter to specific event types (comma-separated prefixes)
        #[arg(long)]
        filter: Option<String>,
    },

    /// List recorded sessions or workspaces
    List {
        /// What to list: sessions, workspaces
        #[arg(value_enum, default_value = "sessions")]
        what: ListTarget,

        /// Directory to search
        #[arg(short, long)]
        dir: Option<PathBuf>,
    },
}

/// UX replay mode
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum UxMode {
    /// Re-render with timing and colors preserved
    Terminal,
    /// Strip ANSI codes, output plain text
    Text,
}

impl From<UxMode> for ReplayMode {
    fn from(mode: UxMode) -> Self {
        match mode {
            UxMode::Terminal => ReplayMode::Terminal,
            UxMode::Text => ReplayMode::Text,
        }
    }
}

/// What to list
#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
enum ListTarget {
    /// List recorded session files
    Sessions,
    /// List workspace directories
    Workspaces,
}

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();

    // Initialize logging
    let filter = if args.verbose { "debug" } else { "info" };
    tracing_subscriber::fmt().with_env_filter(filter).init();

    match args.command {
        Commands::Run {
            tasks,
            record,
            record_dir,
            record_ux,
            output,
            task,
            cleanup,
            keep_last_n,
        } => {
            cmd_run(
                tasks,
                record,
                record_dir,
                record_ux,
                output,
                task,
                cleanup,
                keep_last_n,
            )
            .await
        }
        Commands::Replay {
            session,
            ux_mode,
            speed,
            step,
            filter,
        } => cmd_replay(session, ux_mode, speed, step, filter),
        Commands::List { what, dir } => cmd_list(what, dir),
    }
}

/// Run benchmark tasks
async fn cmd_run(
    tasks_path: PathBuf,
    record: Option<PathBuf>,
    record_dir: Option<PathBuf>,
    record_ux: bool,
    output: Option<PathBuf>,
    task_filter: Option<String>,
    cleanup_policy: String,
    keep_last_n: usize,
) -> Result<()> {
    // Load task suite
    let suite = TaskSuite::from_file(&tasks_path)
        .with_context(|| format!("Failed to load tasks from {:?}", tasks_path))?;

    info!("Loaded {} tasks from {:?}", suite.tasks.len(), tasks_path);

    // Determine tasks to run
    let tasks_to_run: Vec<_> = if let Some(ref name) = task_filter {
        suite.tasks.iter().filter(|t| &t.name == name).collect()
    } else {
        suite.tasks.iter().collect()
    };

    if tasks_to_run.is_empty() {
        if let Some(name) = task_filter {
            anyhow::bail!("No task found with name '{}'", name);
        } else {
            anyhow::bail!("No tasks to run");
        }
    }

    // Setup workspace manager
    let policy = CleanupPolicy::from_str(&cleanup_policy, Some(keep_last_n));
    let base_dir = std::env::temp_dir();
    let manager = WorkspaceManager::new(&base_dir, policy);

    // Get tasks directory (parent of tasks.json)
    let tasks_dir = tasks_path
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."));

    // Ensure record directory exists if specified
    if let Some(ref dir) = record_dir {
        fs::create_dir_all(dir)
            .with_context(|| format!("Failed to create record directory: {:?}", dir))?;
    }

    // Run each task
    let mut results = Vec::new();
    for task in tasks_to_run {
        info!("Running task: {}", task.name);

        // Create workspace
        let workspace = manager
            .create_workspace(task)
            .with_context(|| format!("Failed to create workspace for task '{}'", task.name))?;

        // Setup workspace with task files
        workspace
            .setup(task, &tasks_dir)
            .with_context(|| format!("Failed to setup workspace for task '{}'", task.name))?;

        info!("Workspace created at: {}", workspace.path().display());

        // Determine recording output
        let record_path = if let Some(ref dir) = record_dir {
            Some(dir.join(format!("{}.jsonl", task.name)))
        } else {
            record.clone()
        };

        // Track timing
        let task_start = std::time::Instant::now();

        // Run the orchestration loop for this task
        let (iterations, termination_reason) =
            autoloop_task::run_task_loop(task, &workspace, record_path.as_ref(), record_ux)
                .await
                .with_context(|| format!("Failed to run task '{}'", task.name))?;

        // Run verification command (this works even without full EventLoop integration)
        let verification_result = workspace
            .run_verification(&task.verification)
            .with_context(|| format!("Failed to run verification for task '{}'", task.name))?;

        if verification_result.passed {
            info!(
                "Task '{}' verification: {}",
                task.name,
                verification_result.summary()
            );
        } else {
            tracing::warn!(
                "Task '{}' verification: {}\nstderr: {}",
                task.name,
                verification_result.summary(),
                verification_result.stderr.trim()
            );
        }

        let duration_secs = task_start.elapsed().as_secs_f64();

        // Apply cleanup policy based on verification result
        let mut workspace = workspace;
        let cleaned_up = manager
            .apply_cleanup(&mut workspace, verification_result.passed)
            .with_context(|| format!("Failed to cleanup workspace for task '{}'", task.name))?;

        if !cleaned_up {
            info!(
                "Workspace retained for debugging: {}",
                workspace.path().display()
            );
        }

        // Record task result
        results.push(TaskResult::new(
            task.name.clone(),
            iterations,
            task.expected_iterations,
            duration_secs,
            termination_reason,
            verification_result.passed,
            workspace.path().to_string_lossy().to_string(),
        ));
    }

    // Write results if output specified
    if let Some(output_path) = output {
        let results_json = BenchmarkResults {
            run_id: format!("bench-{}", chrono_timestamp()),
            timestamp: chrono_timestamp(),
            tasks: results,
        };

        let file = File::create(&output_path)
            .with_context(|| format!("Failed to create output file: {:?}", output_path))?;
        serde_json::to_writer_pretty(BufWriter::new(file), &results_json)
            .with_context(|| "Failed to write results JSON")?;

        info!("Results written to: {:?}", output_path);
    }

    Ok(())
}

/// Replay a recorded session
fn cmd_replay(
    session_path: PathBuf,
    ux_mode: UxMode,
    speed: f32,
    step: bool,
    filter: Option<String>,
) -> Result<()> {
    // Open session file
    let file = File::open(&session_path)
        .with_context(|| format!("Failed to open session file: {:?}", session_path))?;

    // Create player
    let mut player = SessionPlayer::from_reader(BufReader::new(file))
        .with_context(|| "Failed to parse session file")?;

    info!(
        "Loaded {} records from {:?}",
        player.record_count(),
        session_path
    );

    // Configure playback
    let mut config = PlayerConfig::default();
    config.replay_mode = ux_mode.into();
    config.speed = speed;
    config.step_mode = step;

    if let Some(f) = filter {
        config.event_filter = f.split(',').map(|s| s.trim().to_string()).collect();
    }

    player = player.with_config(config);

    // Replay to stdout
    let mut stdout = io::stdout();
    player
        .replay_terminal(&mut stdout)
        .with_context(|| "Failed to replay session")?;

    Ok(())
}

/// List sessions or workspaces
fn cmd_list(what: ListTarget, dir: Option<PathBuf>) -> Result<()> {
    let search_dir = dir.unwrap_or_else(|| PathBuf::from("."));

    match what {
        ListTarget::Sessions => {
            // List .jsonl files
            if !search_dir.exists() {
                println!("Directory does not exist: {:?}", search_dir);
                return Ok(());
            }

            let mut sessions: Vec<_> = fs::read_dir(&search_dir)?
                .filter_map(|e| e.ok())
                .filter(|e| e.path().extension().is_some_and(|ext| ext == "jsonl"))
                .collect();

            sessions.sort_by_key(|e| e.file_name());

            if sessions.is_empty() {
                println!("No session files found in {:?}", search_dir);
            } else {
                println!("Sessions in {:?}:", search_dir);
                for entry in sessions {
                    let path = entry.path();
                    let metadata = entry.metadata().ok();
                    let size = metadata.as_ref().map(|m| m.len()).unwrap_or(0);
                    println!(
                        "  {} ({} bytes)",
                        path.file_name().unwrap_or_default().to_string_lossy(),
                        size
                    );
                }
            }
        }
        ListTarget::Workspaces => {
            // List ralph-bench-* directories
            let manager = WorkspaceManager::new(&search_dir, CleanupPolicy::Never);
            let workspaces = manager.list_workspaces()?;

            if workspaces.is_empty() {
                println!("No workspaces found in {:?}", search_dir);
            } else {
                println!("Workspaces in {:?}:", search_dir);
                for ws in workspaces {
                    let task = ws.task_name.as_deref().unwrap_or("unknown");
                    let ts = ws
                        .timestamp
                        .map(|t| t.to_string())
                        .unwrap_or_else(|| "?".to_string());
                    println!("  {} (task: {}, ts: {})", ws.path.display(), task, ts);
                }
            }
        }
    }

    Ok(())
}

/// Task execution result
#[derive(Debug, serde::Serialize)]
struct TaskResult {
    name: String,
    iterations: u32,
    expected_iterations: Option<u32>,
    /// Difference between actual and expected iterations (iterations - expected).
    /// Positive means more iterations than expected, negative means fewer.
    iteration_delta: Option<i32>,
    duration_secs: f64,
    termination_reason: String,
    verification_passed: bool,
    workspace_path: String,
}

impl TaskResult {
    /// Create a new TaskResult, calculating iteration_delta automatically.
    fn new(
        name: String,
        iterations: u32,
        expected_iterations: Option<u32>,
        duration_secs: f64,
        termination_reason: String,
        verification_passed: bool,
        workspace_path: String,
    ) -> Self {
        let iteration_delta =
            expected_iterations.map(|expected| iterations as i32 - expected as i32);

        Self {
            name,
            iterations,
            expected_iterations,
            iteration_delta,
            duration_secs,
            termination_reason,
            verification_passed,
            workspace_path,
        }
    }
}

/// Benchmark results output
#[derive(Debug, serde::Serialize)]
struct BenchmarkResults {
    run_id: String,
    timestamp: String,
    tasks: Vec<TaskResult>,
}

/// Generate a timestamp string
fn chrono_timestamp() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();

    // Format: YYYYMMDD-HHMMSS
    let secs = now.as_secs();
    let days = secs / 86400;
    let time_of_day = secs % 86400;
    let hours = time_of_day / 3600;
    let minutes = (time_of_day % 3600) / 60;
    let seconds = time_of_day % 60;

    // Approximate date calculation (not accounting for leap years perfectly)
    let mut year = 1970;
    let mut remaining_days = days;

    loop {
        let days_in_year = if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) {
            366
        } else {
            365
        };
        if remaining_days < days_in_year {
            break;
        }
        remaining_days -= days_in_year;
        year += 1;
    }

    let days_in_months = if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) {
        [31, 29, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    } else {
        [31, 28, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31]
    };

    let mut month = 1;
    for days_in_month in days_in_months {
        if remaining_days < days_in_month {
            break;
        }
        remaining_days -= days_in_month;
        month += 1;
    }

    let day = remaining_days + 1;

    format!(
        "{:04}{:02}{:02}-{:02}{:02}{:02}",
        year, month, day, hours, minutes, seconds
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chrono_timestamp_format() {
        let ts = chrono_timestamp();
        // Should be YYYYMMDD-HHMMSS format (15 characters)
        assert_eq!(ts.len(), 15);
        assert_eq!(&ts[8..9], "-");
    }

    #[test]
    fn test_ux_mode_conversion() {
        assert_eq!(ReplayMode::from(UxMode::Terminal), ReplayMode::Terminal);
        assert_eq!(ReplayMode::from(UxMode::Text), ReplayMode::Text);
    }
}
