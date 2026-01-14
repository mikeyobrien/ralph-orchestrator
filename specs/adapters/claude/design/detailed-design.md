# Claude Adapter Streaming Output: Detailed Design

---
status: draft
created: 2026-01-14
related:
  - ../research/implementation-gap-analysis.md
  - ../research/adapter-streaming-analysis.md
  - ../idea-honing.md
---

## Overview

This document describes the design for real-time streaming output from the Claude adapter in non-interactive mode (`ralph run`). The goal is to give users visibility into Claude's progress as it works, addressing the problem where "Ralph can be going in the wrong direction for long periods and the user does not know."

## Detailed Requirements

### Output Visibility

| Mode | What's Displayed |
|------|------------------|
| **Default** | Assistant text, tool invocations |
| **Verbose** | Everything: assistant text, tool invocations, tool results, usage stats, end summary |
| **Quiet** | Nothing (suppressed for CI/scripting) |

### Output Format

Plain text streaming:
```
Claude: I'll start by reading the file...
[Tool] Read: src/main.rs
Claude: Now I'll make the changes...
[Tool] Edit: src/main.rs
```

### Verbosity Control

Idiomatic precedence (highest to lowest):
1. CLI flag: `--verbose` / `-v` or `--quiet` / `-q`
2. Environment variable: `RALPH_VERBOSE=1` or `RALPH_QUIET=1`
3. Config file: `verbose: true` or `quiet: true`

### Error Handling

- Errors displayed inline in the stream where they occur
- Errors also written to stderr for Unix-idiomatic separation
- Enables `ralph run -P PROMPT.md 2>errors.log`

### Malformed JSON Handling

- Skip silently, continue processing
- Log skipped lines at DEBUG/TRACE level for troubleshooting

### Scope

- **Non-interactive only:** Affects `ralph run` command
- **Claude-only:** Other adapters unchanged
- **Always-on:** Streaming is default, `--quiet` to opt-out

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────────┐
│                        New Architecture                                  │
├─────────────────────────────────────────────────────────────────────────┤
│                                                                         │
│  CliBackend              PtyExecutor              StreamHandler         │
│  ┌────────────┐         ┌─────────────┐         ┌──────────────┐       │
│  │ claude()   │────────▶│ run_observe │────────▶│ on_text()    │       │
│  │            │         │             │         │ on_tool()    │       │
│  │ +format:   │         │ +parse_json │         │ on_error()   │       │
│  │  StreamJson│         │  lines      │         │ on_complete()│       │
│  └────────────┘         └─────────────┘         └──────────────┘       │
│                                │                        │               │
│                                ▼                        ▼               │
│                         ┌─────────────┐         ┌──────────────┐       │
│                         │ JsonStream  │         │ stdout/stderr│       │
│                         │ Parser      │         │ output       │       │
│                         └─────────────┘         └──────────────┘       │
│                                                                         │
└─────────────────────────────────────────────────────────────────────────┘
```

### Data Flow

1. `CliBackend::claude()` adds `--output-format stream-json` to args
2. `PtyExecutor::run_observe()` spawns Claude with PTY
3. Output read line-by-line from PTY stdout
4. Each line passed to `JsonStreamParser::parse_line()`
5. Parsed events dispatched to `StreamHandler` implementation
6. Handler formats and writes to stdout/stderr based on verbosity

## Components and Interfaces

### 1. OutputFormat Enum

**Location:** `crates/ralph-adapters/src/cli_backend.rs`

```rust
/// Output format supported by a CLI backend
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OutputFormat {
    /// Plain text output (default for most adapters)
    #[default]
    Text,
    /// Newline-delimited JSON stream (Claude with --output-format stream-json)
    StreamJson,
}
```

### 2. Updated CliBackend

**Location:** `crates/ralph-adapters/src/cli_backend.rs`

```rust
pub struct CliBackend {
    pub command: String,
    pub args: Vec<String>,
    pub prompt_mode: PromptMode,
    pub prompt_flag: Option<String>,
    pub output_format: OutputFormat,  // NEW
}

impl CliBackend {
    pub fn claude() -> Self {
        Self {
            command: "claude".to_string(),
            args: vec![
                "--dangerously-skip-permissions".to_string(),
                "--output-format".to_string(),
                "stream-json".to_string(),
            ],
            prompt_mode: PromptMode::Stdin,
            prompt_flag: None,
            output_format: OutputFormat::StreamJson,
        }
    }

