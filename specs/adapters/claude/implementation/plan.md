# Implementation Plan: Claude Adapter Streaming Output

## Checklist

- [ ] Step 1: Add OutputFormat enum to CliBackend
- [ ] Step 2: Create Claude stream event types
- [ ] Step 3: Implement JSON line parser
- [ ] Step 4: Create StreamHandler trait and ConsoleStreamHandler
- [ ] Step 5: Add streaming support to PTY executor
- [ ] Step 6: Add verbosity CLI flags to ralph run
- [ ] Step 7: Wire up streaming in main execution path
- [ ] Step 8: Add quiet mode support
- [ ] Step 9: Integration testing and polish

---

## Step 1: Add OutputFormat enum to CliBackend

**Objective:** Extend the adapter configuration to declare output format capability.

**Implementation guidance:**
- Add `OutputFormat` enum with `Text` (default) and `StreamJson` variants
- Add `output_format` field to `CliBackend` struct
- Update `claude()` constructor to set `OutputFormat::StreamJson` and add `--output-format stream-json` to args
- All other adapter constructors use `OutputFormat::Text` (no changes needed due to `Default`)

**Test requirements:**
- Unit test: `CliBackend::claude()` returns config with `OutputFormat::StreamJson`
- Unit test: `CliBackend::kiro()` returns config with `OutputFormat::Text`
- Unit test: `build_command()` includes `--output-format stream-json` for Claude

**Integration with previous work:** This is the foundation layer; no dependencies.

**Demo:** Run `cargo test -p ralph-adapters` and show passing tests for new OutputFormat field.

---

## Step 2: Create Claude stream event types

**Objective:** Define typed Rust structs for Claude's NDJSON event schema.

**Implementation guidance:**
- Create new file `crates/ralph-adapters/src/claude_stream.rs`
- Define `ClaudeStreamEvent` enum with `#[serde(tag = "type")]` for JSON parsing
- Implement variants: `System`, `Assistant`, `User`, `Result`
- Define supporting types: `AssistantMessage`, `UserMessage`, `ContentBlock`, `Usage`
- Add module to `lib.rs` exports

**Test requirements:**
- Unit test: Parse sample `system` event JSON into `ClaudeStreamEvent::System`
- Unit test: Parse `assistant` event with text content
- Unit test: Parse `assistant` event with `tool_use` content
- Unit test: Parse `result` event with all fields

**Integration with previous work:** Uses serde for JSON parsing (already a dependency).

**Demo:** Run parser tests showing real Claude JSON samples deserialize correctly.

---

## Step 3: Implement JSON line parser

**Objective:** Create parser that handles NDJSON lines with graceful error handling.

**Implementation guidance:**
- Add `ClaudeStreamParser` struct to `claude_stream.rs`
- Implement `parse_line(&str) -> Option<ClaudeStreamEvent>`
- Skip empty lines (return None)
- Skip malformed JSON with `tracing::debug!` log (return None)
- Add helper `truncate(s: &str, max: usize)` for log messages

