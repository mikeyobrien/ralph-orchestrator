## Scope
- Adapter process lifecycle lane covering [crates/ralph-adapters/src/pty_executor.rs](/home/coe/scroll/agent-orchestrator/crates/ralph-adapters/src/pty_executor.rs), [crates/ralph-adapters/src/acp_executor.rs](/home/coe/scroll/agent-orchestrator/crates/ralph-adapters/src/acp_executor.rs), [crates/ralph-adapters/src/cli_executor.rs](/home/coe/scroll/agent-orchestrator/crates/ralph-adapters/src/cli_executor.rs), [crates/ralph-adapters/src/cli_backend.rs](/home/coe/scroll/agent-orchestrator/crates/ralph-adapters/src/cli_backend.rs), [crates/ralph-adapters/src/stream_handler.rs](/home/coe/scroll/agent-orchestrator/crates/ralph-adapters/src/stream_handler.rs), [crates/ralph-adapters/src/pi_stream.rs](/home/coe/scroll/agent-orchestrator/crates/ralph-adapters/src/pi_stream.rs), [crates/ralph-adapters/src/json_rpc_handler.rs](/home/coe/scroll/agent-orchestrator/crates/ralph-adapters/src/json_rpc_handler.rs), and related tests under [crates/ralph-adapters/tests](/home/coe/scroll/agent-orchestrator/crates/ralph-adapters/tests).
- Prioritized bug classes: process lifecycle, orphan cleanup, cancellation, signal handling, streaming/order loss, partial output, deadlock, race conditions, retry/timeout behavior, malformed backend output handling, bootstrap assumptions, and tests-versus-implementation drift.

## Coverage Summary
- Highest-risk files exhausted: `acp_executor.rs`, `pty_executor.rs`, and `cli_executor.rs`.
- Follow-up pass completed on mirrored test surfaces and lower-risk support code in `cli_backend.rs`, `stream_handler.rs`, `pi_stream.rs`, and `json_rpc_handler.rs`.
- Material findings: 5 total.
  - `P1`: 2
  - `P2`: 3
- Residual risk is now concentrated in live backend behavior with real `kiro-cli` / `pi` / `claude` binaries, not in additional static paths inside this scope.

## Report file
- [artifacts/bug-hunter-adapter-process-lifecycle.md](/home/coe/scroll/agent-orchestrator/artifacts/bug-hunter-adapter-process-lifecycle.md)