    // Other adapters remain unchanged with OutputFormat::Text
}
```

### 3. Claude Stream Event Types

**Location:** `crates/ralph-adapters/src/claude_stream.rs` (new file)

```rust
/// Events emitted by Claude's --output-format stream-json
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClaudeStreamEvent {
    /// Session initialization
    System {
        session_id: String,
        model: String,
        #[serde(default)]
        tools: Vec<serde_json::Value>,
    },

    /// Claude's response (text or tool use)
    Assistant {
        message: AssistantMessage,
        #[serde(default)]
        usage: Option<Usage>,
    },

    /// Tool results returned to Claude
    User {
        message: UserMessage,
    },

    /// Session complete
    Result {
        duration_ms: u64,
        total_cost_usd: f64,
        num_turns: u32,
        is_error: bool,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssistantMessage {
    pub content: Vec<ContentBlock>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlock {
    Text { text: String },
    ToolUse { id: String, name: String, input: serde_json::Value },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Usage {
    pub input_tokens: u64,
    pub output_tokens: u64,
}
```

### 4. Stream Parser

**Location:** `crates/ralph-adapters/src/claude_stream.rs`

```rust
/// Parses NDJSON lines from Claude's stream output
pub struct ClaudeStreamParser;

impl ClaudeStreamParser {
    /// Parse a single line of NDJSON output
    /// Returns None for malformed lines (logged at debug level)
    pub fn parse_line(line: &str) -> Option<ClaudeStreamEvent> {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return None;
        }

        match serde_json::from_str::<ClaudeStreamEvent>(trimmed) {
            Ok(event) => Some(event),
            Err(e) => {
                tracing::debug!(
                    "Skipping malformed JSON line: {} (error: {})",
                    truncate(trimmed, 100),
                    e
                );
                None
            }
        }
    }
}
```

### 5. Stream Handler Trait

**Location:** `crates/ralph-adapters/src/stream_handler.rs` (new file)

```rust
/// Handler for streaming output events
pub trait StreamHandler: Send {
    /// Called when Claude emits text
    fn on_text(&mut self, text: &str);

    /// Called when Claude invokes a tool
    fn on_tool_call(&mut self, name: &str, id: &str);

    /// Called when a tool returns results (verbose only)
    fn on_tool_result(&mut self, id: &str, output: &str);

    /// Called when an error occurs
    fn on_error(&mut self, error: &str);

    /// Called when session completes (verbose only)
    fn on_complete(&mut self, result: &SessionResult);
}

pub struct SessionResult {
    pub duration_ms: u64,
    pub total_cost_usd: f64,
    pub num_turns: u32,
    pub is_error: bool,
}
```

### 6. Console Stream Handler

**Location:** `crates/ralph-adapters/src/stream_handler.rs`

```rust
/// Writes streaming output to stdout/stderr
pub struct ConsoleStreamHandler {
    verbose: bool,
    stdout: io::Stdout,
    stderr: io::Stderr,
}

impl ConsoleStreamHandler {
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
        writeln!(self.stdout, "Claude: {}", text).ok();
    }

    fn on_tool_call(&mut self, name: &str, _id: &str) {
        writeln!(self.stdout, "[Tool] {}", name).ok();
    }

    fn on_tool_result(&mut self, _id: &str, output: &str) {
        if self.verbose {
            writeln!(self.stdout, "[Result] {}", truncate(output, 200)).ok();
        }
    }

    fn on_error(&mut self, error: &str) {
        // Write to both stdout (inline) and stderr (for separation)
        writeln!(self.stdout, "[Error] {}", error).ok();
        writeln!(self.stderr, "[Error] {}", error).ok();
    }

    fn on_complete(&mut self, result: &SessionResult) {
        if self.verbose {
            writeln!(
                self.stdout,
                "\n--- Session Complete ---\nDuration: {}ms | Cost: ${:.4} | Turns: {}",
                result.duration_ms,
                result.total_cost_usd,
                result.num_turns
            ).ok();
        }
    }
}
```

### 7. Quiet Handler (No-Op)

```rust
/// Suppresses all streaming output
pub struct QuietStreamHandler;

impl StreamHandler for QuietStreamHandler {
    fn on_text(&mut self, _: &str) {}
    fn on_tool_call(&mut self, _: &str, _: &str) {}
    fn on_tool_result(&mut self, _: &str, _: &str) {}
    fn on_error(&mut self, _: &str) {}
    fn on_complete(&mut self, _: &SessionResult) {}
}
```

### 8. Updated PTY Executor

**Location:** `crates/ralph-adapters/src/pty_executor.rs`

Add streaming support to `run_observe()`:

```rust
impl PtyExecutor {
    /// Run command and observe output with optional streaming handler
    pub fn run_observe_streaming<H: StreamHandler>(
        &self,
        prompt: &str,
        output_format: OutputFormat,
        handler: &mut H,
    ) -> io::Result<PtyExecutionResult> {
        let (pair, mut child, stdin_input, _temp_file) = self.spawn_pty(prompt)?;
        let mut reader = pair.master.try_clone_reader()?;

        // Buffer for line accumulation
        let mut line_buffer = String::new();
        let mut output = Vec::new();
        let mut buf = [0u8; 1024];

        loop {
            match reader.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    let chunk = &buf[..n];
                    output.extend_from_slice(chunk);

                    // For JSON streaming, parse lines as they complete
                    if output_format == OutputFormat::StreamJson {
                        if let Ok(text) = std::str::from_utf8(chunk) {
                            line_buffer.push_str(text);

                            // Process complete lines
                            while let Some(newline_pos) = line_buffer.find('\n') {
                                let line = line_buffer[..newline_pos].to_string();
                                line_buffer = line_buffer[newline_pos + 1..].to_string();

                                if let Some(event) = ClaudeStreamParser::parse_line(&line) {
                                    dispatch_event(event, handler);
                                }
                            }
                        }
                    }
                }
                Err(e) if e.kind() == io::ErrorKind::WouldBlock => continue,
                Err(e) => return Err(e),
            }
        }

        // ... rest of result handling
    }
}

