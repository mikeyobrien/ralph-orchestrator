//! Display functions for terminal output.
//!
//! This module contains functions for formatting and printing
//! iteration separators, termination messages, event tables,
//! and other terminal UI elements.

use ralph_core::{EventRecord, TerminationReason, floor_char_boundary, truncate_with_ellipsis};
use ralph_proto::HatId;
use std::collections::HashMap;
use std::path::Path;
use std::time::Duration;

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

/// Returns the emoji for a hat ID.
pub fn hat_emoji(hat_id: &str) -> &'static str {
    match hat_id {
        "planner" => "?",
        "builder" => "?",
        "reviewer" => "?",
        _ => "?",
    }
}

/// Prints a startup banner with loop discovery info for `--no-tui` runs.
///
/// Advertises the loop ID, key state files, and the tail / resume commands
/// so agents tailing the stream can find structured state without reading
/// docs. Kept to a handful of lines so it doesn't dominate scrollback.
///
/// Format:
/// ```text
/// ── ralph loop abc123 · backend=pi · prompt=PROMPT.md · 0/150 ──
///   events:     .ralph/events.jsonl
///   scratchpad: .ralph/agent/scratchpad.md
///   tail:       ralph events --follow --loop-id abc123
///   resume:     ralph run --continue --loop-id abc123
/// ```
#[allow(clippy::too_many_arguments)]
pub fn print_loop_banner(
    loop_id: &str,
    backend: &str,
    prompt_file: &Path,
    events_path: &Path,
    scratchpad_path: &Path,
    max_iterations: u32,
    resumed: bool,
    use_colors: bool,
) {
    use colors::*;

    let prompt_display = prompt_file.display();
    let events_display = events_path.display();
    let scratchpad_display = scratchpad_path.display();
    let mode = if resumed { "resume" } else { "start" };
    let header = format!(
        " ralph loop {} \u{00b7} {} \u{00b7} backend={} \u{00b7} prompt={} \u{00b7} 0/{} ",
        loop_id, mode, backend, prompt_display, max_iterations
    );

    let tail_cmd = format!("ralph events --follow --loop-id {}", loop_id);
    let resume_cmd = format!("ralph run --continue --loop-id {}", loop_id);

    if use_colors {
        println!("\n{BOLD}{CYAN}\u{2500}\u{2500}{header}\u{2500}\u{2500}{RESET}");
        println!("  {DIM}events:    {RESET} {events_display}");
        println!("  {DIM}scratchpad:{RESET} {scratchpad_display}");
        println!("  {DIM}tail:      {RESET} {tail_cmd}");
        println!("  {DIM}resume:    {RESET} {resume_cmd}");
    } else {
        println!("\n--{header}--");
        println!("  events:     {events_display}");
        println!("  scratchpad: {scratchpad_display}");
        println!("  tail:       {tail_cmd}");
        println!("  resume:     {resume_cmd}");
    }
}

