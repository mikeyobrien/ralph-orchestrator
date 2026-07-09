//! Display functions for terminal output.
//!
//! This module contains functions for formatting and printing
//! termination messages, event tables, and other terminal UI elements.

use ralph_core::{EventRecord, TerminationReason, floor_char_boundary, truncate_with_ellipsis};

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

/// Truncates a string to max_len characters, adding ellipsis if truncated.
pub fn truncate(s: &str, max_len: usize) -> String {
    truncate_with_ellipsis(s, max_len)
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
    use colors::*;

    // Determine status color and message based on termination reason
    let (color, icon, label) = match reason {
        TerminationReason::CompletionPromise => (GREEN, "?", "Completion promise detected"),
        TerminationReason::MaxIterations => (YELLOW, "?", "Maximum iterations reached"),
        TerminationReason::MaxRuntime => (YELLOW, "?", "Maximum runtime exceeded"),
        TerminationReason::MaxCost => (YELLOW, "?", "Maximum cost exceeded"),
        TerminationReason::ConsecutiveFailures => (RED, "?", "Too many consecutive failures"),
        TerminationReason::LoopThrashing => (RED, "?", "Loop thrashing detected"),
        TerminationReason::LoopStale => (RED, "?", "Stale loop detected"),
        TerminationReason::ValidationFailure => (RED, "?", "Too many malformed JSONL events"),
        TerminationReason::Stopped => (CYAN, "?", "Manually stopped"),
        TerminationReason::Interrupted => (YELLOW, "?", "Interrupted by signal"),
        TerminationReason::RestartRequested => (CYAN, "↻", "Restarting by human request"),
        TerminationReason::WorkspaceGone => (RED, "?", "Workspace directory removed"),
        TerminationReason::Cancelled => (CYAN, "⏹", "Cancelled gracefully"),
    };

    let separator = "-".repeat(58);

    if use_colors {
        println!("\n{BOLD}+{separator}+{RESET}");
        println!(
            "{BOLD}|{RESET} {color}{BOLD}{icon}{RESET} Loop terminated: {color}{label}{RESET}"
        );
        println!("{BOLD}+{separator}+{RESET}");
        println!(
            "{BOLD}|{RESET}   Iterations:  {CYAN}{}{RESET}",
            state.iterations
        );
        println!(
            "{BOLD}|{RESET}   Elapsed:     {CYAN}{:.1}s{RESET}",
            state.elapsed.as_secs_f64()
        );
        if state.cost_usd > 0.0 {
            println!(
                "{BOLD}|{RESET}   Est. cost:   {CYAN}${:.2}{RESET}",
                state.cost_usd
            );
        }
        println!("{BOLD}+{separator}+{RESET}");
    } else {
        println!("\n+{}+", "-".repeat(58));
        println!("| {icon} Loop terminated: {label}");
        println!("+{}+", "-".repeat(58));
        println!("|   Iterations:  {}", state.iterations);
        println!("|   Elapsed:     {:.1}s", state.elapsed.as_secs_f64());
        if state.cost_usd > 0.0 {
            println!("|   Est. cost:   ${:.2}", state.cost_usd);
        }
        println!("+{}+", "-".repeat(58));
    }

    // Resume hint: only for recoverable reasons and when we know the loop id.
    if let Some(id) = loop_id
        && let Some(cmd) = resume_hint_for(reason, id)
    {
        if use_colors {
            println!("  {DIM}Resume:{RESET} {CYAN}{cmd}{RESET}");
        } else {
            println!("  Resume: {cmd}");
        }
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
    use colors::*;

    // Header
    if use_colors {
        println!(
            "{BOLD}{DIM}  # | Time     | Iteration | Hat           | Topic              | Triggered      | Payload{RESET}"
        );
        println!(
            "{DIM}----+----------+-----------+---------------+--------------------+----------------+-----------------{RESET}"
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
        let payload_one_line = record.payload.replace('\n', " ");
        let payload_preview = truncate_with_ellipsis(&payload_one_line, 40);

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
                // Take only HH:MM:SS (usually ASCII), but still ensure we slice on a valid UTF-8
                // boundary for robustness. Otherwise, an unexpected `ts` (e.g. CJK/emoji) can make
                // `&s[..N]` panic.
                let boundary = floor_char_boundary(time_str, 8);
                Some(&time_str[..boundary])
            })
            .unwrap_or("-");

        if use_colors {
            println!(
                "{DIM}{:>3}{RESET} | {:<8} | {:>9} | {:<13} | {topic_color}{:<18}{RESET} | {:<14} | {DIM}{}{RESET}",
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
    fn test_truncate_short_string() {
        assert_eq!(truncate("hello", 10), "hello");
    }

    #[test]
    fn test_truncate_long_string() {
        assert_eq!(truncate("hello world", 8), "hello...");
    }

    #[test]
    fn test_truncate_does_not_panic_on_multibyte_chars() {
        // Let a multi-byte character straddle the truncation boundary. The old implementation
        // would panic because `&s[..N]` was not on a UTF-8 boundary.
        let s = format!("{}✅{}", "x".repeat(39), "y".repeat(10));

        let out = truncate(&s, 40);

        // Verify output is valid UTF-8 (iterating `chars()` should not panic).
        for _ in out.chars() {}
        assert!(out.ends_with("..."));
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
