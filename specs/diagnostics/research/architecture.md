# TUI vs Non-TUI Architecture

## Mode Detection

```rust
let tui_enabled = match &cli.command {
    Some(Commands::Run(args)) => args.tui,
    Some(Commands::Resume(args)) => args.tui,
    _ => false,
};

// Only enable if stdout is a terminal
let enable_tui = enable_tui && stdout().is_terminal();
```

## StreamHandler Abstraction

All output flows through the `StreamHandler` trait:

```rust
pub trait StreamHandler: Send {
    fn on_text(&mut self, text: &str);
    fn on_tool_call(&mut self, name: &str, id: &str, input: &serde_json::Value);
    fn on_tool_result(&mut self, id: &str, output: &str);
    fn on_error(&mut self, error: &str);
    fn on_complete(&mut self, result: &SessionResult);
}
```

### Implementations

| Handler | Destination | Use Case |
|---------|-------------|----------|
| `TuiStreamHandler` | `Arc<Mutex<Vec<Line>>>` | TUI mode |
| `PrettyStreamHandler` | stdout (markdown) | StreamJson + TTY |
| `ConsoleStreamHandler` | stdout (raw) | Text format |
| `QuietStreamHandler` | (none) | CI/scripting |

## Output Flow Diagram

```
Agent (Claude/Kiro)
      ↓
   [PTY/CLI]
      ↓
┌─────────────────────┐
│  Raw Output Bytes   │
└─────────────────────┘
      ↓
   CliCapture → UX events (TerminalWrite)
      ↓
   UTF-8 → strip-ansi → NDJSON parsing
      ↓
   ClaudeStreamParser
      ↓
   dispatch_stream_event()
      ↓
   StreamHandler (Console/TUI/Quiet)
```

## Mode Comparison

| Aspect | TUI Mode | Non-TUI Mode |
|--------|----------|--------------|
| **Output** | `Arc<Mutex<Vec<Line>>>` | stdout/stderr |
| **Logging** | Suppressed | stdout |
| **Debug logs** | `.agent/ralph.log` (opt-in) | `.agent/ralph.log` |
| **Terminal** | TUI owns raw mode | Main loop owns |
| **Rendering** | ~60fps via ratatui | Direct streaming |

## Diagnostic Tap Points

### TUI Mode
1. Observer callbacks (event bus)
2. `IterationBuffer.lines` (shared Arc)
3. stderr (not suppressed)
4. TUI internal state

### Non-TUI Mode
1. stdout (via StreamHandler)
2. stderr
3. EventLogger → `.ralph/events.jsonl`
4. Debug logging → `.agent/ralph.log`

## Key Files

| Component | File |
|-----------|------|
| CLI Flag Parsing | `crates/ralph-cli/src/main.rs:293-300` |
| TUI Initialization | `crates/ralph-cli/src/main.rs:1666-1690` |
| Stream Handlers | `crates/ralph-adapters/src/stream_handler.rs` |
| PTY Executor | `crates/ralph-adapters/src/pty_executor.rs` |
