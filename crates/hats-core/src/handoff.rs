//! Handoff prompt generation for session continuity.
//!
//! Generates `.hats/agent/handoff.md` on loop completion, containing:
//! - What was completed (closed tasks)
//! - What remains (open tasks with dependencies)
//! - Context (last commit, branch, key files)
//! - Ready-to-paste prompt for next session
//!
//! This enables clean session boundaries and seamless handoffs between
//! Hats loops, supporting the "land the plane" pattern.

use crate::git_ops::{get_commit_summary, get_current_branch, get_head_sha, get_recent_files};
use crate::loop_context::LoopContext;
use crate::task::{Task, TaskStatus};
use crate::task_store::TaskStore;
use std::io;
use std::path::PathBuf;

/// Result of generating a handoff file.
#[derive(Debug, Clone)]
pub struct HandoffResult {
    /// Path to the generated handoff file.
    pub path: PathBuf,

    /// Number of completed tasks mentioned.
    pub completed_tasks: usize,

    /// Number of open tasks mentioned.
    pub open_tasks: usize,

    /// Whether a continuation prompt was included.
    pub has_continuation_prompt: bool,
}

/// Errors that can occur during handoff generation.
#[derive(Debug, thiserror::Error)]
pub enum HandoffError {
    /// IO error writing the handoff file.
    #[error("IO error: {0}")]
    Io(#[from] io::Error),
}

/// Generates handoff files for session continuity.
pub struct HandoffWriter {
    context: LoopContext,
}

impl HandoffWriter {
    /// Creates a new handoff writer for the given loop context.
    pub fn new(context: LoopContext) -> Self {
        Self { context }
    }

    /// Generates the handoff file with session context.
    ///
    /// # Arguments
    ///
    /// * `original_prompt` - The prompt that started this loop
    ///
    /// # Returns
    ///
    /// Information about what was written, or an error if generation failed.
    pub fn write(&self, original_prompt: &str) -> Result<HandoffResult, HandoffError> {
        let path = self.context.handoff_path();

        // Ensure parent directory exists
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let content = self.generate_content(original_prompt);

        // Count tasks for result
        let (completed_tasks, open_tasks) = self.count_tasks();

        std::fs::write(&path, &content)?;

        Ok(HandoffResult {
            path,
            completed_tasks,
            open_tasks,
            has_continuation_prompt: open_tasks > 0,
        })
    }

    /// Generates the handoff markdown content.
    fn generate_content(&self, original_prompt: &str) -> String {
        let mut content = String::new();

        // Header
        content.push_str("# Session Handoff\n\n");
        content.push_str(&format!(
            "_Generated: {}_\n\n",
            chrono::Utc::now().format("%Y-%m-%d %H:%M:%S UTC")
        ));

        // Git context section
        content.push_str("## Git Context\n\n");
        self.write_git_context(&mut content);

        // Tasks section
        content.push_str("\n## Tasks\n\n");
        self.write_tasks_section(&mut content);

        // Key files section
        content.push_str("\n## Key Files\n\n");
        self.write_key_files(&mut content);

        // Continuation prompt section
        content.push_str("\n## Next Session\n\n");
        self.write_continuation_prompt(&mut content, original_prompt);

        content
    }

    /// Writes git context (branch, commit, status).
    fn write_git_context(&self, content: &mut String) {
        let workspace = self.context.workspace();

        // Branch
        match get_current_branch(workspace) {
            Ok(branch) => content.push_str(&format!("- **Branch:** `{}`\n", branch)),
            Err(_) => content.push_str("- **Branch:** _(unknown)_\n"),
        }

        // Commit
        match get_head_sha(workspace) {
            Ok(sha) => {
                let summary = get_commit_summary(workspace).unwrap_or_default();
                if summary.is_empty() {
                    content.push_str(&format!("- **HEAD:** `{}`\n", &sha[..7.min(sha.len())]));
                } else {
                    content.push_str(&format!("- **HEAD:** {}\n", summary));
                }
            }
            Err(_) => content.push_str("- **HEAD:** _(no commits)_\n"),
        }

        // Loop ID if worktree
        if let Some(loop_id) = self.context.loop_id() {
            content.push_str(&format!("- **Loop ID:** `{}`\n", loop_id));
        }
    }

    /// Writes the tasks section with completed and open tasks.
    fn write_tasks_section(&self, content: &mut String) {
        let tasks_path = self.context.tasks_path();
        let store = match TaskStore::load(&tasks_path) {
            Ok(s) => s,
            Err(_) => {
                content.push_str("_No task history available._\n");
                return;
            }
        };

        let tasks = store.all();
        if tasks.is_empty() {
            content.push_str("_No tasks tracked in this session._\n");
            return;
        }

        // Completed tasks
        let completed: Vec<&Task> = tasks
            .iter()
            .filter(|t| t.status == TaskStatus::Closed)
            .collect();

        if !completed.is_empty() {
            content.push_str("### Completed\n\n");
            for task in &completed {
                content.push_str(&format!("- [x] {}\n", task.title));
            }
            content.push('\n');
        }

        // Open tasks (including failed)
        let open: Vec<&Task> = tasks
            .iter()
            .filter(|t| t.status != TaskStatus::Closed)
            .collect();

        if !open.is_empty() {
            content.push_str("### Remaining\n\n");
            for task in &open {
                let status_marker = match task.status {
                    TaskStatus::Failed => "[~]",
                    _ => "[ ]",
                };
                let blocked = if task.blocked_by.is_empty() {
                    String::new()
                } else {
                    format!(" _(blocked by: {})_", task.blocked_by.join(", "))
                };
                content.push_str(&format!("- {} {}{}\n", status_marker, task.title, blocked));
            }
        }
    }

