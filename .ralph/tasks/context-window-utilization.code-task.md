# Code Task: context-window-utilization

- **Slug:** `context-window-utilization`
- **Spec:** `.ralph/specs/context-window-utilization.md`
- **Issue:** mikeyobrien/ralph-orchestrator#182
- **Status:** Ready for implementation
- **Scope guardrail:** This is bounded, additive plumbing. Do NOT redesign the event loop, the hat router, or the backend trait surface. No automatic compaction. Nine source files + one deletion.

## Outcome

After this task, a Ralph iteration rendered by the Pretty / Console / TUI handlers ends with:

```
Duration: 12345ms | Est. cost: $0.0526 | Turns: 3 | Context: 45% (90K/200K)
```

`events.jsonl` contains one synthetic `iteration.summary` row per iteration carrying the same numbers. `RpcEvent::IterationEnd` carries `context_window` and `context_tokens`. `LoopState` exposes per-hat peak token usage. The `Context:` suffix is hidden when `context_window` is 0 (unknown backend) or when no token data was captured.

## Preconditions

- Read `.ralph/specs/context-window-utilization.md` first. This task derives entirely from that spec; sections referenced below are spec sections.
- `cargo test` green on the starting branch.
- Optional: record a fresh Claude stream-json session with real `message.usage` for test fixtures:
  ```
  cargo run --bin ralph -- run -c ralph.claude.yml --record-session /tmp/ctx-usage.jsonl -p "say hello"
  ```

## Step-by-step

### Step 1 — Fix `Usage` struct (prerequisite; without this, everything reads zero)

**File:** `crates/ralph-adapters/src/claude_stream.rs`

- Move `usage: Option<Usage>` from the `Assistant` variant into `AssistantMessage` (adjacent to `content`). The real Claude `stream-json` nests `usage` inside `message`.
- Add `cache_creation_input_tokens` and `cache_read_input_tokens` to `Usage`, both `#[serde(default)]`.
- Final shape per spec §5.

**Test:** add a unit test that round-trip parses a real-shape `Assistant` event JSON payload (include both cache fields) and asserts `message.usage.is_some()` with all four fields populated non-zero. Use a realistic fixture payload inline — do not mock.

**Gate:** `cargo test -p ralph-adapters claude_stream` green.

### Step 2 — Add `context_window` to `SessionResult`

**File:** `crates/ralph-adapters/src/stream_handler.rs`

- Add `pub context_window: u64` field to `SessionResult` (spec §7.2).
- Derive `Default` continues to zero-initialise. Existing `..Default::default()` call sites continue to compile.

**Gate:** `cargo build` green.

### Step 3 — Add `EventLoopConfig.context_window_tokens` + `resolve_context_window`

**File:** `crates/ralph-core/src/config.rs`

- Add `pub context_window_tokens: Option<u64>` to `EventLoopConfig`.
- Add public `pub fn resolve_context_window(cfg: &RalphConfig) -> u64` per spec §7.8.
- Add a `#[test]` module covering the four cases in spec §12.1 item 3 (explicit override / claude default / pi default / unknown → 0).

**Gate:** `cargo test -p ralph-core config` green.

### Step 4 — Add Pi peak-input tracking

**File:** `crates/ralph-adapters/src/pi_stream.rs`

- Add `pub peak_input_tokens: u64` to `PiSessionState`.
- In the `TurnEnd` handler (currently at ~`pi_stream.rs:242-265`), update:
  ```rust
  state.peak_input_tokens = state
      .peak_input_tokens
      .max(turn.usage.input + turn.usage.cache_read);
  ```
- Add a unit test: construct a `PiSessionState`, feed two `TurnEnd` events (one with larger `input + cache_read`, one smaller), assert `peak_input_tokens` == max.

**Gate:** `cargo test -p ralph-adapters pi_stream` green.

### Step 5 — Add Claude session-state aggregation

**File:** `crates/ralph-adapters/src/pty_executor.rs`

- Introduce a `ClaudeSessionState` struct (local to the executor module, analogous to `PiSessionState`) with:
  - `peak_input_tokens: u64`
  - `total_output_tokens: u64`
  - `peak_cache_read_tokens: u64`
  - `peak_cache_write_tokens: u64`
- In `dispatch_stream_event` for `ClaudeStreamEvent::Assistant { message }`, update from `message.usage` per spec §7.4 code block.
- Replace the `..Default::default()` in the Claude `Result` branch with the aggregated values.

**Gate:** `cargo build` green. Do not yet wire `context_window` — Step 6.

### Step 6 — Thread `context_window` into `SessionResult` at construction

