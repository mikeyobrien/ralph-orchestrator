# Spec: Context Window Utilization Tracking

- **Issue:** mikeyobrien/ralph-orchestrator#182
- **Slug:** `context-window-utilization`
- **Status:** Ready for implementation
- **Scope:** bounded, incremental plumbing — thread token fields through the existing pipeline, render a new display column, emit a new synthetic events.jsonl row, track per-hat peaks. **No event-loop redesign. No automatic compaction.**

## 1. Summary

Ralph already receives token usage on Claude `Assistant` stream events and Pi `TurnEnd` events, but the Claude path destructures `usage` then drops it, and `SessionResult` is built with `..Default::default()` — so every display downstream reads zero. Pi is wired correctly but lacks a context-window display.

This spec captures the tokens, renders a single new summary column, and surfaces the same data through RPC and `events.jsonl` so dashboards/tooling can consume it.

Target iteration summary (append-only change to existing line):

```
Duration: 12345ms | Est. cost: $0.0526 | Turns: 3 | Context: 45% (90K/200K)
```

The `Context:` column is appended only when `context_window > 0` and `used > 0`. All other backends/legacy code paths render the existing three-column line unchanged.

## 2. Goals

1. Capture `Usage.input_tokens` / `Usage.output_tokens` / `cache_creation_input_tokens` / `cache_read_input_tokens` from Claude stream events. (Requires fixing a latent parse bug — see §5.)
2. Keep Pi token plumbing working; add a per-turn peak so the `Context:` display accurately reflects the peak prompt size.
3. Extend `SessionResult` with `context_window: u64`. Reinterpret `input_tokens` as **peak across turns** (not cumulative sum).
4. Render `Context: NN% (USED_K/MAX_K)` in `PrettyStreamHandler`, `ConsoleStreamHandler` (verbose), and `TuiStreamHandler` via a shared `format_session_summary` helper.
5. Add `context_window_tokens: Option<u64>` to `EventLoopConfig`; resolve to 200_000 default for claude/pi and 0 (suppress display) for other backends.
6. Track per-hat peak `(input + cache_read + cache_write)` tokens in `LoopState` for future dashboarding. No rendering of per-hat data in this feature.
7. Emit a synthetic `iteration.summary` event to `events.jsonl` with a stable JSON payload. Extend `RpcEvent::IterationEnd` with `context_window` and `context_tokens` so RPC consumers see it.

## 3. Non-Goals

- Event-loop, hat-router, or backend-trait redesign.
- Automatic compaction, summarization, or proactive context pruning based on utilization.
- Threshold warnings (e.g. `> 80%` alerts).
- ACP / Copilot / Roo / Gemini / Kiro / Codex token capture. Those backends render zero; the `Context:` suffix is simply hidden for them.
- Web dashboard UI. Payload is added; rendering is a separate task.
- Model-aware context-window auto-detection (parsing `System.model` → lookup table). v2.
- Refactoring `ExecutionOutcome` or `SessionResult` beyond the specified field additions.
- Backwards compatibility for existing `SessionResult` callers. Per CLAUDE.md, backwards compatibility adds clutter and is not required.

## 4. Decisions

