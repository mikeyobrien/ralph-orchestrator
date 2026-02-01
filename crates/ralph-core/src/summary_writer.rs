//! Summary file generation for loop termination.
//!
//! Per spec: "On termination, the orchestrator writes `.ralph/agent/summary.md`"
//! with status, iterations, duration, task list, events summary, and commit info.

use crate::event_logger::EventHistory;
use crate::event_loop::{LoopState, TerminationReason};
use crate::landing::LandingResult;
use crate::loop_context::LoopContext;
use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

/// Writes the loop summary file on termination.
///
/// Per spec section "Exit Summary":
/// ```markdown
/// # Loop Summary
///
/// **Status:** Completed successfully
/// **Iterations:** 12
/// **Duration:** 23m 45s
///
/// ## Tasks
/// - [x] Add refresh token support
/// - [x] Update login endpoint
/// - [~] Add rate limiting (cancelled: out of scope)
///
/// ## Events
/// - 12 total events
/// - 6 build.task
/// - 5 build.done
/// - 1 build.blocked
///
/// ## Final Commit
/// abc1234: feat(auth): complete auth overhaul
/// ```
#[derive(Debug)]
pub struct SummaryWriter {
    path: PathBuf,
    /// Path to the events file for reading history.
    /// If None, uses the default path relative to current directory.
    events_path: Option<PathBuf>,
}

impl Default for SummaryWriter {
    fn default() -> Self {
        Self::new(".ralph/agent/summary.md")
    }
}

