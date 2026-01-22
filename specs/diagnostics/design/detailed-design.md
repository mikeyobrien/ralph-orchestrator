# Detailed Design: Diagnostic Logging for Ralph

## Overview

This design adds comprehensive diagnostic logging to Ralph that captures agent output, orchestration events, tracing logs, performance metrics, and errors. The system is opt-in via environment variable, writes structured JSONL files to timestamped directories, and provides complete visibility into Ralph's operation for debugging purposes.

## Detailed Requirements

### Functional Requirements

1. **Capture all diagnostic data types:**
   - Agent output (stripped text + parsed JSON events)
   - Event bus events (with full metadata)
   - Orchestration decisions (hat selection, backpressure, termination)
   - Tracing logs (info/debug/warn/error from existing macros)
   - Performance metrics (durations, latencies, token counts)
   - Errors and failures (parse errors, validation failures)

2. **File structure:**
   - Separate JSONL files by type
   - Stored in `.ralph/diagnostics/<timestamp>/`
   - Same behavior in TUI and non-TUI modes

3. **Activation:**
   - Opt-in via `RALPH_DIAGNOSTICS=1` environment variable
   - No performance impact when disabled

4. **Metadata per entry:**
   - Timestamp (ISO 8601)
   - Iteration number
   - Active hat
   - Type-specific fields

5. **Cleanup:**
   - Manual via `ralph clean --diagnostics`
   - Removes all diagnostic directories

6. **Crash safety:**
   - Incremental flush (no buffering)
   - Logs survive crashes mid-run

### Non-Functional Requirements

1. **Minimal overhead** when diagnostics disabled
2. **No stdout/stderr interference** in either mode
3. **Queryable** via standard Unix tools (`jq`, `grep`)
4. **Consistent** across TUI and non-TUI modes

## Architecture Overview

```
┌─────────────────────────────────────────────────────────────────────┐
│                         RALPH ORCHESTRATOR                          │
├─────────────────────────────────────────────────────────────────────┤
│                                                                     │
│  ┌─────────────┐    ┌─────────────┐    ┌─────────────┐             │
│  │   Agent     │    │  Event Bus  │    │ Event Loop  │             │
│  │  Executor   │    │             │    │             │             │
│  └──────┬──────┘    └──────┬──────┘    └──────┬──────┘             │
│         │                  │                  │                     │
│         ▼                  ▼                  ▼                     │
│  ┌─────────────────────────────────────────────────────┐           │
│  │              DiagnosticsCollector                    │           │
│  │  (Central coordinator - enabled via RALPH_DIAGNOSTICS)│          │
│  └──────────────────────────┬──────────────────────────┘           │
│                             │                                       │
│         ┌───────────────────┼───────────────────┐                  │
│         ▼                   ▼                   ▼                  │
│  ┌─────────────┐    ┌─────────────┐    ┌─────────────┐            │
│  │AgentOutput  │    │Orchestration│    │   Trace     │            │
│  │  Logger     │    │   Logger    │    │   Logger    │            │
│  └──────┬──────┘    └──────┬──────┘    └──────┬──────┘            │
│         │                  │                  │                    │
│         ▼                  ▼                  ▼                    │
│  ┌─────────────────────────────────────────────────────┐          │
│  │           .ralph/diagnostics/<timestamp>/            │          │
│  │  ┌────────────┬────────────┬────────────┬─────────┐ │          │
│  │  │agent-output│orchestration│   trace   │errors   │ │          │
│  │  │  .jsonl    │   .jsonl    │  .jsonl   │.jsonl   │ │          │
│  │  └────────────┴────────────┴────────────┴─────────┘ │          │
│  └─────────────────────────────────────────────────────┘          │
│                                                                    │
└────────────────────────────────────────────────────────────────────┘
```

### Key Design Decisions