- **Canonical location:** `.ralph/specs/context-window-utilization.md` (this file) and `.ralph/tasks/context-window-utilization.code-task.md`, per CLAUDE.md which designates `.ralph/specs/` and `.ralph/tasks/` as authoritative. Replaces stale `docs/specs/context-window-utilization.md` — delete the stale file in the implementation commit.
- **`input_tokens` semantics change from "cumulative sum" to "peak across turns"** for both Claude and Pi. Every Claude `Assistant` event's `message.usage.input_tokens` already represents the full prompt context for that turn; the maximum across turns is the high-water mark of context occupancy. Pi reports per-turn deltas — we take `max(peak, turn.input + turn.cache_read)`. Output tokens remain a cumulative sum (they're generated content).
- **Cache tokens count toward context occupancy.** `cache_read_input_tokens` and `cache_creation_input_tokens` are included in the numerator of the utilization percentage — they occupy real prompt space.
- **Flat 200K default with per-config override, not per-model lookup.** YAML stays clean; users with Sonnet 4.x `[1m]` override by setting `event_loop.context_window_tokens: 1_000_000`. Model-aware defaults are explicitly out of scope.
- **Synthetic `iteration.summary` events.jsonl row, not schema change.** Zero churn on `EventRecord`; `ralph events --topic iteration.summary` works without code changes; rows only appear once per iteration (no bloat on per-event rows).
- **Integer percentages, no decimals.** `45%`, not `45.3%`. Fractional precision isn't actionable for users. Overflow (`>100%`) renders verbatim, not clamped — signals a misconfigured window.
- **K-rounding:** round half-up via `(tokens + 500) / 1000`. So `89_700 → 90K`, `89_499 → 89K`. Applied to both numerator and denominator so `200_000 → 200K`, `1_000_000 → 1000K`.
- **Shared helper prevents handler drift.** `format_session_summary(&SessionResult) -> String` lives in `stream_handler.rs`; all three `on_complete` impls call it. `QuietStreamHandler` remains silent.

## 5. Prerequisite bug fix: Claude `Usage` parse

The existing `ClaudeStreamEvent::Assistant` variant mis-nests `usage` at the event top level and omits cache fields; real Claude `stream-json` puts `usage` inside `message` alongside `content` and includes `cache_creation_input_tokens` and `cache_read_input_tokens`. `#[serde(default)]` silently yields `None` on every real event, so the current field is dead code.

```rust
// BEFORE:
Assistant {
    message: AssistantMessage,
    #[serde(default)]
    usage: Option<Usage>,            // wrong nesting — always None
},
pub struct Usage {
    pub input_tokens: u64,
    pub output_tokens: u64,
}

// AFTER:
Assistant {
    message: AssistantMessage,        // usage lives inside
},
pub struct AssistantMessage {
    pub content: Vec<ContentBlock>,
    #[serde(default)]
    pub usage: Option<Usage>,
}
pub struct Usage {
    #[serde(default)] pub input_tokens: u64,
    #[serde(default)] pub output_tokens: u64,
    #[serde(default)] pub cache_creation_input_tokens: u64,
    #[serde(default)] pub cache_read_input_tokens: u64,
}
```

A unit test round-tripping a real stream-json payload (with nested `message.usage` including both cache fields) must be added. Without this fix, every other change in this spec produces zero values.

## 6. Data flow

```
Claude Assistant (message.usage) ─┐
                                  ├─► SessionResult { input_tokens=peak, output_tokens=sum,
Pi TurnEnd (usage)                ─┘                    cache_read_tokens, cache_write_tokens,
                                                        context_window }
                                                        │
                                                        ▼
                           PtyExecutionResult (already has token fields)
                                                        │
                                                        ▼
                           ExecutionOutcome { + context_window, + context_tokens }
                                                        │
                           ┌─────────────────────────────┼────────────────────────────┐
                           ▼                             ▼                            ▼
            StreamHandler::on_complete       RpcEvent::IterationEnd        events.jsonl:
            (Pretty/Console/Tui +            { + context_window,            iteration.summary
             JsonRpcStreamHandler)             + context_tokens }           (synthetic row,
                                                        │                    JSON payload)
                                                        ▼
                                           LoopState.hat_peak_input_tokens
                                           LoopState.peak_input_tokens
                                           LoopState.last_input_tokens
```

## 7. Struct changes

### 7.1 `crates/ralph-adapters/src/claude_stream.rs`

See §5. Move `usage` inside `AssistantMessage`; add `cache_creation_input_tokens` and `cache_read_input_tokens` to `Usage`.

### 7.2 `crates/ralph-adapters/src/stream_handler.rs::SessionResult`

```rust
pub struct SessionResult {
    pub duration_ms: u64,
    pub total_cost_usd: f64,
    pub num_turns: u32,
    pub is_error: bool,
    pub input_tokens: u64,             // PEAK across turns (semantic change)
    pub output_tokens: u64,            // cumulative sum
    pub cache_read_tokens: u64,        // peak (Claude) or cumulative (Pi)
    pub cache_write_tokens: u64,       // peak (Claude) or cumulative (Pi)
    pub context_window: u64,           // NEW — resolved ceiling; 0 suppresses display
}
```

