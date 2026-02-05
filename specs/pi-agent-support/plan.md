# Implementation Plan: Pi Agent Support

## Checklist

- [ ] Step 1: Pi stream parser types and parsing
- [ ] Step 2: Pi stream event dispatch
- [ ] Step 3: CliBackend constructors and registration
- [ ] Step 4: OutputFormat extension and PtyExecutor integration
- [ ] Step 5: Auto-detection
- [ ] Step 6: Smoke test fixture and replay test
- [ ] Step 7: Documentation and ralph.yml examples

---

## Step 1: Pi stream parser types and parsing

**Objective:** Create `pi_stream.rs` in `ralph-adapters` with the `PiStreamEvent` enum, sub-types, and `PiStreamParser::parse_line()`.

**Implementation guidance:**
- Create `crates/ralph-adapters/src/pi_stream.rs`
- Define `PiStreamEvent` enum with `#[serde(tag = "type", rename_all = "snake_case")]` and variants: `MessageUpdate`, `ToolExecutionStart`, `ToolExecutionEnd`, `TurnEnd`, `Other`
- Define sub-types: `PiAssistantEvent`, `PiToolResult`, `PiContentBlock`, `PiTurnMessage`, `PiUsage`, `PiCost`
- Implement `PiStreamParser::parse_line()` matching `ClaudeStreamParser::parse_line()` pattern
- Add `mod pi_stream;` and `pub use` to `lib.rs`

**Test requirements:**
- Parse each variant from JSON string literals
- Verify `#[serde(other)]` handles unknown `type` values
- Verify `#[serde(other)]` handles unknown `assistantMessageEvent.type` values
- Verify malformed JSON returns `None` (no panic)
- Verify empty/whitespace lines return `None`
- Parse real captured NDJSON lines from research samples

**Integration notes:** No integration yet — pure types and parsing.

**Demo:** Unit tests pass. Parse a real pi NDJSON line and get the correct Rust enum variant.

---

## Step 2: Pi stream event dispatch

**Objective:** Implement `dispatch_pi_stream_event()` and `PiSessionState` that map pi events to `StreamHandler` calls.

**Implementation guidance:**
- Add `PiSessionState` struct with `total_cost_usd: f64` and `num_turns: u32`
- Implement `dispatch_pi_stream_event()` per the design doc
- `TextDelta` → `handler.on_text()` + `extracted_text` append
- `ThinkingDelta` → `handler.on_text()` only if `verbose`
- `Error` → `handler.on_error()`
- `ToolExecutionStart` → `handler.on_tool_call()`
- `ToolExecutionEnd` → `handler.on_tool_result()` or `handler.on_error()` based on `is_error`
- `TurnEnd` → accumulate cost and turn count in state
- `Other` → no-op
- Export from `lib.rs`

**Test requirements:**
- Mock `StreamHandler` that records calls
- Verify `text_delta` calls `on_text` with correct delta
- Verify `text_delta` appends to `extracted_text`
- Verify `thinking_delta` calls `on_text` when verbose=true, skips when false
- Verify `tool_execution_start` calls `on_tool_call` with name, id, args
- Verify `tool_execution_end` (isError=false) calls `on_tool_result`
- Verify `tool_execution_end` (isError=true) calls `on_error`
- Verify `turn_end` accumulates cost (3 turns → summed cost)
- Verify `turn_end` increments turn count
- Verify `Other` variant does nothing

**Integration notes:** Still self-contained in `ralph-adapters`. No changes to other crates.

**Demo:** Feed a sequence of `PiStreamEvent` values through dispatch, verify handler received correct calls and state has correct totals.

---

## Step 3: CliBackend constructors and registration

**Objective:** Add `pi()` and `pi_interactive()` to `CliBackend`, register in all resolution paths.

**Implementation guidance:**
- Add `CliBackend::pi()` — command: `"pi"`, args: `["-p", "--mode", "json", "--no-session"]`, prompt_mode: `Arg`, prompt_flag: `None`, output_format: `PiStreamJson`
- Add `CliBackend::pi_interactive()` — command: `"pi"`, args: `["--no-session"]`, prompt_mode: `Arg`, prompt_flag: `None`, output_format: `Text`
- Add `"pi"` arm to `from_name()`
- Add `"pi"` arm to `from_config()`
- Add `"pi"` arm to `for_interactive_prompt()`
- No large-prompt temp file handling (only Claude needs that)