**Test requirements:**
- Unit test: Valid JSON returns `Some(event)`
- Unit test: Empty line returns `None`
- Unit test: Malformed JSON returns `None` (doesn't panic)
- Unit test: Partial/truncated JSON returns `None`

**Integration with previous work:** Builds on event types from Step 2.

**Demo:** Run tests showing parser handles mixed valid/invalid input gracefully.

---

## Step 4: Create StreamHandler trait and ConsoleStreamHandler

**Objective:** Create abstraction for handling parsed events and default console output.

**Implementation guidance:**
- Create new file `crates/ralph-adapters/src/stream_handler.rs`
- Define `StreamHandler` trait with methods: `on_text`, `on_tool_call`, `on_tool_result`, `on_error`, `on_complete`
- Define `SessionResult` struct for completion data
- Implement `ConsoleStreamHandler` with verbosity flag
- Implement `QuietStreamHandler` (no-op)
- Format output as plain text per requirements

**Test requirements:**
- Unit test: `ConsoleStreamHandler` with mock stdout captures expected output
- Unit test: `on_error` writes to both stdout and stderr
- Unit test: `on_tool_result` only outputs in verbose mode
- Unit test: `on_complete` only outputs in verbose mode
- Unit test: `QuietStreamHandler` produces no output

**Integration with previous work:** Independent of parser; uses event types from Step 2.

**Demo:** Create small test binary that feeds events to handler and shows formatted output.

---

## Step 5: Add streaming support to PTY executor

**Objective:** Extend PTY executor to parse JSON lines and dispatch to handler in real-time.

**Implementation guidance:**
- Add `run_observe_streaming<H: StreamHandler>()` method to `PtyExecutor`
- Accept `OutputFormat` parameter to determine parsing behavior
- Implement line buffering for chunked PTY reads
- For `StreamJson` format: parse complete lines and dispatch events
- For `Text` format: pass through to existing behavior
- Add `dispatch_event()` helper function

**Test requirements:**
- Integration test: Mock process emitting NDJSON, verify handler receives events
- Unit test: Line buffering correctly handles partial lines across chunks
- Unit test: `OutputFormat::Text` skips JSON parsing

**Integration with previous work:** Combines parser (Step 3) and handler (Step 4) in execution context.

**Demo:** Run test with mock Claude output showing real-time event handling.

---

## Step 6: Add verbosity CLI flags to ralph run

**Objective:** Add `--verbose` and `--quiet` flags with proper precedence.

**Implementation guidance:**
- Add `Verbosity` enum to appropriate config module
- Implement `Verbosity::resolve(cli_verbose, cli_quiet, config)` with precedence logic
- Add `--verbose`/`-v` and `--quiet`/`-q` flags to `ralph run` command (clap)
- Support `RALPH_VERBOSE` and `RALPH_QUIET` environment variables
- Add optional `verbose` and `quiet` fields to config file schema

**Test requirements:**
- Unit test: CLI flags override env vars
- Unit test: Env vars override config
- Unit test: Default is `Verbosity::Normal`
- Unit test: Both flags set → quiet wins (or error)

**Integration with previous work:** Independent CLI/config work; will be wired to handler in Step 7.

**Demo:** Run `ralph run --help` showing new flags; test precedence with env vars.

---

## Step 7: Wire up streaming in main execution path

**Objective:** Connect all components so `ralph run` produces streaming output for Claude.

**Implementation guidance:**
- In `ralph run` execution path, check if backend is Claude with `StreamJson` format
- Resolve verbosity from CLI/env/config
- Create appropriate `StreamHandler` based on verbosity
- Call `run_observe_streaming()` instead of `run_observe()` for Claude
- Ensure non-Claude backends continue working unchanged

**Test requirements:**
- Integration test: `ralph run` with Claude produces streaming output
- Integration test: `ralph run` with Kiro produces no streaming (text format)
- Manual test: End-to-end with real Claude CLI

**Integration with previous work:** Combines Steps 1-6 into working feature.

**Demo:** Run `ralph run -P test-prompt.md` with Claude and watch streaming output appear in real-time.

---

## Step 8: Add quiet mode support

**Objective:** Implement `--quiet` flag to suppress all streaming output.

**Implementation guidance:**
- When `Verbosity::Quiet`, use `QuietStreamHandler`
- Verify iteration still completes and returns result
- Ensure errors still written to stderr even in quiet mode (or configurable)

**Test requirements:**
- Integration test: `--quiet` produces no stdout
- Integration test: Errors still appear on stderr in quiet mode
- Manual test: Verify CI-friendly behavior

**Integration with previous work:** Uses handler from Step 4, wiring from Step 7.

**Demo:** Run `ralph run -P test.md --quiet` and verify no output until completion.

---

## Step 9: Integration testing and polish

**Objective:** Comprehensive testing, documentation, and cleanup.

**Implementation guidance:**
- Add integration tests covering all verbosity modes
- Test error scenarios (malformed JSON, session errors)
- Update `specs/adapters/claude.spec.md` to mark streaming as implemented
- Add usage examples to documentation
- Clean up any TODO comments or temporary code

**Test requirements:**
- Full test suite passes
- Manual end-to-end testing with various prompts
- Test with `--verbose` for full output
- Test error handling with intentionally failing prompts

**Integration with previous work:** Final validation of complete feature.

**Demo:** Full demo of streaming output in action with various verbosity levels.

---

## Notes

- Each step builds incrementally on previous steps
- Steps 1-4 can be developed with unit tests only (no full ralph run needed)
- Step 5 is the key integration point for execution
- Steps 6-7 wire everything to CLI
- Steps 8-9 are polish and validation
