//! Stream handler trait and implementations for processing Claude stream events.
//!
//! The `StreamHandler` trait abstracts over how stream events are displayed,
//! allowing for different output strategies (console, quiet, TUI, etc.).

use ansi_to_tui::IntoText;
use crossterm::{
    QueueableCommand,
    style::{self, Color},
};
use ratatui::{
    style::{Color as RatatuiColor, Style},
    text::{Line, Span},
};
use std::{
    borrow::Cow,
    io::{self, Write},
    sync::{Arc, Mutex},
};
use termimad::MadSkin;

/// Detects if text contains ANSI escape sequences.
///
/// Checks for the common ANSI escape sequence prefix `\x1b[` (ESC + `[`)
/// which is used for colors, formatting, and cursor control.
#[inline]
pub(crate) fn contains_ansi(text: &str) -> bool {
    text.contains("\x1b[")
}

/// Normalizes terminal control characters that commonly break ratatui rendering.
///
/// In particular:
/// - `\r` (carriage return) is used by many CLIs (git, cargo, etc.) to render
///   progress updates on a single line. When embedded in ratatui content it can
///   move the cursor and corrupt layout.
/// - Some other C0 controls (bell, backspace, vertical tab, form feed) can also
///   cause display corruption or odd glyphs.
///
/// We keep `\n` and `\t` intact.
fn sanitize_tui_block_text(text: &str) -> Cow<'_, str> {
    let has_cr = text.contains('\r');
    let has_other_ctrl = text
        .chars()
        .any(|c| matches!(c, '\u{0007}' | '\u{0008}' | '\u{000b}' | '\u{000c}'));

    if !has_cr && !has_other_ctrl {
        return Cow::Borrowed(text);
    }

    let mut s = if has_cr {
        // Normalize CRLF and bare CR to LF.
        text.replace("\r\n", "\n").replace('\r', "\n")
    } else {
        text.to_string()
    };

    if has_other_ctrl {
        s.retain(|c| !matches!(c, '\u{0007}' | '\u{0008}' | '\u{000b}' | '\u{000c}'));
    }

    Cow::Owned(s)
}

/// Sanitizes text that must stay on a *single* TUI line (tool summaries, errors).
/// Removes embedded newlines and carriage returns entirely.
fn sanitize_tui_inline_text(text: &str) -> String {
    let mut s = text.replace("\r\n", " ").replace(['\r', '\n'], " ");

    // Drop other control characters that can corrupt the terminal.
    s.retain(|c| !matches!(c, '\u{0007}' | '\u{0008}' | '\u{000b}' | '\u{000c}'));

    s
}

/// Session completion result data.
#[derive(Debug, Clone)]
pub struct SessionResult {
    pub duration_ms: u64,
    pub total_cost_usd: f64,
    pub num_turns: u32,
    pub is_error: bool,
}

/// Renders streaming output with colors and markdown.
pub struct PrettyStreamHandler {
    stdout: io::Stdout,
    verbose: bool,
    /// Buffer for accumulating text before markdown rendering
    text_buffer: String,
    /// Skin for markdown rendering
    skin: MadSkin,
}

impl PrettyStreamHandler {
    /// Creates a new pretty handler.
    pub fn new(verbose: bool) -> Self {
        Self {
            stdout: io::stdout(),
            verbose,
            text_buffer: String::new(),
            skin: MadSkin::default(),
        }
    }

    /// Flush buffered text as rendered markdown.
    fn flush_text_buffer(&mut self) {
        if self.text_buffer.is_empty() {
            return;
        }
        // Render markdown to string, then write
        let rendered = self.skin.term_text(&self.text_buffer);
        let _ = self.stdout.write(rendered.to_string().as_bytes());
        let _ = self.stdout.flush();
        self.text_buffer.clear();
    }
}

impl StreamHandler for PrettyStreamHandler {
    fn on_text(&mut self, text: &str) {
        // Buffer text for markdown rendering
        // Text is flushed when: tool calls arrive, on_complete is called, or on_error is called
        // This works well for StreamJson backends (Claude) which have natural flush points
        // Text format backends should use ConsoleStreamHandler for immediate output
        self.text_buffer.push_str(text);
    }

    fn on_tool_result(&mut self, _id: &str, output: &str) {
        if self.verbose {
            let _ = self
                .stdout
                .queue(style::SetForegroundColor(Color::DarkGrey));
            let _ = self
                .stdout
                .write(format!(" \u{2713} {}\n", truncate(output, 200)).as_bytes());
            let _ = self.stdout.queue(style::ResetColor);
            let _ = self.stdout.flush();
        }
    }

    fn on_error(&mut self, error: &str) {
        let _ = self.stdout.queue(style::SetForegroundColor(Color::Red));
        let _ = self
            .stdout
            .write(format!("\n\u{2717} Error: {}\n", error).as_bytes());
        let _ = self.stdout.queue(style::ResetColor);
        let _ = self.stdout.flush();
    }

    fn on_complete(&mut self, result: &SessionResult) {
        // Flush any remaining buffered text
        self.flush_text_buffer();

        let _ = self.stdout.write(b"\n");
        let color = if result.is_error {
            Color::Red
        } else {
            Color::Green
        };
        let _ = self.stdout.queue(style::SetForegroundColor(color));
        let _ = self.stdout.write(
            format!(
                "Duration: {}ms | Est. cost: ${:.4} | Turns: {}\n",
                result.duration_ms, result.total_cost_usd, result.num_turns
            )
            .as_bytes(),
        );
        let _ = self.stdout.queue(style::ResetColor);
        let _ = self.stdout.flush();
    }

