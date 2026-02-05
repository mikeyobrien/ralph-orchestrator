# Design: Pi Agent Support for Ralph Orchestrator

## Overview

Add pi-coding-agent (`pi`) as a first-class backend in ralph-orchestrator with NDJSON stream parsing, structured tool call display, cost tracking, and per-hat configuration. Pi becomes the second backend (after Claude) with structured streaming output, giving it rich TUI/console display.

## Detailed Requirements

1. **CLI Backend**: `pi` as a named backend with headless (`pi()`) and interactive (`pi_interactive()`) constructors, registered in all backend resolution paths (`from_name`, `from_config`, `for_interactive_prompt`).
2. **Auto-Detection**: `pi` added last in `DEFAULT_PRIORITY`. Detection via `pi --version`.
3. **NDJSON Stream Parser**: New `PiStreamParser` and `PiStreamEvent` types in `ralph-adapters` that parse pi's `--mode json` output and dispatch to `StreamHandler`.
4. **Output Format**: New `OutputFormat::PiStreamJson` variant, branched in `PtyExecutor::run_observe_streaming()`.
5. **Cost Tracking**: Accumulate `turn_end.message.usage.cost.total` across turns. Synthesize `SessionResult` for `on_complete()`.
6. **Tool Call Display**: Use `tool_execution_start` for `on_tool_call()`, `tool_execution_end` for `on_tool_result()`.
7. **Thinking Output**: Stream `thinking_delta` events to handler only in verbose mode.
8. **Text Extraction**: Accumulate `text_delta` content into `extracted_text` for Ralph's `EventParser` (LOOP_COMPLETE detection).
9. **Configuration**: Pi-specific options (provider, model, thinking, extensions, skills) via pass-through args using existing `NamedWithArgs` hat backend type.
10. **Interactive Mode**: `pi_interactive()` constructor for `ralph plan` — runs pi TUI with initial prompt.

## Architecture Overview

```
┌─────────────────────────────────────────────────────────┐
│                    ralph-cli                              │
│  loop_runner.rs                                          │
│  ┌─────────────────────────────────────────────────┐    │
│  │ run_observe_streaming()                          │    │
│  │                                                   │    │
│  │  match output_format {                           │    │
│  │    StreamJson    → ClaudeStreamParser → dispatch  │    │
│  │    PiStreamJson  → PiStreamParser    → dispatch  │ ◄──NEW
│  │    Text          → raw passthrough               │    │
│  │  }                                                │    │
│  └─────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────┐
│                  ralph-adapters                           │
│                                                           │
│  cli_backend.rs                                          │
│  ┌───────────────────────────────────────────────┐      │
│  │ CliBackend::pi()          ← headless mode      │ ◄──NEW
│  │ CliBackend::pi_interactive() ← TUI mode        │ ◄──NEW
│  │ OutputFormat::PiStreamJson                      │ ◄──NEW
│  └───────────────────────────────────────────────┘      │
│                                                           │
│  pi_stream.rs                                    ◄──NEW  │
│  ┌───────────────────────────────────────────────┐      │
│  │ PiStreamEvent (enum)                           │      │
│  │ PiStreamParser::parse_line()                   │      │
│  │ dispatch_pi_stream_event()                     │      │
│  └───────────────────────────────────────────────┘      │
│                                                           │
│  auto_detect.rs                                          │
│  ┌───────────────────────────────────────────────┐      │
│  │ DEFAULT_PRIORITY += "pi"                       │ ◄──MOD
│  └───────────────────────────────────────────────┘      │
└─────────────────────────────────────────────────────────┘
```

## Components and Interfaces

### 1. CliBackend Constructors (`cli_backend.rs`)

**`CliBackend::pi()`** — Headless execution:
```rust
pub fn pi() -> Self {
    Self {
        command: "pi".to_string(),
        args: vec![
            "-p".to_string(),
            "--mode".to_string(),
            "json".to_string(),
            "--no-session".to_string(),
        ],
        prompt_mode: PromptMode::Arg,
        prompt_flag: None,  // Positional argument
        output_format: OutputFormat::PiStreamJson,
    }
}
```