1. **Central DiagnosticsCollector** — Single entry point that checks `RALPH_DIAGNOSTICS` once and manages all loggers
2. **Separate files by type** — Enables targeted queries without parsing unrelated data
3. **Observer pattern for events** — Hook into existing EventBus observer mechanism
4. **StreamHandler wrapper for agent output** — Intercept without modifying existing handlers
5. **Custom tracing Layer** — Capture existing log macros without changing call sites

## Components and Interfaces

### 1. DiagnosticsCollector

Central coordinator that initializes and manages all diagnostic loggers.

```rust
// crates/ralph-core/src/diagnostics/collector.rs

pub struct DiagnosticsCollector {
    enabled: bool,
    session_dir: PathBuf,
    agent_logger: Option<AgentOutputLogger>,
    orchestration_logger: Option<OrchestrationLogger>,
    trace_logger: Option<TraceLogger>,
    performance_logger: Option<PerformanceLogger>,
    error_logger: Option<ErrorLogger>,
}

impl DiagnosticsCollector {
    /// Create new collector. Checks RALPH_DIAGNOSTICS env var.
    pub fn new(base_dir: &Path) -> Self;

    /// Returns true if diagnostics are enabled
    pub fn is_enabled(&self) -> bool;

    /// Get the session directory path
    pub fn session_dir(&self) -> Option<&Path>;

    /// Create EventBus observer for orchestration events
    pub fn event_observer(&self) -> Option<impl Fn(&Event)>;

    /// Create StreamHandler wrapper for agent output
    pub fn wrap_stream_handler<H: StreamHandler>(&self, inner: H) -> impl StreamHandler;

    /// Log orchestration decision
    pub fn log_orchestration(&self, event: OrchestrationEvent);

    /// Log performance metric
    pub fn log_performance(&self, metric: PerformanceMetric);

    /// Log error
    pub fn log_error(&self, error: DiagnosticError);

    /// Flush all loggers (call on graceful shutdown)
    pub fn flush(&self);
}
```

### 2. AgentOutputLogger

Captures agent output as stripped text and parsed JSON events.

```rust
// crates/ralph-core/src/diagnostics/agent_output.rs

pub struct AgentOutputLogger {
    file: BufWriter<File>,
    iteration: u32,
    hat: String,
}

#[derive(Serialize)]
pub struct AgentOutputEntry {
    ts: String,           // ISO 8601
    iteration: u32,
    hat: String,
    #[serde(flatten)]
    content: AgentOutputContent,
}

#[derive(Serialize)]
#[serde(tag = "type")]
pub enum AgentOutputContent {
    #[serde(rename = "text")]
    Text { text: String },

    #[serde(rename = "tool_call")]
    ToolCall {
        name: String,
        id: String,
        input: serde_json::Value
    },

    #[serde(rename = "tool_result")]
    ToolResult {
        id: String,
        output: String
    },

    #[serde(rename = "error")]
    Error { message: String },

    #[serde(rename = "complete")]
    Complete {
        input_tokens: Option<u64>,
        output_tokens: Option<u64>,
    },
}

impl AgentOutputLogger {
    pub fn new(session_dir: &Path) -> io::Result<Self>;
    pub fn set_context(&mut self, iteration: u32, hat: &str);
    pub fn log(&mut self, content: AgentOutputContent);
    pub fn flush(&mut self);
}
```

### 3. DiagnosticStreamHandler

Wrapper that captures output while delegating to inner handler.

```rust
// crates/ralph-core/src/diagnostics/stream_handler.rs

pub struct DiagnosticStreamHandler<H: StreamHandler> {
    inner: H,
    logger: Arc<Mutex<AgentOutputLogger>>,
}

impl<H: StreamHandler> StreamHandler for DiagnosticStreamHandler<H> {
    fn on_text(&mut self, text: &str) {
        self.logger.lock().unwrap().log(AgentOutputContent::Text {
            text: text.to_string()
        });
        self.inner.on_text(text);
    }

    fn on_tool_call(&mut self, name: &str, id: &str, input: &serde_json::Value) {
        self.logger.lock().unwrap().log(AgentOutputContent::ToolCall {
            name: name.to_string(),
            id: id.to_string(),
            input: input.clone(),
        });
        self.inner.on_tool_call(name, id, input);
    }

    // ... similar for on_tool_result, on_error, on_complete
}
```

