# ralph-core

`ralph-core` provides shared configuration and coordination state for Ralph.
It is **not** the v3 orchestration engine: autoloop owns role dispatch,
iteration state, event routing, and completion judgment. `ralph-cli` launches
the engine, while `ralph-adapters` observe its journal, event stream, and
summary.

## Current Responsibilities

- Load and validate `ralph.yml` through `RalphConfig`
- Check the external autoloop dependency through `autoloop_health`
- Track loop identity, locks, history, and registry entries
- Coordinate worktrees and the event-sourced merge queue
- Store Ralph memories and Ralph runtime task records
- Provide diagnostics, hooks, preflight, skills, and planning support

## Important Modules

| Module | Purpose |
|--------|---------|
| `autoloop_health` | Version and availability checks for the engine dependency |
| `loop_context` | Resolve primary/worktree state paths |
| `loop_lock` | Coordinate the primary loop slot |
| `loop_registry` | Track active and completed loops |
| `merge_queue` | Persist merge coordination as JSONL events |
| `memory_store` | Read and write persistent Ralph memories |
| `task_store` | Read and write Ralph runtime tracking tasks |
| `diagnostics` | Capture observation and error diagnostics |
| `hooks` | Run configured lifecycle hooks |
| `preflight` | Evaluate preflight checks |

## Configuration

The public configuration type is `RalphConfig`:

```rust
use ralph_core::RalphConfig;

let config = RalphConfig::default();
assert_eq!(config.core.specs_dir, ".ralph/specs/");
```

Relevant top-level sections include `event_loop`, `cli`, `core`, `hats`,
`events`, `memories`, `tasks`, `features`, `hooks`, `skills`, and `RObot`.
Ralph translates supported execution settings into an autoloop preset before
launch; it does not instantiate a `ralph_core::EventLoop`.

## Runtime Tasks and Completion

`TaskStore` manages Ralph's `.ralph/agent/tasks.jsonl` records. These records
are useful for coordination and observation, but their format differs from
autoloop's canonical task store. Consequently they do not participate in the
v3 engine's completion gate. Autoloop alone decides engine completion; Ralph
may warn if its separate task store still contains open tasks afterward.

## Testing

Use the normal crate and workspace tests:

```bash
cargo test -p ralph-core
cargo test
```

The former replay smoke-test command does not select tests in the default
feature set and is not a v3 gate. The `recording` Cargo feature still exposes
low-level recorder/player utilities for library consumers, but `ralph run`
does not wire those utilities into the autoloop path.

## Related Engine Integration

The engine-facing code lives outside this crate:

- `crates/ralph-cli/src/autoloop_engine.rs`
- `crates/ralph-cli/src/autoloop_preset_gen.rs`
- `crates/ralph-adapters/src/autoloop_runner.rs`
- `crates/ralph-adapters/src/autoloop_events.rs`
- `crates/ralph-adapters/src/autoloop_journal.rs`
- `crates/ralph-adapters/src/autoloop_event_tailer.rs`
