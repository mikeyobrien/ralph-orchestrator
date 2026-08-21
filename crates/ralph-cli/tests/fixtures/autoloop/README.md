# Fake autoloop fixtures

These JSONL fixtures drive
`ralph_core::testing::fake_autoloop::build_fake_autoloop`. The helper is
available on Unix when `ralph-core`'s `test-support` feature is enabled. It
materializes an `autoloop` executable and its private state beneath the caller's
chosen directory.

## File format

A fixture contains one JSON object per non-empty line. Each object describes one
`autoloop` invocation and has a `steps` array. Steps execute in array order.
Unknown fields and malformed JSON are rejected when the fake is built.

```json
{"steps":[{"events":["<raw NDJSON LoopEvent line>"]},{"barrier":{"ready_env":"STREAM_READY","release_env":"STREAM_RELEASE","then_exit":1}},{"journal":["<raw NDJSON journal line>"]},{"stdout":["<literal stdout line>"]},{"stderr":["[autoloops] [info] <literal engine log line>"]},{"summary":{"run_id":"run-test","iterations":1,"stop_reason":"completed","cost_usd":0.01,"journal":"${AUTOLOOP_STATE_DIR}/journal.jsonl","memory":"${AUTOLOOP_STATE_DIR}/memory.jsonl"}},{"exit":0}]}
```

`cost_usd` and `barrier.then_exit` are optional. All other fields shown for a
step are required. Environment-variable names in barriers must be valid shell
environment names.

Each element of `events` and `journal` is one raw NDJSON line represented as a
JSON string, so its inner quotes must be escaped. Keep each invocation object on
a single physical line; blank lines between invocation objects are allowed.

## Step behavior

- **`events`** appends its lines, in order, to the path following `--events` in
  the fake's argv. If `--events` is absent or has no following path, the fake
  writes an error to stderr and exits with status 64.
- **`barrier`** is conditional. If the variable named by `ready_env` is unset or
  empty, the entire step is skipped. Otherwise the variable named by
  `release_env` must contain a path; if it does not, the fake reports an error
  and exits with status 64. The fake touches the path in `$ready_env`, then
  polls every 10 ms until the path in `$release_env` exists. If `then_exit` is
  present, the fake exits with that status after release; it applies only when
  the barrier ran. This supports one fixture serving both normal and
  coordinated-crash test paths.
- **`journal`** appends its lines to `$JOURNAL_OUT` when set. Otherwise it uses
  `$AUTOLOOP_STATE_DIR/journal.jsonl` when that root is set, and finally falls
  back to `./.autoloop/journal.jsonl`. Parent directories are created.
- **`stdout`** and **`stderr`** write their literal lines to the corresponding
  process stream. They can appear anywhere in the steps array, including before
  or after `summary`, so tests can model engine logs and surrounding output
  noise while preserving exact step order.
- **`summary`** writes the canonical summary block to stdout:

  ```text
  autoloops summary
  ===================
  run_id: <run_id>
  iterations: <iterations>
  stop_reason: <stop_reason>
  cost_usd: <cost_usd, only when present>
  journal: <journal>
  memory: <memory>
  ```

  The `cost_usd` line is omitted when the fixture does not provide it. A
`${AUTOLOOP_STATE_DIR}/` prefix in `journal` or `memory` expands at runtime,
using the standalone `./.autoloop` fallback when the variable is unset. Ralph
runtime success fixtures should use that prefix because Ralph rejects summaries
that report anything except its exact owned journal and memory files. Tests may
set `$SUMMARY_OUT` to record those two resolved paths, one per line.
- **`exit`** immediately terminates with the specified status. If execution
  reaches the end of the steps, the fake exits with status 0.

On every invocation the dispatcher records argv, one argument per line, to
`$ARGV_OUT` when that variable is set. `FakeAutoloop::argv_out()` returns the
helper's suggested recording path and `recorded_argv()` reads it.

Invocation dispatch is stateful and one-based: invocation 1 executes fixture
line 1, invocation 2 executes line 2, and so on. Once all lines have been used,
every later invocation replays the final line. This supports tests that invoke
Ralph repeatedly or launch merge children.

## Worked example

`headless_stream.jsonl` models an event stream split by a synchronization
point: three events are appended before the fake announces readiness, the test
creates the release file, then the completion event and summary are emitted.
Its complete fixture is one JSONL line:

```json
{"steps":[{"events":["{\"type\":\"iteration.banner\",\"runId\":\"run-stream\",\"iteration\":1,\"maxIterations\":2,\"allowedRoles\":[\"planner\"]}","{\"type\":\"iteration.start\",\"runId\":\"run-stream\",\"iteration\":1,\"maxIterations\":2}","{\"type\":\"progress\",\"runId\":\"run-stream\",\"iteration\":1,\"allowedRoles\":[\"planner\"],\"emittedTopic\":\"plan.ready\",\"outcome\":\"continue:routed_event\"}"]},{"barrier":{"ready_env":"STREAM_READY","release_env":"STREAM_RELEASE"}},{"events":["{\"type\":\"loop.finish\",\"runId\":\"run-stream\",\"iterations\":1,\"stopReason\":\"completed\",\"costUsd\":0.01}"]},{"summary":{"run_id":"run-stream","iterations":1,"stop_reason":"completed","cost_usd":0.01,"journal":"${AUTOLOOP_STATE_DIR}/journal.jsonl","memory":"${AUTOLOOP_STATE_DIR}/memory.jsonl"}}]}
```

A test builds and runs it by prefixing the returned bin directory onto `PATH`:

```rust
let fake = build_fake_autoloop(&fake_dir, &fixture_path)?;
command.env("PATH", format!("{}:{old_path}", fake.bin_dir().display()));
command.env("ARGV_OUT", fake.argv_out());
```

See the fixtures in this directory for normal completion, conditional crash,
merge lifecycle, and full headless-stream examples.

## Capturing a real journal

For a loop launched by Ralph, copy the Ralph-owned journal from the workspace:

```sh
cp .ralph/autoloop/journal.jsonl path/to/captured.journal.jsonl
```

`RALPH_DIAGNOSTICS` is not required; Ralph configures Autoloop to write
`.ralph/autoloop/journal.jsonl` during a normal live run. Standalone Autoloop
uses its own `.autoloop/journal.jsonl` default unless configured otherwise; the
fake helper's fallback above intentionally models that standalone default.
Preserve the file as JSONL. To use its records in a fake-autoloop invocation,
JSON-encode each captured line as one string in a `journal` step's array.

The existing raw-journal fixture at
`crates/ralph-adapters/tests/fixtures/autoloop/max_iterations.journal.jsonl`
is the repository precedent for captured journal fixtures.
