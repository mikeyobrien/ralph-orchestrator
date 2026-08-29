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
| `ralph wave …` (wave system) | Removed. Use hat `concurrency:`/`aggregate:` — they map to autoloop's declarative per-role concurrency. `presets/wave-review.yml` is ported. |
| `ralph run --rpc` (JSON-lines protocol) | Removed (tracked as #343). |
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
- Engine state: the run journal is at `.ralph/autoloop/journal.jsonl`, with
  run-scoped state under `.ralph/autoloop/runs/`.
- Ralph's coordination stores remain separate under `.ralph/agent/`, and its
  diagnostic logs remain under `.ralph/diagnostics/`.

## Config migration

Existing `ralph.yml` files work unchanged unless they reference removed
features above. To pin an explicit engine preset instead of hat
translation, set `core.autoloop_preset: /path/to/preset`.