impl SummaryWriter {
    /// Creates a new summary writer with the given path.
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            events_path: None,
        }
    }

    /// Creates a summary writer using paths from a LoopContext.
    ///
    /// This ensures the writer outputs to the correct location and reads
    /// events from the correct events file when running in a worktree
    /// or other isolated workspace.
    pub fn from_context(context: &LoopContext) -> Self {
        Self {
            path: context.summary_path(),
            events_path: Some(context.events_path()),
        }
    }

    /// Writes the summary file based on loop state and termination reason.
    ///
    /// This is called by the orchestrator when the loop terminates.
    pub fn write(
        &self,
        reason: &TerminationReason,
        state: &LoopState,
        scratchpad_path: Option<&Path>,
        final_commit: Option<&str>,
    ) -> io::Result<()> {
        self.write_with_landing(reason, state, scratchpad_path, final_commit, None)
    }

    /// Writes the summary file with optional landing information.
    ///
    /// This is called by the orchestrator when the loop terminates with landing.
    pub fn write_with_landing(
        &self,
        reason: &TerminationReason,
        state: &LoopState,
        scratchpad_path: Option<&Path>,
        final_commit: Option<&str>,
        landing: Option<&LandingResult>,
    ) -> io::Result<()> {
        // Ensure parent directory exists
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }

        let content = self.generate_content_with_landing(
            reason,
            state,
            scratchpad_path,
            final_commit,
            landing,
        );
        fs::write(&self.path, content)
    }

    /// Generates the markdown content for the summary with optional landing info.
    fn generate_content_with_landing(
        &self,
        reason: &TerminationReason,
        state: &LoopState,
        scratchpad_path: Option<&Path>,
        final_commit: Option<&str>,
        landing: Option<&LandingResult>,
    ) -> String {
        let mut content = String::new();

        // Header
        content.push_str("# Loop Summary\n\n");

        // Status
        let status = self.status_text(reason);
        content.push_str(&format!("**Status:** {status}\n"));
        content.push_str(&format!("**Iterations:** {}\n", state.iteration));
        content.push_str(&format!(
            "**Duration:** {}\n",
            format_duration(state.elapsed())
        ));

        // Cost (if tracked)
        if state.cumulative_cost > 0.0 {
            content.push_str(&format!("**Est. cost:** ${:.2}\n", state.cumulative_cost));
        }

        // Tasks section (read from scratchpad if available)
        content.push('\n');
        content.push_str("## Tasks\n\n");
        if let Some(tasks) = scratchpad_path.and_then(|p| self.extract_tasks(p)) {
            content.push_str(&tasks);
        } else {
            content.push_str("_No scratchpad found._\n");
        }

        // Events section
        content.push('\n');
        content.push_str("## Events\n\n");
        content.push_str(&self.summarize_events());

        // Final commit section
        if let Some(commit) = final_commit {
            content.push('\n');
            content.push_str("## Final Commit\n\n");
            content.push_str(commit);
            content.push('\n');
        }

        // Landing section (if landing was performed)
        if let Some(landing_result) = landing {
            content.push('\n');
            content.push_str("## Landing\n\n");

            if landing_result.committed {
                content.push_str(&format!(
                    "- **Auto-committed:** Yes ({})\n",
                    landing_result.commit_sha.as_deref().unwrap_or("unknown")
                ));
            } else {
                content.push_str("- **Auto-committed:** No (working tree was clean)\n");
            }

            content.push_str(&format!(
                "- **Handoff:** `{}`\n",
                landing_result.handoff_path.display()
            ));

            if !landing_result.open_tasks.is_empty() {
                content.push_str(&format!(
                    "- **Open tasks:** {}\n",
                    landing_result.open_tasks.len()
                ));
            }

            if landing_result.stashes_cleared > 0 {
                content.push_str(&format!(
                    "- **Stashes cleared:** {}\n",
                    landing_result.stashes_cleared
                ));
            }

            content.push_str(&format!(
                "- **Working tree clean:** {}\n",
                if landing_result.working_tree_clean {
                    "Yes"
                } else {
                    "No"
                }
            ));
        }

        content
    }

    /// Returns a human-readable status based on termination reason.
    fn status_text(&self, reason: &TerminationReason) -> &'static str {
        match reason {
            TerminationReason::CompletionPromise => "Completed successfully",
            TerminationReason::MaxIterations => "Stopped: max iterations reached",
            TerminationReason::MaxRuntime => "Stopped: max runtime exceeded",
            TerminationReason::MaxCost => "Stopped: max cost exceeded",
            TerminationReason::ConsecutiveFailures => "Failed: too many consecutive failures",
            TerminationReason::LoopThrashing => "Failed: loop thrashing detected",
            TerminationReason::ValidationFailure => "Failed: too many malformed JSONL events",
            TerminationReason::Stopped => "Stopped manually",
            TerminationReason::Interrupted => "Interrupted by signal",
            TerminationReason::RestartRequested => "Restarting by human request",
        }
    }

    /// Extracts task lines from the scratchpad file.
    ///
    /// Looks for lines matching `- [ ]`, `- [x]`, or `- [~]` patterns.
    fn extract_tasks(&self, scratchpad_path: &Path) -> Option<String> {
        let content = fs::read_to_string(scratchpad_path).ok()?;
        let mut tasks = String::new();

        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with("- [ ]")
                || trimmed.starts_with("- [x]")
                || trimmed.starts_with("- [~]")
            {
                tasks.push_str(trimmed);
                tasks.push('\n');
            }
        }

        if tasks.is_empty() { None } else { Some(tasks) }
    }

    /// Summarizes events from the event history file.
    fn summarize_events(&self) -> String {
        let history = match &self.events_path {
            Some(path) => EventHistory::new(path),
            None => EventHistory::default_path(),
        };

        let records = match history.read_all() {
            Ok(r) => r,
            Err(_) => return "_No event history found._\n".to_string(),
        };

        if records.is_empty() {
            return "_No events recorded._\n".to_string();
        }

        // Count events by topic
        let mut topic_counts: HashMap<String, usize> = HashMap::new();
        for record in &records {
            *topic_counts.entry(record.topic.clone()).or_insert(0) += 1;
        }

        let mut summary = format!("- {} total events\n", records.len());

        // Sort by count descending for consistent output
        let mut sorted: Vec<_> = topic_counts.into_iter().collect();
        sorted.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));

        for (topic, count) in sorted {
            summary.push_str(&format!("- {} {}\n", count, topic));
        }

        summary
    }
}

