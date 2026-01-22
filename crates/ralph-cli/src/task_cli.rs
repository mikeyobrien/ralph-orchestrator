//! CLI commands for the `ralph task` namespace.
//!
//! Provides subcommands for managing tasks:
//! - `add`: Create a new task
//! - `list`: List all tasks
//! - `ready`: Show unblocked tasks
//! - `close`: Mark a task as complete
//! - `show`: Show a single task by ID

use anyhow::{Context, Result};
use clap::{Parser, Subcommand, ValueEnum};
use ralph_core::{Task, TaskStatus, TaskStore};
use std::path::{Path, PathBuf};

/// Output format for task commands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, ValueEnum)]
pub enum OutputFormat {
    /// Human-readable table format
    #[default]
    Table,
    /// JSON format for programmatic access
    Json,
    /// ID-only output for scripting
    Quiet,
}

/// Task management commands for tracking work items.
#[derive(Parser, Debug)]
pub struct TaskArgs {
    #[command(subcommand)]
    pub command: TaskCommands,

    /// Working directory (default: current directory)
    #[arg(long, global = true)]
    pub root: Option<PathBuf>,
}

#[derive(Subcommand, Debug)]
pub enum TaskCommands {
    /// Create a new task
    Add(AddArgs),

    /// List all tasks
    List(ListArgs),

    /// Show unblocked tasks
    Ready(ReadyArgs),

    /// Mark a task as complete
    Close(CloseArgs),

    /// Show a single task by ID
    Show(ShowArgs),
}

/// Arguments for the `task add` command.
#[derive(Parser, Debug)]
pub struct AddArgs {
    /// Task title
    pub title: String,

    /// Priority (1-5, default 3)
    #[arg(short = 'p', long, default_value = "3")]
    pub priority: u8,

    /// Task description
    #[arg(short = 'd', long)]
    pub description: Option<String>,

    /// Task IDs that must complete first (comma-separated)
    #[arg(long)]
    pub blocked_by: Option<String>,

    /// Output format
    #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
    pub format: OutputFormat,
}

/// Arguments for the `task list` command.
#[derive(Parser, Debug)]
pub struct ListArgs {
    /// Filter by status: open, in_progress, closed
    #[arg(short = 's', long)]
    pub status: Option<String>,

    /// Output format
    #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
    pub format: OutputFormat,
}

/// Arguments for the `task ready` command.
#[derive(Parser, Debug)]
pub struct ReadyArgs {
    /// Output format
    #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
    pub format: OutputFormat,
}

/// Arguments for the `task close` command.
#[derive(Parser, Debug)]
pub struct CloseArgs {
    /// Task ID to close
    pub id: String,
}

/// Arguments for the `task show` command.
#[derive(Parser, Debug)]
pub struct ShowArgs {
    /// Task ID
    pub id: String,

    /// Output format
    #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
    pub format: OutputFormat,
}

/// Gets the tasks file path.
fn get_tasks_path(root: Option<&PathBuf>) -> PathBuf {
    let base = root.map(|p| p.as_path()).unwrap_or(Path::new("."));
    base.join(".agent").join("tasks.jsonl")
}

/// Executes task CLI commands.
pub fn execute(args: TaskArgs) -> Result<()> {
    let root = args.root.clone();

    match args.command {
        TaskCommands::Add(add_args) => execute_add(add_args, root.as_ref()),
        TaskCommands::List(list_args) => execute_list(list_args, root.as_ref()),
        TaskCommands::Ready(ready_args) => execute_ready(ready_args, root.as_ref()),
        TaskCommands::Close(close_args) => execute_close(close_args, root.as_ref()),
        TaskCommands::Show(show_args) => execute_show(show_args, root.as_ref()),
    }
}

fn execute_add(args: AddArgs, root: Option<&PathBuf>) -> Result<()> {
    let path = get_tasks_path(root);
    let mut store = TaskStore::load(&path).context("Failed to load tasks")?;

    let mut task = Task::new(args.title, args.priority);

    if let Some(desc) = args.description {
        task = task.with_description(Some(desc));
    }

    if let Some(blockers) = args.blocked_by {
        for blocker_id in blockers.split(',').map(|s| s.trim()) {
            task = task.with_blocker(blocker_id.to_string());
        }
    }

    let task_id = task.id.clone();
    store.add(task.clone());
    store.save().context("Failed to save tasks")?;

    match args.format {
        OutputFormat::Table => {
            println!("Created task {}", task_id);
            println!("  Title: {}", task.title);
            println!("  Priority: {}", task.priority);
            if !task.blocked_by.is_empty() {
                println!("  Blocked by: {}", task.blocked_by.join(", "));
            }
        }
        OutputFormat::Json => {
            println!("{}", serde_json::to_string(&task)?);
        }
        OutputFormat::Quiet => {
            println!("{}", task_id);
        }
    }

    Ok(())
}

