# Implementation Plan: Diagnostic Logging for Ralph

## Implementation Checklist

- [ ] Step 1: Create diagnostics module structure and DiagnosticsCollector
- [ ] Step 2: Implement AgentOutputLogger with JSONL writing
- [ ] Step 3: Implement DiagnosticStreamHandler wrapper
- [ ] Step 4: Implement OrchestrationLogger
- [ ] Step 5: Integrate orchestration logging into event loop
- [ ] Step 6: Implement TraceLogger as tracing Layer
- [ ] Step 7: Integrate trace layer into subscriber initialization
- [ ] Step 8: Implement PerformanceLogger
- [ ] Step 9: Implement ErrorLogger
- [ ] Step 10: Add `ralph clean --diagnostics` command
- [ ] Step 11: End-to-end integration and smoke tests

---

## Step 1: Create diagnostics module structure and DiagnosticsCollector

**Objective:** Establish the foundation for the diagnostics system with the central coordinator.

**Implementation guidance:**
1. Create new module at `crates/ralph-core/src/diagnostics/mod.rs`
2. Implement `DiagnosticsCollector` that:
   - Checks `RALPH_DIAGNOSTICS` env var on construction
   - Creates timestamped session directory if enabled
   - Provides `is_enabled()` method for conditional logic
3. Export from `ralph-core` lib.rs

**Test requirements:**
- Unit test: `DiagnosticsCollector::new()` returns disabled when env var not set
- Unit test: `DiagnosticsCollector::new()` creates directory when enabled
- Unit test: Session directory uses correct timestamp format

**Integration with previous work:** N/A - foundation step

**Demo:**
```rust
std::env::set_var("RALPH_DIAGNOSTICS", "1");
let collector = DiagnosticsCollector::new(Path::new("."));
assert!(collector.is_enabled());
assert!(collector.session_dir().unwrap().exists());
// Shows: .ralph/diagnostics/2024-01-15T10-23-45/
```

---

## Step 2: Implement AgentOutputLogger with JSONL writing

**Objective:** Create the logger for capturing agent output as structured JSONL.

**Implementation guidance:**
1. Create `crates/ralph-core/src/diagnostics/agent_output.rs`
2. Implement `AgentOutputLogger` with:
   - File handle to `agent-output.jsonl`
   - `set_context(iteration, hat)` method
   - `log(AgentOutputContent)` method
   - Immediate flush after each write
3. Define `AgentOutputEntry` and `AgentOutputContent` enum with serde

**Test requirements:**
- Unit test: Logger creates file in correct location
- Unit test: Each `log()` call writes valid JSONL line
- Unit test: Flush happens after each write (verify file content immediately)
- Unit test: All content types serialize correctly

**Integration with previous work:** Wire into `DiagnosticsCollector`

**Demo:**
```rust
let mut logger = AgentOutputLogger::new(session_dir)?;
logger.set_context(1, "ralph");
logger.log(AgentOutputContent::Text { text: "Hello".to_string() });
logger.log(AgentOutputContent::ToolCall { name: "Read".to_string(), id: "1".to_string(), input: json!({}) });
// File contains 2 JSONL lines with correct schema
```

---

## Step 3: Implement DiagnosticStreamHandler wrapper

**Objective:** Create wrapper that captures output while delegating to inner handler.

**Implementation guidance:**
1. Create `crates/ralph-core/src/diagnostics/stream_handler.rs`
2. Implement `DiagnosticStreamHandler<H: StreamHandler>` that:
   - Wraps any StreamHandler implementation
   - Logs to `AgentOutputLogger` before delegating
   - Uses `Arc<Mutex<AgentOutputLogger>>` for thread safety
3. Add `wrap_stream_handler()` method to `DiagnosticsCollector`