**File:** `crates/ralph-adapters/src/pty_executor.rs` (+ callers)

- Per spec §8 Option A: add a `context_window: u64` parameter to the executor's entry point (whichever `execute`-style function constructs `SessionResult`).
- Populate `SessionResult.context_window` from that parameter at both the Claude `Result` branch and the two Pi construction sites (~`pty_executor.rs:1126-1138, 1182-1193`).
- Replace Pi's `input_tokens` source with `pi_state.peak_input_tokens` (semantic change per spec §4).
- Caller(s) in `loop_runner.rs` (and anywhere else) pass `resolve_context_window(&cfg)`.

**Gate:** `cargo build` green. `SessionResult.context_window` is non-zero when run against claude/pi.

### Step 7 — Extend `ExecutionOutcome` + `RpcEvent::IterationEnd`

**Files:**
- `crates/ralph-cli/src/loop_runner.rs` — `ExecutionOutcome`
- `crates/ralph-proto/src/json_rpc.rs` — `RpcEvent::IterationEnd`
- `crates/ralph-adapters/src/json_rpc_handler.rs` — `on_complete` forwarding

Changes:
- Add `context_window: u64` and `context_tokens: u64` to both structs (spec §7.5, §7.6).
- `context_tokens` is computed once at `ExecutionOutcome` construction: `input_tokens + cache_read_tokens + cache_write_tokens`.
- Update **all three** `ExecutionOutcome` construction sites in `loop_runner.rs` (grep confirms roughly lines 1782-1794, 4214-4223, 4359-4368 at time of spec authoring — find them via `ExecutionOutcome {` in current tree; do not trust the line numbers blindly).
- `JsonRpcStreamHandler::on_complete` forwards both fields from `SessionResult` into `RpcEvent::IterationEnd`.

**Gate:** `cargo build` green across the workspace.

### Step 8 — Shared `format_session_summary` helper + handler updates

**File:** `crates/ralph-adapters/src/stream_handler.rs`

- Add the `format_session_summary(&SessionResult) -> String` helper per spec §9. Keep it a free function (not a method) so all three handlers can reuse it without trait gymnastics.
- Replace the inline summary formatting in:
  - `PrettyStreamHandler::on_complete` (~line 160-180)
  - `ConsoleStreamHandler::on_complete` (~line 297-306; verbose branch only)
  - `TuiStreamHandler::on_complete` (~line 559-578; wrap output in `Line::from(Span::styled(...))`)
- `QuietStreamHandler::on_complete` (~line 316): unchanged silent no-op.
- Add a table-driven unit test covering all five rows in spec §9 rendering table + the `context_window = 0` case.

**Gate:** `cargo test -p ralph-adapters stream_handler` green.

### Step 9 — Emit `iteration.summary` events.jsonl row

**File:** `crates/ralph-cli/src/loop_runner.rs`

- After `log_events_from_output` at the same spot that builds `RpcEvent::IterationEnd` (roughly line 1925-1939 in current tree), emit one `EventRecord` with:
  - `topic = "iteration.summary"`
  - `hat = None`, `triggered = false`, `blocked_count = 0`, `wave_* = None`
  - `payload = serde_json::json!({ ... })` matching the schema in spec §10 (all ten fields always present; integers for tokens/context_pct; float for cost_usd)

**Gate:** Run a short iteration; `jq 'select(.topic == "iteration.summary")' .ralph/diagnostics/*/events.jsonl` returns exactly one row with all ten fields.

### Step 10 — `LoopState` peaks + per-hat map

**Files:**
- `crates/ralph-core/src/event_loop/loop_state.rs` — add the three fields (spec §7.7).
- `crates/ralph-cli/src/loop_runner.rs` — after each iteration's `ExecutionOutcome` is produced and hat id is known, apply the update block from spec §11.

Notes:
- Peaks are session-scoped; do not reset on iteration boundaries.
- No rendering in this feature — data lands in `LoopState` and stays there.

**Gate:** `cargo test -p ralph-core loop_state` green. Add a unit test exercising the update logic via a mocked outcome.

### Step 11 — Delete stale doc

**File:** `docs/specs/context-window-utilization.md` — DELETE.

The canonical spec is `.ralph/specs/context-window-utilization.md`. The stale file predates Pi token plumbing and contradicts current code.

### Step 12 — Record a multi-turn smoke fixture

Only if the current fixtures under `crates/ralph-core/tests/fixtures/` lack Claude stream-json with real `message.usage` — which they do (confirmed in research, §9). Either:

- Record a new `.jsonl` session via `cargo run --bin ralph -- run -c ralph.claude.yml --record-session ...` and add a minimal integration test asserting `SessionResult.input_tokens == peak (not sum)`.
- Or hand-craft a minimal fixture with two `Assistant` events, increasing `usage.input_tokens`, for a focused integration test.

Hand-crafted is acceptable and faster. Commit the fixture alongside the test.

**Gate:** new integration test passes; `cargo test` green workspace-wide.

### Step 13 — Final validation

```
cargo test
./scripts/setup-hooks.sh                     # if not already installed
cargo run -p ralph-e2e -- --mock             # smoke-check mock E2E
cargo run --bin ralph -- run -c ralph.claude.yml -p "say hello" --max-iterations 1
# → summary line ends with " | Context: NN% (KK/200K)"
# → .ralph/diagnostics/*/events.jsonl contains one iteration.summary row
```

Then manual checks from spec §12.3:
- Set `event_loop.context_window_tokens: 1_000_000` in `ralph.yml`; rerun; verify `(KK/1000K)`.
- Run against an ACP backend; verify `Context:` suffix absent and `events.jsonl` shows `context_window: 0`.

## Acceptance criteria

Reproduced verbatim from spec §15 — implementation must satisfy **all ten**:

- **AC1:** Claude backend with no override → summary ends with ` | Context: NN% (KK/200K)` (`KK > 0`).
- **AC2:** Pi backend with increasing per-turn input → `SessionResult.input_tokens == max(turn.input + turn.cache_read)` (not the sum); summary shows `Context:`.
- **AC3:** `event_loop.context_window_tokens: 1_000_000` → display `(KK/1000K)`; `events.jsonl` payload `context_window: 1000000`.
- **AC4:** Unknown backend (ACP/Copilot/Roo) → no `Context:` suffix; payload `context_window: 0, context_tokens: 0, context_pct: 0`.
- **AC5:** Every completed iteration → exactly one `events.jsonl` row with `topic: "iteration.summary"`; `ralph events --topic iteration.summary` renders it.
- **AC6:** `RpcEvent::IterationEnd` payload contains `context_window: u64` and `context_tokens: u64`.
- **AC7:** Multi-hat run → `LoopState.hat_peak_input_tokens` has one entry per observed hat, each equal to the max `context_tokens` seen for that hat; `peak_input_tokens` equals the overall max.
- **AC8:** Real Claude stream-json round-trip test parses nested `message.usage` with both cache fields as `Some(Usage { ... })`, not `None`.
- **AC9:** `cargo test` green at workspace root.
- **AC10:** `docs/specs/context-window-utilization.md` deleted; `.ralph/specs/context-window-utilization.md` is the sole canonical spec.

## Out of scope (do NOT implement)

- Automatic compaction / prompt pruning.
- Threshold warnings (`> 80%`).
- Dashboard / web frontend rendering of the new data.
- Model-aware context-window auto-detection from `System.model`.
- Token capture for ACP / Copilot / Roo / Gemini / Kiro / Codex — they continue to report zero.
- Refactoring `ExecutionOutcome` or `SessionResult` beyond the specified additions.
- Backwards compatibility shims for old `SessionResult` callers.

## Reference: file touch list

```
crates/ralph-adapters/src/claude_stream.rs         MODIFY  (Step 1)
crates/ralph-adapters/src/stream_handler.rs        MODIFY  (Steps 2, 8)
crates/ralph-adapters/src/pi_stream.rs             MODIFY  (Step 4)
crates/ralph-adapters/src/pty_executor.rs          MODIFY  (Steps 5, 6)
crates/ralph-adapters/src/json_rpc_handler.rs      MODIFY  (Step 7)
crates/ralph-core/src/event_loop/loop_state.rs     MODIFY  (Step 10)
crates/ralph-core/src/config.rs                    MODIFY  (Step 3)
crates/ralph-cli/src/loop_runner.rs                MODIFY  (Steps 6, 7, 9, 10)
crates/ralph-proto/src/json_rpc.rs                 MODIFY  (Step 7)
docs/specs/context-window-utilization.md           DELETE  (Step 11)
crates/ralph-core/tests/fixtures/<new>.jsonl       CREATE  (Step 12, optional)
```

## Commit guidance

- Suggested commit subject: `feat(telemetry): track context window utilization per iteration (#182)`
- One commit for the whole feature is fine (it's bounded and internally consistent). Split only if Step 1 (`Usage` struct fix) benefits from isolation — it's a pure bug fix and could land independently.
- Do not skip hooks. If pre-commit fails, fix the issue and create a NEW commit (don't amend).