fn execute_list(args: ListArgs, root: Option<&PathBuf>) -> Result<()> {
    let path = get_tasks_path(root);
    let store = TaskStore::load(&path).context("Failed to load tasks")?;

    let tasks: Vec<_> = if let Some(status_str) = args.status {
        store
            .all()
            .iter()
            .filter(|t| format!("{:?}", t.status).to_lowercase() == status_str.to_lowercase())
            .collect()
    } else {
        store.all().iter().collect()
    };

    match args.format {
        OutputFormat::Table => {
            if tasks.is_empty() {
                println!("No tasks found");
            } else {
                println!(
                    "{:<20} {:<15} {:<8} {:<30}",
                    "ID", "Status", "Priority", "Title"
                );
                println!("{}", "-".repeat(80));
                for task in &tasks {
                    let status_str = match task.status {
                        TaskStatus::Open => "open",
                        TaskStatus::InProgress => "in_progress",
                        TaskStatus::Closed => "closed",
                    };
                    let title_truncated = if task.title.len() > 30 {
                        format!("{}...", &task.title[..27])
                    } else {
                        task.title.clone()
                    };
                    println!(
                        "{:<20} {:<15} {:<8} {:<30}",
                        task.id, status_str, task.priority, title_truncated
                    );
                }
            }
        }
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&tasks)?);
        }
        OutputFormat::Quiet => {
            for task in &tasks {
                println!("{}", task.id);
            }
        }
    }

    Ok(())
}

fn execute_ready(args: ReadyArgs, root: Option<&PathBuf>) -> Result<()> {
    let path = get_tasks_path(root);
    let store = TaskStore::load(&path).context("Failed to load tasks")?;

    let ready = store.ready();

    match args.format {
        OutputFormat::Table => {
            if ready.is_empty() {
                println!("No ready tasks");
            } else {
                println!("{:<20} {:<8} {:<40}", "ID", "Priority", "Title");
                println!("{}", "-".repeat(70));
                for task in &ready {
                    let title_truncated = if task.title.len() > 40 {
                        format!("{}...", &task.title[..37])
                    } else {
                        task.title.clone()
                    };
                    println!(
                        "{:<20} {:<8} {:<40}",
                        task.id, task.priority, title_truncated
                    );
                }
            }
        }
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&ready)?);
        }
        OutputFormat::Quiet => {
            for task in &ready {
                println!("{}", task.id);
            }
        }
    }

    Ok(())
}

fn execute_close(args: CloseArgs, root: Option<&PathBuf>) -> Result<()> {
    let path = get_tasks_path(root);
    let mut store = TaskStore::load(&path).context("Failed to load tasks")?;

    let task_id = args.id.clone();
    let title = store
        .close(&task_id)
        .context(format!("Task {} not found", task_id))?
        .title
        .clone();

    store.save().context("Failed to save tasks")?;

    println!("Closed task: {} - {}", task_id, title);

    Ok(())
}

fn execute_show(args: ShowArgs, root: Option<&PathBuf>) -> Result<()> {
    let path = get_tasks_path(root);
    let store = TaskStore::load(&path).context("Failed to load tasks")?;

    let task = store
        .get(&args.id)
        .context(format!("Task {} not found", args.id))?;

    match args.format {
        OutputFormat::Table => {
            let status_str = match task.status {
                TaskStatus::Open => "open",
                TaskStatus::InProgress => "in_progress",
                TaskStatus::Closed => "closed",
            };
            println!("ID:          {}", task.id);
            println!("Title:       {}", task.title);
            if let Some(desc) = &task.description {
                println!("Description: {}", desc);
            }
            println!("Status:      {}", status_str);
            println!("Priority:    {}", task.priority);
            if !task.blocked_by.is_empty() {
                println!("Blocked by:  {}", task.blocked_by.join(", "));
            }
            println!("Created:     {}", task.created);
            if let Some(closed) = &task.closed {
                println!("Closed:      {}", closed);
            }
        }
        OutputFormat::Json => {
            println!("{}", serde_json::to_string_pretty(&task)?);
        }
        OutputFormat::Quiet => {
            println!("{}", task.id);
        }
    }

    Ok(())
}