### 4. OrchestrationLogger

Captures orchestration decisions and event flow.

```rust
// crates/ralph-core/src/diagnostics/orchestration.rs

pub struct OrchestrationLogger {
    file: BufWriter<File>,
}

#[derive(Serialize)]
pub struct OrchestrationEntry {
    ts: String,
    iteration: u32,
    hat: String,
    event: String,
    details: serde_json::Value,
}

#[derive(Debug)]
pub enum OrchestrationEvent {
    HatSelected { hat: String, reason: String },
    EventPublished { topic: String, source: Option<String>, target: Option<String> },
    BackpressureTriggered { reason: String, evidence: String },
    IterationStarted { iteration: u32 },
    IterationCompleted { iteration: u32, duration_ms: u64 },
    LoopTerminated { reason: String, iterations: u32, duration_ms: u64 },
    TaskAbandoned { task: String, block_count: u32 },
}

impl OrchestrationLogger {
    pub fn new(session_dir: &Path) -> io::Result<Self>;
    pub fn log(&mut self, iteration: u32, hat: &str, event: OrchestrationEvent);
    pub fn flush(&mut self);
}
```

### 5. TraceLogger (Custom tracing Layer)

Captures existing `info!`/`debug!`/`warn!`/`error!` macros.

```rust
// crates/ralph-core/src/diagnostics/trace_layer.rs

use tracing_subscriber::Layer;

pub struct DiagnosticTraceLayer {
    file: Arc<Mutex<BufWriter<File>>>,
    iteration: Arc<AtomicU32>,
    hat: Arc<Mutex<String>>,
}

#[derive(Serialize)]
pub struct TraceEntry {
    ts: String,
    iteration: u32,
    hat: String,
    level: String,
    target: String,
    message: String,
    fields: serde_json::Value,
}

impl<S> Layer<S> for DiagnosticTraceLayer
where
    S: tracing::Subscriber,
{
    fn on_event(&self, event: &tracing::Event<'_>, _ctx: tracing_subscriber::layer::Context<'_, S>) {
        // Extract level, target, message, fields
        // Write to JSONL file
    }
}

impl DiagnosticTraceLayer {
    pub fn new(session_dir: &Path) -> io::Result<Self>;
    pub fn set_context(&self, iteration: u32, hat: &str);
}
```

### 6. PerformanceLogger

Captures timing and resource metrics.

```rust
// crates/ralph-core/src/diagnostics/performance.rs

pub struct PerformanceLogger {
    file: BufWriter<File>,
}

#[derive(Serialize)]
pub struct PerformanceEntry {
    ts: String,
    iteration: u32,
    hat: String,
    metric: String,
    value: f64,
    unit: String,
}

pub enum PerformanceMetric {
    IterationDuration { iteration: u32, duration_ms: u64 },
    AgentLatency { latency_ms: u64 },
    TokensUsed { input: u64, output: u64 },
    EventProcessingTime { topic: String, duration_ms: u64 },
}

impl PerformanceLogger {
    pub fn new(session_dir: &Path) -> io::Result<Self>;
    pub fn log(&mut self, iteration: u32, hat: &str, metric: PerformanceMetric);
    pub fn flush(&mut self);
}
```

### 7. ErrorLogger

Captures errors and failures.

```rust
// crates/ralph-core/src/diagnostics/errors.rs

pub struct ErrorLogger {
    file: BufWriter<File>,
}

#[derive(Serialize)]
pub struct ErrorEntry {
    ts: String,
    iteration: u32,
    hat: String,
    error_type: String,
    message: String,
    context: serde_json::Value,
}

pub enum DiagnosticError {
    ParseError { source: String, message: String, input: String },
    ValidationFailure { rule: String, message: String, evidence: String },
    BackendError { backend: String, message: String },
    Timeout { operation: String, duration_ms: u64 },
    MalformedEvent { line: String, error: String },
}

impl ErrorLogger {
    pub fn new(session_dir: &Path) -> io::Result<Self>;
    pub fn log(&mut self, iteration: u32, hat: &str, error: DiagnosticError);
    pub fn flush(&mut self);
}
```

