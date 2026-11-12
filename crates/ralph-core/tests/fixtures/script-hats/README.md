# Script hat fixtures

Replay fixtures for runs that mix model hats and script hats (`run:`).

## llm_then_script_session.jsonl

A two-iteration session in `SessionRecorder` format, matching a real
`ralph run` with a `builder` model hat and a `checker` script hat:

1. `_meta.iteration` (`builder`), the model's terminal output, and the
   `build.ready` event it emitted (`file=greeting.txt`).
2. `_meta.iteration` (`checker`), the script's output, and the
   `LOOP_COMPLETE` it emitted.

`tests/script_hat_smoke.rs` splits the file at `_meta.iteration` records and
replays each iteration through a real `EventLoop`, as the loop runner does:
the model hat gets a prompt, the script hat gets `begin_script_iteration`, and
each iteration's `bus.publish` records are appended to the events file the way
`ralph emit` writes them. The config the session was recorded with is in that
test (`CONFIG`), including the `build.ready` gate.

Run it with:

```bash
cargo test -p ralph-core smoke_runner
```

The script's command is not executed during replay. Real script execution is
covered by `crates/ralph-cli/tests/integration_script_hats.rs`.