    /// Writes key files that were modified.
    fn write_key_files(&self, content: &mut String) {
        match get_recent_files(self.context.workspace(), 10) {
            Ok(files) if !files.is_empty() => {
                content.push_str("Recently modified:\n\n");
                for file in files {
                    content.push_str(&format!("- `{}`\n", file));
                }
            }
            _ => {
                content.push_str("_No recent file modifications tracked._\n");
            }
        }
    }

    /// Writes the continuation prompt for the next session.
    fn write_continuation_prompt(&self, content: &mut String, original_prompt: &str) {
        let tasks_path = self.context.tasks_path();
        let store = TaskStore::load(&tasks_path).ok();

        let open_tasks: Vec<String> = store
            .as_ref()
            .map(|s| {
                s.all()
                    .iter()
                    .filter(|t| t.status != TaskStatus::Closed)
                    .map(|t| t.title.clone())
                    .collect()
            })
            .unwrap_or_default();

        if open_tasks.is_empty() {
            content.push_str("Session completed successfully. No pending work.\n\n");
            content.push_str("**Original objective:**\n\n");
            content.push_str("```\n");
            content.push_str(&truncate_prompt(original_prompt, 500));
            content.push_str("\n```\n");
        } else {
            content.push_str(
                "The following prompt can be used to continue where this session left off:\n\n",
            );
            content.push_str("```\n");

            // Build continuation prompt
            content.push_str("Continue the previous work. ");
            content.push_str(&format!("Remaining tasks ({}):\n", open_tasks.len()));
            for task in &open_tasks {
                content.push_str(&format!("- {}\n", task));
            }
            content.push_str("\nOriginal objective: ");
            content.push_str(&truncate_prompt(original_prompt, 200));

            content.push_str("\n```\n");
        }
    }

    /// Counts completed and open tasks.
    fn count_tasks(&self) -> (usize, usize) {
        let tasks_path = self.context.tasks_path();
        let store = match TaskStore::load(&tasks_path) {
            Ok(s) => s,
            Err(_) => return (0, 0),
        };

        let completed = store
            .all()
            .iter()
            .filter(|t| t.status == TaskStatus::Closed)
            .count();
        let open = store
            .all()
            .iter()
            .filter(|t| t.status != TaskStatus::Closed)
            .count();

        (completed, open)
    }
}

/// Truncates a prompt to a maximum length, adding ellipsis if needed.
fn truncate_prompt(prompt: &str, max_len: usize) -> String {
    let prompt = prompt.trim();
    if prompt.len() <= max_len {
        prompt.to_string()
    } else {
        format!("{}...", &prompt[..max_len])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn setup_test_context() -> (TempDir, LoopContext) {
        let temp = TempDir::new().unwrap();
        let ctx = LoopContext::primary(temp.path().to_path_buf());
        ctx.ensure_directories().unwrap();
        (temp, ctx)
    }

    #[test]
    fn test_handoff_writer_creates_file() {
        let (_temp, ctx) = setup_test_context();
        let writer = HandoffWriter::new(ctx.clone());

        let result = writer.write("Test prompt").unwrap();

        assert!(result.path.exists());
        assert_eq!(result.path, ctx.handoff_path());
    }

    #[test]
    fn test_handoff_content_has_sections() {
        let (_temp, ctx) = setup_test_context();
        let writer = HandoffWriter::new(ctx.clone());

        writer.write("Test prompt").unwrap();

        let content = fs::read_to_string(ctx.handoff_path()).unwrap();

        assert!(content.contains("# Session Handoff"));
        assert!(content.contains("## Git Context"));
        assert!(content.contains("## Tasks"));
        assert!(content.contains("## Key Files"));
        assert!(content.contains("## Next Session"));
    }

    #[test]
    fn test_handoff_with_no_tasks() {
        let (_temp, ctx) = setup_test_context();
        let writer = HandoffWriter::new(ctx.clone());

        let result = writer.write("Test prompt").unwrap();

        assert_eq!(result.completed_tasks, 0);
        assert_eq!(result.open_tasks, 0);
        assert!(!result.has_continuation_prompt);
    }

    #[test]
    fn test_handoff_with_tasks() {
        let (_temp, ctx) = setup_test_context();

        // Create some tasks
        let mut store = TaskStore::load(&ctx.tasks_path()).unwrap();
        let task1 = crate::task::Task::new("Completed task".to_string(), 1);
        let id1 = task1.id.clone();
        store.add(task1);
        store.close(&id1);

        let task2 = crate::task::Task::new("Open task".to_string(), 2);
        store.add(task2);
        store.save().unwrap();

        let writer = HandoffWriter::new(ctx.clone());
        let result = writer.write("Test prompt").unwrap();

        assert_eq!(result.completed_tasks, 1);
        assert_eq!(result.open_tasks, 1);
        assert!(result.has_continuation_prompt);

        let content = fs::read_to_string(ctx.handoff_path()).unwrap();
        assert!(content.contains("[x] Completed task"));
        assert!(content.contains("[ ] Open task"));
        assert!(content.contains("Remaining tasks"));
    }

    #[test]
    fn test_truncate_prompt_short() {
        let result = truncate_prompt("short prompt", 100);
        assert_eq!(result, "short prompt");
    }

    #[test]
    fn test_truncate_prompt_long() {
        let long_prompt = "a".repeat(200);
        let result = truncate_prompt(&long_prompt, 50);
        assert_eq!(result.len(), 53); // 50 + "..."
        assert!(result.ends_with("..."));
    }
}
