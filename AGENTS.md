# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

> The orchestrator is a thin coordination layer, not a platform. Agents are smart; let them do the work.

## Build & Test

```bash
cargo build
cargo test
cargo test -p ralph-cli -p ralph-core        # Focused CLI/core gate
cargo test -p ralph-core test_core_config_defaults  # Run one named test
```

**IMPORTANT**: Run `cargo test` before declaring any task done.

### Web Dashboard

```bash
ralph web                                    # Launch Rust API (:3000) + frontend (:5173)
npm install                                  # Install all dependencies
npm run dev                                  # Frontend only (default)
npm run dev:api                              # Rust RPC API only
npm run dev:web                              # Frontend only (explicit)
npm run test:server                          # Deprecated Node backend tests
```

## Architecture

**autoloop is the engine; Ralph is the TUI frontend and observation/coordination
plane.** Ralph never re-implements engine judgment. It observes autoloop's
contracts (journal, `--events`, and summary) and coordinates around them through
the merge queue, worktrees, loop registry, TUI, and—once implemented—HITL relay.

```
ralph-cli      → CLI/TUI frontend, autoloop launch, completion and merge coordination
ralph-core     → Shared config and coordination state (memories, tasks, registry, merge queue)
ralph-adapters → Autoloop process integration and journal/event/summary contracts
ralph-telegram → Telegram relay components (inactive with the autoloop engine; see RObot below)
ralph-tui      → Terminal UI (ratatui-based)
ralph-e2e      → End-to-end test framework
ralph-proto    → Protocol definitions
ralph-bench    → Benchmarking

backend/       → Web server (@ralph-web/server) - Fastify + tRPC + SQLite
frontend/      → Web dashboard (@ralph-web/dashboard) - React + Vite + TailwindCSS
```

### Key Files

| File | Purpose |
|------|---------|
| `.ralph/agent/memories.md` | Persistent learning across sessions |
| `.ralph/agent/tasks.jsonl` | Runtime work tracking |
| `.ralph/loop.lock` | Contains PID + prompt of primary loop |
| `.ralph/loops.json` | Registry of all tracked loops |
| `.ralph/merge-queue.jsonl` | Event-sourced merge queue |
| `.ralph/telegram-state.json` | Telegram bot state (chat ID, pending questions) |

### Code Locations

- **Autoloop engine driver**: `crates/ralph-cli/src/autoloop_engine.rs`
- **Autoloop preset generation**: `crates/ralph-cli/src/autoloop_preset_gen.rs`
- **Completion coordination**: `crates/ralph-cli/src/completion_coord.rs`
- **Merge processing**: `crates/ralph-cli/src/merge_processing.rs`
- **Autoloop adapters**: `crates/ralph-adapters/src/autoloop_runner.rs`, `autoloop_events.rs`, `autoloop_journal.rs`, `autoloop_event_tailer.rs`
- **Autoloop dependency health**: `crates/ralph-core/src/autoloop_health.rs`
- **Memory system**: `crates/ralph-core/src/memory.rs`, `memory_store.rs`
- **Task system**: `crates/ralph-core/src/task.rs`, `task_store.rs`
- **Lock coordination**: `crates/ralph-core/src/worktree.rs`
- **Loop registry**: `crates/ralph-core/src/loop_registry.rs`
- **Merge queue**: `crates/ralph-core/src/merge_queue.rs`
- **CLI commands**: `crates/ralph-cli/src/loops.rs`, `task_cli.rs`
- **Telegram integration**: `crates/ralph-telegram/src/` (bot, service, state, handler)
- **RObot config**: `crates/ralph-core/src/config.rs` (`RobotConfig`, `TelegramBotConfig`)
- **Web server**: `backend/ralph-web-server/src/` (tRPC routes in `api/`, runners in `runner/`)
- **Web dashboard**: `frontend/ralph-web/src/` (React components in `components/`)

## The Ralph Tenets

1. **Fresh Context Is Reliability** — Each iteration clears context. Re-read specs, plan, code every cycle. Optimize for the "smart zone" (40-60% of ~176K usable tokens).

2. **Backpressure Over Prescription** — Don't prescribe how; create gates that reject bad work. Tests, typechecks, builds, lints. For subjective criteria, use LLM-as-judge with binary pass/fail.

3. **The Plan Is Disposable** — Regeneration costs one planning loop. Cheap. Never fight to save a plan.

4. **Disk Is State, Git Is Memory** — Memories and Tasks are the handoff mechanisms. No sophisticated coordination needed.

5. **Steer With Signals, Not Scripts** — The codebase is the instruction manual. When Ralph fails a specific way, add a sign for next time.

6. **Let Ralph Ralph** — Sit *on* the loop, not *in* it. Tune like a guitar, don't conduct like an orchestra.

## Anti-Patterns

- ❌ Building features into the orchestrator that agents can handle
- ❌ Complex retry logic (fresh context handles recovery)
- ❌ Detailed step-by-step instructions (use backpressure instead)
- ❌ Scoping work at task selection time (scope at plan creation instead)
- ❌ Assuming functionality is missing without code verification

