# Scope

Scoped lane `adapter-pty-acp-cancel`, limited to:

- `crates/ralph-adapters/src/pty_executor.rs`
- `crates/ralph-adapters/src/acp_executor.rs`
- `crates/ralph-adapters/tests/pty_executor_integration.rs`
- `crates/ralph-adapters/tests/acp_process_cleanup.rs`

# Findings

## P1: Observe-mode PTY idle timeouts are disabled whenever `interactive=false`

- Impacted files:
  - `crates/ralph-adapters/src/pty_executor.rs`
- Why it is a bug:
  - Observe-mode documentation says idle timeout should terminate silent commands, but the implementation drops the timeout when the executor is non-interactive.
- Exact evidence:
  - `run_observe()` docs promise idle timeout behavior: `crates/ralph-adapters/src/pty_executor.rs:321`.
  - The effective timeout becomes `None` when `!self.config.interactive`: `crates/ralph-adapters/src/pty_executor.rs:367`.
  - The same logic exists in streaming observe mode: `crates/ralph-adapters/src/pty_executor.rs:665`.
- Triggering scenario:
  - Run observe mode with `PtyConfig { interactive: false, idle_timeout_secs: 30, .. }` against a silent or hung command.
  - The command never hits idle timeout and runs until an external interrupt.
- Likely impact:
  - Hung observe-mode runs that ignore configured safety limits.
- Recommended fix direction:
  - Apply idle timeout independently from `interactive`, or split those concerns explicitly so non-interactive observe mode still honors timeout.
- Confidence:
  - High.
- Whether current tests cover it:
  - No. The inspected PTY integration tests only exercised observe mode with `idle_timeout_secs: 0`.

## P2: ACP `terminal_output()` never exposes live output before EOF

- Impacted files:
  - `crates/ralph-adapters/src/acp_executor.rs`
- Why it is a bug:
  - The terminal reader accumulates output in local buffers and only writes shared state once the command finishes, so polling during execution returns empty or stale output.
- Exact evidence:
  - Reader task flushes shared state only at the end: `crates/ralph-adapters/src/acp_executor.rs:209` and `crates/ralph-adapters/src/acp_executor.rs:241`.
  - `terminal_output()` reads only the shared state buffer: `crates/ralph-adapters/src/acp_executor.rs:259`.
- Triggering scenario:
  - Start a terminal command like `sh -c 'printf ready; sleep 60'`.
  - Poll `terminal_output()` before process exit.
  - It returns empty because the shared output buffer is not updated until EOF.
- Likely impact:
  - Broken live terminal UX for ACP-backed sessions.
- Recommended fix direction:
  - Update shared output state incrementally as chunks arrive rather than only at terminal completion.
- Confidence:
  - High.
- Whether current tests cover it:
  - No. The inspected ACP coverage checks output only after `wait_for_terminal_exit()`.

# No-Finding Coverage Notes

- `crates/ralph-adapters/tests/acp_process_cleanup.rs`
  - Checked ACP top-level cleanup after `execute()`.
  - Existing cleanup tests already cover the top-level ACP process-tree path well enough that I did not report an additional orphan leak there.
- `crates/ralph-adapters/src/pty_executor.rs`
  - Checked `run_observe`, `run_observe_streaming`, `run_interactive`, `terminate_child`, and `wait_for_exit`.
  - No separate P0-P2 defect confirmed in the inspected path beyond the idle-timeout issue.

# Remaining Blind Spots

- This lane did not inspect `cli_executor`, `cli_backend`, or JSON-RPC adapter code.
- I did not validate concurrent ACP terminal operations outside the requested files.

# Recommended Next Search

- Add a PTY regression test for observe mode with `interactive=false` and `idle_timeout_secs > 0`.
- Add an ACP regression test that polls `terminal_output()` before process exit.