### 7.3 `crates/ralph-adapters/src/pi_stream.rs::PiSessionState`

Add `peak_input_tokens: u64`. On each `TurnEnd`:

```rust
state.peak_input_tokens = state
    .peak_input_tokens
    .max(turn.usage.input + turn.usage.cache_read);
```

Used by `SessionResult` construction at `pty_executor.rs:1126-1138, 1182-1193` in place of the current cumulative `input_tokens` sum.

### 7.4 `crates/ralph-adapters/src/pty_executor.rs`

- Add `ClaudeSessionState` (analogous to `PiSessionState`) with `peak_input_tokens`, `total_output_tokens`, `peak_cache_read_tokens`, `peak_cache_write_tokens`.
- In `dispatch_stream_event` for `ClaudeStreamEvent::Assistant { message }`, when `message.usage` is `Some`:

  ```rust
  if let Some(usage) = &message.usage {
      let turn_input = usage.input_tokens
          + usage.cache_creation_input_tokens
          + usage.cache_read_input_tokens;
      state.peak_input_tokens = state.peak_input_tokens.max(turn_input);
      state.total_output_tokens += usage.output_tokens;
      state.peak_cache_read_tokens =
          state.peak_cache_read_tokens.max(usage.cache_read_input_tokens);
      state.peak_cache_write_tokens =
          state.peak_cache_write_tokens.max(usage.cache_creation_input_tokens);
  }
  ```

- In the Claude `Result` branch, replace `..Default::default()` with real values from `ClaudeSessionState`. Populate `context_window` from `resolve_context_window(&cfg)` threaded in via the executor's existing config access (see §8).

### 7.5 `crates/ralph-cli/src/loop_runner.rs::ExecutionOutcome`

```rust
pub struct ExecutionOutcome {
    // … existing fields …
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_write_tokens: u64,
    pub context_window: u64,           // NEW
    pub context_tokens: u64,           // NEW = input + cache_read + cache_write
}
```

`context_tokens` is computed once at construction; downstream consumers never recompute. All three construction sites (lines ~1782-1794, ~4214-4223, ~4359-4368 at time of spec writing; verify in current tree) must be updated in lockstep.

### 7.6 `crates/ralph-proto/src/json_rpc.rs::RpcEvent::IterationEnd`

Add `context_window: u64` and `context_tokens: u64`. `JsonRpcStreamHandler::on_complete` forwards from `SessionResult`.

### 7.7 `crates/ralph-core/src/event_loop/loop_state.rs::LoopState`

```rust
pub struct LoopState {
    // … existing …
    pub peak_input_tokens: u64,
    pub last_input_tokens: Option<u64>,
    pub hat_peak_input_tokens: HashMap<HatId, u64>,
}
```

Peaks are session-scoped; they are never reset. The per-hat map grows as hats are observed. Data is surfaced but not rendered in this feature.

### 7.8 `crates/ralph-core/src/config.rs::EventLoopConfig`

```rust
pub struct EventLoopConfig {
    // … existing …
    /// Context window size in tokens. When `None`, resolved from backend default
    /// (claude/pi = 200_000; others = 0 = suppress display).
    pub context_window_tokens: Option<u64>,
}

pub fn resolve_context_window(cfg: &RalphConfig) -> u64 {
    if let Some(n) = cfg.event_loop.context_window_tokens {
        return n;
    }
    match cfg.cli.backend.as_str() {
        "claude" | "pi" => 200_000,
        _ => 0,
    }
}
```

## 8. Threading `context_window` into `SessionResult`

`pty_executor.rs` does not currently accept a `RalphConfig`. Options:

- **Option A (preferred):** Pass `resolved_context_window: u64` into the executor's entry point (`execute`-style function) alongside existing args. `loop_runner.rs` calls `resolve_context_window(&cfg)` once per iteration and passes the value through.
- **Option B:** Build the final `SessionResult` in `loop_runner.rs` (not the executor), injecting `context_window` at that boundary.