### 8. Cleanup Command

Extension to `ralph clean` command.

```rust
// In crates/ralph-cli/src/main.rs

#[derive(Args)]
pub struct CleanArgs {
    /// Clean diagnostic logs
    #[arg(long)]
    diagnostics: bool,

    // ... existing args
}

fn clean_diagnostics(base_dir: &Path) -> io::Result<()> {
    let diagnostics_dir = base_dir.join(".ralph").join("diagnostics");
    if diagnostics_dir.exists() {
        fs::remove_dir_all(&diagnostics_dir)?;
        println!("Removed {}", diagnostics_dir.display());
    }
    Ok(())
}
```

## Data Models

### Directory Structure

```
.ralph/
├── events.jsonl           # (existing - unchanged)
├── diagnostics/
│   └── 2024-01-15T10-23-45/
│       ├── agent-output.jsonl
│       ├── orchestration.jsonl
│       ├── trace.jsonl
│       ├── performance.jsonl
│       └── errors.jsonl
```

### JSONL Schemas

#### agent-output.jsonl
```json
{"ts":"2024-01-15T10:23:45.123Z","iteration":1,"hat":"ralph","type":"text","text":"I'll help you implement..."}
{"ts":"2024-01-15T10:23:46.456Z","iteration":1,"hat":"ralph","type":"tool_call","name":"Read","id":"tool_1","input":{"file_path":"/src/main.rs"}}
{"ts":"2024-01-15T10:23:47.789Z","iteration":1,"hat":"ralph","type":"tool_result","id":"tool_1","output":"fn main() {...}"}
{"ts":"2024-01-15T10:23:50.000Z","iteration":1,"hat":"ralph","type":"complete","input_tokens":1500,"output_tokens":800}
```

#### orchestration.jsonl
```json
{"ts":"2024-01-15T10:23:45.000Z","iteration":1,"hat":"loop","event":"iteration_started","details":{"iteration":1}}
{"ts":"2024-01-15T10:23:45.001Z","iteration":1,"hat":"loop","event":"hat_selected","details":{"hat":"ralph","reason":"pending_events"}}
{"ts":"2024-01-15T10:23:50.000Z","iteration":1,"hat":"loop","event":"event_published","details":{"topic":"build.done","source":"ralph"}}
{"ts":"2024-01-15T10:23:50.001Z","iteration":1,"hat":"loop","event":"backpressure_triggered","details":{"reason":"missing_evidence","evidence":"tests: missing"}}
```

#### trace.jsonl
```json
{"ts":"2024-01-15T10:23:45.000Z","iteration":1,"hat":"loop","level":"info","target":"ralph_core::event_loop","message":"Starting iteration","fields":{}}
{"ts":"2024-01-15T10:23:45.500Z","iteration":1,"hat":"ralph","level":"debug","target":"ralph_adapters::pty_executor","message":"PTY output received","fields":{"bytes":1024}}
{"ts":"2024-01-15T10:23:46.000Z","iteration":1,"hat":"ralph","level":"warn","target":"ralph_core::event_parser","message":"Incomplete event tag","fields":{"partial":"<event topic="}}
```

#### performance.jsonl
```json
{"ts":"2024-01-15T10:23:50.000Z","iteration":1,"hat":"ralph","metric":"iteration_duration","value":5000,"unit":"ms"}
{"ts":"2024-01-15T10:23:50.001Z","iteration":1,"hat":"ralph","metric":"tokens_input","value":1500,"unit":"tokens"}
{"ts":"2024-01-15T10:23:50.002Z","iteration":1,"hat":"ralph","metric":"tokens_output","value":800,"unit":"tokens"}
{"ts":"2024-01-15T10:23:50.003Z","iteration":1,"hat":"ralph","metric":"agent_latency","value":4500,"unit":"ms"}
```

