//! Stream handler trait and implementations for processing Claude stream events.
//!
//! The `StreamHandler` trait abstracts over how stream events are displayed,
//! allowing for different output strategies (console, quiet, TUI, etc.).

use std::io::{self, Write};

/// Session completion result data.
#[derive(Debug, Clone)]
pub struct SessionResult {
    pub duration_ms: u64,
    pub total_cost_usd: f64,
    pub num_turns: u32,
    pub is_error: bool,
}

/// Handler for streaming output events from Claude.
///
/// Implementors receive events as Claude processes and can format/display
/// them in various ways (console output, TUI updates, logging, etc.).
pub trait StreamHandler: Send {
    /// Called when Claude emits text.
    fn on_text(&mut self, text: &str);

    /// Called when Claude invokes a tool.
    ///
    /// # Arguments
    /// * `name` - Tool name (e.g., "Read", "Bash", "Grep")
    /// * `id` - Unique tool invocation ID
    /// * `input` - Tool input parameters as JSON (file paths, commands, patterns, etc.)
    fn on_tool_call(&mut self, name: &str, id: &str, input: &serde_json::Value);

    /// Called when a tool returns results (verbose only).
    fn on_tool_result(&mut self, id: &str, output: &str);

    /// Called when an error occurs.
    fn on_error(&mut self, error: &str);

    /// Called when session completes (verbose only).
    fn on_complete(&mut self, result: &SessionResult);
}

/// Writes streaming output to stdout/stderr.
///
/// In normal mode, displays assistant text and tool invocations.
/// In verbose mode, also displays tool results and session summary.
pub struct ConsoleStreamHandler {
    verbose: bool,
    stdout: io::Stdout,
    stderr: io::Stderr,
}

impl ConsoleStreamHandler {
    /// Creates a new console handler.
    ///
    /// # Arguments
    /// * `verbose` - If true, shows tool results and session summary.
    pub fn new(verbose: bool) -> Self {
        Self {
            verbose,
            stdout: io::stdout(),
            stderr: io::stderr(),
        }
    }
}

impl StreamHandler for ConsoleStreamHandler {
    fn on_text(&mut self, text: &str) {
        let _ = writeln!(self.stdout, "Claude: {}", text);
    }

    fn on_tool_call(&mut self, name: &str, _id: &str, input: &serde_json::Value) {
        match format_tool_summary(name, input) {
            Some(summary) => {
                let _ = writeln!(self.stdout, "[Tool] {}: {}", name, summary);
            }
            None => {
                let _ = writeln!(self.stdout, "[Tool] {}", name);
            }
        }
    }

    fn on_tool_result(&mut self, _id: &str, output: &str) {
        if self.verbose {
            let _ = writeln!(self.stdout, "[Result] {}", truncate(output, 200));
        }
    }

    fn on_error(&mut self, error: &str) {
        // Write to both stdout (inline) and stderr (for separation)
        let _ = writeln!(self.stdout, "[Error] {}", error);
        let _ = writeln!(self.stderr, "[Error] {}", error);
    }

    fn on_complete(&mut self, result: &SessionResult) {
        if self.verbose {
            let _ = writeln!(
                self.stdout,
                "\n--- Session Complete ---\nDuration: {}ms | Cost: ${:.4} | Turns: {}",
                result.duration_ms, result.total_cost_usd, result.num_turns
            );
        }
    }
}

/// Suppresses all streaming output (for CI/silent mode).
pub struct QuietStreamHandler;

impl StreamHandler for QuietStreamHandler {
    fn on_text(&mut self, _: &str) {}
    fn on_tool_call(&mut self, _: &str, _: &str, _: &serde_json::Value) {}
    fn on_tool_result(&mut self, _: &str, _: &str) {}
    fn on_error(&mut self, _: &str) {}
    fn on_complete(&mut self, _: &SessionResult) {}
}

/// Extracts the most relevant field from tool input for display.
///
/// Returns a human-readable summary (file path, command, pattern, etc.) based on the tool type.
/// Returns `None` for unknown tools or if the expected field is missing.
fn format_tool_summary(name: &str, input: &serde_json::Value) -> Option<String> {
    match name {
        "Read" | "Edit" | "Write" => input.get("file_path")?.as_str().map(|s| s.to_string()),
        "Bash" => {
            let cmd = input.get("command")?.as_str()?;
            Some(truncate(cmd, 60))
        }
        "Grep" => input.get("pattern")?.as_str().map(|s| s.to_string()),
        "Glob" => input.get("pattern")?.as_str().map(|s| s.to_string()),
        "Task" => input.get("description")?.as_str().map(|s| s.to_string()),
        "WebFetch" => input.get("url")?.as_str().map(|s| s.to_string()),
        "WebSearch" => input.get("query")?.as_str().map(|s| s.to_string()),
        "LSP" => {
            let op = input.get("operation")?.as_str()?;
            let file = input.get("filePath")?.as_str()?;
            Some(format!("{} @ {}", op, file))
        }
        "NotebookEdit" => input.get("notebook_path")?.as_str().map(|s| s.to_string()),
        "TodoWrite" => Some("updating todo list".to_string()),
        _ => None,
    }
}