**`CliBackend::pi_interactive()`** — TUI with initial prompt:
```rust
pub fn pi_interactive() -> Self {
    Self {
        command: "pi".to_string(),
        args: vec![
            "--no-session".to_string(),
        ],
        prompt_mode: PromptMode::Arg,
        prompt_flag: None,  // Positional argument
        output_format: OutputFormat::Text,
    }
}
```

Registration points:
- `from_name("pi")` → `Ok(Self::pi())`
- `from_config()` match arm for `"pi"`
- `for_interactive_prompt("pi")` → `Ok(Self::pi_interactive())`

### 2. PiStreamEvent (`pi_stream.rs`)

```rust
/// Events from pi's `--mode json` NDJSON output.
///
/// Only the events Ralph needs are modeled. All other event types
/// (session, agent_start, turn_start, message_start, message_end,
/// message_update sub-types for toolcall_*, text_start, text_end, done)
/// are captured by the `Other` variant and ignored.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PiStreamEvent {
    /// Streaming text/thinking deltas from assistant.
    MessageUpdate {
        #[serde(rename = "assistantMessageEvent")]
        assistant_message_event: PiAssistantEvent,
    },

    /// Tool begins execution.
    ToolExecutionStart {
        #[serde(rename = "toolCallId")]
        tool_call_id: String,
        #[serde(rename = "toolName")]
        tool_name: String,
        args: serde_json::Value,
    },

    /// Tool completes execution.
    ToolExecutionEnd {
        #[serde(rename = "toolCallId")]
        tool_call_id: String,
        #[serde(rename = "toolName")]
        tool_name: String,
        result: PiToolResult,
        #[serde(rename = "isError")]
        is_error: bool,
    },

    /// Turn completes — contains per-turn usage/cost.
    TurnEnd {
        message: Option<PiTurnMessage>,
    },

    /// All other events (session, agent_start, turn_start, message_start,
    /// message_end, tool_execution_update, etc.)
    #[serde(other)]
    Other,
}
```

**Sub-types:**

```rust
/// Assistant message event within a message_update.
/// Only text_delta, thinking_delta, and error are actionable.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PiAssistantEvent {
    TextDelta {
        delta: String,
    },
    ThinkingDelta {
        delta: String,
    },
    Error {
        reason: String,
    },
    /// All other sub-types (text_start, text_end, thinking_start, thinking_end,
    /// toolcall_start, toolcall_delta, toolcall_end, done)
    #[serde(other)]
    Other,
}

/// Tool execution result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PiToolResult {
    pub content: Vec<PiContentBlock>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum PiContentBlock {
    Text { text: String },
    #[serde(other)]
    Other,
}

/// Message in turn_end — contains usage data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PiTurnMessage {
    #[serde(rename = "stopReason")]
    pub stop_reason: Option<String>,
    pub usage: Option<PiUsage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PiUsage {
    pub input: u64,
    pub output: u64,
    #[serde(rename = "cacheRead")]
    pub cache_read: u64,
    #[serde(rename = "cacheWrite")]
    pub cache_write: u64,
    pub cost: Option<PiCost>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PiCost {
    pub total: f64,
}
```

### 3. PiStreamParser (`pi_stream.rs`)

```rust
pub struct PiStreamParser;

impl PiStreamParser {
    /// Parse a single line of NDJSON output from pi.
    pub fn parse_line(line: &str) -> Option<PiStreamEvent> {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            return None;
        }
        match serde_json::from_str::<PiStreamEvent>(trimmed) {
            Ok(event) => Some(event),
            Err(e) => {
                tracing::debug!("Skipping malformed pi JSON: {} (error: {})", truncate(trimmed, 100), e);
                None
            }
        }
    }
}
```

