# Migrating to Ralph 3.0 (the autoloop engine)

Ralph 3.0 replaces the in-house orchestration engine with the
[autoloop](https://github.com/mikeyobrien/autoloop) runtime, spawned as a
subprocess. Ralph is now the TUI frontend and observation/coordination
plane — merge queue, worktree loops, registry, doctor — while autoloop owns
loop execution, role dispatch, completion judgment, and budgets.

## New requirement: the autoloop engine

Ralph 3.0 requires `autoloop >= 0.10.0`. The recommended npm installation
pulls it in automatically. Cargo, GitHub Releases installer, and prebuilt-binary
users can just run `ralph run`: first-run provisioning offers to download the
SHA256-verified standalone engine executable into `~/.ralph/engine/`
interactively. For CI and other non-interactive environments, opt in with
`RALPH_AUTO_INSTALL_ENGINE=1 ralph run -p "your task"`.

To provision the standalone engine manually, run
`ralph doctor --install-engine`. No Node runtime is needed. `ralph doctor`
shows which engine resolution is active; a declined or non-interactive run
without opt-in fails fast with install guidance. The global engine executable
location (controlled by `RALPH_ENGINE_DIR`) is distinct from per-project
runtime state: Ralph launches autoloop with a Ralph-owned state root at
`<workspace>/.ralph/autoloop`.

## What breaks

| v2 | v3 |
|----|----|
| `ralph wave …` (wave system) | Removed; the command now exits with a migration message. Use hat `concurrency:`/`aggregate:`. Ralph turns on autoloop's `parallel.enabled`, sets `parallel.max_branches` to the largest `concurrency`, maps a concurrent hat's `timeout` to `parallel.branch_timeout_ms`, and moves an aggregator hat's `aggregate` onto the concurrent hat that feeds it. `presets/wave-review.yml` is ported and verified live. The TUI wave drill-down (`w`, `[WAVE]`) is gone because autoloop's `--events` stream has no per-branch output; branch records live in the journal (`wave.*` topics). |
| Lifecycle hooks (`hooks.events`) | Only `post.loop.complete` and `post.loop.error` fire. They ride the engine's finish notification: Ralph writes `notify.command` (a hidden `ralph hooks notify`), `notify.on`, and `notify.timeout_ms` into the generated preset, and the engine journals `notify.sent` / `notify.failed`. Hooks still receive Ralph's payload (`loop`, `iteration`, `context`) with `context.termination_reason` set to the engine stop reason, and without the engine's `AUTOLOOP_*` variables. `post.loop.error` fires on any run that did not complete (engine classes `failed` and `stopped`). Every other event, `mutate.enabled`, and `on_error: block\|suspend` refuse to start and name the hook. With an explicit `core.autoloop_preset`, Ralph cannot edit the preset and prints the exact `notify.*` lines to add. |
| (new) Jev workflow routing | `core.routing.jev` (`enabled`, `routes_file`, `model`, `min_confidence`, `timeout_ms`) becomes the generated preset's `[routing.jev]`; `routes_file` resolves against the workspace. An explicit `core.autoloop_preset` owns its own `[routing.jev]` and is never rewritten, so setting `core.routing.jev` with it refuses to start. A `-H` preset that enables routing refuses too, because a hats overlay cannot carry it. Routing needs autoloop >= 0.12.0; on an older engine, which would silently ignore the block, Ralph refuses to start. `TYPESAFE_API_KEY` belongs in the environment only, never in TOML, a route catalog, or an argument. `ralph doctor` checks the key, the catalog, the settings, and the engine version. Example: `examples/jev-routing/`. |
| (new) Jev completion judge | `core.completion.jev` (`enabled`, `model`, `threshold`, `timeout_ms`) registers `ralph gate jev-judge` in the generated preset's `acceptance.verify_cmds`, so the engine holds every done-claim until Jev returns verdict `approved` with `completion_verified` at or above the threshold (default 0.8). A missing key or provider failure holds completion (`marker_fallback` provenance). The judge also turns off the metareview (`review.enabled = false`), because its `EXIT` verdict completes a run without the acceptance gate. Needs autoloop >= 0.11.0. |
| (new) Jev topology routing | `core.routing.topology.jev` makes Jev choose the next hat after every `step.done`, through a `pre_emit` hook (`ralph gate jev-route`) that rewrites the event to `route.<hat>` before the engine routes it; the journal records the decision and the hat that ran. Hats in this mode must not declare `triggers`, `publishes`, `default_publishes`, `concurrency`, or `aggregate` (refuses to start, naming the hat and field). Routing fails closed with no fallback to hats. Needs autoloop >= 0.11.0. See the configuration guide for the topology, chain, and dynamic-chain layers. |
| `ralph run --rpc` (JSON-lines protocol) | Restored. Autoloop `--events` is mapped onto the existing `RpcEvent` contract on stdout. |
| `ralph run --record-session` (smoke fixtures) | Removed. Replay tests use the fake-autoloop fixture substrate (`tests/fixtures/autoloop/`). |
| `core.engine` config field | Autoloop is the only engine. `autoloop` remains valid; any other value is rejected because the in-house engine was removed in v3. Remove the field or set it to `autoloop`. |
| Telegram RObot HITL during runs | Wired on the primary loop: Autoloop `ask.pending` is relayed through Telegram/Web; answers and `human.guidance` use `autoloop control`. TUI still displays asks only. |
| In-house smoke corpus (`smoke_runner`) | Replaced by the fake-autoloop replay substrate and `ralph-e2e --mock`. |

## What keeps working (now via the engine)

- `ralph run -p/-P`, `--max-iterations`, `--max-runtime`, `--max-cost`:
  budgets are forwarded to the engine and enforced there.
- `event_loop.max_consecutive_failures` is **not enforced** under the autoloop
  engine. Ralph's default or configured value is retained in its config but is
  deliberately not translated to a differently behaving engine limit. Ralph
  emits a preflight warning in `ralph run` and `ralph doctor`. Autoloop 0.10.x
  was checked and has no equivalent general consecutive-backend-failure budget.
- `-b`/`cli.backend`: mapped to autoloop backend kinds (claude-sdk, pi,
  ACP, command). Unmappable backends fail fast — nothing is silently
  ignored.
- Hats: translated into a generated autoloop preset (topology, roles,
  instructions, concurrency). Explicit `core.autoloop_preset` skips
  generation; limits then live in the preset.
- Parallel worktree loops, merge queue, `ralph loops`, landing: unchanged
  surfaces, now coordinated around the engine's journal/summary contracts.
- Tasks and memories: `.ralph/current-loop-id` semantics, `--loop-id`,
  `--continue`. Completion judgment is the engine's; open ralph tasks at
  completion produce a loud warning.

## Observability

- TUI and headless runs both render the engine's `--events` stream live.
- `ralph run --rpc` emits the same `--events` stream as Ralph `RpcEvent` JSON lines.
- `ralph resume` persists Autoloop `run_id` under `.ralph/autoloop/current-run-id` and invokes `autoloop resume <run_id>`.
- Engine state: the run journal is at `.ralph/autoloop/journal.jsonl`, with
  run-scoped state under `.ralph/autoloop/runs/`.
- Ralph's coordination stores remain separate under `.ralph/agent/`, and its
  diagnostic logs remain under `.ralph/diagnostics/`.

## Config migration

Existing `ralph.yml` files work unchanged unless they reference removed
features above. To pin an explicit engine preset instead of hat
translation, set `core.autoloop_preset: /path/to/preset`.
