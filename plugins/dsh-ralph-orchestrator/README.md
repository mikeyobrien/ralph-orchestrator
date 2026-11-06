# DSH Ralph Orchestrator

A DSH sidebar plugin backed by the real Ralph CLI RPC engine. Each run gets a retained Git worktree and DSH workspace. Iterations run as fresh DSH sessions; the final session is `DONE - Summary`.

## Requirements

- DSH with agent, workspace, worktree, durable domain storage, permission, shell, and connection services.
- Ralph built from this checkout with custom `prompt_mode: file` support: `cargo build -p ralph-cli`.
- Node.js 22 or newer.

The local bundle uses this checkout's `target/debug/ralph` when present; otherwise it uses `ralph` on PATH. Set plugin Config `ralphPath` to select an absolute executable. `sessionTimeoutMs` bounds each managed session (default 10 minutes).

Install this directory with DSH's plugin manager. Open **Ralph Orchestrator** in the left sidebar. Select a source workspace, prompt, configuration, task short name, budget, and permissions. Runs start from committed HEAD; uncommitted source edits stay untouched. Worktrees and branches are retained, never merged or deleted automatically.

**Design hats** generates native Ralph YAML in an isolated workspace. Review the draft, provide feedback, and revise. Only the user's **Save configuration** accepts a revision. Existing accepted configurations do not change during generation.

An interrupted or failed design, or a design that stopped before proposing a draft, can be restored from configuration YAML you already have. The Host validates the exact supplied YAML; restoring is not acceptance and does not start a designer session.

## Integration

`ralph run --rpc` owns hat routing, full prompt construction, task/memory state, events, and completion gates. A local, authenticated backend helper executes prompts in the existing DSH Host. Its scoped `ralph` tool uses each invocation's exact executable, workspace, and events file. It does not infer completion from assistant claims.

Workers use the dedicated **Ralph Worker** DSH mode; Ralph supplies each iteration's active hat instructions. **Hat Designer** is a separate read/propose-only mode and cannot accept its own drafts. Final summaries use read-only **Ralph Summary** mode. There is no duplicated mode per hat. DSH may still present Agent-local MCP schemas; role guards block their execution.

Sequential execution is supported. Per-hat backend overrides, waves/aggregate execution, and engine cost budgets are rejected until their session/metrics contracts are supported. Raw approved YAML is retained, rather than roundtripping through the HTTP graph exporter. DSH permissions apply to agent tools; accepting a draft is user-only.

A stopped, exhausted, or failed run also has a terminal summary; the `DONE` title does not claim task success. Host restart marks active work interrupted and does not replay it.

## Development checks

```sh
npm install --ignore-scripts
npm run check
npm test
DSH_RUNTIME_ROOT=/path/to/dsh node --test tests/native-mode-policy.mjs
cargo test
cargo test -p ralph-core --features recording --test smoke_runner
```

Real installed UI and same-Host iteration sessions must also be verified; syntax checks and plugin installation alone are insufficient.