### 4. dispatch_pi_stream_event (`pi_stream.rs`)

```rust
/// State accumulated across events for session summary.
pub struct PiSessionState {
    pub total_cost_usd: f64,
    pub num_turns: u32,
}

impl PiSessionState {
    pub fn new() -> Self {
        Self { total_cost_usd: 0.0, num_turns: 0 }
    }
}

/// Dispatch a pi stream event to the StreamHandler.
///
/// Accumulates cost/turn data in `state` for the final `on_complete()` call.
/// Appends text content to `extracted_text` for LOOP_COMPLETE detection.
fn dispatch_pi_stream_event<H: StreamHandler>(
    event: PiStreamEvent,
    handler: &mut H,
    extracted_text: &mut String,
    state: &mut PiSessionState,
    verbose: bool,
) {
    match event {
        PiStreamEvent::MessageUpdate { assistant_message_event } => {
            match assistant_message_event {
                PiAssistantEvent::TextDelta { delta } => {
                    handler.on_text(&delta);
                    extracted_text.push_str(&delta);
                }
                PiAssistantEvent::ThinkingDelta { delta } => {
                    if verbose {
                        handler.on_text(&delta);
                    }
                }
                PiAssistantEvent::Error { reason } => {
                    handler.on_error(&reason);
                }
                PiAssistantEvent::Other => {}
            }
        }
        PiStreamEvent::ToolExecutionStart { tool_name, tool_call_id, args } => {
            handler.on_tool_call(&tool_name, &tool_call_id, &args);
        }
        PiStreamEvent::ToolExecutionEnd { tool_call_id, result, is_error, .. } => {
            let output = result.content.iter()
                .filter_map(|b| match b {
                    PiContentBlock::Text { text } => Some(text.as_str()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("\n");
            if is_error {
                handler.on_error(&output);
            } else {
                handler.on_tool_result(&tool_call_id, &output);
            }
        }
        PiStreamEvent::TurnEnd { message } => {
            state.num_turns += 1;
            if let Some(msg) = &message {
                if let Some(usage) = &msg.usage {
                    if let Some(cost) = &usage.cost {
                        state.total_cost_usd += cost.total;
                    }
                }
            }
        }
        PiStreamEvent::Other => {}
    }
}
```

### 5. PtyExecutor Integration (`pty_executor.rs`)

In `run_observe_streaming()`, add a third branch:

```rust
let is_stream_json = output_format == OutputFormat::StreamJson;
let is_pi_stream = output_format == OutputFormat::PiStreamJson;

// In the data processing loop:
if is_stream_json {
    // Existing Claude parsing...
} else if is_pi_stream {
    line_buffer.push_str(text);
    while let Some(newline_pos) = line_buffer.find('\n') {
        let line = line_buffer[..newline_pos].to_string();
        line_buffer = line_buffer[newline_pos + 1..].to_string();
        if let Some(event) = PiStreamParser::parse_line(&line) {
            dispatch_pi_stream_event(event, handler, &mut extracted_text, &mut pi_state, verbose);
        }
    }
} else {
    handler.on_text(text);
}
```

After the event loop exits, synthesize `on_complete()`:

```rust
if is_pi_stream {
    handler.on_complete(&SessionResult {
        duration_ms: start_time.elapsed().as_millis() as u64,
        total_cost_usd: pi_state.total_cost_usd,
        num_turns: pi_state.num_turns,
        is_error: !success,
    });
}
```

### 6. Auto-Detection (`auto_detect.rs`)

```rust
pub const DEFAULT_PRIORITY: &[&str] = &[
    "claude", "kiro", "gemini", "codex", "amp", "copilot", "opencode", "pi",
];
```

No `detection_command()` mapping needed — binary name matches backend name.