/// Prints a one-line summary at the end of each iteration.
///
/// Gives agents tailing the stream budget awareness (elapsed, cost, iteration
/// progress) without having to parse `events.jsonl`. Emitted after the
/// backend finishes each iteration but before the next separator.
///
/// Format:
/// ```text
/// [iter 3/150 done] dur=1m12s total=4m30s cost=+$0.14 (cum $0.42) budget=2%
/// ```
#[allow(clippy::too_many_arguments)]
pub fn print_iteration_footer(
    iteration: u32,
    max_iterations: u32,
    iteration_duration: Duration,
    cumulative_elapsed: Duration,
    iteration_cost: f64,
    cumulative_cost: f64,
    use_colors: bool,
) {
    use colors::*;

    let pct = if max_iterations > 0 {
        (f64::from(iteration) / f64::from(max_iterations)) * 100.0
    } else {
        0.0
    };

    let iter_dur = format_elapsed(iteration_duration);
    let total_dur = format_elapsed(cumulative_elapsed);

    // Cost fragment: only emit when we actually have cost signal, otherwise
    // noise accumulates for free / self-hosted backends.
    let cost_fragment = if iteration_cost > 0.0 || cumulative_cost > 0.0 {
        format!(
            " cost=+${:.2} (cum ${:.2})",
            iteration_cost, cumulative_cost
        )
    } else {
        String::new()
    };

    if use_colors {
        println!(
            "{DIM}[iter {iteration}/{max_iterations} done]{RESET} dur={CYAN}{iter_dur}{RESET} total={CYAN}{total_dur}{RESET}{cost_fragment} budget={CYAN}{pct:.0}%{RESET}"
        );
    } else {
        println!(
            "[iter {iteration}/{max_iterations} done] dur={iter_dur} total={total_dur}{cost_fragment} budget={pct:.0}%"
        );
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

/// Prints the iteration demarcation separator.
///
/// Per spec: "Each iteration must be clearly demarcated in the output so users can
/// visually distinguish where one iteration ends and another begins."
///
/// Format:
/// ```text
/// ===============================================================================
///  ITERATION 3 | ? builder | 2m 15s elapsed | 3/100
/// ===============================================================================
/// ```
pub fn print_iteration_separator(
    iteration: u32,
    hat_id: &str,
    elapsed: Duration,
    max_iterations: u32,
    use_colors: bool,
) {
    use colors::*;

    let emoji = hat_emoji(hat_id);
    let elapsed_str = format_elapsed(elapsed);

    // Build the content line (without box chars for measuring)
    let content = format!(
        " ITERATION {} | {} {} | {} elapsed | {}/{}",
        iteration, emoji, hat_id, elapsed_str, iteration, max_iterations
    );

    // Use fixed width of 79 characters for the box (standard terminal width)
    let box_width = 79;
    let separator = "=".repeat(box_width);

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
pub fn format_elapsed(d: Duration) -> String {
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
    state: &ralph_core::LoopState,
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
            state.iteration
        );
        println!(
            "{BOLD}|{RESET}   Elapsed:     {CYAN}{:.1}s{RESET}",
            state.elapsed().as_secs_f64()
        );
        if state.cumulative_cost > 0.0 {
            println!(
                "{BOLD}|{RESET}   Est. cost:   {CYAN}${:.2}{RESET}",
                state.cumulative_cost
            );
        }
        println!("{BOLD}+{separator}+{RESET}");
    } else {
        println!("\n+{}+", "-".repeat(58));
        println!("| {icon} Loop terminated: {label}");
        println!("+{}+", "-".repeat(58));
        println!("|   Iterations:  {}", state.iteration);
        println!("|   Elapsed:     {:.1}s", state.elapsed().as_secs_f64());
        if state.cumulative_cost > 0.0 {
            println!("|   Est. cost:   ${:.2}", state.cumulative_cost);
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

/// Prints the wave header separator when a wave is detected.
///
/// Format:
/// ```text
/// ── WAVE: 🔍 Reviewer | 3 workers | timeout 600s ─────────────────────────────
/// ```
pub fn print_wave_header(hat_name: &str, worker_count: usize, timeout_secs: u64, use_colors: bool) {
    use colors::*;

    let emoji = hat_emoji(hat_name);
    let content = format!(
        " WAVE: {} {} | {} workers | timeout {}s ",
        emoji, hat_name, worker_count, timeout_secs
    );

    let box_width = 79;
    let content_len = content.len();
    let pad = if content_len + 2 < box_width {
        box_width - content_len - 2
    } else {
        0
    };

    if use_colors {
        eprintln!("\n{BOLD}{MAGENTA}──{content}{}{RESET}", "─".repeat(pad));
    } else {
        eprintln!("\n──{content}{}", "─".repeat(pad));
    }
}

/// Prints a per-worker completion line as each wave worker finishes.
///
/// Format:
/// ```text
///   ✓ Worker 1/3 done (45s) — ROLE: Rust Reviewer. Focus on ...
///   ✗ Worker 3/3 failed (600s) — ROLE: Documentation Reviewer. Focus on ...
/// ```
pub fn print_wave_worker_done(
    index: u32,
    total: u32,
    duration: Duration,
    success: bool,
    payload_preview: &str,
    use_colors: bool,
) {
    use colors::*;

    let elapsed = format_elapsed(duration);
    let status_word = if success { "done" } else { "failed" };
    let preview = truncate(payload_preview, 60);

    if use_colors {
        let (icon, color) = if success {
            ("✓", GREEN)
        } else {
            ("✗", RED)
        };
        eprintln!(
            "  {color}{BOLD}{icon}{RESET} Worker {}/{} {} ({}) — {}",
            index + 1,
            total,
            status_word,
            elapsed,
            preview
        );
    } else {
        let icon = if success { "✓" } else { "✗" };
        eprintln!(
            "  {} Worker {}/{} {} ({}) — {}",
            icon,
            index + 1,
            total,
            status_word,
            elapsed,
            preview
        );
    }
}

/// Prints the wave summary separator after all workers finish.
///
/// Format:
/// ```text
/// ── Wave complete: 2 succeeded, 1 failed (52s) ────────────────────────────────
/// ```
pub fn print_wave_summary(
    succeeded: usize,
    failed: usize,
    total_duration: Duration,
    use_colors: bool,
) {
    use colors::*;

    let elapsed = format_elapsed(total_duration);
    let content = format!(
        " Wave complete: {} succeeded, {} failed ({}) ",
        succeeded, failed, elapsed
    );

    let box_width = 79;
    let content_len = content.len();
    let pad = if content_len + 2 < box_width {
        box_width - content_len - 2
    } else {
        0
    };

    if use_colors {
        let summary_color = if failed > 0 { YELLOW } else { GREEN };
        eprintln!("{BOLD}{summary_color}──{content}{}{RESET}", "─".repeat(pad));
    } else {
        eprintln!("──{content}{}", "─".repeat(pad));
    }
}

/// Builds a map of event topics to hat display information for the TUI.
///
/// This allows the TUI to dynamically resolve which hat should be displayed
/// for any event topic, including custom hats (e.g., "review.security" -> "Security Reviewer").
///
/// Only exact topic patterns (non-wildcard) are included to avoid pattern matching complexity.
pub fn build_tui_hat_map(registry: &ralph_core::HatRegistry) -> HashMap<String, (HatId, String)> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use ralph_core::RalphConfig;

    #[test]
    fn test_format_elapsed_seconds_only() {
        let d = Duration::from_secs(45);
        assert_eq!(format_elapsed(d), "45s");
    }

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
    #[ignore = "visual demo; run with: cargo test -- --ignored --nocapture zzz_visual_demo"]
    fn zzz_visual_demo() {
        use ralph_core::LoopState;
        use std::path::PathBuf;
        use std::time::Duration;

        println!("\n===== --no-tui UX demo (plain / no color) =====\n");
        print_loop_banner(
            "lp-2026-05-01-abc123",
            "pi",
            &PathBuf::from("PROMPT.md"),
            &PathBuf::from(".ralph/events-abc123.jsonl"),
            &PathBuf::from(".ralph/agent/scratchpad.md"),
            150,
            false,
            false,
        );

        println!();
        print_iteration_separator(1, "planner", Duration::from_secs(0), 150, false);
        println!("  (… backend streams tool calls and text here …)");
        print_iteration_footer(
            1,
            150,
            Duration::from_secs(42),
            Duration::from_secs(42),
            0.03,
            0.03,
            false,
        );

        print_iteration_separator(2, "builder", Duration::from_secs(42), 150, false);
        println!("  (… builder output …)");
        print_iteration_footer(
            2,
            150,
            Duration::from_secs(68),
            Duration::from_secs(110),
            0.08,
            0.11,
            false,
        );

        print_iteration_separator(3, "reviewer", Duration::from_secs(110), 150, false);
        println!("  (… reviewer output …)");
        print_iteration_footer(
            3,
            150,
            Duration::from_secs(72),
            Duration::from_secs(182),
            0.14,
            0.25,
            false,
        );

        let mut state = LoopState::new();
        state.iteration = 150;
        state.cumulative_cost = 7.42;
        print_termination(
            &TerminationReason::MaxIterations,
            &state,
            false,
            Some("lp-2026-05-01-abc123"),
        );

        println!("\n\n===== alternate: completion promise (no resume hint) =====\n");
        let mut state = LoopState::new();
        state.iteration = 87;
        state.cumulative_cost = 3.15;
        print_termination(
            &TerminationReason::CompletionPromise,
            &state,
            false,
            Some("lp-2026-05-01-abc123"),
        );

        println!("\n===== alternate: cancelled (no resume hint — was C1 autosde fix) =====\n");
        print_termination(
            &TerminationReason::Cancelled,
            &state,
            false,
            Some("lp-2026-05-01-abc123"),
        );
    }

    #[test]
    fn test_resume_hint_present_for_interrupted() {
        assert!(resume_hint_for(&TerminationReason::Interrupted, "xy").is_some());
    }

    #[test]
    fn test_print_loop_banner_does_not_panic() {
        // Smoke test — printing should not panic for either color mode.
        let prompt = std::path::PathBuf::from("PROMPT.md");
        let events = std::path::PathBuf::from(".ralph/events.jsonl");
        let scratchpad = std::path::PathBuf::from(".ralph/agent/scratchpad.md");
        print_loop_banner(
            "loop-42",
            "pi",
            &prompt,
            &events,
            &scratchpad,
            150,
            false,
            false,
        );
        print_loop_banner(
            "loop-42",
            "pi",
            &prompt,
            &events,
            &scratchpad,
            150,
            true,
            true,
        );
    }

    #[test]
    fn test_print_iteration_footer_handles_zero_cost() {
        // Zero cost path should omit the cost fragment without panicking.
        print_iteration_footer(
            3,
            150,
            Duration::from_secs(72),
            Duration::from_secs(270),
            0.0,
            0.0,
            false,
        );
    }

    #[test]
    fn test_print_iteration_footer_handles_nonzero_cost() {
        print_iteration_footer(
            3,
            150,
            Duration::from_secs(72),
            Duration::from_secs(270),
            0.14,
            0.42,
            true,
        );
    }

    #[test]
    fn test_print_iteration_footer_handles_zero_max_iterations() {
        // Defensive: max_iterations == 0 should not divide-by-zero.
        print_iteration_footer(
            1,
            0,
            Duration::from_secs(1),
            Duration::from_secs(1),
            0.0,
            0.0,
            false,
        );
    }

    #[test]
    fn test_format_elapsed_minutes_and_seconds() {
        let d = Duration::from_secs(125); // 2m 5s
        assert_eq!(format_elapsed(d), "2m 5s");
    }

    #[test]
    fn test_format_elapsed_hours_minutes_seconds() {
        let d = Duration::from_secs(3725); // 1h 2m 5s
        assert_eq!(format_elapsed(d), "1h 2m 5s");
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

    #[test]
    fn test_hat_emoji_known_hats() {
        assert_eq!(hat_emoji("planner"), "?");
        assert_eq!(hat_emoji("builder"), "?");
        assert_eq!(hat_emoji("reviewer"), "?");
    }

    #[test]
    fn test_hat_emoji_unknown_hat() {
        assert_eq!(hat_emoji("custom_hat"), "?");
    }

    #[test]
    fn test_build_tui_hat_map_extracts_custom_hats() {
        // Given: A config with custom hats from pr-review preset
        let yaml = r#"
hats:
  security_reviewer:
    name: "Security Reviewer"
    triggers: ["review.security"]
    publishes: ["security.done"]
  correctness_reviewer:
    name: "Correctness Reviewer"
    triggers: ["review.correctness"]
    publishes: ["correctness.done"]
  architecture_reviewer:
    name: "Architecture Reviewer"
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
        assert_eq!(hat_display, "Security Reviewer");

        // Correctness reviewer
        assert!(
            hat_map.contains_key("review.correctness"),
            "Should map review.correctness topic"
        );
        let (hat_id, hat_display) = &hat_map["review.correctness"];
        assert_eq!(hat_id.as_str(), "correctness_reviewer");
        assert_eq!(hat_display, "Correctness Reviewer");

        // Architecture reviewer - exact topic only
        assert!(
            hat_map.contains_key("review.architecture"),
            "Should map review.architecture topic"
        );
        let (hat_id, hat_display) = &hat_map["review.architecture"];
        assert_eq!(hat_id.as_str(), "architecture_reviewer");
        assert_eq!(hat_display, "Architecture Reviewer");

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
    name: "Planner"
    triggers: ["task.*", "build.*"]
    publishes: ["plan.done"]
"#;
        let config: RalphConfig = serde_yaml::from_str(yaml).unwrap();
        let registry = ralph_core::HatRegistry::from_config(&config);

        // When: Building the TUI hat map
        let hat_map = build_tui_hat_map(&registry);

        // Then: Map should be empty (all patterns are wildcards)
        assert_eq!(
            hat_map.len(),
            0,
            "Wildcard-only patterns should produce empty hat map"
        );
    }
}