fn dispatch_event<H: StreamHandler>(event: ClaudeStreamEvent, handler: &mut H) {
    match event {
        ClaudeStreamEvent::Assistant { message, .. } => {
            for block in message.content {
                match block {
                    ContentBlock::Text { text } => handler.on_text(&text),
                    ContentBlock::ToolUse { name, id, .. } => handler.on_tool_call(&name, &id),
                }
            }
        }
        ClaudeStreamEvent::User { message } => {
            // Tool results - verbose mode handled by handler
            for block in message.content {
                if let ContentBlock::ToolResult { tool_use_id, content, .. } = block {
                    handler.on_tool_result(&tool_use_id, &content);
                }
            }
        }
        ClaudeStreamEvent::Result { duration_ms, total_cost_usd, num_turns, is_error } => {
            if is_error {
                handler.on_error("Session ended with error");
            }
            handler.on_complete(&SessionResult {
                duration_ms,
                total_cost_usd,
                num_turns,
                is_error,
            });
        }
        ClaudeStreamEvent::System { .. } => {
            // Session initialization - could log in verbose mode
        }
    }
}
```

## Data Models

### Verbosity Configuration

```rust
/// Verbosity level for ralph run output
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Verbosity {
    /// Suppress all streaming output
    Quiet,
    /// Show assistant text and tool invocations (default)
    #[default]
    Normal,
    /// Show everything including tool results and session summary
    Verbose,
}

impl Verbosity {
    /// Resolve verbosity from CLI args, env, and config (in precedence order)
    pub fn resolve(cli_verbose: bool, cli_quiet: bool, config: &Config) -> Self {
        // CLI flags take precedence
        if cli_quiet {
            return Verbosity::Quiet;
        }
        if cli_verbose {
            return Verbosity::Verbose;
        }

        // Environment variables
        if std::env::var("RALPH_QUIET").is_ok() {
            return Verbosity::Quiet;
        }
        if std::env::var("RALPH_VERBOSE").is_ok() {
            return Verbosity::Verbose;
        }

        // Config file
        if config.quiet.unwrap_or(false) {
            return Verbosity::Quiet;
        }
        if config.verbose.unwrap_or(false) {
            return Verbosity::Verbose;
        }

        Verbosity::Normal
    }
}
```

## Error Handling

### Malformed JSON

- Lines that fail JSON parsing are skipped silently
- Logged at `DEBUG` level with truncated content and error message
- Processing continues with next line

### PTY Read Errors

- `WouldBlock` errors are retried (non-blocking I/O)
- Other errors propagate up and fail the iteration
- Partial output accumulated before error is preserved

### Claude Session Errors

- `result.is_error: true` triggers `on_error()` callback
- Error message written to both stdout (inline) and stderr
- Iteration marked as failed in Ralph's loop

## Testing Strategy

### Unit Tests

1. **JSON parsing** — Test `ClaudeStreamParser::parse_line()` with valid events, malformed JSON, empty lines
2. **Event dispatch** — Test `dispatch_event()` routes to correct handler methods
3. **Verbosity resolution** — Test precedence order (CLI > env > config)

### Integration Tests

1. **Mock Claude output** — Spawn process that emits fake NDJSON, verify handler receives correct events
2. **Error handling** — Test with mixed valid/invalid JSON lines
3. **Quiet mode** — Verify no output when `--quiet` specified

### Manual Testing

1. Run `ralph run -P PROMPT.md` with Claude and verify streaming output
2. Test `--verbose` flag shows tool results and summary
3. Test `--quiet` flag suppresses all output
4. Verify errors appear in both stdout and stderr

## Appendices

### A. Technology Choices

| Choice | Rationale |
|--------|-----------|
| NDJSON (not framed codec) | Claude emits one JSON object per line; simple line parsing is sufficient |
| Trait-based handlers | Allows easy testing with mock handlers; extensible for future output targets (TUI, file) |
| Line buffering | PTY reads are chunked; need to reassemble complete lines before parsing |

### B. Research Findings

See `research/implementation-gap-analysis.md` for current vs. required implementation details.

See `research/adapter-streaming-analysis.md` for extensibility considerations across adapters.

### C. Alternative Approaches Considered

1. **tokio-util FramedRead** — More complex than needed; Claude uses simple newline delimiters
2. **Event bus integration** — Considered for TUI forwarding; deferred since scope is non-interactive only
3. **Async streaming** — PTY executor is currently sync; async would require larger refactor

### D. Future Considerations

1. **TUI integration** — If interactive mode wants structured events, extend `StreamHandler` to publish to EventBus
2. **Other adapters** — When Kiro/Codex add streaming flags, update their `OutputFormat` and add parsers
3. **Custom formatters** — Could add JSON output mode for `ralph run` for machine consumption