## Findings
### P1. ACP terminal capture can deadlock on stderr-heavy commands
- Impacted files: [acp_executor.rs](/home/coe/scroll/agent-orchestrator/crates/ralph-adapters/src/acp_executor.rs#L202), [acp_executor.rs](/home/coe/scroll/agent-orchestrator/crates/ralph-adapters/src/acp_executor.rs#L280)
- Why this is a bug: `create_terminal()` drains `stdout` to EOF before it starts reading `stderr`. If a child keeps `stdout` open while writing enough data to fill `stderr`, the child blocks on `stderr`, the reader never reaches the `stderr` loop, and `wait_for_terminal_exit()` can hang forever.
- Exact evidence: [acp_executor.rs](/home/coe/scroll/agent-orchestrator/crates/ralph-adapters/src/acp_executor.rs#L209) reads `stdout` in a full loop, only then enters the `stderr` loop at line 221. `wait_for_terminal_exit()` then awaits process exit at [acp_executor.rs](/home/coe/scroll/agent-orchestrator/crates/ralph-adapters/src/acp_executor.rs#L315).
- Trigger scenario: an ACP terminal command like `python -c 'import sys,time; sys.stderr.write(\"x\"*200000); sys.stderr.flush(); time.sleep(60)'` or any tool that writes large stderr before closing stdout.
- Likely impact: stuck ACP tool call, hung `wait_for_terminal_exit`, leaked child until outer cancellation, and a blocked orchestration turn.
- Root cause: sequential pipe draining on two bounded OS pipes.
- Fix direction: read `stdout` and `stderr` concurrently, or multiplex both into a shared channel exactly as `cli_executor.rs` already does.
- Confidence: High.
- Current tests: insufficient. [acp_executor.rs](/home/coe/scroll/agent-orchestrator/crates/ralph-adapters/src/acp_executor.rs#L759) only exercises `echo`, then waits for exit before checking output.

### P2. ACP `terminal_output` never exposes live output before process exit
- Impacted files: [acp_executor.rs](/home/coe/scroll/agent-orchestrator/crates/ralph-adapters/src/acp_executor.rs#L209), [acp_executor.rs](/home/coe/scroll/agent-orchestrator/crates/ralph-adapters/src/acp_executor.rs#L241), [acp_executor.rs](/home/coe/scroll/agent-orchestrator/crates/ralph-adapters/src/acp_executor.rs#L259)
- Why this is a bug: the shared output buffer is assigned once, after both read loops finish. `terminal_output()` therefore returns an empty or stale buffer while the terminal is still running.
- Exact evidence: `combined` is accumulated in a local variable and only copied into `state.output` at [acp_executor.rs](/home/coe/scroll/agent-orchestrator/crates/ralph-adapters/src/acp_executor.rs#L241). `terminal_output()` reads only `state.output` at [acp_executor.rs](/home/coe/scroll/agent-orchestrator/crates/ralph-adapters/src/acp_executor.rs#L270).
- Trigger scenario: any long-running terminal command that prints progress and is polled with `terminal_output()` before completion.
- Likely impact: ACP clients cannot surface interactive terminal progress, and polling code can incorrectly assume the command is silent or hung.
- Root cause: output buffering is post-exit, not incremental.
- Fix direction: append to the shared buffer incrementally inside each read task and preserve truncation logic there.
- Confidence: High.
- Current tests: misleading. [acp_executor.rs](/home/coe/scroll/agent-orchestrator/crates/ralph-adapters/src/acp_executor.rs#L772) waits for exit before asserting on output, so it encodes the broken assumption.

### P1. CLI executor can hang forever if a descendant inherits stdout/stderr
- Impacted files: [cli_executor.rs](/home/coe/scroll/agent-orchestrator/crates/ralph-adapters/src/cli_executor.rs#L142), [cli_executor.rs](/home/coe/scroll/agent-orchestrator/crates/ralph-adapters/src/cli_executor.rs#L184), [cli_executor.rs](/home/coe/scroll/agent-orchestrator/crates/ralph-adapters/src/cli_executor.rs#L202), [cli_executor.rs](/home/coe/scroll/agent-orchestrator/crates/ralph-adapters/src/cli_executor.rs#L242)
- Why this is a bug: the executor does not finish until both pipe-reader tasks see EOF. It only signals the direct child PID, not a process group. If the child exits but a background descendant still holds the inherited pipe FDs open, the readers never finish and `execute()` never returns.
- Exact evidence: the main loop waits for `StdoutEof` and `StderrEof` at [cli_executor.rs](/home/coe/scroll/agent-orchestrator/crates/ralph-adapters/src/cli_executor.rs#L175), then unconditionally awaits both reader tasks at [cli_executor.rs](/home/coe/scroll/agent-orchestrator/crates/ralph-adapters/src/cli_executor.rs#L186). `terminate_child()` only signals the direct child PID at [cli_executor.rs](/home/coe/scroll/agent-orchestrator/crates/ralph-adapters/src/cli_executor.rs#L210). `read_stream()` does not return until EOF at [cli_executor.rs](/home/coe/scroll/agent-orchestrator/crates/ralph-adapters/src/cli_executor.rs#L251).
- Trigger scenario: shell wrappers such as `sh -c 'sleep 300 &'`, agent CLIs that spawn helper daemons, or any backend that backgrounds a child without closing inherited stdio.
- Likely impact: wedged orchestration iteration and orphaned helper processes.
- Root cause: completion is tied to inherited pipe closure, but teardown is only applied to the direct child.
- Fix direction: run the child in its own process group and terminate the whole group on timeout/cancellation; also cancel pipe readers once the child tree is known to be dead.
- Confidence: High.
- Current tests: insufficient. The `cli_executor.rs` timeout tests at [cli_executor.rs](/home/coe/scroll/agent-orchestrator/crates/ralph-adapters/src/cli_executor.rs#L367) and [cli_executor.rs](/home/coe/scroll/agent-orchestrator/crates/ralph-adapters/src/cli_executor.rs#L428) use direct children only.

### P2. CLI inactivity timeout is keyed to newline delivery, not byte activity
- Impacted files: [cli_executor.rs](/home/coe/scroll/agent-orchestrator/crates/ralph-adapters/src/cli_executor.rs#L142), [cli_executor.rs](/home/coe/scroll/agent-orchestrator/crates/ralph-adapters/src/cli_executor.rs#L160), [cli_executor.rs](/home/coe/scroll/agent-orchestrator/crates/ralph-adapters/src/cli_executor.rs#L250)
- Why this is a bug: `read_stream()` uses `BufReadExt::lines()`, so activity is invisible until a newline or EOF arrives. Backends that emit progress dots, carriage-return updates, or long partial chunks without `\n` are treated as idle and can be killed despite continuous output.
- Exact evidence: timeout is enforced around `event_rx.recv()` at [cli_executor.rs](/home/coe/scroll/agent-orchestrator/crates/ralph-adapters/src/cli_executor.rs#L143), but the only events come from `lines().next_line()` at [cli_executor.rs](/home/coe/scroll/agent-orchestrator/crates/ralph-adapters/src/cli_executor.rs#L251).
- Trigger scenario: `python -u -c 'import sys,time; [sys.stdout.write(\".\") or sys.stdout.flush() or time.sleep(0.1) for _ in range(20)]'` with a 300ms timeout.
- Likely impact: false timeouts and loss of partial output on CLIs that stream without newline framing.
- Root cause: line buffering is being used as the transport boundary for timeout accounting.
- Fix direction: read raw bytes or chunks, reset the timeout on any read, and only apply line splitting for presentation.
- Confidence: High.
- Current tests: narrow. Existing timeout coverage at [cli_executor.rs](/home/coe/scroll/agent-orchestrator/crates/ralph-adapters/src/cli_executor.rs#L397) and [cli_executor.rs](/home/coe/scroll/agent-orchestrator/crates/ralph-adapters/src/cli_executor.rs#L428) only uses newline-terminated output.

### P2. PTY streaming drops split UTF-8 chunks and can corrupt NDJSON parsing
- Impacted files: [pty_executor.rs](/home/coe/scroll/agent-orchestrator/crates/ralph-adapters/src/pty_executor.rs#L751), [pty_executor.rs](/home/coe/scroll/agent-orchestrator/crates/ralph-adapters/src/pty_executor.rs#L880), [pty_executor.rs](/home/coe/scroll/agent-orchestrator/crates/ralph-adapters/src/pty_executor.rs#L941)
- Why this is a bug: every PTY read chunk is ignored unless `std::str::from_utf8(&data)` succeeds for the whole chunk. PTY reads can legally split a multibyte UTF-8 code point across two reads, so both halves are discarded. For JSON backends, that can make the line buffer permanently malformed and lose tool/text/result events.
- Exact evidence: all three streaming paths gate parsing on `if let Ok(text) = std::str::from_utf8(&data)` at [pty_executor.rs](/home/coe/scroll/agent-orchestrator/crates/ralph-adapters/src/pty_executor.rs#L755), [pty_executor.rs](/home/coe/scroll/agent-orchestrator/crates/ralph-adapters/src/pty_executor.rs#L882), and [pty_executor.rs](/home/coe/scroll/agent-orchestrator/crates/ralph-adapters/src/pty_executor.rs#L943). There is no carry-over byte buffer for incomplete UTF-8 sequences.
- Trigger scenario: a backend emits NDJSON containing non-ASCII text, and a multibyte character is split across PTY reads. This is especially plausible with long `pi` lines or Claude JSON lines containing Unicode text.
- Likely impact: dropped assistant text, missing tool events, and false negatives in `extracted_text`-based loop-complete detection.
- Root cause: chunk-level UTF-8 validation without a remainder buffer.
- Fix direction: keep a `Vec<u8>` remainder across reads, append new bytes, decode with a UTF-8 streaming decoder or split at valid boundaries, then feed complete text into `line_buffer`.
- Confidence: High.
- Current tests: incomplete. The regression tests at [pty_executor_integration.rs](/home/coe/scroll/agent-orchestrator/crates/ralph-adapters/tests/pty_executor_integration.rs#L558) and [pty_executor_integration.rs](/home/coe/scroll/agent-orchestrator/crates/ralph-adapters/tests/pty_executor_integration.rs#L623) only exercise long ASCII NDJSON lines, not split multibyte boundaries.

## Evidence
- ACP lifecycle cleanup itself looks intentionally covered: [acp_process_cleanup.rs](/home/coe/scroll/agent-orchestrator/crates/ralph-adapters/tests/acp_process_cleanup.rs#L91) checks parent cleanup, [acp_process_cleanup.rs](/home/coe/scroll/agent-orchestrator/crates/ralph-adapters/tests/acp_process_cleanup.rs#L130) checks grandchild cleanup, and [acp_process_cleanup.rs](/home/coe/scroll/agent-orchestrator/crates/ralph-adapters/tests/acp_process_cleanup.rs#L169) checks consecutive hat transitions.
- The defects above are therefore not generic “ACP cleanup is broken” claims; they are narrower terminal-streaming and CLI lifecycle failures not covered by those tests.
- I did not modify source code in this lane. I attempted throwaway runtime repro harnesses outside the repo, but the environment blocked dependency resolution / cargo execution, so the findings above are grounded in direct code paths plus existing test coverage gaps.

## Areas inspected
- No material P0-P2 finding in [cli_backend.rs](/home/coe/scroll/agent-orchestrator/crates/ralph-adapters/src/cli_backend.rs). `build_command()` preserves temp-file lifetimes and interactive arg filtering in a way that matches its extensive unit tests, including large prompts and ACP-specific config at [cli_backend.rs](/home/coe/scroll/agent-orchestrator/crates/ralph-adapters/src/cli_backend.rs#L659).
- No material P0-P2 finding in [json_rpc_handler.rs](/home/coe/scroll/agent-orchestrator/crates/ralph-adapters/src/json_rpc_handler.rs). Broken-pipe suppression is implemented at [json_rpc_handler.rs](/home/coe/scroll/agent-orchestrator/crates/ralph-adapters/src/json_rpc_handler.rs#L75) and regression-tested at [json_rpc_handler.rs](/home/coe/scroll/agent-orchestrator/crates/ralph-adapters/src/json_rpc_handler.rs#L393).
- No material P0-P2 finding in [pi_stream.rs](/home/coe/scroll/agent-orchestrator/crates/ralph-adapters/src/pi_stream.rs). Unknown event forward-compatibility and malformed-line skipping are explicit in [pi_stream.rs](/home/coe/scroll/agent-orchestrator/crates/ralph-adapters/src/pi_stream.rs#L121) and [pi_stream.rs](/home/coe/scroll/agent-orchestrator/crates/ralph-adapters/src/pi_stream.rs#L185), with matching parser/dispatch tests in the same file.
- No material P0-P2 finding in [stream_handler.rs](/home/coe/scroll/agent-orchestrator/crates/ralph-adapters/src/stream_handler.rs). The ordering-preservation design in `TuiStreamHandler` is explicit at [stream_handler.rs](/home/coe/scroll/agent-orchestrator/crates/ralph-adapters/src/stream_handler.rs#L446) and the surface is heavily unit-tested elsewhere in that file.

## Recommended next search
- Add black-box regressions for the four failure modes above before changing implementation:
  - ACP terminal command with stdout kept open and `stderr` pipe flood.
  - ACP `terminal_output()` polled before exit.
  - CLI command with newline-free incremental output under inactivity timeout.
  - PTY JSON stream containing a multibyte UTF-8 code point deliberately split across two writes.
- After those regressions, the next most valuable lane is a live-backend validation pass against real `kiro-cli` and `pi` binaries, because the remaining risk is concentrated in runtime behavior that static inspection cannot confirm.
