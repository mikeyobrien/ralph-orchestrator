# Research: Ralph's Stream Handler Architecture

## Summary

Ralph has a clean trait-based streaming architecture that pi can plug into. The critical integration point is in `PtyExecutor::run_observe_streaming()` which branches on `OutputFormat` to choose between NDJSON parsing and raw text passthrough.

## Architecture Overview

```
┌─────────────────┐     ┌──────────────┐     ┌─────────────────┐
│   PtyExecutor   │────▶│ StreamHandler│────▶│  TUI / Console  │
│ (pty_executor.rs)│     │   (trait)    │     │                 │
└────────┬────────┘     └──────────────┘     └─────────────────┘
         │
         ├── OutputFormat::StreamJson → ClaudeStreamParser::parse_line() → dispatch_stream_event()
         └── OutputFormat::Text → handler.on_text(raw) passthrough
```

## StreamHandler Trait

```rust
trait StreamHandler: Send {
    fn on_text(&mut self, text: &str);
    fn on_tool_call(&mut self, name: &str, id: &str, input: &serde_json::Value);
    fn on_tool_result(&mut self, id: &str, output: &str);
    fn on_error(&mut self, error: &str);
    fn on_complete(&mut self, result: &SessionResult);
}
```

4 implementations:
- `ConsoleStreamHandler` — immediate text output (for Text format backends)
- `PrettyStreamHandler` — markdown-rendered output (for StreamJson backends on TTY)
- `QuietStreamHandler` — silent (CI mode)
- `TuiStreamHandler` — ratatui Lines for TUI display

## OutputFormat Enum

```rust
enum OutputFormat {
    Text,        // Plain text output (default for most adapters)
    StreamJson,  // Newline-delimited JSON stream (Claude only currently)
}
```

Currently only Claude uses `StreamJson`. All other backends (Kiro, Gemini, Codex, Amp, Copilot, OpenCode) use `Text`.

## Parsing Flow in PtyExecutor

In `run_observe_streaming()`:

```rust
let is_stream_json = output_format == OutputFormat::StreamJson;

// In the data processing loop:
if is_stream_json {
    // Buffer lines, parse as JSON
    line_buffer.push_str(text);
    while let Some(newline_pos) = line_buffer.find('\n') {
        let line = line_buffer[..newline_pos].to_string();
        if let Some(event) = ClaudeStreamParser::parse_line(&line) {
            dispatch_stream_event(event, handler, &mut extracted_text);
        }
    }
} else {
    // Raw text passthrough
    handler.on_text(text);
}
```

## dispatch_stream_event()

Maps Claude events to StreamHandler calls:

```rust
fn dispatch_stream_event<H: StreamHandler>(event: ClaudeStreamEvent, handler: &mut H, extracted_text: &mut String) {
    match event {
        System { .. } => { /* ignore */ }
        Assistant { message, .. } => {
            for block in message.content {
                Text { text } => { handler.on_text(&text); extracted_text.push_str(&text); }
                ToolUse { name, id, input } => { handler.on_tool_call(&name, &id, &input); }
            }
        }
        User { message } => {
            for block in message.content {
                ToolResult { tool_use_id, content } => { handler.on_tool_result(&tool_use_id, &content); }
            }
        }
        Result { duration_ms, total_cost_usd, num_turns, is_error } => {
            handler.on_complete(&SessionResult { ... });
        }
    }
}
```

## extracted_text

Critical for Ralph's core loop: the accumulated text output is used by `EventParser` to find `LOOP_COMPLETE` and event tags. For NDJSON backends, `extracted_text` collects only the text content (not JSON). For Text backends, `stripped_output` (ANSI-stripped raw output) is used instead.

## Integration Options for Pi

### Option A: New OutputFormat variant (Recommended)

Add `OutputFormat::PiStreamJson` and branch in `run_observe_streaming()`:

```rust
match output_format {
    OutputFormat::StreamJson => { /* Claude parsing */ }
    OutputFormat::PiStreamJson => { /* Pi parsing */ }
    OutputFormat::Text => { /* raw passthrough */ }
}
```

Pros: Clean separation, no risk of breaking Claude parsing
Cons: Another branch in the hot path

### Option B: Unified parser with format detection

Parse the first JSON line to detect format (`{"type":"session",...}` = pi, `{"type":"system",...}` = Claude), then dispatch accordingly.

Pros: Single `StreamJson` format, auto-detection
Cons: More complex, fragile detection

### Option C: Text mode only

Use `OutputFormat::Text` and treat pi like Kiro/Gemini (raw text output).

Pros: Zero parsing work
Cons: No structured tool call display, no cost tracking, no rich TUI output

**Recommendation: Option A.** Pi would be only the second NDJSON backend, and the schemas are different enough that a separate parser is cleaner. The branch cost is negligible.