**Test requirements:**
- Unit test: Wrapper calls inner handler for all methods
- Unit test: Wrapper logs all event types
- Unit test: Thread safety (concurrent calls don't panic)

**Integration with previous work:** Uses `AgentOutputLogger` from Step 2

**Demo:**
```rust
let inner = ConsoleStreamHandler::new();
let wrapped = collector.wrap_stream_handler(inner);
wrapped.on_text("Hello");
// Both: printed to console AND logged to agent-output.jsonl
```

---

## Step 4: Implement OrchestrationLogger

**Objective:** Create logger for orchestration decisions and events.

**Implementation guidance:**
1. Create `crates/ralph-core/src/diagnostics/orchestration.rs`
2. Implement `OrchestrationLogger` with:
   - File handle to `orchestration.jsonl`
   - `log(iteration, hat, OrchestrationEvent)` method
3. Define `OrchestrationEvent` enum for all decision types
4. Add `log_orchestration()` to `DiagnosticsCollector`

**Test requirements:**
- Unit test: All event types serialize correctly
- Unit test: Iteration and hat captured in each entry
- Unit test: Immediate flush behavior

**Integration with previous work:** Wire into `DiagnosticsCollector`

**Demo:**
```rust
collector.log_orchestration(1, "loop", OrchestrationEvent::HatSelected {
    hat: "ralph".to_string(),
    reason: "pending_events".to_string(),
});
// orchestration.jsonl contains hat selection entry
```

---

## Step 5: Integrate orchestration logging into event loop

**Objective:** Wire orchestration logger into the actual event loop.

**Implementation guidance:**
1. Add `DiagnosticsCollector` field to `EventLoop` struct
2. Add logging calls at key decision points:
   - `IterationStarted` at loop iteration start
   - `HatSelected` when choosing next hat
   - `EventPublished` when publishing to bus
   - `BackpressureTriggered` when validation fails
   - `LoopTerminated` at loop exit
   - `TaskAbandoned` when abandoning tasks
3. Create EventBus observer for event flow logging

**Test requirements:**
- Integration test: Run event loop, verify orchestration.jsonl populated
- Test all orchestration event types appear

**Integration with previous work:** Uses `OrchestrationLogger` from Step 4

**Demo:**
```bash
RALPH_DIAGNOSTICS=1 ralph run -p "simple task"
cat .ralph/diagnostics/*/orchestration.jsonl
# Shows iteration_started, hat_selected, event_published, loop_terminated
```

---

## Step 6: Implement TraceLogger as tracing Layer

**Objective:** Capture existing `info!`/`debug!`/`warn!`/`error!` macros to JSONL.

**Implementation guidance:**
1. Create `crates/ralph-core/src/diagnostics/trace_layer.rs`
2. Implement `DiagnosticTraceLayer` that:
   - Implements `tracing_subscriber::Layer`
   - Writes to `trace.jsonl`
   - Captures level, target, message, fields
   - Has `set_context(iteration, hat)` for context injection
3. Use `Arc<Mutex<>>` for file handle (Layer must be `Sync`)

**Test requirements:**
- Unit test: Layer captures events at all levels
- Unit test: Fields serialized correctly
- Unit test: Context (iteration/hat) included

**Integration with previous work:** Standalone component

**Demo:**
```rust
// After layer is installed
info!("Starting iteration");
debug!(bytes = 1024, "PTY output received");
// trace.jsonl contains both entries with metadata
```

---

## Step 7: Integrate trace layer into subscriber initialization

**Objective:** Wire the diagnostic trace layer into Ralph's tracing setup.

**Implementation guidance:**
1. Modify `crates/ralph-cli/src/main.rs` logging initialization
2. When `RALPH_DIAGNOSTICS=1`:
   - Create `DiagnosticTraceLayer`
   - Add to subscriber using `.with()` combinator
3. Ensure layer is optional (subscriber works without it)
4. Pass layer reference to `DiagnosticsCollector` for context updates

**Test requirements:**
- Integration test: Existing logs appear in trace.jsonl
- Test: Subscriber still works when diagnostics disabled
- Test: TUI mode uses same layer (writes to file)

**Integration with previous work:** Uses `DiagnosticTraceLayer` from Step 6

**Demo:**
```bash
RALPH_DIAGNOSTICS=1 ralph run -p "task"
cat .ralph/diagnostics/*/trace.jsonl
# Shows all info!/debug!/warn!/error! from the run
```

---

## Step 8: Implement PerformanceLogger

**Objective:** Capture timing and resource metrics.

**Implementation guidance:**
1. Create `crates/ralph-core/src/diagnostics/performance.rs`
2. Implement `PerformanceLogger` with:
   - File handle to `performance.jsonl`
   - `log(iteration, hat, PerformanceMetric)` method
3. Define `PerformanceMetric` enum
4. Add timing instrumentation to event loop:
   - Iteration duration
   - Agent execution latency
   - Token counts (from session result)

**Test requirements:**
- Unit test: All metric types serialize correctly
- Integration test: Metrics captured during real run

**Integration with previous work:** Wire into event loop alongside orchestration logger

**Demo:**
```bash
RALPH_DIAGNOSTICS=1 ralph run -p "task"
cat .ralph/diagnostics/*/performance.jsonl
# Shows iteration_duration, agent_latency, tokens metrics
```

---

## Step 9: Implement ErrorLogger

**Objective:** Capture errors and failures for debugging.

**Implementation guidance:**
1. Create `crates/ralph-core/src/diagnostics/errors.rs`
2. Implement `ErrorLogger` with:
   - File handle to `errors.jsonl`
   - `log(iteration, hat, DiagnosticError)` method
3. Define `DiagnosticError` enum
4. Add error logging calls at:
   - Event parsing failures
   - Validation failures (backpressure)
   - Backend errors
   - Malformed events

**Test requirements:**
- Unit test: All error types serialize correctly
- Integration test: Errors captured when they occur

**Integration with previous work:** Wire into event loop and parser

**Demo:**
```bash
# Trigger a validation failure
RALPH_DIAGNOSTICS=1 ralph run -p "task that triggers backpressure"
cat .ralph/diagnostics/*/errors.jsonl
# Shows validation_failure entries
```

---

## Step 10: Add `ralph clean --diagnostics` command

**Objective:** Implement cleanup mechanism for diagnostic files.

**Implementation guidance:**
1. Add `--diagnostics` flag to `CleanArgs` struct
2. Implement `clean_diagnostics()` function that:
   - Removes `.ralph/diagnostics/` directory recursively
   - Reports what was removed
3. Wire into existing `clean` command handler

**Test requirements:**
- Unit test: Removes diagnostics directory
- Unit test: Handles case where directory doesn't exist
- Integration test: Command works end-to-end

**Integration with previous work:** Uses directory structure from all previous steps

**Demo:**
```bash
# After several diagnostic sessions
ls .ralph/diagnostics/
# 2024-01-15T10-23-45/  2024-01-15T14-30-00/

ralph clean --diagnostics
# Removed .ralph/diagnostics

ls .ralph/diagnostics/
# ls: .ralph/diagnostics: No such file or directory
```

---

## Step 11: End-to-end integration and smoke tests

**Objective:** Verify complete system works together.

**Implementation guidance:**
1. Add smoke test that:
   - Sets `RALPH_DIAGNOSTICS=1`
   - Runs a replay session
   - Verifies all 5 log files created
   - Verifies each contains expected entry types
2. Add integration test for TUI mode diagnostics
3. Update documentation (CLAUDE.md)

**Test requirements:**
- Smoke test: All file types created
- Smoke test: Files contain valid JSONL
- Smoke test: TUI mode produces same files
- Manual test: Real run with diagnostics

**Integration with previous work:** Exercises all components together

**Demo:**
```bash
RALPH_DIAGNOSTICS=1 ralph run -p "implement a feature"
# After completion:
ls .ralph/diagnostics/*/
# agent-output.jsonl  errors.jsonl  orchestration.jsonl  performance.jsonl  trace.jsonl

# Query errors
jq 'select(.error_type)' .ralph/diagnostics/*/errors.jsonl

# Get all tool calls
jq 'select(.type == "tool_call")' .ralph/diagnostics/*/agent-output.jsonl

# Check iteration timing
jq 'select(.metric == "iteration_duration")' .ralph/diagnostics/*/performance.jsonl
```

---

## Implementation Notes

### Dependency Changes

Add to `Cargo.toml` (ralph-core):
```toml
[dependencies]
chrono = { version = "0.4", features = ["serde"] }  # If not already present
```

### Module Structure

```
crates/ralph-core/src/
├── diagnostics/
│   ├── mod.rs           # DiagnosticsCollector, re-exports
│   ├── agent_output.rs  # AgentOutputLogger
│   ├── stream_handler.rs # DiagnosticStreamHandler
│   ├── orchestration.rs # OrchestrationLogger
│   ├── trace_layer.rs   # DiagnosticTraceLayer
│   ├── performance.rs   # PerformanceLogger
│   └── errors.rs        # ErrorLogger
└── lib.rs               # Add: pub mod diagnostics;
```

### Thread Safety Considerations

- `AgentOutputLogger`: Used via `Arc<Mutex<>>` in stream handler
- `DiagnosticTraceLayer`: Must be `Send + Sync` for tracing (use `Arc<Mutex<>>` for file)
- `OrchestrationLogger`, `PerformanceLogger`, `ErrorLogger`: Single-threaded access from event loop