    fn on_tool_call(&mut self, name: &str, _id: &str, input: &serde_json::Value) {
        // Flush any buffered text before showing tool call
        self.flush_text_buffer();

        // ⚙️ [ToolName]
        let _ = self.stdout.queue(style::SetForegroundColor(Color::Blue));
        let _ = self.stdout.write(format!("\u{2699} [{}]", name).as_bytes());

        if let Some(summary) = format_tool_summary(name, input) {
            let _ = self
                .stdout
                .queue(style::SetForegroundColor(Color::DarkGrey));
            let _ = self.stdout.write(format!(" {}\n", summary).as_bytes());
        } else {
            let _ = self.stdout.write(b"\n");
        }
        let _ = self.stdout.queue(style::ResetColor);
        let _ = self.stdout.flush();
    }
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

    /// Called when a tool returns results.
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
    /// Tracks whether last output ended with a newline
    last_was_newline: bool,
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
            last_was_newline: true, // Start true so first output doesn't get extra newline
        }
    }

    /// Ensures output starts on a new line if the previous output didn't end with one.
    fn ensure_newline(&mut self) {
        if !self.last_was_newline {
            let _ = writeln!(self.stdout);
            self.last_was_newline = true;
        }
    }
}

impl StreamHandler for ConsoleStreamHandler {
    fn on_text(&mut self, text: &str) {
        let _ = write!(self.stdout, "{}", text);
        self.last_was_newline = text.ends_with('\n');
    }

    fn on_tool_call(&mut self, name: &str, _id: &str, input: &serde_json::Value) {
        self.ensure_newline();
        match format_tool_summary(name, input) {
            Some(summary) => {
                let _ = writeln!(self.stdout, "[Tool] {}: {}", name, summary);
            }
            None => {
                let _ = writeln!(self.stdout, "[Tool] {}", name);
            }
        }
        // writeln always ends with newline
        self.last_was_newline = true;
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
                "\n--- Session Complete ---\nDuration: {}ms | Est. cost: ${:.4} | Turns: {}",
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

/// Converts text to styled ratatui Lines, handling both ANSI and markdown.
///
/// When text contains ANSI escape sequences (e.g., from CLI tools like Kiro),
/// uses `ansi_to_tui` to preserve colors and formatting. Otherwise, uses
/// `termimad` to parse markdown (matching non-TUI mode behavior), then
/// converts the ANSI output via `ansi_to_tui`.
///
/// Using `termimad` ensures parity between TUI and non-TUI modes, as both
/// use the same markdown processing engine with the same line-breaking rules.
fn text_to_lines(text: &str) -> Vec<Line<'static>> {
    if text.is_empty() {
        return Vec::new();
    }

    // Ratatui content must not contain control characters like carriage returns.
    // See sanitize_tui_block_text() for rationale.
    let text = sanitize_tui_block_text(text);
    let text = text.as_ref();
    if text.is_empty() {
        return Vec::new();
    }

    // Convert text to ANSI-styled string
    // - If already contains ANSI: use as-is
    // - If plain/markdown: process through termimad (matches non-TUI behavior)
    let ansi_text = if contains_ansi(text) {
        text.to_string()
    } else {
        // Use termimad to process markdown - this matches PrettyStreamHandler behavior
        // and ensures consistent line-breaking between TUI and non-TUI modes
        let skin = MadSkin::default();
        skin.term_text(text).to_string()
    };

    // Parse ANSI codes to ratatui Text
    match ansi_text.as_str().into_text() {
        Ok(parsed_text) => {
            // Convert Text to owned Lines
            parsed_text
                .lines
                .into_iter()
                .map(|line| {
                    let owned_spans: Vec<Span<'static>> = line
                        .spans
                        .into_iter()
                        .map(|span| Span::styled(span.content.into_owned(), span.style))
                        .collect();
                    Line::from(owned_spans)
                })
                .collect()
        }
        Err(_) => {
            // Fallback: split on newlines and treat as plain text
            text.split('\n')
                .map(|line| Line::from(line.to_string()))
                .collect()
        }
    }
}

/// A content block in the chronological stream.
///
/// Used to preserve ordering between text and non-text content (tool calls, errors).
#[derive(Clone)]
enum ContentBlock {
    /// Markdown/ANSI text that was accumulated before being frozen
    Text(String),
    /// A single non-text line (tool call, error, completion summary, etc.)
    NonText(Line<'static>),
}

/// Renders streaming output as ratatui Lines for TUI display.
///
/// This handler produces output visually equivalent to `PrettyStreamHandler`
/// but stores it as `Line<'static>` objects for rendering in a ratatui-based TUI.
///
/// Text content is parsed as markdown, producing styled output for bold, italic,
/// code, headers, etc. Tool calls and errors bypass markdown parsing to preserve
/// their explicit styling.
///
/// **Chronological ordering**: When a tool call arrives, the current text buffer
/// is "frozen" into a content block, preserving the order in which events arrived.
pub struct TuiStreamHandler {
    /// Buffer for accumulating current markdown text (not yet frozen)
    current_text_buffer: String,
    /// Chronological sequence of content blocks (frozen text + non-text events)
    blocks: Vec<ContentBlock>,
    /// Reserved for parity with non-TUI handlers.
    _verbose: bool,
    /// Collected output lines for rendering
    lines: Arc<Mutex<Vec<Line<'static>>>>,
}

impl TuiStreamHandler {
    /// Creates a new TUI handler.
    ///
    /// # Arguments
    /// * `verbose` - If true, shows session summary.
    pub fn new(verbose: bool) -> Self {
        Self {
            current_text_buffer: String::new(),
            blocks: Vec::new(),
            _verbose: verbose,
            lines: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Creates a TUI handler with shared lines storage.
    ///
    /// Use this to share output lines with the TUI application.
    pub fn with_lines(verbose: bool, lines: Arc<Mutex<Vec<Line<'static>>>>) -> Self {
        Self {
            current_text_buffer: String::new(),
            blocks: Vec::new(),
            _verbose: verbose,
            lines,
        }
    }

    /// Returns a clone of the collected lines.
    pub fn get_lines(&self) -> Vec<Line<'static>> {
        self.lines.lock().unwrap().clone()
    }

    /// Flushes any buffered markdown text by re-parsing and updating lines.
    pub fn flush_text_buffer(&mut self) {
        self.update_lines();
    }

    /// Freezes the current text buffer into a content block.
    ///
    /// This is called when a non-text event (tool call, error) arrives,
    /// ensuring that text before the event stays before it in the output.
    fn freeze_current_text(&mut self) {
        if !self.current_text_buffer.is_empty() {
            self.blocks
                .push(ContentBlock::Text(self.current_text_buffer.clone()));
            self.current_text_buffer.clear();
        }
    }

    /// Re-renders all content blocks and updates the shared lines.
    ///
    /// Iterates through frozen blocks in chronological order, then appends
    /// any current (unfrozen) text buffer content. This preserves the
    /// interleaved ordering of text and non-text content.
    fn update_lines(&mut self) {
        let mut all_lines = Vec::new();

        // Render frozen blocks in chronological order
        for block in &self.blocks {
            match block {
                ContentBlock::Text(text) => {
                    all_lines.extend(text_to_lines(text));
                }
                ContentBlock::NonText(line) => {
                    all_lines.push(line.clone());
                }
            }
        }

        // Render current (unfrozen) text buffer for real-time updates
        if !self.current_text_buffer.is_empty() {
            all_lines.extend(text_to_lines(&self.current_text_buffer));
        }

        // Note: Long lines are NOT truncated here. The TUI's ContentPane widget
        // handles soft-wrapping at viewport boundaries, preserving full content.

        // Update shared lines
        *self.lines.lock().unwrap() = all_lines;
    }

    /// Adds a non-text line (tool call, error, etc.) and updates display.
    ///
    /// First freezes any pending text buffer to preserve chronological order.
    fn add_non_text_line(&mut self, line: Line<'static>) {
        self.freeze_current_text();
        self.blocks.push(ContentBlock::NonText(line));
        self.update_lines();
    }
}

impl StreamHandler for TuiStreamHandler {
    fn on_text(&mut self, text: &str) {
        // Append text to current buffer
        self.current_text_buffer.push_str(text);

        // Re-parse and update lines on each text chunk
        // This handles streaming markdown correctly
        self.update_lines();
    }

