//! Display functions for terminal output.
//!
//! This module contains functions for formatting and printing
//! termination messages, event tables, and other terminal UI elements.

use ralph_core::{EventRecord, TerminationReason, truncate_with_ellipsis};

/// ANSI color codes for terminal output.
pub mod colors {
    pub const RESET: &str = "\x1b[0m";
    pub const BOLD: &str = "\x1b[1m";
    pub const DIM: &str = "\x1b[2m";
    pub const GREEN: &str = "\x1b[32m";
    pub const YELLOW: &str = "\x1b[33m";
    pub const RED: &str = "\x1b[31m";
    pub const CYAN: &str = "\x1b[36m";
    pub const BLUE: &str = "\x1b[34m";
    pub const MAGENTA: &str = "\x1b[35m";
    pub const GRAY: &str = "\x1b[90m";
}

/// Color palette that returns either real ANSI codes or empty strings.
///
/// Eliminates `if use_colors { … } else { … }` branches at call sites.
/// Use `Palette::new(use_colors)` and then reference fields like `p.green`, `p.reset`, etc.
#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
pub struct Palette {
    pub reset: &'static str,
    pub bold: &'static str,
    pub dim: &'static str,
    pub green: &'static str,
    pub yellow: &'static str,
    pub red: &'static str,
    pub cyan: &'static str,
    pub blue: &'static str,
    pub magenta: &'static str,
    pub gray: &'static str,
}

impl Palette {
    pub fn new(use_colors: bool) -> Self {
        if use_colors {
            Self {
                reset: colors::RESET,
                bold: colors::BOLD,
                dim: colors::DIM,
                green: colors::GREEN,
                yellow: colors::YELLOW,
                red: colors::RED,
                cyan: colors::CYAN,
                blue: colors::BLUE,
                magenta: colors::MAGENTA,
                gray: colors::GRAY,
            }
        } else {
            Self {
                reset: "",
                bold: "",
                dim: "",
                green: "",
                yellow: "",
                red: "",
                cyan: "",
                blue: "",
                magenta: "",
                gray: "",
            }
        }
    }
}

/// Returns the resume command hint for a given termination reason, if the
/// reason is recoverable by re-running with `--continue`.
///
/// Returns `None` when:
/// - `CompletionPromise`: loop succeeded, nothing to resume.
/// - `WorkspaceGone`: workspace removed, `--continue` would fail.
/// - `Cancelled`: explicit human cancellation — resume hint would be misleading.
/// - `RestartRequested`: `main` already auto-restarts the loop — hint is redundant.
fn resume_hint_for(reason: &TerminationReason, loop_id: &str) -> Option<String> {
    match reason {
        TerminationReason::CompletionPromise
        | TerminationReason::WorkspaceGone
        | TerminationReason::Cancelled
        | TerminationReason::RestartRequested => None,
        _ => Some(format!("ralph run --continue --loop-id {loop_id}")),
    }
}


/// Prints termination message with status.
///
/// When `loop_id` is provided, also prints a `Resume:` hint line for
/// recoverable termination reasons (budget exhausted, thrashing, interrupt,
/// etc.) so an agent or user running `--no-tui` can recover without hunting
/// through the scrollback.
pub fn print_termination(
    reason: &TerminationReason,
    state: &ralph_core::RunStats,
    use_colors: bool,
    loop_id: Option<&str>,
) {
    let p = Palette::new(use_colors);
    let (b, r, d, c) = (p.bold, p.reset, p.dim, p.cyan);

    let (color, icon, label) = match reason {
        TerminationReason::CompletionPromise => (p.green, "?", "Completion promise detected"),
        TerminationReason::MaxIterations => (p.yellow, "?", "Maximum iterations reached"),
        TerminationReason::MaxRuntime => (p.yellow, "?", "Maximum runtime exceeded"),
        TerminationReason::MaxCost => (p.yellow, "?", "Maximum cost exceeded"),
        TerminationReason::ConsecutiveFailures => (p.red, "?", "Too many consecutive failures"),
        TerminationReason::LoopThrashing => (p.red, "?", "Loop thrashing detected"),
        TerminationReason::LoopStale => (p.red, "?", "Stale loop detected"),
        TerminationReason::ValidationFailure => (p.red, "?", "Too many malformed JSONL events"),
        TerminationReason::Stopped => (p.cyan, "?", "Manually stopped"),
        TerminationReason::Interrupted => (p.yellow, "?", "Interrupted by signal"),
        TerminationReason::RestartRequested => (p.cyan, "↻", "Restarting by human request"),
        TerminationReason::WorkspaceGone => (p.red, "?", "Workspace directory removed"),
        TerminationReason::Cancelled => (p.cyan, "⏹", "Cancelled gracefully"),
    };

    let separator = "-".repeat(58);

    println!("\n{b}+{separator}+{r}");
    println!("{b}|{r} {color}{b}{icon}{r} Loop terminated: {color}{label}{r}");
    println!("{b}+{separator}+{r}");
    println!("{b}|{r}   Iterations:  {c}{}{r}", state.iterations);
    println!("{b}|{r}   Elapsed:     {c}{:.1}s{r}", state.elapsed.as_secs_f64());
    if state.cost_usd > 0.0 {
        println!("{b}|{r}   Est. cost:   {c}${:.2}{r}", state.cost_usd);
    }
    println!("{b}+{separator}+{r}");

    if let Some(id) = loop_id
        && let Some(cmd) = resume_hint_for(reason, id)
    {
        println!("  {d}Resume:{r} {c}{cmd}{r}");
    }
}