**Test requirements:**
- `test_pi_backend()` — verify command, args, prompt as positional arg
- `test_pi_interactive_backend()` — verify no `-p`, no `--mode json`
- `test_from_name_pi()` — verify resolution
- `test_for_interactive_prompt_pi()` — verify resolution
- `test_from_config_pi()` — verify resolution
- `test_from_hat_backend_named_with_args_pi()` — verify extra args appended
- `test_pi_interactive_mode_unchanged()` — verify interactive flag filtering is no-op for pi

**Integration notes:** Requires `OutputFormat::PiStreamJson` from Step 4, but the enum variant can be added here first since it's in the same file.

**Demo:** `CliBackend::pi().build_command("test prompt", false)` returns `("pi", ["-p", "--mode", "json", "--no-session", "test prompt"], None, None)`.

---

## Step 4: OutputFormat extension and PtyExecutor integration

**Objective:** Add `OutputFormat::PiStreamJson`, wire pi parsing into `PtyExecutor::run_observe_streaming()`.

**Implementation guidance:**
- Add `PiStreamJson` variant to `OutputFormat` enum in `cli_backend.rs`
- In `pty_executor.rs`, import `PiStreamParser`, `dispatch_pi_stream_event`, `PiSessionState`
- Add `is_pi_stream` check alongside `is_stream_json`
- Add pi parsing branch in the data processing loop (same line-buffering pattern as Claude)
- Initialize `PiSessionState` before the event loop
- After event loop exits, if `is_pi_stream`, call `handler.on_complete()` with synthesized `SessionResult` using wall-clock duration and accumulated state
- Pass `verbose` flag through to `dispatch_pi_stream_event()`

**Test requirements:**
- Existing Claude streaming tests still pass (no regression)
- Pi stream parsing branch activates for `OutputFormat::PiStreamJson`
- `on_complete()` called with accumulated cost and turn count after pi session ends

**Integration notes:** This is the main integration point. Changes `pty_executor.rs` which is a critical path. Careful to only add a new branch, not modify existing Claude/Text paths.

**Demo:** Run Ralph with a mock pi binary that outputs NDJSON fixtures. TUI shows tool calls with ⚙️ icons and text output streams in real-time.

---

## Step 5: Auto-detection

**Objective:** Add `pi` to `DEFAULT_PRIORITY` as the last entry.

**Implementation guidance:**
- Append `"pi"` to `DEFAULT_PRIORITY` in `auto_detect.rs`
- No `detection_command()` mapping needed (binary name matches)

**Test requirements:**
- `test_default_priority_includes_pi()` — verify `pi` is in the list
- `test_default_priority_pi_is_last()` — verify `pi` is the last element
- Existing auto-detection tests still pass

**Integration notes:** Minimal change. One line added to the constant.

**Demo:** `ralph doctor` (or equivalent) shows pi as a detected backend when installed.

---

## Step 6: Smoke test fixture and replay test

**Objective:** Record a real pi NDJSON session and create a replay-based smoke test.

**Implementation guidance:**
- Capture a pi session: `pi -p --mode json --no-session --thinking off "create a file called test.txt with 'hello world', then read it back" > fixture.jsonl`
- Place fixture in `crates/ralph-adapters/tests/fixtures/pi-basic-session.jsonl` (or `crates/ralph-core/tests/fixtures/`)
- Write a test that reads the fixture line by line, parses with `PiStreamParser`, dispatches through `dispatch_pi_stream_event()` with a recording handler
- Verify: text output extracted, tool calls captured, cost accumulated, turn count correct

**Test requirements:**
- Fixture parses without errors
- All expected `on_text`, `on_tool_call`, `on_tool_result` calls present
- `extracted_text` contains the agent's text output
- Cost > 0
- Turn count > 0

**Integration notes:** Tests the full parse+dispatch pipeline against real data.

**Demo:** `cargo test -p ralph-adapters pi_smoke` passes.

---

## Step 7: Documentation and ralph.yml examples

**Objective:** Document pi backend in user-facing docs and provide example configurations.

**Implementation guidance:**
- Update `docs/installation.md` or equivalent to mention pi as a supported backend
- Add pi to the backend comparison table in docs
- Add example `ralph.pi.yml` config file in project root or `examples/`
- Update `AGENTS.md` / `CLAUDE.md` architecture section to mention pi in the backend list
- Update the `NoBackendError` display to include pi install instructions

**Test requirements:**
- Example config is valid YAML
- `NoBackendError` display includes pi

**Integration notes:** Documentation only, no code behavior changes beyond `NoBackendError` message.

**Demo:** User can copy `ralph.pi.yml`, set `backend: pi`, and run `ralph run -c ralph.pi.yml -p "prompt"`.
