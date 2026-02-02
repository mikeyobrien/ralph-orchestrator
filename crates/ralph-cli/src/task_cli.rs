//! CLI commands for the `ralph task` namespace.
//!
//! Provides subcommands for managing tasks:
//! - `add`: Create a new task
//! - `list`: List all tasks
//! - `ready`: Show unblocked tasks
//! - `close`: Mark a task as complete
//! - `show`: Show a single task by ID

use crate::display::colors;
use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
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

    /// Mark a task as failed
    Fail(FailArgs),

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
    /// Filter by status: open, in_progress, closed, failed
    #[arg(short = 's', long)]
    pub status: Option<String>,

    /// Show only tasks from the last N days
    #[arg(long, short = 'd')]
    pub days: Option<i64>,

    /// Limit the number of tasks displayed
    #[arg(long, short = 'l')]
    pub limit: Option<usize>,

    /// Show all tasks including closed and failed (hidden by default)
    #[arg(long, short = 'a')]
    pub all: bool,

    /// Output format
    #[arg(long, value_enum, default_value_t = OutputFormat::Table)]
    pub format: OutputFormat,
}

/// Arguments for the `task ready` command.
#[derive(Parser, Debug)]
pub struct ReadyArgs {
    /// Show tasks from all loops, not just the current one
    #[arg(long, short = 'a')]
    pub all: bool,

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

/// Arguments for the `task fail` command.
#[derive(Parser, Debug)]
pub struct FailArgs {
    /// Task ID to mark as failed
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
    base.join(".ralph").join("agent").join("tasks.jsonl")
}

fn status_matches_filter(status: TaskStatus, filter: &str) -> bool {
    let normalized = filter.to_lowercase().replace(['_', '-'], "");
    match status {
        TaskStatus::Open => normalized == "open",
        TaskStatus::InProgress => normalized == "inprogress",
        TaskStatus::Closed => normalized == "closed",
        TaskStatus::Failed => normalized == "failed",
    }
}

fn filter_tasks_for_list(store: &TaskStore, args: &ListArgs) -> Vec<Task> {
    let mut tasks: Vec<_> = if let Some(status_str) = args.status.as_deref() {
        store
            .all()
            .iter()
            .filter(|t| status_matches_filter(t.status, status_str))
            .cloned()
            .collect()
    } else if args.all {
        store.all().to_vec()
    } else {
        store
            .all()
            .iter()
            .filter(|t| !matches!(t.status, TaskStatus::Closed | TaskStatus::Failed))
            .cloned()
            .collect()
    };

    if let Some(days) = args.days {
        let cutoff = Utc::now() - chrono::Duration::days(days);
        tasks.retain(|t| {
            if DateTime::parse_from_rfc3339(&t.created)
                .map(|c| c.with_timezone(&Utc) > cutoff)
                .unwrap_or(false)
            {
                return true;
            }

            if t.closed.as_ref().is_some_and(|closed_str| {
                DateTime::parse_from_rfc3339(closed_str)
                    .map(|c| c.with_timezone(&Utc) > cutoff)
                    .unwrap_or(false)
            }) {
                return true;
            }
            false
        });
    }

    tasks.sort_by(|a, b| {
        let status_rank = |s: TaskStatus| match s {
            TaskStatus::InProgress => 0,
            TaskStatus::Open => 1,
            TaskStatus::Closed => 2,
            TaskStatus::Failed => 3,
        };

        let rank_a = status_rank(a.status);
        let rank_b = status_rank(b.status);

        if rank_a != rank_b {
            return rank_a.cmp(&rank_b);
        }

        if a.priority != b.priority {
            return a.priority.cmp(&b.priority);
        }

        a.created.cmp(&b.created)
    });

    if let Some(limit) = args.limit {
        tasks.truncate(limit);
    }

    tasks
}

fn filter_tasks_for_ready(
    store: &TaskStore,
    args: &ReadyArgs,
    root: Option<&PathBuf>,
) -> Vec<Task> {
    let mut ready: Vec<Task> = store.ready().into_iter().cloned().collect();

    if !args.all {
        let loop_id_marker = get_tasks_path(root)
            .parent()
            .and_then(|p| p.parent())
            .map(|p| p.join("current-loop-id"));
        if let Some(marker_path) = loop_id_marker
            && let Ok(current_loop_id) = std::fs::read_to_string(&marker_path)
        {
            let current_loop_id = current_loop_id.trim().to_string();
            if !current_loop_id.is_empty() {
                ready.retain(|t| t.loop_id.as_ref() == Some(&current_loop_id));
            }
        }
    }

    ready
}

/// Executes task CLI commands.
pub fn execute(args: TaskArgs, use_colors: bool) -> Result<()> {
    let root = args.root.clone();

    match args.command {
        TaskCommands::Add(add_args) => execute_add(add_args, root.as_ref(), use_colors),
        TaskCommands::List(list_args) => execute_list(list_args, root.as_ref(), use_colors),
        TaskCommands::Ready(ready_args) => execute_ready(ready_args, root.as_ref(), use_colors),
        TaskCommands::Close(close_args) => execute_close(close_args, root.as_ref(), use_colors),
        TaskCommands::Fail(fail_args) => execute_fail(fail_args, root.as_ref(), use_colors),
        TaskCommands::Show(show_args) => execute_show(show_args, root.as_ref(), use_colors),
    }
}

fn execute_add(args: AddArgs, root: Option<&PathBuf>, use_colors: bool) -> Result<()> {
    let path = get_tasks_path(root);
    let mut store = TaskStore::load(&path).context("Failed to load tasks")?;

    let mut task = Task::new(args.title, args.priority);

    // Auto-tag with loop ID from marker file if available
    let loop_id_marker = get_tasks_path(root)
        .parent()
        .and_then(|p| p.parent()) // .ralph/agent -> .ralph
        .map(|p| p.join("current-loop-id"));
    if let Some(marker_path) = loop_id_marker
        && let Ok(loop_id) = std::fs::read_to_string(&marker_path)
    {
        let loop_id = loop_id.trim().to_string();
        if !loop_id.is_empty() {
            task = task.with_loop_id(Some(loop_id));
        }
    }

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
            if use_colors {
                println!("{}Created task {}{}", colors::GREEN, task_id, colors::RESET);
            } else {
                println!("Created task {}", task_id);
            }
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

fn execute_list(args: ListArgs, root: Option<&PathBuf>, use_colors: bool) -> Result<()> {
    let path = get_tasks_path(root);
    let store = TaskStore::load(&path).context("Failed to load tasks")?;

    let tasks = filter_tasks_for_list(&store, &args);

    match args.format {
        OutputFormat::Table => {
            if tasks.is_empty() {
                println!("No tasks found");
            } else {
                if use_colors {
                    println!(
                        "{}{:<20} {:<15} {:<8} {:<60}{}",
                        colors::DIM,
                        "ID",
                        "Status",
                        "Priority",
                        "Title",
                        colors::RESET
                    );
                    println!("{}{}{}", colors::DIM, "-".repeat(106), colors::RESET);
                } else {
                    println!(
                        "{:<20} {:<15} {:<8} {:<60}",
                        "ID", "Status", "Priority", "Title"
                    );
                    println!("{}", "-".repeat(106));
                }

                for task in &tasks {
                    let (status_str, status_color) = match task.status {
                        TaskStatus::Open => ("open", colors::GREEN),
                        TaskStatus::InProgress => ("in_progress", colors::BLUE),
                        TaskStatus::Closed => ("closed", colors::DIM),
                        TaskStatus::Failed => ("failed", colors::RED),
                    };

                    let priority_color = match task.priority {
                        1 => colors::RED,
                        2 => colors::YELLOW,
                        _ => colors::RESET,
                    };

                    let title_truncated = if task.title.len() > 60 {
                        crate::display::truncate(&task.title, 60)
                    } else {
                        task.title.clone()
                    };

                    if use_colors {
                        println!(
                            "{}{:<20}{} {}{:<15}{} {}{:<8}{} {:<60}",
                            colors::DIM,
                            task.id,
                            colors::RESET,
                            status_color,
                            status_str,
                            colors::RESET,
                            priority_color,
                            task.priority,
                            colors::RESET,
                            title_truncated
                        );
                    } else {
                        println!(
                            "{:<20} {:<15} {:<8} {:<60}",
                            task.id, status_str, task.priority, title_truncated
                        );
                    }
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

fn execute_ready(args: ReadyArgs, root: Option<&PathBuf>, use_colors: bool) -> Result<()> {
    let path = get_tasks_path(root);
    let store = TaskStore::load(&path).context("Failed to load tasks")?;

    let ready = filter_tasks_for_ready(&store, &args, root);

    match args.format {
        OutputFormat::Table => {
            if ready.is_empty() {
                println!("No ready tasks");
            } else {
                if use_colors {
                    println!(
                        "{}{:<20} {:<8} {:<60}{}",
                        colors::DIM,
                        "ID",
                        "Priority",
                        "Title",
                        colors::RESET
                    );
                    println!("{}{}{}", colors::DIM, "-".repeat(90), colors::RESET);
                } else {
                    println!("{:<20} {:<8} {:<60}", "ID", "Priority", "Title");
                    println!("{}", "-".repeat(90));
                }

                for task in &ready {
                    let title_truncated = if task.title.len() > 60 {
                        crate::display::truncate(&task.title, 60)
                    } else {
                        task.title.clone()
                    };

                    let priority_color = match task.priority {
                        1 => colors::RED,
                        2 => colors::YELLOW,
                        _ => colors::RESET,
                    };

                    if use_colors {
                        println!(
                            "{}{:<20}{} {}{:<8}{} {:<60}",
                            colors::DIM,
                            task.id,
                            colors::RESET,
                            priority_color,
                            task.priority,
                            colors::RESET,
                            title_truncated
                        );
                    } else {
                        println!(
                            "{:<20} {:<8} {:<60}",
                            task.id, task.priority, title_truncated
                        );
                    }
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

fn execute_close(args: CloseArgs, root: Option<&PathBuf>, use_colors: bool) -> Result<()> {
    let path = get_tasks_path(root);
    let mut store = TaskStore::load(&path).context("Failed to load tasks")?;

    let task_id = args.id.clone();
    let title = store
        .close(&task_id)
        .context(format!("Task {} not found", task_id))?
        .title
        .clone();

    store.save().context("Failed to save tasks")?;

    if use_colors {
        println!(
            "{}Closed task: {} - {}{}",
            colors::GREEN,
            task_id,
            title,
            colors::RESET
        );
    } else {
        println!("Closed task: {} - {}", task_id, title);
    }

    Ok(())
}

fn execute_fail(args: FailArgs, root: Option<&PathBuf>, use_colors: bool) -> Result<()> {
    let path = get_tasks_path(root);
    let mut store = TaskStore::load(&path).context("Failed to load tasks")?;

    let task_id = args.id.clone();
    let title = store
        .fail(&task_id)
        .context(format!("Task {} not found", task_id))?
        .title
        .clone();

    store.save().context("Failed to save tasks")?;

    if use_colors {
        println!(
            "{}Failed task: {} - {}{}",
            colors::RED,
            task_id,
            title,
            colors::RESET
        );
    } else {
        println!("Failed task: {} - {}", task_id, title);
    }

    Ok(())
}

fn execute_show(args: ShowArgs, root: Option<&PathBuf>, use_colors: bool) -> Result<()> {
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
                TaskStatus::Failed => "failed",
            };

            if use_colors {
                let status_color = match task.status {
                    TaskStatus::Open => colors::GREEN,
                    TaskStatus::InProgress => colors::BLUE,
                    TaskStatus::Closed => colors::DIM,
                    TaskStatus::Failed => colors::RED,
                };
                let priority_color = match task.priority {
                    1 => colors::RED,
                    2 => colors::YELLOW,
                    _ => colors::RESET,
                };

                println!("{}ID:          {}{}", colors::DIM, task.id, colors::RESET);
                println!("Title:       {}", task.title);
                if let Some(desc) = &task.description {
                    println!("Description: {}", desc);
                }
                println!(
                    "Status:      {}{}{}",
                    status_color,
                    status_str,
                    colors::RESET
                );
                println!(
                    "Priority:    {}{}{}",
                    priority_color,
                    task.priority,
                    colors::RESET
                );
                if !task.blocked_by.is_empty() {
                    println!("Blocked by:  {}", task.blocked_by.join(", "));
                }
                println!("Created:     {}", task.created);
                if let Some(closed) = &task.closed {
                    println!("Closed:      {}", closed);
                }
            } else {
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn write_tasks(root: &Path, tasks: Vec<Task>) -> TaskStore {
        let root_buf = root.to_path_buf();
        let path = get_tasks_path(Some(&root_buf));
        let mut store = TaskStore::load(&path).expect("load task store");
        for task in tasks {
            store.add(task);
        }
        store.save().expect("save task store");
        TaskStore::load(&path).expect("reload task store")
    }

    #[test]
    fn test_list_status_filter_accepts_in_progress() {
        let temp_dir = TempDir::new().expect("temp dir");
        let mut open_task = Task::new("Open".to_string(), 2);
        open_task.status = TaskStatus::Open;
        let mut in_progress = Task::new("In progress".to_string(), 2);
        in_progress.status = TaskStatus::InProgress;

        let store = write_tasks(temp_dir.path(), vec![open_task, in_progress]);

        let args = ListArgs {
            status: Some("in_progress".to_string()),
            days: None,
            limit: None,
            all: true,
            format: OutputFormat::Quiet,
        };

        let filtered = filter_tasks_for_list(&store, &args);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].status, TaskStatus::InProgress);
    }

    #[test]
    fn test_ready_filters_by_loop_id_marker() {
        let temp_dir = TempDir::new().expect("temp dir");
        let root = temp_dir.path().to_path_buf();

        let mut task_loop_a = Task::new("Loop A task".to_string(), 1);
        task_loop_a.loop_id = Some("loop-a".to_string());
        let mut task_loop_b = Task::new("Loop B task".to_string(), 1);
        task_loop_b.loop_id = Some("loop-b".to_string());

        let store = write_tasks(temp_dir.path(), vec![task_loop_a, task_loop_b]);

        let marker_dir = root.join(".ralph");
        std::fs::create_dir_all(&marker_dir).expect("marker dir");
        std::fs::write(marker_dir.join("current-loop-id"), "loop-a").expect("write marker");

        let args = ReadyArgs {
            all: false,
            format: OutputFormat::Quiet,
        };

        let ready = filter_tasks_for_ready(&store, &args, Some(&root));
        assert_eq!(ready.len(), 1);
        assert_eq!(ready[0].loop_id.as_deref(), Some("loop-a"));
    }
}