Choose **Option A** — keeps the value attached to `SessionResult` at construction, so display handlers see it without any loop-runner coordination. One new `u64` parameter; no struct changes to existing executor inputs.

## 9. Display formatting (shared helper)

```rust
/// Builds the one-line session summary used by Pretty / Console / TUI on_complete.
/// Context suffix is omitted when context_window == 0 or used == 0.
fn format_session_summary(r: &SessionResult) -> String {
    let base = format!(
        "Duration: {}ms | Est. cost: ${:.4} | Turns: {}",
        r.duration_ms, r.total_cost_usd, r.num_turns
    );
    let used = r.input_tokens + r.cache_read_tokens + r.cache_write_tokens;
    if r.context_window > 0 && used > 0 {
        let pct = used.saturating_mul(100) / r.context_window;
        let used_k = (used + 500) / 1_000;
        let max_k = (r.context_window + 500) / 1_000;
        format!("{} | Context: {}% ({}K/{}K)", base, pct, used_k, max_k)
    } else {
        base
    }
}
```

- `PrettyStreamHandler::on_complete` — replace inline format with helper call.
- `ConsoleStreamHandler::on_complete` — same (verbose branch only).
- `TuiStreamHandler::on_complete` — same; wrap in `Line::from(Span::styled(…))`.
- `QuietStreamHandler::on_complete` — unchanged (silent no-op).

Example outputs:

| context_window | used (sum of input+cache_read+cache_write) | Rendered suffix |
|---|---|---|
| `0` | any | *(suffix omitted — base line only)* |
| `200_000` | `0` | *(suffix omitted)* |
| `200_000` | `90_000` | ` \| Context: 45% (90K/200K)` |
| `200_000` | `1_500` | ` \| Context: 0% (2K/200K)` |
| `200_000` | `250_000` | ` \| Context: 125% (250K/200K)` |
| `1_000_000` | `123_456` | ` \| Context: 12% (123K/1000K)` |

## 10. `events.jsonl` — synthetic `iteration.summary`

Emitted **once per iteration** from `loop_runner.rs` immediately after `log_events_from_output`, at the same point that emits `RpcEvent::IterationEnd`.

- Topic: `iteration.summary` (reserved; never emitted by agents)
- `EventRecord` fields: `topic = "iteration.summary"`, `hat = None`, `triggered = false`, `blocked_count = 0`, `wave_* = None`. No schema change to `EventRecord`.
- Payload (always-present fields — keep schema rigid for downstream consumers):

```json
{
  "duration_ms": 12345,
  "cost_usd": 0.0526,
  "num_turns": 3,
  "input_tokens": 88000,
  "output_tokens": 1200,
  "cache_read_tokens": 0,
  "cache_write_tokens": 0,
  "context_window": 200000,
  "context_tokens": 88000,
  "context_pct": 44
}
```

`context_pct` is integer (matches display). Redundant with `(context_tokens / context_window) * 100` but cheap — avoids float divergence in downstream readers.

## 11. Per-hat peak tracking

In `loop_runner.rs`, after each iteration's `ExecutionOutcome` is available and the hat id is known:

```rust
if outcome.context_tokens > 0 {
    let entry = loop_state
        .hat_peak_input_tokens
        .entry(hat_id.clone())
        .or_insert(0);
    *entry = (*entry).max(outcome.context_tokens);
    loop_state.peak_input_tokens =
        loop_state.peak_input_tokens.max(outcome.context_tokens);
    loop_state.last_input_tokens = Some(outcome.context_tokens);
}
```

Data is *available* but not rendered. Rendering via `ralph loops info` or the dashboard is a follow-up.

## 12. Testing strategy

### 12.1 Unit tests