    fn on_tool_call(&mut self, name: &str, _id: &str, input: &serde_json::Value) {
        // Build spans: ⚙️ [ToolName] summary
        let mut spans = vec![Span::styled(
            format!("\u{2699} [{}]", name),
            Style::default().fg(RatatuiColor::Blue),
        )];

        if let Some(summary) = format_tool_summary(name, input) {
            let summary = sanitize_tui_inline_text(&summary);
            spans.push(Span::styled(
                format!(" {}", summary),
                Style::default().fg(RatatuiColor::DarkGray),
            ));
        }

        self.add_non_text_line(Line::from(spans));
    }

    fn on_tool_result(&mut self, _id: &str, output: &str) {
        let clean = sanitize_tui_inline_text(output);
        let line = Line::from(Span::styled(
            format!(" \u{2713} {}", truncate(&clean, 200)),
            Style::default().fg(RatatuiColor::DarkGray),
        ));
        self.add_non_text_line(line);
    }

    fn on_error(&mut self, error: &str) {
        let clean = sanitize_tui_inline_text(error);
        let line = Line::from(Span::styled(
            format!("\u{2717} Error: {}", clean),
            Style::default().fg(RatatuiColor::Red),
        ));
        self.add_non_text_line(line);
    }

    fn on_complete(&mut self, result: &SessionResult) {
        // Flush any remaining buffered text
        self.flush_text_buffer();

        // Add blank line
        self.add_non_text_line(Line::from(""));

        // Add summary with color based on error status
        let color = if result.is_error {
            RatatuiColor::Red
        } else {
            RatatuiColor::Green
        };
        let summary = format!(
            "Duration: {}ms | Est. cost: ${:.4} | Turns: {}",
            result.duration_ms, result.total_cost_usd, result.num_turns
        );
        let line = Line::from(Span::styled(summary, Style::default().fg(color)));
        self.add_non_text_line(line);
    }
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
    fn test_sanitize_tui_inline_text_removes_newlines_and_carriage_returns() {
        let s = "hello\r\nworld\nbye\rok";
        let clean = sanitize_tui_inline_text(s);
        assert!(!clean.contains('\r'));
        assert!(!clean.contains('\n'));
    }