/// Formats a duration as human-readable string (e.g., "23m 45s" or "1h 5m 30s").
fn format_duration(d: Duration) -> String {
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;
    use tempfile::TempDir;

    fn test_state() -> LoopState {
        LoopState {
            iteration: 12,
            consecutive_failures: 0,
            cumulative_cost: 1.50,
            started_at: Instant::now(),
            last_hat: None,
            consecutive_blocked: 0,
            last_blocked_hat: None,
            task_block_counts: std::collections::HashMap::new(),
            abandoned_tasks: Vec::new(),
            abandoned_task_redispatches: 0,
            consecutive_malformed_events: 0,
            completion_requested: false,
            hat_activation_counts: std::collections::HashMap::new(),
            exhausted_hats: std::collections::HashSet::new(),
            last_checkin_at: None,
        }
    }

    #[test]
    fn test_status_text() {
        let writer = SummaryWriter::default();

        assert_eq!(
            writer.status_text(&TerminationReason::CompletionPromise),
            "Completed successfully"
        );
        assert_eq!(
            writer.status_text(&TerminationReason::MaxIterations),
            "Stopped: max iterations reached"
        );
        assert_eq!(
            writer.status_text(&TerminationReason::ConsecutiveFailures),
            "Failed: too many consecutive failures"
        );
        assert_eq!(
            writer.status_text(&TerminationReason::Interrupted),
            "Interrupted by signal"
        );
    }

    #[test]
    fn test_format_duration() {
        assert_eq!(format_duration(Duration::from_secs(45)), "45s");
        assert_eq!(format_duration(Duration::from_secs(125)), "2m 5s");
        assert_eq!(format_duration(Duration::from_secs(3725)), "1h 2m 5s");
    }

    #[test]
    fn test_extract_tasks() {
        let tmp = TempDir::new().unwrap();
        let scratchpad = tmp.path().join("scratchpad.md");

        let content = r"# Tasks

Some intro text.

- [x] Implement feature A
- [ ] Implement feature B
- [~] Feature C (cancelled: not needed)

## Notes

More text here.
";
        fs::write(&scratchpad, content).unwrap();

        let writer = SummaryWriter::default();
        let tasks = writer.extract_tasks(&scratchpad).unwrap();

        assert!(tasks.contains("- [x] Implement feature A"));
        assert!(tasks.contains("- [ ] Implement feature B"));
        assert!(tasks.contains("- [~] Feature C"));
    }

    #[test]
    fn test_generate_content_basic() {
        let writer = SummaryWriter::default();
        let state = test_state();

        let content = writer.generate_content_with_landing(
            &TerminationReason::CompletionPromise,
            &state,
            None,
            Some("abc1234: feat(auth): add tokens"),
            None,
        );

        assert!(content.contains("# Loop Summary"));
        assert!(content.contains("**Status:** Completed successfully"));
        assert!(content.contains("**Iterations:** 12"));
        assert!(content.contains("**Est. cost:** $1.50"));
        assert!(content.contains("## Tasks"));
        assert!(content.contains("## Events"));
        assert!(content.contains("## Final Commit"));
        assert!(content.contains("abc1234: feat(auth): add tokens"));
    }

    #[test]
    fn test_write_creates_directory() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("nested/dir/summary.md");

        let writer = SummaryWriter::new(&path);
        let state = test_state();

        writer
            .write(&TerminationReason::CompletionPromise, &state, None, None)
            .unwrap();

        assert!(path.exists());
        let content = fs::read_to_string(path).unwrap();
        assert!(content.contains("# Loop Summary"));
    }

    #[test]
    fn test_write_with_landing() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("summary.md");

        let writer = SummaryWriter::new(&path);
        let state = test_state();

        let landing = LandingResult {
            committed: true,
            commit_sha: Some("abc1234".to_string()),
            handoff_path: tmp.path().join("handoff.md"),
            open_tasks: vec!["task-1".to_string(), "task-2".to_string()],
            stashes_cleared: 2,
            working_tree_clean: true,
        };

        writer
            .write_with_landing(
                &TerminationReason::CompletionPromise,
                &state,
                None,
                None,
                Some(&landing),
            )
            .unwrap();

        let content = fs::read_to_string(path).unwrap();
        assert!(content.contains("## Landing"));
        assert!(content.contains("**Auto-committed:** Yes (abc1234)"));
        assert!(content.contains("**Handoff:**"));
        assert!(content.contains("**Open tasks:** 2"));
        assert!(content.contains("**Stashes cleared:** 2"));
        assert!(content.contains("**Working tree clean:** Yes"));
    }
}