## Specs & Tasks

- Create specs in `.ralph/specs/` — do NOT implement without an approved spec first
- Create code tasks in `.ralph/tasks/` using `.code-task.md` extension
- Work step-by-step: spec → dogfood spec → implement → dogfood implementation → done

### Memories, Tasks, and Scratchpad

Memories, Ralph tasks, and `core.scratchpad` are enabled by default and are
independently configurable. Under the v3 engine, autoloop owns completion
judgment using its canonical task store. Ralph may inspect its own task store
for coordination and warnings, but those tasks do not participate in the
engine's completion gate because the two task formats are incompatible.

## Parallel Loops

Ralph supports multiple orchestration loops in parallel using git worktrees.

```
Primary Loop (holds .ralph/loop.lock)
├── Runs in main workspace
├── Processes merge queue on completion
└── Spawns merge-ralph for queued loops

Worktree Loops (.worktrees/<loop-id>/)
├── Isolated filesystem via git worktree
├── Symlinked memories, specs, tasks → main repo
├── Queue for merge on completion
└── Exit cleanly (no spawn)
```

### Testing Parallel Loops

```bash
cd $(mktemp -d) && git init && echo "<p>Hello</p>" > index.html && git add . && git commit -m "init"

# Terminal 1: Primary loop
ralph run -p "Add header before <p>" --max-iterations 5

# Terminal 2: Worktree loop
ralph run -p "Add footer after </p>" --max-iterations 5

# Monitor
ralph loops
```

## Autoloop Role Concurrency

Hat concurrency and aggregation are translated into the generated autoloop topology:

```yaml
hats:
  reviewer:
    name: "Reviewer"
    triggers: ["review.file"]
    publishes: ["review.done"]
    concurrency: 4              # Declarative parallel branches
    instructions: "..."

  synthesizer:
    triggers: ["review.done"]
    publishes: ["review.complete"]
    aggregate:                   # Wait for all branch results
      mode: wait_for_all
      timeout: 300               # Seconds
```

- `concurrency > 1` maps to autoloop per-role `concurrency`; routing one event to the role
  launches that many declarative branches, each prefixed with `[branch i/N]`.
- `aggregate` maps to autoloop role aggregation; Ralph's timeout seconds are converted to
  `timeout_ms` in the generated topology.
- A hat cannot have both `concurrency > 1` and `aggregate`.
- Agents publish normal handoff events through the live autoloop harness event tool; autoloop
  owns parallel dispatch and result aggregation.

### Presets

- `presets/wave-review.yml` — Declarative autoloop-concurrency scatter-gather review

## Legacy E2E Scenario Inventory

The legacy cassette E2E scenarios target the deleted in-house loop and are not
a v3 GA gate. Their inventory remains available while replacement coverage is
tracked in the v3 GA-readiness R-matrix:

```bash
cargo run -p ralph-e2e -- --list
```

## RObot (Human-in-the-Loop)

**Telegram HITL is inactive under the autoloop engine pending autoloop#345.**
The `ralph-telegram` crate, `ralph bot` command, and configuration surface
remain in the tree for when the engine relay lands, but an autoloop-backed
`ralph run` does not currently relay agent questions or human guidance.

```yaml
# Reserved configuration shape in ralph.yml
RObot:
  enabled: true
  timeout_seconds: 300
  telegram:
    bot_token: "your-token"  # Or RALPH_TELEGRAM_BOT_TOKEN
```

See `crates/ralph-telegram/README.md` for the retained bot setup.

## Diagnostics

TUI mode always logs to `.ralph/diagnostics/logs/ralph-{timestamp}.log` (last 5 kept automatically).

```bash
RALPH_DIAGNOSTICS=1 ralph run -p "your prompt"
```

Output in `.ralph/diagnostics/<timestamp>/`:
- `agent-output.jsonl` — Agent text, tool calls, results
- `orchestration.jsonl` — Hat selection, events, backpressure
- `errors.jsonl` — Parse errors, validation failures

```bash
jq 'select(.type == "tool_call")' .ralph/diagnostics/*/agent-output.jsonl
ralph clean --diagnostics
```

## IMPORTANT

- Run `cargo test` before declaring any task done
- Backwards compatibility doesn't matter — it adds clutter for no reason
- Use current Rust integration gates; do not treat legacy cassette E2E as a v3 gate
- BDD/Cucumber tests MUST exercise real runtime code paths via integration tests (not placeholder/source-only assertions)
- Run python tests using a .venv
- You MUST not commit ephemeral files
- When I ask you to view something that means to use playwright/chrome tools to go view it.
- When adding or changing `ralph tools` subcommands, update the appropriate file in `crates/ralph-core/data/`: `ralph-tools.md` (shared commands), `ralph-tools-tasks.md` (task commands), or `ralph-tools-memories.md` (memory commands). `.claude/skills/ralph-tools/SKILL.md` is a symlink to the base `ralph-tools.md`
- Design docs and specs go in `.ralph/specs` and one-off code tasks and bug fixes go in `.ralph/tasks`