1. **`claude_stream.rs`:** round-trip parse a real `stream-json` Assistant event with nested `message.usage` including `cache_creation_input_tokens` and `cache_read_input_tokens`. Assert the parsed `message.usage` is `Some(...)` and fields match.
2. **`stream_handler.rs::format_session_summary`** — table test:
   - `context_window = 0` → no suffix
   - `used = 0` → no suffix
   - `used = 90_000, window = 200_000` → ` | Context: 45% (90K/200K)`
   - `used = 1_500, window = 200_000` → ` | Context: 0% (2K/200K)`
   - `used = 250_000, window = 200_000` → ` | Context: 125% (250K/200K)` (no clamp)
3. **`config.rs::resolve_context_window`** — table: explicit override, claude default, pi default, unknown backend (→ 0).
4. **`pi_stream.rs::PiSessionState` update-turn-end:** asserts `peak_input_tokens = max(peak, turn.input + turn.cache_read)`.

### 12.2 Integration / smoke

- New smoke fixture with recorded Claude stream containing ≥2 Assistant events with `usage` showing increasing context. Assert `SessionResult.input_tokens` equals the **peak**, not the sum.
- Existing smoke fixtures under `crates/ralph-core/tests/fixtures/` are terminal-write replays — unaffected by Usage parsing changes. No re-record needed.
- Smoke tests that construct `SessionResult { …, ..Default::default() }` tolerate the new field via `Default`. No manual updates.

### 12.3 Manual validation

- `cargo run --bin ralph -- run -c ralph.claude.yml --record-session /tmp/ctx.jsonl -p "simple task"` — final line shows `Context: NN% (K/200K)`.
- Set `event_loop.context_window_tokens: 1_000_000` in `ralph.yml`; run with Sonnet-4.x `[1m]`. Suffix reads `(NNK/1000K)`.
- Run against an ACP backend (no token data). Suffix absent (not `Context: 0% (0K/200K)`).
- `jq 'select(.topic == "iteration.summary")' .ralph/diagnostics/*/events.jsonl` — row present, all ten fields populated.
- `ralph events --topic iteration.summary` — table renders the new topic.

## 13. Risks and mitigations

| Risk | Mitigation |
|---|---|
| `Usage` struct fix changes observable SDK behavior for downstream consumers. | None exist — current `Option<Usage>` is always `None`; no consumer depends on it. Safe to fix. |
| Default of 200K misrepresents Sonnet 4.x `[1m]`. | Accepted for v1. Config override is one YAML line. Model-aware detection is documented follow-up. |
| TUI summary Line wraps on narrow terminals. | No new wrap risk; current three-column line already wraps identically. |
| Integer K-rounding shows `90K` then `91K` both as `45%`. | Accepted. Fractional percent precision isn't actionable for users. |
| `cache_read` + `cache_write` summed into numerator could double-count if Anthropic changes caching semantics. | Low risk — current semantics are stable. Rule documented in `format_session_summary` comment. Adjust if semantics change. |

## 14. File touch list

```
crates/ralph-adapters/src/claude_stream.rs           — §5: fix Usage nesting + add cache fields
crates/ralph-adapters/src/stream_handler.rs          — §7.2, §9: SessionResult.context_window; format_session_summary; 3 on_complete updates
crates/ralph-adapters/src/pty_executor.rs            — §7.4: ClaudeSessionState; capture usage; Result populates SessionResult; Pi uses peak_input_tokens
crates/ralph-adapters/src/pi_stream.rs               — §7.3: PiSessionState.peak_input_tokens
crates/ralph-adapters/src/json_rpc_handler.rs        — §7.6: forward context_window + context_tokens
crates/ralph-core/src/event_loop/loop_state.rs       — §7.7: peaks
crates/ralph-core/src/config.rs                      — §7.8: EventLoopConfig.context_window_tokens; resolve_context_window()
crates/ralph-cli/src/loop_runner.rs                  — §7.5, §10, §11: ExecutionOutcome fields; resolve+propagate context_window; emit iteration.summary; update LoopState peaks
crates/ralph-proto/src/json_rpc.rs                   — §7.6: RpcEvent::IterationEnd fields
docs/specs/context-window-utilization.md             — DELETE (stale; replaced by this file)
```

Nine source files touched, one deleted, this spec created. No crate-boundary API breakage outside the additive `ralph-proto` struct additions.

## 15. Acceptance criteria (Given-When-Then)

