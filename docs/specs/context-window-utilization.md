# Spec: Context Window Utilization Tracking

## Problem

Ralph currently shows `Duration | Est. cost | Turns` after each iteration but has zero visibility into context window usage. Token data arrives in Claude/Pi stream events but is **dropped** before reaching the display layer. Operators have no way to know how close an agent is to hitting the context window limit.

## Goal

Add context utilization (%) to the iteration summary and events, so you can see how much of each agent's context window is being used.

**Target output:**
```
Duration: 12345ms | Est. cost: $0.0526 | Turns: 3 | Context: 45% (90K/200K)
```

## What Changes

### 1. Extend `SessionResult` with token fields
**File:** `crates/ralph-adapters/src/stream_handler.rs`

Add three optional fields:
```rust
pub struct SessionResult {
    pub duration_ms: u64,
    pub total_cost_usd: f64,
    pub num_turns: u32,
    pub is_error: bool,
    pub input_tokens: Option<u64>,   // Last turn's input = context utilization
    pub output_tokens: Option<u64>,  // Cumulative output tokens
    pub context_window: Option<u64>, // Model's max context size
}
```

All `Option` because text-only backends (Kiro, Gemini, Codex, etc.) can't report tokens.

### 2. Capture Claude token data (currently dropped)
**File:** `crates/ralph-adapters/src/pty_executor.rs`

- Add a small private `TokenState { last_input_tokens: Option<u64>, total_output_tokens: u64 }` struct
- In `dispatch_stream_event`, stop ignoring `usage` on `ClaudeStreamEvent::Assistant { message, usage }`
- Track `last_input_tokens` (last turn's input = current context size) and accumulate `total_output_tokens`
- Pass through to `SessionResult` when constructing it from `ClaudeStreamEvent::Result`
- Add `input_tokens` and `output_tokens` to `PtyExecutionResult` so data flows out of the executor

### 3. Capture Pi token data (currently dropped)
**File:** `crates/ralph-adapters/src/pi_stream.rs`

- Add `last_input_tokens: Option<u64>` and `total_output_tokens: u64` to `PiSessionState`
- In `dispatch_pi_stream_event` → `TurnEnd`, capture `usage.input` and accumulate `usage.output`
- Populate new `SessionResult` fields in the synthesized `on_complete` calls (pty_executor.rs)

### 4. Update iteration summary display
**File:** `crates/ralph-adapters/src/stream_handler.rs`

Update `on_complete()` in all three active handlers to append context info:

| Condition | Display |
|-----------|---------|
| `input_tokens` + `context_window` both present | `Context: 45% (90K/200K)` |
| Only `input_tokens` present | `Tokens: 90K` |
| Neither present (text backends) | Nothing extra |

Format: `Duration: Xms | Est. cost: $X | Turns: X | Context: 45% (90K/200K)`

Handlers to update:
- `PrettyStreamHandler::on_complete` (terminal)
- `ConsoleStreamHandler::on_complete` (verbose console)
- `TuiStreamHandler::on_complete` (ratatui TUI)

### 5. Context window size: defaults + config override
**File:** `crates/ralph-core/src/config.rs`

Add `context_window_tokens: Option<u64>` to `EventLoopConfig`.

Hardcoded defaults when config is `None`:
- `claude` / `pi` → `200_000`
- All others → `None`

Config override:
```yaml
event_loop:
  context_window_tokens: 128000  # For non-default models
```

### 6. Track per-hat token stats in LoopState
**File:** `crates/ralph-core/src/event_loop/loop_state.rs`

Add to `LoopState`:
- `last_input_tokens: Option<u64>` — latest iteration's context usage
- `peak_input_tokens: u64` — high water mark across all iterations
- `hat_peak_input_tokens: HashMap<HatId, u64>` — per-hat peak

Add `EventLoop::update_token_stats(hat_id, input_tokens)` method.

### 7. Add token data to events.jsonl
**File:** `crates/ralph-core/src/event_logger.rs`

Add optional fields to `EventRecord`:
- `input_tokens: Option<u64>`
- `context_utilization_pct: Option<f64>`

Both `skip_serializing_if = "Option::is_none"` to keep existing events clean. Populated when logging iteration completion.

### 8. Wire through loop_runner
**File:** `crates/ralph-cli/src/loop_runner.rs`

- Extract `input_tokens`/`output_tokens` from `PtyExecutionResult` into `ExecutionOutcome`
- After each iteration, call `event_loop.update_token_stats()`
- Set `context_window` on `SessionResult` from config + defaults before display

## Implementation Order

1. `SessionResult` + fix all existing test constructions (widest-reaching change)
2. `TokenState` in pty_executor + `dispatch_stream_event` Claude capture
3. `PiSessionState` + `dispatch_pi_stream_event` Pi capture
4. Display updates in all `StreamHandler::on_complete` impls
5. `context_window_tokens` config + `default_context_window()` helper
6. `LoopState` token tracking + `update_token_stats()`
7. `EventRecord` token fields + events.jsonl logging
8. Loop runner wiring
9. Tests for all of the above

## Testing

- Update all existing `SessionResult` constructions in tests (add `..Default::default()` or explicit `None`)
- Unit tests for display formatting with/without token data
- Unit tests for `dispatch_stream_event` capturing `Usage` from Claude `Assistant` events
- Unit tests for Pi `TurnEnd` token accumulation
- Unit tests for `EventRecord` serialization with new optional fields
- `cargo test` must pass before done

## Key Insight: Where Tokens Are Today

```
Claude: Assistant { usage: Some(Usage { input_tokens, output_tokens }) }
        → pty_executor line 1592: destructured as { message, .. }  ← DROPPED

Pi:     TurnEnd { message: { usage: { input, output, cache_read, cache_write } } }
        → dispatch_pi_stream_event line 243: only cost.total extracted  ← DROPPED
```

The token data is already arriving — we just need to stop throwing it away and pipe it through to the display layer.

## Verification

```bash
cargo test                               # All tests pass
cargo build                              # Clean build
# Manual: run ralph with claude backend, verify iteration summary shows Context: X%
# Manual: ralph events shows token data in output
```