#### errors.jsonl
```json
{"ts":"2024-01-15T10:23:47.000Z","iteration":1,"hat":"ralph","error_type":"parse_error","message":"Invalid JSON in stream","context":{"input":"{invalid","source":"agent_output"}}
{"ts":"2024-01-15T10:23:48.000Z","iteration":1,"hat":"loop","error_type":"validation_failure","message":"build.done missing evidence","context":{"required":["tests","lint","typecheck"],"found":[]}}
```

## Error Handling

### Diagnostic System Errors

The diagnostic system should **never** cause Ralph to fail. All errors are:
1. Logged to stderr (if diagnostics can't write)
2. Silently ignored to allow Ralph to continue

```rust
impl DiagnosticsCollector {
    fn log_internal_error(&self, error: &str) {
        eprintln!("[diagnostics] Error: {}", error);
    }

    pub fn log_orchestration(&self, event: OrchestrationEvent) {
        if let Some(ref logger) = self.orchestration_logger {
            if let Err(e) = logger.lock().unwrap().log(event) {
                self.log_internal_error(&format!("Failed to write orchestration log: {}", e));
            }
        }
    }
}
```

### Crash Safety

All loggers use **immediate flush** after each write:

```rust
impl AgentOutputLogger {
    pub fn log(&mut self, content: AgentOutputContent) {
        let entry = AgentOutputEntry { /* ... */ };
        if let Ok(json) = serde_json::to_string(&entry) {
            let _ = writeln!(self.file, "{}", json);
            let _ = self.file.flush();  // Immediate flush
        }
    }
}
```

## Testing Strategy

### Unit Tests

1. **DiagnosticsCollector initialization**
   - Test enabled/disabled based on env var
   - Test directory creation
   - Test session timestamp format

2. **Individual loggers**
   - Test JSONL format correctness
   - Test field serialization
   - Test flush behavior

3. **DiagnosticStreamHandler**
   - Test delegation to inner handler
   - Test logging of all event types
   - Test thread safety

### Integration Tests

1. **Full diagnostic capture**
   - Run Ralph with `RALPH_DIAGNOSTICS=1`
   - Verify all file types created
   - Verify content matches execution

2. **TUI mode diagnostics**
   - Verify no stdout interference
   - Verify same files created as non-TUI

3. **Cleanup command**
   - Test `ralph clean --diagnostics`
   - Verify all diagnostic dirs removed

### Smoke Tests

Add to existing smoke test suite:
```rust
#[test]
fn test_diagnostics_capture() {
    std::env::set_var("RALPH_DIAGNOSTICS", "1");
    // Run replay session
    // Verify diagnostic files created and contain expected entries
}
```

## Appendices

### A. Technology Choices

| Choice | Rationale |
|--------|-----------|
| JSONL format | Append-safe, streamable, queryable with `jq` |
| Separate files | Targeted queries, smaller files, clear separation |
| tracing Layer | Captures existing macros without code changes |
| Immediate flush | Crash safety, no lost data |
| Env var activation | Simple, scriptable, no config changes needed |

### B. Research Findings Summary

- Ralph already has `tracing` + `tracing-subscriber` (basic config)
- EventBus supports observers for non-invasive capture
- StreamHandler trait enables wrapping without modification
- Session recording exists but is separate from debug logging
- TUI suppresses stdout; file-based logging required

### C. Alternative Approaches Considered

1. **Single unified log file** — Rejected: harder to query specific data types
2. **Always-on logging** — Rejected: performance overhead, disk usage
3. **In-TUI log widget** — Rejected: adds complexity, files are more flexible
4. **Structured tracing with spans** — Deferred: would require code changes at call sites

### D. Future Enhancements (Out of Scope)

1. Log rotation (keep last N sessions automatically)
2. `ralph diagnostics` CLI for querying
3. OpenTelemetry export
4. Real-time log streaming
5. Compression of old sessions