### AC1 — Claude path shows the Context column
- **Given** a Claude backend run with `ralph.claude.yml` and no `event_loop.context_window_tokens` override
- **When** the iteration completes after ≥1 `Assistant` event carrying `message.usage`
- **Then** the rendered summary line ends with ` | Context: NN% (KK/200K)` where `KK > 0` and `NN` is integer-truncated

### AC2 — Pi path shows the Context column with peak-input semantics
- **Given** a Pi backend run with ≥2 `TurnEnd` events (increasing `usage.input` across turns)
- **When** the iteration completes
- **Then** `SessionResult.input_tokens` equals `max(turn.input + turn.cache_read)` across turns (not the sum), and the summary line shows `Context: NN% (...)`

### AC3 — Config override wins over default
- **Given** `ralph.yml` with `event_loop.context_window_tokens: 1_000_000`
- **When** any iteration completes
- **Then** the display renders `(NNK/1000K)`, and the `iteration.summary` payload carries `"context_window": 1000000`

### AC4 — Unknown backend suppresses the column
- **Given** an ACP / Copilot / Roo backend with no token data captured
- **When** the iteration completes
- **Then** the summary line renders `Duration: … | Est. cost: … | Turns: …` with no Context suffix, and `events.jsonl iteration.summary` carries `context_window: 0`, `context_tokens: 0`, `context_pct: 0`

### AC5 — events.jsonl carries the summary row
- **Given** any completed iteration
- **When** reading `.ralph/diagnostics/*/events.jsonl`
- **Then** exactly one row with `topic: "iteration.summary"` exists for that iteration, and `ralph events --topic iteration.summary` renders it

### AC6 — RpcEvent::IterationEnd carries the new fields
- **Given** any consumer of the JSON-RPC stream
- **When** the `iteration_end` event is emitted
- **Then** the payload contains `context_window: u64` and `context_tokens: u64`

### AC7 — Per-hat peaks are tracked
- **Given** a multi-hat loop with ≥2 hats each producing iterations with non-zero `context_tokens`
- **When** inspecting `LoopState`
- **Then** `hat_peak_input_tokens` contains an entry per observed hat, each holding the max `context_tokens` seen for that hat, and `peak_input_tokens` equals the max across all hats

### AC8 — Usage struct parses real stream-json
- **Given** a recorded Claude `stream-json` event with nested `message.usage` including both cache fields
- **When** deserializing into `ClaudeStreamEvent::Assistant`
- **Then** `message.usage` is `Some(Usage { ... })` with all four fields populated from the payload (not `None`)

### AC9 — cargo test passes
- **When** running `cargo test` at the repo root
- **Then** all tests pass, including the new unit tests from §12.1

### AC10 — Stale doc removed
- **Given** the implementation commit
- **When** inspecting the tree
- **Then** `docs/specs/context-window-utilization.md` no longer exists and `.ralph/specs/context-window-utilization.md` is the sole canonical spec

## 16. Implementation order (suggested)

1. Fix `Usage` struct (§5) + unit test. Confirms parsing works before wiring.
2. Add `context_window` to `SessionResult`, `ExecutionOutcome`, `RpcEvent::IterationEnd`. Everything builds with zeros.
3. Add `resolve_context_window` and `EventLoopConfig.context_window_tokens`. Thread through executor entry points.
4. Add `ClaudeSessionState`; populate from `Assistant { message }` in `dispatch_stream_event`. Populate `SessionResult` at the Claude `Result` branch.
5. Add `PiSessionState.peak_input_tokens`; use it at the Pi `SessionResult` construction sites.
6. Add `format_session_summary`; refactor Pretty / Console / TUI on_complete. Add table-test.
7. Emit `iteration.summary` events.jsonl row from `loop_runner.rs`. Add integration assertion.
8. Add `LoopState` peak fields; update in `loop_runner.rs` after each outcome.
9. Delete `docs/specs/context-window-utilization.md`.
10. Record a new smoke fixture with real `usage` data; write the ≥2-turn peak-not-sum integration test.
11. `cargo test` green. Manual validation per §12.3.