/// Gets the color for a topic based on its prefix.
pub fn get_topic_color(topic: &str) -> &'static str {
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

/// Prints a table of event records.
pub fn print_events_table(records: &[EventRecord], use_colors: bool) {
    let p = Palette::new(use_colors);
    let (b, d, r) = (p.bold, p.dim, p.reset);

    println!(
        "{b}{d}  # | Time     | Iteration | Hat           | Topic              | Triggered      | Payload{r}"
    );
    println!(
        "{d}----+----------+-----------+---------------+--------------------+----------------+-----------------{r}"
    );

    for (i, record) in records.iter().enumerate() {
        let tc = if r.is_empty() { "" } else { get_topic_color(&record.topic) };
        let triggered = record.triggered.as_deref().unwrap_or("-");
        let payload_one_line = record.payload.replace('\n', " ");
        let payload_preview = truncate_with_ellipsis(&payload_one_line, 40);

        let time = record
            .ts
            .find('T')
            .and_then(|t_pos| {
                let after_t = &record.ts[t_pos + 1..];
                let end = after_t
                    .find(|c| c == 'Z' || c == '+' || c == '-')
                    .unwrap_or(after_t.len());
                let time_str = &after_t[..end];
                let boundary = time_str.floor_char_boundary(8);
                Some(&time_str[..boundary])
            })
            .unwrap_or("-");

        println!(
            "{d}{:>3}{r} | {:<8} | {:>9} | {:<13} | {tc}{:<18}{r} | {:<14} | {d}{}{r}",
            i + 1,
            time,
            record.iteration,
            truncate_with_ellipsis(&record.hat, 13),
            truncate_with_ellipsis(&record.topic, 18),
            truncate_with_ellipsis(triggered, 14),
            payload_preview
        );
    }

    println!("\n{d}Total: {} events{r}", records.len());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resume_hint_skipped_for_completion_promise() {
        assert!(resume_hint_for(&TerminationReason::CompletionPromise, "abc").is_none());
    }

    #[test]
    fn test_resume_hint_skipped_for_workspace_gone() {
        assert!(resume_hint_for(&TerminationReason::WorkspaceGone, "abc").is_none());
    }

    #[test]
    fn test_resume_hint_skipped_for_cancelled() {
        // Explicit human cancellation — suggesting --continue would be misleading.
        assert!(resume_hint_for(&TerminationReason::Cancelled, "abc").is_none());
    }

    #[test]
    fn test_resume_hint_skipped_for_restart_requested() {
        // Loop auto-restarts via main; hint would be redundant noise.
        assert!(resume_hint_for(&TerminationReason::RestartRequested, "abc").is_none());
    }

    #[test]
    fn test_resume_hint_present_for_max_iterations() {
        let hint = resume_hint_for(&TerminationReason::MaxIterations, "loop-42").unwrap();
        assert!(hint.contains("--continue"));
        assert!(hint.contains("--loop-id loop-42"));
    }

    #[test]
    fn test_resume_hint_present_for_interrupted() {
        assert!(resume_hint_for(&TerminationReason::Interrupted, "xy").is_some());
    }


    #[test]
    fn test_print_events_table_does_not_panic_on_multibyte_payload() {
        // Trigger the `payload_preview` truncation path (>40 bytes) and place an emoji near the
        // boundary.
        let payload = format!("{}✅{}", "x".repeat(39), "y".repeat(10));
        let record = EventRecord {
            ts: "2026-01-23T00:00:00Z".to_string(),
            iteration: 1,
            hat: "hat".to_string(),
            topic: "task.start".to_string(),
            triggered: None,
            payload,
            blocked_count: None,
            wave_id: None,
            wave_index: None,
            wave_total: None,
        };

        print_events_table(&[record], false);
    }

    #[test]
    fn test_print_events_table_does_not_panic_on_multibyte_ts() {
        // Make a multi-byte character land on the "take the first 8 bytes" boundary. The old
        // implementation would panic because `&time_str[..8]` was not a UTF-8 boundary.
        let record = EventRecord {
            ts: "2026-01-23Txxxxxxx✅Z".to_string(),
            iteration: 1,
            hat: "hat".to_string(),
            topic: "task.start".to_string(),
            triggered: None,
            payload: "ok".to_string(),
            blocked_count: None,
            wave_id: None,
            wave_index: None,
            wave_total: None,
        };

        print_events_table(&[record], false);
    }
}
