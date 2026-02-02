//! Display functions for terminal output.
//!
//! This module contains functions for formatting and printing
//! iteration separators, termination messages, event tables,
//! and other terminal UI elements.

use ralph_core::{EventRecord, TerminationReason};
use ralph_proto::HatId;
use std::collections::HashMap;
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
    // Note: `max_len` keeps the legacy behavior (byte-count-ish), but we must slice only on a
    // valid UTF-8 character boundary. Otherwise, multi-byte characters (e.g. CJK, emoji) can make
    // `&s[..N]` panic.
    if max_len == 0 {
        return String::new();
    }

    if s.len() <= max_len {
        return s.to_string();
    }

    truncate_prefix_bytes(s, max_len.saturating_sub(1))
}

/// Truncates the string to at most `prefix_max_bytes` bytes (as close as possible) and appends "...".
///
/// Key point: always use `char_indices()` to find a valid UTF-8 boundary before slicing, so we
/// never slice through a multi-byte character and panic.
fn truncate_prefix_bytes(s: &str, prefix_max_bytes: usize) -> String {
    if s.len() <= prefix_max_bytes {
        return s.to_string();
    }

    // Find the last character start before `prefix_max_bytes`, then add its byte length to get a
    // valid boundary.
    let boundary = s
        .char_indices()
        .take_while(|(i, _)| *i < prefix_max_bytes)
        .last()
        .map(|(i, c)| i + c.len_utf8())
        .unwrap_or(0);

    format!("{}...", &s[..boundary])
}

/// Prints termination message with status.
pub fn print_termination(
    reason: &TerminationReason,
    state: &ralph_core::LoopState,
    use_colors: bool,
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
        TerminationReason::ValidationFailure => (RED, "?", "Too many malformed JSONL events"),
        TerminationReason::Stopped => (CYAN, "?", "Manually stopped"),
        TerminationReason::Interrupted => (YELLOW, "?", "Interrupted by signal"),
        TerminationReason::RestartRequested => (CYAN, "↻", "Restarting by human request"),
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
        let payload_preview = if payload_one_line.len() > 40 {
            truncate_prefix_bytes(&payload_one_line, 40)
        } else {
            payload_one_line
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
                // Take only HH:MM:SS (usually ASCII), but still ensure we slice on a valid UTF-8
                // boundary for robustness. Otherwise, an unexpected `ts` (e.g. CJK/emoji) can make
                // `&s[..N]` panic.
                let mut boundary = time_str.len().min(8);
                while boundary > 0 && !time_str.is_char_boundary(boundary) {
                    boundary -= 1;
                }
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
        assert_eq!(truncate("hello world", 8), "hello w...");
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