/// Truncates a string to approximately `max_len` characters, adding "..." if truncated.
///
/// Uses `char_indices` to find a valid UTF-8 boundary, ensuring we never slice
/// in the middle of a multi-byte character.
fn truncate(s: &str, max_len: usize) -> String {
    if s.chars().count() <= max_len {
        s.to_string()
    } else {
        // Find the byte index of the max_len-th character
        let byte_idx = s
            .char_indices()
            .nth(max_len)
            .map(|(idx, _)| idx)
            .unwrap_or(s.len());
        format!("{}...", &s[..byte_idx])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_console_handler_verbose_shows_results() {
        let mut handler = ConsoleStreamHandler::new(true);
        let bash_input = json!({"command": "ls -la"});

        // These calls should not panic
        handler.on_text("Hello");
        handler.on_tool_call("Bash", "tool_1", &bash_input);
        handler.on_tool_result("tool_1", "output");
        handler.on_complete(&SessionResult {
            duration_ms: 1000,
            total_cost_usd: 0.01,
            num_turns: 1,
            is_error: false,
        });
    }

    #[test]
    fn test_console_handler_normal_skips_results() {
        let mut handler = ConsoleStreamHandler::new(false);
        let read_input = json!({"file_path": "src/main.rs"});

        // These should not show tool results
        handler.on_text("Hello");
        handler.on_tool_call("Read", "tool_1", &read_input);
        handler.on_tool_result("tool_1", "output"); // Should be silent
        handler.on_complete(&SessionResult {
            duration_ms: 1000,
            total_cost_usd: 0.01,
            num_turns: 1,
            is_error: false,
        }); // Should be silent
    }

    #[test]
    fn test_quiet_handler_is_silent() {
        let mut handler = QuietStreamHandler;
        let empty_input = json!({});

        // All of these should be no-ops
        handler.on_text("Hello");
        handler.on_tool_call("Read", "tool_1", &empty_input);
        handler.on_tool_result("tool_1", "output");
        handler.on_error("Something went wrong");
        handler.on_complete(&SessionResult {
            duration_ms: 1000,
            total_cost_usd: 0.01,
            num_turns: 1,
            is_error: false,
        });
    }

    #[test]
    fn test_truncate_helper() {
        assert_eq!(truncate("short", 10), "short");
        assert_eq!(truncate("this is a long string", 10), "this is a ...");
    }

    #[test]
    fn test_truncate_utf8_boundaries() {
        // Arrow → is 3 bytes (U+2192: E2 86 92)
        let with_arrows = "→→→→→→→→→→";
        // Should truncate at character boundary, not byte boundary
        assert_eq!(truncate(with_arrows, 5), "→→→→→...");

        // Mixed ASCII and multi-byte
        let mixed = "a→b→c→d→e";
        assert_eq!(truncate(mixed, 5), "a→b→c...");

        // Emoji (4-byte characters)
        let emoji = "🎉🎊🎁🎈🎄";
        assert_eq!(truncate(emoji, 3), "🎉🎊🎁...");
    }

    #[test]
    fn test_format_tool_summary_file_tools() {
        assert_eq!(
            format_tool_summary("Read", &json!({"file_path": "src/main.rs"})),
            Some("src/main.rs".to_string())
        );
        assert_eq!(
            format_tool_summary("Edit", &json!({"file_path": "/path/to/file.txt"})),
            Some("/path/to/file.txt".to_string())
        );
        assert_eq!(
            format_tool_summary("Write", &json!({"file_path": "output.json"})),
            Some("output.json".to_string())
        );
    }

    #[test]
    fn test_format_tool_summary_bash_truncates() {
        let short_cmd = json!({"command": "ls -la"});
        assert_eq!(
            format_tool_summary("Bash", &short_cmd),
            Some("ls -la".to_string())
        );

        let long_cmd = json!({"command": "this is a very long command that should be truncated because it exceeds sixty characters"});
        let result = format_tool_summary("Bash", &long_cmd).unwrap();
        assert!(result.ends_with("..."));
        assert!(result.len() <= 70); // 60 chars + "..."
    }

    #[test]
    fn test_format_tool_summary_search_tools() {
        assert_eq!(
            format_tool_summary("Grep", &json!({"pattern": "TODO"})),
            Some("TODO".to_string())
        );
        assert_eq!(
            format_tool_summary("Glob", &json!({"pattern": "**/*.rs"})),
            Some("**/*.rs".to_string())
        );
    }

    #[test]
    fn test_format_tool_summary_unknown_tool_returns_none() {
        assert_eq!(
            format_tool_summary("UnknownTool", &json!({"some_field": "value"})),
            None
        );
    }

    #[test]
    fn test_format_tool_summary_missing_field_returns_none() {
        // Read without file_path
        assert_eq!(
            format_tool_summary("Read", &json!({"wrong_field": "value"})),
            None
        );
        // Bash without command
        assert_eq!(format_tool_summary("Bash", &json!({})), None);
    }
}