### 7. OutputFormat Extension (`cli_backend.rs`)

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OutputFormat {
    #[default]
    Text,
    StreamJson,
    PiStreamJson,
}
```

## Data Models

### Pi NDJSON Events (Input)

See [research/06-pi-ndjson-schema.md](research/06-pi-ndjson-schema.md) for the complete schema.

Events consumed by the parser:
| Event | Fields used |
|-------|-------------|
| `message_update` (text_delta) | `assistantMessageEvent.delta` |
| `message_update` (thinking_delta) | `assistantMessageEvent.delta` (verbose only) |
| `message_update` (error) | `assistantMessageEvent.reason` |
| `tool_execution_start` | `toolName`, `toolCallId`, `args` |
| `tool_execution_end` | `toolCallId`, `result.content[].text`, `isError` |
| `turn_end` | `message.usage.cost.total`, `message.stopReason` |

Events ignored: `session`, `agent_start`, `turn_start`, `message_start`, `message_end`, `tool_execution_update`, all other `message_update` sub-types.

### StreamHandler Output (Existing)

No changes to the `StreamHandler` trait or `SessionResult` struct.

### Configuration (Existing)

No changes to `RalphConfig`, `CliConfig`, or `HatBackend`. Pi-specific options use existing `NamedWithArgs`:

```yaml
# ralph.yml
cli:
  backend: pi

# or per-hat
hats:
  planner:
    backend:
      type: pi
      args: ["--provider", "anthropic", "--model", "claude-sonnet-4", "--thinking", "medium"]
  builder:
    backend:
      type: pi
      args: ["--provider", "openai-codex", "--model", "gpt-5.2-codex"]
```

## Error Handling

- **Malformed JSON lines**: Logged at debug level, skipped (matches Claude parser behavior).
- **Missing usage data**: If `turn_end` lacks `message.usage`, cost stays at 0. No error.
- **Pi process failure**: Handled by existing PTY executor error paths. Non-zero exit = `is_error: true`.
- **Pi not installed**: Auto-detection skips it. Explicit `backend: pi` with missing binary produces the existing `NoBackendError` with install instructions.

## Acceptance Criteria

### Backend Registration
- **Given** a ralph.yml with `cli.backend: pi`, **when** Ralph resolves the backend, **then** it creates a `CliBackend` with command `pi`, args `["-p", "--mode", "json", "--no-session"]`, and `OutputFormat::PiStreamJson`.
- **Given** `backend: pi` with extra args `["--provider", "anthropic"]`, **when** Ralph resolves the backend, **then** the extra args are appended to the default args.

### Auto-Detection
- **Given** `agent: auto` and only `pi` is installed, **when** Ralph detects backends, **then** `pi` is selected.
- **Given** `agent: auto` and both `claude` and `pi` are installed, **when** Ralph detects backends, **then** `claude` is selected (higher priority).

### NDJSON Parsing
- **Given** pi emits a `message_update` with `text_delta`, **when** the parser processes it, **then** `handler.on_text(delta)` is called and `extracted_text` is updated.
- **Given** pi emits `tool_execution_start`, **when** the parser processes it, **then** `handler.on_tool_call(name, id, args)` is called.
- **Given** pi emits `tool_execution_end` with `isError: false`, **when** the parser processes it, **then** `handler.on_tool_result(id, output)` is called.
- **Given** pi emits `tool_execution_end` with `isError: true`, **when** the parser processes it, **then** `handler.on_error(output)` is called.
- **Given** pi emits a `message_update` with `thinking_delta` and verbose is true, **when** the parser processes it, **then** `handler.on_text(delta)` is called.
- **Given** pi emits a `message_update` with `thinking_delta` and verbose is false, **when** the parser processes it, **then** nothing is emitted.

### Cost Tracking
- **Given** pi emits 3 `turn_end` events with costs 0.05, 0.03, 0.01, **when** the session ends, **then** `on_complete()` receives `total_cost_usd: 0.09`.

### Turn Count
- **Given** pi emits 3 `turn_end` events, **when** the session ends, **then** `on_complete()` receives `num_turns: 3`.

### Duration
- **Given** pi runs for 5 seconds, **when** the session ends, **then** `on_complete()` receives `duration_ms` approximately 5000 (wall-clock).

### Interactive Mode
- **Given** `ralph plan` with `backend: pi`, **when** launching interactive mode, **then** pi is invoked as `pi --no-session "prompt"` (no `-p`, no `--mode json`).

### LOOP_COMPLETE Detection
- **Given** pi's text output contains `LOOP_COMPLETE`, **when** Ralph's `EventParser` processes `extracted_text`, **then** the completion is detected.

### Unknown Events
- **Given** pi emits a JSON line with an unknown `type`, **when** the parser processes it, **then** it is silently ignored (deserialized as `PiStreamEvent::Other`).

## Testing Strategy

### Unit Tests (`pi_stream.rs`)
- Parse each event type from JSON fixtures
- Verify `dispatch_pi_stream_event` calls correct `StreamHandler` methods
- Test cost accumulation across multiple `turn_end` events
- Test malformed JSON handling (skip, no panic)
- Test `#[serde(other)]` catches unknown event types
- Test thinking_delta gated by verbose flag
- Test tool error handling (`isError: true`)

