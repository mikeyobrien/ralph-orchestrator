# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

> The orchestrator is a thin coordination layer, not a platform. Agents are smart; let them do the work.

## Build & Test

```bash
cargo build
cargo test
cargo test -p ralph-core test_name           # Run single test
cargo test -p ralph-core smoke_runner        # Smoke tests (replay-based)
cargo run -p ralph-e2e -- --mock             # E2E tests (CI-safe)
./scripts/setup-hooks.sh                     # Install pre-commit hooks (once)
```

**IMPORTANT**: Run `cargo test` before declaring any task done. Smoke test after code changes.

### Web Dashboard

```bash
ralph web                                    # Launch both servers (backend:3000, frontend:5173)
npm install                                  # Install all dependencies
npm run dev                                  # Dev mode (both)
npm run dev:server                           # Backend only
npm run dev:web                              # Frontend only
npm run test:server                          # Backend tests
```

## Architecture

```
ralph-cli      → CLI entry point, commands (run, plan, task, loops, web)
ralph-core     → Orchestration logic, event loop, hats, memories, tasks
ralph-adapters → Backend integrations (Claude, Kiro, Gemini, Codex, etc.)
ralph-telegram → Telegram bot for human-in-the-loop communication
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

- **Event loop**: `crates/ralph-core/src/event_loop/mod.rs`
- **Hat system**: `crates/ralph-core/src/hatless_ralph.rs`
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

### Memories and Tasks (Default Mode)

Memories and tasks are enabled by default. Both must be enabled/disabled together:

When enabled (default):
- Scratchpad is disabled
- Tasks replace scratchpad for completion verification
- Loop terminates when no open tasks + consecutive LOOP_COMPLETE

To disable (legacy scratchpad mode):
```yaml
memories:
  enabled: false
tasks:
  enabled: false
```

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

## Smoke Tests (Replay-Based)

Smoke tests use recorded JSONL fixtures instead of live API calls:

```bash
cargo test -p ralph-core smoke_runner        # All smoke tests
cargo test -p ralph-core kiro                # Kiro-specific
```

**Fixtures location:** `crates/ralph-core/tests/fixtures/`

### Recording New Fixtures

```bash
cargo run --bin ralph -- run -c ralph.claude.yml --record-session session.jsonl -p "your prompt"
```

## E2E Testing

```bash
cargo run -p ralph-e2e -- claude             # Live API tests
cargo run -p ralph-e2e -- --mock             # CI-safe mock mode
cargo run -p ralph-e2e -- --mock --filter connect  # Filter scenarios
cargo run -p ralph-e2e -- --list             # List scenarios
```

Reports generated in `.e2e-tests/`.

## RObot (Human-in-the-Loop)

Ralph supports human interaction during orchestration via Telegram. Agents can ask questions and humans can send proactive guidance.

### Configuration

```yaml
# ralph.yml
RObot:
  enabled: true
  timeout_seconds: 300    # How long to block waiting for a response
  telegram:
    bot_token: "your-token"  # Or set RALPH_TELEGRAM_BOT_TOKEN env var
```

### Event Types

| Event / Command | Direction | Purpose |
|-------|-----------|---------|
| `human.interact` | Agent to Human | Agent asks a question; loop blocks until response or timeout |
| `human.response` | Human to Agent | Reply to a `human.interact` question |
| `human.guidance` | Human to Agent | Proactive guidance injected as `## ROBOT GUIDANCE` in prompt |
| `ralph tools interact progress` | Agent to Human | Non-blocking progress notification via Telegram (no event, direct send) |

### How It Works

- The Telegram bot starts only on the **primary loop** (the one holding `.ralph/loop.lock`)
- When an agent emits `human.interact`, the event loop sends the question via Telegram and **blocks**
- Responses are published as `human.response` events on the bus
- Proactive messages become `human.guidance` events, squashed into a numbered list in the prompt
- Send failures retry with exponential backoff (3 attempts); if all fail, treated as timeout
- Parallel loops route messages via reply-to, `@loop-id` prefix, or default to primary

See `crates/ralph-telegram/README.md` for setup instructions.

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
- Prefer replay-based smoke tests over live API calls for CI
- Run python tests using a .venv
- You MUST not commit ephemeral files
- When I ask you to view something that means to use playwright/chrome tools to go view it.
- When adding or changing `ralph tools` subcommands, update `crates/ralph-core/data/ralph-tools.md` — this is the single source of truth for the ralph-tools skill (`.claude/skills/ralph-tools/SKILL.md` is a symlink to it)
- Design docs and specs go in `.ralph/specs` and one-off code tasks and bug fixes go in `.ralph/tasks`