    #[test]
    fn test_text_to_lines_sanitizes_carriage_returns() {
        let lines = text_to_lines("alpha\rbravo\ncharlie");
        for line in lines {
            for span in line.spans {
                assert!(
                    !span.content.contains('\r'),
                    "Span content should not contain carriage returns: {:?}",
                    span.content
                );
            }
        }
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

    // ========================================================================
    // TuiStreamHandler Tests
    // ========================================================================

    mod tui_stream_handler {
        use super::*;
        use ratatui::style::{Color, Modifier};

        /// Helper to collect lines from TuiStreamHandler
        fn collect_lines(handler: &TuiStreamHandler) -> Vec<ratatui::text::Line<'static>> {
            handler.lines.lock().unwrap().clone()
        }

        #[test]
        fn text_creates_line_on_newline() {
            // Given TuiStreamHandler
            let mut handler = TuiStreamHandler::new(false);

            // When on_text("hello\n") is called
            handler.on_text("hello\n");

            // Then a Line with "hello" content is produced
            // Note: termimad (like non-TUI mode) doesn't create empty line for trailing \n
            let lines = collect_lines(&handler);
            assert_eq!(
                lines.len(),
                1,
                "termimad doesn't create trailing empty line"
            );
            assert_eq!(lines[0].to_string(), "hello");
        }

        #[test]
        fn partial_text_buffering() {
            // Given TuiStreamHandler
            let mut handler = TuiStreamHandler::new(false);

            // When on_text("hel") then on_text("lo\n") is called
            // Note: With markdown parsing, partial text is rendered immediately
            // (markdown doesn't require newlines for paragraphs)
            handler.on_text("hel");
            handler.on_text("lo\n");

            // Then the combined "hello" text is present
            let lines = collect_lines(&handler);
            let full_text: String = lines.iter().map(|l| l.to_string()).collect();
            assert!(
                full_text.contains("hello"),
                "Combined text should contain 'hello'. Lines: {:?}",
                lines
            );
        }

        #[test]
        fn tool_call_produces_formatted_line() {
            // Given TuiStreamHandler
            let mut handler = TuiStreamHandler::new(false);

            // When on_tool_call("Read", "id", &json!({"file_path": "src/main.rs"})) is called
            handler.on_tool_call("Read", "tool_1", &json!({"file_path": "src/main.rs"}));

            // Then a Line starting with "⚙️" and containing "Read" and file path is produced
            let lines = collect_lines(&handler);
            assert_eq!(lines.len(), 1);
            let line_text = lines[0].to_string();
            assert!(
                line_text.contains('\u{2699}'),
                "Should contain gear emoji: {}",
                line_text
            );
            assert!(
                line_text.contains("Read"),
                "Should contain tool name: {}",
                line_text
            );
            assert!(
                line_text.contains("src/main.rs"),
                "Should contain file path: {}",
                line_text
            );
        }

        #[test]
        fn tool_result_verbose_shows_content() {
            // Given TuiStreamHandler with verbose=true
            let mut handler = TuiStreamHandler::new(true);

            // When on_tool_result(...) is called
            handler.on_tool_result("tool_1", "file contents here");

            // Then result content appears in output
            let lines = collect_lines(&handler);
            assert_eq!(lines.len(), 1);
            let line_text = lines[0].to_string();
            assert!(
                line_text.contains('\u{2713}'),
                "Should contain checkmark: {}",
                line_text
            );
            assert!(
                line_text.contains("file contents here"),
                "Should contain result content: {}",
                line_text
            );
        }

        #[test]
        fn tool_result_quiet_shows_content() {
            // Given TuiStreamHandler with verbose=false
            let mut handler = TuiStreamHandler::new(false);

            // When on_tool_result(...) is called
            handler.on_tool_result("tool_1", "file contents here");

            // Then result content appears in output
            let lines = collect_lines(&handler);
            assert_eq!(lines.len(), 1);
            let line_text = lines[0].to_string();
            assert!(
                line_text.contains('\u{2713}'),
                "Should contain checkmark: {}",
                line_text
            );
            assert!(
                line_text.contains("file contents here"),
                "Should contain result content: {}",
                line_text
            );
        }

        #[test]
        fn error_produces_red_styled_line() {
            // Given TuiStreamHandler
            let mut handler = TuiStreamHandler::new(false);

            // When on_error("fail") is called
            handler.on_error("Something went wrong");

            // Then a Line with red foreground style is produced
            let lines = collect_lines(&handler);
            assert_eq!(lines.len(), 1);
            let line_text = lines[0].to_string();
            assert!(
                line_text.contains('\u{2717}'),
                "Should contain X mark: {}",
                line_text
            );
            assert!(
                line_text.contains("Error"),
                "Should contain 'Error': {}",
                line_text
            );
            assert!(
                line_text.contains("Something went wrong"),
                "Should contain error message: {}",
                line_text
            );

            // Check style is red
            let first_span = &lines[0].spans[0];
            assert_eq!(
                first_span.style.fg,
                Some(Color::Red),
                "Error line should have red foreground"
            );
        }

        #[test]
        fn long_lines_preserved_without_truncation() {
            // Given TuiStreamHandler
            let mut handler = TuiStreamHandler::new(false);

            // When on_text() receives a very long string (500+ chars)
            let long_string: String = "a".repeat(500) + "\n";
            handler.on_text(&long_string);

            // Then content is preserved fully (termimad may wrap at terminal width)
            // Note: termimad wraps at ~80 chars by default, so 500 chars = multiple lines
            let lines = collect_lines(&handler);

            // Verify total content is preserved (all 500 'a's present)
            let total_content: String = lines.iter().map(|l| l.to_string()).collect();
            let a_count = total_content.chars().filter(|c| *c == 'a').count();
            assert_eq!(
                a_count, 500,
                "All 500 'a' chars should be preserved. Got {}",
                a_count
            );

            // Should not have truncation ellipsis
            assert!(
                !total_content.contains("..."),
                "Content should not have ellipsis truncation"
            );
        }

        #[test]
        fn multiple_lines_in_single_text_call() {
            // When text contains multiple newlines
            let mut handler = TuiStreamHandler::new(false);
            handler.on_text("line1\nline2\nline3\n");

            // Then all text content is present
            // Note: Markdown parsing may combine lines into paragraphs differently
            let lines = collect_lines(&handler);
            let full_text: String = lines
                .iter()
                .map(|l| l.to_string())
                .collect::<Vec<_>>()
                .join(" ");
            assert!(
                full_text.contains("line1")
                    && full_text.contains("line2")
                    && full_text.contains("line3"),
                "All lines should be present. Lines: {:?}",
                lines
            );
        }

        #[test]
        fn termimad_parity_with_non_tui_mode() {
            // Verify that TUI mode (using termimad) matches non-TUI mode output
            // This ensures the "★ Insight" box renders consistently in both modes
            let text = "Some text before:★ Insight ─────\nKey point here";

            let mut handler = TuiStreamHandler::new(false);
            handler.on_text(text);

            let lines = collect_lines(&handler);

            // termimad wraps after "★ Insight " putting dashes on their own line
            // This matches PrettyStreamHandler (non-TUI) behavior
            assert!(
                lines.len() >= 2,
                "termimad should produce multiple lines. Got: {:?}",
                lines.iter().map(|l| l.to_string()).collect::<Vec<_>>()
            );

            // Content should be preserved
            let full_text: String = lines.iter().map(|l| l.to_string()).collect();
            assert!(
                full_text.contains("★ Insight"),
                "Content should contain insight marker"
            );
        }

        #[test]
        fn tool_call_flushes_text_buffer() {
            // Given buffered text
            let mut handler = TuiStreamHandler::new(false);
            handler.on_text("partial text");

            // When tool call arrives
            handler.on_tool_call("Read", "id", &json!({}));

            // Then buffered text is flushed as a line before tool call line
            let lines = collect_lines(&handler);
            assert_eq!(lines.len(), 2);
            assert_eq!(lines[0].to_string(), "partial text");
            assert!(lines[1].to_string().contains('\u{2699}'));
        }

        #[test]
        fn interleaved_text_and_tools_preserves_chronological_order() {
            // Given: text1 → tool1 → text2 → tool2
            // Expected output order: text1, tool1, text2, tool2
            // NOT: text1 + text2, then tool1 + tool2 (the bug we fixed)
            let mut handler = TuiStreamHandler::new(false);

            // Simulate Claude's streaming output pattern
            handler.on_text("I'll start by reviewing the scratchpad.\n");
            handler.on_tool_call("Read", "id1", &json!({"file_path": "scratchpad.md"}));
            handler.on_text("I found the task. Now checking the code.\n");
            handler.on_tool_call("Read", "id2", &json!({"file_path": "main.rs"}));
            handler.on_text("Done reviewing.\n");

            let lines = collect_lines(&handler);

            // Find indices of key content
            let text1_idx = lines
                .iter()
                .position(|l| l.to_string().contains("reviewing the scratchpad"));
            let tool1_idx = lines
                .iter()
                .position(|l| l.to_string().contains("scratchpad.md"));
            let text2_idx = lines
                .iter()
                .position(|l| l.to_string().contains("checking the code"));
            let tool2_idx = lines.iter().position(|l| l.to_string().contains("main.rs"));
            let text3_idx = lines
                .iter()
                .position(|l| l.to_string().contains("Done reviewing"));

            // All content should be present
            assert!(text1_idx.is_some(), "text1 should be present");
            assert!(tool1_idx.is_some(), "tool1 should be present");
            assert!(text2_idx.is_some(), "text2 should be present");
            assert!(tool2_idx.is_some(), "tool2 should be present");
            assert!(text3_idx.is_some(), "text3 should be present");

            // Chronological order must be preserved
            let text1_idx = text1_idx.unwrap();
            let tool1_idx = tool1_idx.unwrap();
            let text2_idx = text2_idx.unwrap();
            let tool2_idx = tool2_idx.unwrap();
            let text3_idx = text3_idx.unwrap();

            assert!(
                text1_idx < tool1_idx,
                "text1 ({}) should come before tool1 ({}). Lines: {:?}",
                text1_idx,
                tool1_idx,
                lines.iter().map(|l| l.to_string()).collect::<Vec<_>>()
            );
            assert!(
                tool1_idx < text2_idx,
                "tool1 ({}) should come before text2 ({}). Lines: {:?}",
                tool1_idx,
                text2_idx,
                lines.iter().map(|l| l.to_string()).collect::<Vec<_>>()
            );
            assert!(
                text2_idx < tool2_idx,
                "text2 ({}) should come before tool2 ({}). Lines: {:?}",
                text2_idx,
                tool2_idx,
                lines.iter().map(|l| l.to_string()).collect::<Vec<_>>()
            );
            assert!(
                tool2_idx < text3_idx,
                "tool2 ({}) should come before text3 ({}). Lines: {:?}",
                tool2_idx,
                text3_idx,
                lines.iter().map(|l| l.to_string()).collect::<Vec<_>>()
            );
        }

        #[test]
        fn on_complete_flushes_buffer_and_shows_summary() {
            // Given buffered text and verbose mode
            let mut handler = TuiStreamHandler::new(true);
            handler.on_text("final output");

            // When on_complete is called
            handler.on_complete(&SessionResult {
                duration_ms: 1500,
                total_cost_usd: 0.0025,
                num_turns: 3,
                is_error: false,
            });

            // Then buffer is flushed and summary line appears
            let lines = collect_lines(&handler);
            assert!(lines.len() >= 2, "Should have at least 2 lines");
            assert_eq!(lines[0].to_string(), "final output");

            // Find summary line
            let summary = lines.last().unwrap().to_string();
            assert!(
                summary.contains("1500"),
                "Should contain duration: {}",
                summary
            );
            assert!(
                summary.contains("0.0025"),
                "Should contain cost: {}",
                summary
            );
            assert!(summary.contains('3'), "Should contain turns: {}", summary);
        }

        #[test]
        fn on_complete_error_uses_red_style() {
            let mut handler = TuiStreamHandler::new(true);
            handler.on_complete(&SessionResult {
                duration_ms: 1000,
                total_cost_usd: 0.01,
                num_turns: 1,
                is_error: true,
            });

            let lines = collect_lines(&handler);
            assert!(!lines.is_empty());

            // Last line should be red styled for error
            let last_line = lines.last().unwrap();
            assert_eq!(
                last_line.spans[0].style.fg,
                Some(Color::Red),
                "Error completion should have red foreground"
            );
        }

        #[test]
        fn on_complete_success_uses_green_style() {
            let mut handler = TuiStreamHandler::new(true);
            handler.on_complete(&SessionResult {
                duration_ms: 1000,
                total_cost_usd: 0.01,
                num_turns: 1,
                is_error: false,
            });

            let lines = collect_lines(&handler);
            assert!(!lines.is_empty());

            // Last line should be green styled for success
            let last_line = lines.last().unwrap();
            assert_eq!(
                last_line.spans[0].style.fg,
                Some(Color::Green),
                "Success completion should have green foreground"
            );
        }

        #[test]
        fn tool_call_with_no_summary_shows_just_name() {
            let mut handler = TuiStreamHandler::new(false);
            handler.on_tool_call("UnknownTool", "id", &json!({}));

            let lines = collect_lines(&handler);
            assert_eq!(lines.len(), 1);
            let line_text = lines[0].to_string();
            assert!(line_text.contains("UnknownTool"));
            // Should not crash or show "null" for missing summary
        }

        #[test]
        fn get_lines_returns_clone_of_internal_lines() {
            let mut handler = TuiStreamHandler::new(false);
            handler.on_text("test\n");

            let lines1 = handler.get_lines();
            let lines2 = handler.get_lines();

            // Both should have same content
            assert_eq!(lines1.len(), lines2.len());
            assert_eq!(lines1[0].to_string(), lines2[0].to_string());
        }

        // =====================================================================
        // Markdown Rendering Tests
        // =====================================================================

        #[test]
        fn markdown_bold_text_renders_with_bold_modifier() {
            // Given TuiStreamHandler
            let mut handler = TuiStreamHandler::new(false);

            // When on_text("**important**\n") is called
            handler.on_text("**important**\n");

            // Then the text "important" appears with BOLD modifier
            let lines = collect_lines(&handler);
            assert!(!lines.is_empty(), "Should have at least one line");

            // Find a span containing "important" and check it's bold
            let has_bold = lines.iter().any(|line| {
                line.spans.iter().any(|span| {
                    span.content.contains("important")
                        && span.style.add_modifier.contains(Modifier::BOLD)
                })
            });
            assert!(
                has_bold,
                "Should have bold 'important' span. Lines: {:?}",
                lines
            );
        }

        #[test]
        fn markdown_italic_text_renders_with_italic_modifier() {
            // Given TuiStreamHandler
            let mut handler = TuiStreamHandler::new(false);

            // When on_text("*emphasized*\n") is called
            handler.on_text("*emphasized*\n");

            // Then the text "emphasized" appears with ITALIC modifier
            let lines = collect_lines(&handler);
            assert!(!lines.is_empty(), "Should have at least one line");

            let has_italic = lines.iter().any(|line| {
                line.spans.iter().any(|span| {
                    span.content.contains("emphasized")
                        && span.style.add_modifier.contains(Modifier::ITALIC)
                })
            });
            assert!(
                has_italic,
                "Should have italic 'emphasized' span. Lines: {:?}",
                lines
            );
        }

        #[test]
        fn markdown_inline_code_renders_with_distinct_style() {
            // Given TuiStreamHandler
            let mut handler = TuiStreamHandler::new(false);

            // When on_text("`code`\n") is called
            handler.on_text("`code`\n");

            // Then the text "code" appears with distinct styling (different from default)
            let lines = collect_lines(&handler);
            assert!(!lines.is_empty(), "Should have at least one line");

            let has_code_style = lines.iter().any(|line| {
                line.spans.iter().any(|span| {
                    span.content.contains("code")
                        && (span.style.fg.is_some() || span.style.bg.is_some())
                })
            });
            assert!(
                has_code_style,
                "Should have styled 'code' span. Lines: {:?}",
                lines
            );
        }

        #[test]
        fn markdown_header_renders_content() {
            // Given TuiStreamHandler
            let mut handler = TuiStreamHandler::new(false);

            // When on_text("## Section Title\n") is called
            handler.on_text("## Section Title\n");

            // Then "Section Title" appears in the output
            // Note: termimad applies ANSI styling to headers
            let lines = collect_lines(&handler);
            assert!(!lines.is_empty(), "Should have at least one line");

            let has_header_content = lines.iter().any(|line| {
                line.spans
                    .iter()
                    .any(|span| span.content.contains("Section Title"))
            });
            assert!(
                has_header_content,
                "Should have header content. Lines: {:?}",
                lines
            );
        }

        #[test]
        fn markdown_streaming_continuity_handles_split_formatting() {
            // Given TuiStreamHandler
            let mut handler = TuiStreamHandler::new(false);

            // When markdown arrives in chunks: "**bo" then "ld**\n"
            handler.on_text("**bo");
            handler.on_text("ld**\n");

            // Then the complete "bold" text renders with BOLD modifier
            let lines = collect_lines(&handler);

            let has_bold = lines.iter().any(|line| {
                line.spans
                    .iter()
                    .any(|span| span.style.add_modifier.contains(Modifier::BOLD))
            });
            assert!(
                has_bold,
                "Split markdown should still render bold. Lines: {:?}",
                lines
            );
        }

        #[test]
        fn markdown_mixed_content_renders_correctly() {
            // Given TuiStreamHandler
            let mut handler = TuiStreamHandler::new(false);

            // When on_text() receives mixed markdown
            handler.on_text("Normal **bold** and *italic* text\n");

            // Then appropriate spans have appropriate styling
            let lines = collect_lines(&handler);
            assert!(!lines.is_empty(), "Should have at least one line");

            let has_bold = lines.iter().any(|line| {
                line.spans.iter().any(|span| {
                    span.content.contains("bold")
                        && span.style.add_modifier.contains(Modifier::BOLD)
                })
            });
            let has_italic = lines.iter().any(|line| {
                line.spans.iter().any(|span| {
                    span.content.contains("italic")
                        && span.style.add_modifier.contains(Modifier::ITALIC)
                })
            });

            assert!(has_bold, "Should have bold span. Lines: {:?}", lines);
            assert!(has_italic, "Should have italic span. Lines: {:?}", lines);
        }

        #[test]
        fn markdown_tool_call_styling_preserved() {
            // Given TuiStreamHandler with markdown text then tool call
            let mut handler = TuiStreamHandler::new(false);

            // When markdown text followed by tool call
            handler.on_text("**bold**\n");
            handler.on_tool_call("Read", "id", &json!({"file_path": "src/main.rs"}));

            // Then tool call still has blue styling
            let lines = collect_lines(&handler);
            assert!(lines.len() >= 2, "Should have at least 2 lines");

            // Last line should be the tool call with blue color
            let tool_line = lines.last().unwrap();
            let has_blue = tool_line
                .spans
                .iter()
                .any(|span| span.style.fg == Some(Color::Blue));
            assert!(
                has_blue,
                "Tool call should preserve blue styling. Line: {:?}",
                tool_line
            );
        }

        #[test]
        fn markdown_error_styling_preserved() {
            // Given TuiStreamHandler with markdown text then error
            let mut handler = TuiStreamHandler::new(false);

            // When markdown text followed by error
            handler.on_text("**bold**\n");
            handler.on_error("Something went wrong");

            // Then error still has red styling
            let lines = collect_lines(&handler);
            assert!(lines.len() >= 2, "Should have at least 2 lines");

            // Last line should be the error with red color
            let error_line = lines.last().unwrap();
            let has_red = error_line
                .spans
                .iter()
                .any(|span| span.style.fg == Some(Color::Red));
            assert!(
                has_red,
                "Error should preserve red styling. Line: {:?}",
                error_line
            );
        }

        #[test]
        fn markdown_partial_formatting_does_not_crash() {
            // Given TuiStreamHandler
            let mut handler = TuiStreamHandler::new(false);

            // When incomplete markdown is sent and flushed
            handler.on_text("**unclosed bold");
            handler.flush_text_buffer();

            // Then no panic occurs and text is present
            let lines = collect_lines(&handler);
            // Should have some output (either the partial text or nothing)
            // Main assertion is that we didn't panic
            let _ = lines; // Use the variable to avoid warning
        }

        // =====================================================================
        // ANSI Color Preservation Tests
        // =====================================================================

        #[test]
        fn ansi_green_text_produces_green_style() {
            // Given TuiStreamHandler
            let mut handler = TuiStreamHandler::new(false);

            // When on_text receives ANSI green text
            handler.on_text("\x1b[32mgreen text\x1b[0m\n");

            // Then the text should have green foreground color
            let lines = collect_lines(&handler);
            assert!(!lines.is_empty(), "Should have at least one line");

            let has_green = lines.iter().any(|line| {
                line.spans
                    .iter()
                    .any(|span| span.style.fg == Some(Color::Green))
            });
            assert!(
                has_green,
                "Should have green styled span. Lines: {:?}",
                lines
            );
        }

        #[test]
        fn ansi_bold_text_produces_bold_modifier() {
            // Given TuiStreamHandler
            let mut handler = TuiStreamHandler::new(false);

            // When on_text receives ANSI bold text
            handler.on_text("\x1b[1mbold text\x1b[0m\n");

            // Then the text should have BOLD modifier
            let lines = collect_lines(&handler);
            assert!(!lines.is_empty(), "Should have at least one line");

            let has_bold = lines.iter().any(|line| {
                line.spans
                    .iter()
                    .any(|span| span.style.add_modifier.contains(Modifier::BOLD))
            });
            assert!(has_bold, "Should have bold styled span. Lines: {:?}", lines);
        }

        #[test]
        fn ansi_mixed_styles_preserved() {
            // Given TuiStreamHandler
            let mut handler = TuiStreamHandler::new(false);

            // When on_text receives mixed ANSI styles (bold + green)
            handler.on_text("\x1b[1;32mbold green\x1b[0m normal\n");

            // Then the text should have appropriate styles
            let lines = collect_lines(&handler);
            assert!(!lines.is_empty(), "Should have at least one line");

            // Check for green color
            let has_styled = lines.iter().any(|line| {
                line.spans.iter().any(|span| {
                    span.style.fg == Some(Color::Green)
                        || span.style.add_modifier.contains(Modifier::BOLD)
                })
            });
            assert!(
                has_styled,
                "Should have styled span with color or bold. Lines: {:?}",
                lines
            );
        }

        #[test]
        fn ansi_plain_text_renders_without_crash() {
            // Given TuiStreamHandler
            let mut handler = TuiStreamHandler::new(false);

            // When on_text receives plain text (no ANSI)
            handler.on_text("plain text without ansi\n");

            // Then text renders normally (fallback to markdown)
            let lines = collect_lines(&handler);
            assert!(!lines.is_empty(), "Should have at least one line");

            let full_text: String = lines.iter().map(|l| l.to_string()).collect();
            assert!(
                full_text.contains("plain text"),
                "Should contain the text. Lines: {:?}",
                lines
            );
        }

        #[test]
        fn ansi_red_error_text_produces_red_style() {
            // Given TuiStreamHandler
            let mut handler = TuiStreamHandler::new(false);

            // When on_text receives ANSI red text (like error output)
            handler.on_text("\x1b[31mError: something failed\x1b[0m\n");

            // Then the text should have red foreground color
            let lines = collect_lines(&handler);
            assert!(!lines.is_empty(), "Should have at least one line");

            let has_red = lines.iter().any(|line| {
                line.spans
                    .iter()
                    .any(|span| span.style.fg == Some(Color::Red))
            });
            assert!(has_red, "Should have red styled span. Lines: {:?}", lines);
        }

        #[test]
        fn ansi_cyan_text_produces_cyan_style() {
            // Given TuiStreamHandler
            let mut handler = TuiStreamHandler::new(false);

            // When on_text receives ANSI cyan text
            handler.on_text("\x1b[36mcyan text\x1b[0m\n");

            // Then the text should have cyan foreground color
            let lines = collect_lines(&handler);
            assert!(!lines.is_empty(), "Should have at least one line");

            let has_cyan = lines.iter().any(|line| {
                line.spans
                    .iter()
                    .any(|span| span.style.fg == Some(Color::Cyan))
            });
            assert!(has_cyan, "Should have cyan styled span. Lines: {:?}", lines);
        }

        #[test]
        fn ansi_underline_produces_underline_modifier() {
            // Given TuiStreamHandler
            let mut handler = TuiStreamHandler::new(false);

            // When on_text receives ANSI underlined text
            handler.on_text("\x1b[4munderlined\x1b[0m\n");

            // Then the text should have UNDERLINED modifier
            let lines = collect_lines(&handler);
            assert!(!lines.is_empty(), "Should have at least one line");

            let has_underline = lines.iter().any(|line| {
                line.spans
                    .iter()
                    .any(|span| span.style.add_modifier.contains(Modifier::UNDERLINED))
            });
            assert!(
                has_underline,
                "Should have underlined styled span. Lines: {:?}",
                lines
            );
        }

        #[test]
        fn ansi_multiline_preserves_colors() {
            // Given TuiStreamHandler
            let mut handler = TuiStreamHandler::new(false);

            // When on_text receives multiple ANSI-colored lines
            handler.on_text("\x1b[32mline 1 green\x1b[0m\n\x1b[31mline 2 red\x1b[0m\n");

            // Then both colors should be present
            let lines = collect_lines(&handler);
            assert!(lines.len() >= 2, "Should have at least two lines");

            let has_green = lines.iter().any(|line| {
                line.spans
                    .iter()
                    .any(|span| span.style.fg == Some(Color::Green))
            });
            let has_red = lines.iter().any(|line| {
                line.spans
                    .iter()
                    .any(|span| span.style.fg == Some(Color::Red))
            });

            assert!(has_green, "Should have green line. Lines: {:?}", lines);
            assert!(has_red, "Should have red line. Lines: {:?}", lines);
        }
    }
}

// =========================================================================
// ANSI Detection Tests (module-level)
// =========================================================================

#[cfg(test)]
mod ansi_detection_tests {
    use super::*;

    #[test]
    fn contains_ansi_with_color_code() {
        assert!(contains_ansi("\x1b[32mgreen\x1b[0m"));
    }

    #[test]
    fn contains_ansi_with_bold() {
        assert!(contains_ansi("\x1b[1mbold\x1b[0m"));
    }

    #[test]
    fn contains_ansi_plain_text_returns_false() {
        assert!(!contains_ansi("hello world"));
    }

    #[test]
    fn contains_ansi_markdown_returns_false() {
        assert!(!contains_ansi("**bold** and *italic*"));
    }

    #[test]
    fn contains_ansi_empty_string_returns_false() {
        assert!(!contains_ansi(""));
    }

    #[test]
    fn contains_ansi_with_escape_in_middle() {
        assert!(contains_ansi("prefix \x1b[31mred\x1b[0m suffix"));
    }
}