### Unit Tests (`cli_backend.rs`)
- Test `CliBackend::pi()` command and args
- Test `CliBackend::pi_interactive()` command and args
- Test `from_name("pi")` resolution
- Test `for_interactive_prompt("pi")` resolution
- Test `from_config()` with `backend: "pi"`
- Test `from_hat_backend()` with `NamedWithArgs { type: "pi", args: [...] }`
- Test large prompt handling (pi should NOT use temp files — only Claude does)

### Unit Tests (`auto_detect.rs`)
- Test `pi` in `DEFAULT_PRIORITY`
- Test priority ordering (pi is last)

### Smoke Tests
- Record a pi session fixture (`pi -p --mode json --no-session "prompt" > fixture.jsonl`)
- Replay through `PiStreamParser` + `dispatch_pi_stream_event`
- Verify extracted_text, cost, turn count

### Integration Tests
- E2E test with mock pi binary (echo NDJSON fixtures to stdout)

## Appendices

### Technology Choices

- **Serde for parsing**: Pi's NDJSON maps cleanly to Rust enums with `#[serde(tag = "type")]`. The `#[serde(other)]` variant handles forward compatibility with new event types.
- **No new dependencies**: Uses existing `serde`, `serde_json`, `tracing` from `ralph-adapters`.

### Research Findings

See `research/` directory:
1. [Stream format comparison](research/01-stream-format-comparison.md) — Pi vs Claude NDJSON schemas
2. [Stream handler architecture](research/02-stream-handler-architecture.md) — Ralph's trait-based streaming
3. [RPC mode analysis](research/03-rpc-mode-analysis.md) — Deferred to v2
4. [Backend patterns](research/04-backend-patterns.md) — How other backends are implemented
5. [CLI flags](research/05-pi-cli-flags.md) — Pi's flag reference for Ralph
6. [NDJSON schema](research/06-pi-ndjson-schema.md) — Definitive pi event schema from real output

### Alternative Approaches

1. **Text-only backend (rejected)**: Could treat pi like Kiro/Gemini with raw text output. Rejected because pi's NDJSON gives structured tool calls, cost tracking, and rich TUI display for free.
2. **RPC mode (deferred)**: Could use pi's bidirectional RPC for persistent sessions and steering. Deferred — requires new executor type and architectural changes to Ralph's core loop.
3. **Unified StreamJson parser (rejected)**: Could auto-detect pi vs Claude from first JSON line. Rejected — fragile detection, cleaner to have explicit `PiStreamJson` variant.
4. **Structured config fields (deferred)**: Could add pi-specific fields to `HatBackend` enum (like `KiroAgent`). Deferred — `NamedWithArgs` works now, structured fields can be added later as sugar.
