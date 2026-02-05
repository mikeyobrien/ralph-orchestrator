# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

> The orchestrator is a thin coordination layer, not a platform. Agents are smart; let them do the work.

## Build & Test

```bash
cargo build
cargo test
cargo test -p hats-core test_name           # Run single test
cargo test -p hats-core smoke_runner        # Smoke tests (replay-based)
cargo run -p hats-e2e -- --mock             # E2E tests (CI-safe)
./scripts/setup-hooks.sh                     # Install pre-commit hooks (once)
```

**IMPORTANT**: Run `cargo test` before declaring any task done. Smoke test after code changes.

### Web Dashboard

```bash
hats web                                    # Launch both servers (backend:3000, frontend:5173)
npm install                                  # Install all dependencies
npm run dev                                  # Dev mode (both)
npm run dev:server                           # Backend only
npm run dev:web                              # Frontend only
npm run test:server                          # Backend tests
```

## Architecture

```
hats-cli      → CLI entry point, commands (run, plan, task, loops, web)
hats-core     → Orchestration logic, event loop, hats, memories, tasks
hats-adapters → Backend integrations (Claude, Kiro, Gemini, Codex, etc.)
hats-telegram → Telegram bot for human-in-the-loop communication
hats-tui      → Terminal UI (ratatui-based)
hats-e2e      → End-to-end test framework
hats-proto    → Protocol definitions
hats-bench    → Benchmarking

backend/       → Web server (@hats-web/server) - Fastify + tRPC + SQLite
frontend/      → Web dashboard (@hats-web/dashboard) - React + Vite + TailwindCSS
```

### Key Files

| File | Purpose |
|------|---------|
| `.hats/agent/memories.md` | Persistent learning across sessions |
| `.hats/agent/tasks.jsonl` | Runtime work tracking |
| `.hats/loop.lock` | Contains PID + prompt of primary loop |
| `.hats/loops.json` | Registry of all tracked loops |
| `.hats/merge-queue.jsonl` | Event-sourced merge queue |
| `.hats/telegram-state.json` | Telegram bot state (chat ID, pending questions) |

### Code Locations

- **Event loop**: `crates/hats-core/src/event_loop/mod.rs`
- **Hat system**: `crates/hats-core/src/hatless.rs`
- **Memory system**: `crates/hats-core/src/memory.rs`, `memory_store.rs`
- **Task system**: `crates/hats-core/src/task.rs`, `task_store.rs`
- **Lock coordination**: `crates/hats-core/src/worktree.rs`
- **Loop registry**: `crates/hats-core/src/loop_registry.rs`
- **Merge queue**: `crates/hats-core/src/merge_queue.rs`
- **CLI commands**: `crates/hats-cli/src/loops.rs`, `task_cli.rs`
- **Telegram integration**: `crates/hats-telegram/src/` (bot, service, state, handler)
- **RObot config**: `crates/hats-core/src/config.rs` (`RobotConfig`, `TelegramBotConfig`)
- **Web server**: `backend/hats-web-server/src/` (tRPC routes in `api/`, runners in `runner/`)
- **Web dashboard**: `frontend/hats-web/src/` (React components in `components/`)

## The Hats Tenets

1. **Fresh Context Is Reliability** — Each iteration clears context. Re-read specs, plan, code every cycle. Optimize for the "smart zone" (40-60% of ~176K usable tokens).

2. **Backpressure Over Prescription** — Don't prescribe how; create gates that reject bad work. Tests, typechecks, builds, lints. For subjective criteria, use LLM-as-judge with binary pass/fail.

3. **The Plan Is Disposable** — Regeneration costs one planning loop. Cheap. Never fight to save a plan.

4. **Disk Is State, Git Is Memory** — Memories and Tasks are the handoff mechanisms. No sophisticated coordination needed.

5. **Steer With Signals, Not Scripts** — The codebase is the instruction manual. When Hats fails a specific way, add a sign for next time.

6. **Let Hats Hats** — Sit *on* the loop, not *in* it. Tune like a guitar, don't conduct like an orchestra.

## Anti-Patterns

- ❌ Building features into the orchestrator that agents can handle
- ❌ Complex retry logic (fresh context handles recovery)
- ❌ Detailed step-by-step instructions (use backpressure instead)
- ❌ Scoping work at task selection time (scope at plan creation instead)
- ❌ Assuming functionality is missing without code verification

## Specs & Tasks

- Create specs in `.hats/specs/` — do NOT implement without an approved spec first
- Create code tasks in `.hats/tasks/` using `.code-task.md` extension
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

Hats supports multiple orchestration loops in parallel using git worktrees.

```
Primary Loop (holds .hats/loop.lock)
├── Runs in main workspace
├── Processes merge queue on completion
└── Spawns merge-hats for queued loops

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
hats run -p "Add header before <p>" --max-iterations 5

# Terminal 2: Worktree loop
hats run -p "Add footer after </p>" --max-iterations 5

# Monitor
hats loops
```

## Smoke Tests (Replay-Based)

Smoke tests use recorded JSONL fixtures instead of live API calls:

```bash
cargo test -p hats-core smoke_runner        # All smoke tests
cargo test -p hats-core kiro                # Kiro-specific
```

**Fixtures location:** `crates/hats-core/tests/fixtures/`

### Recording New Fixtures

```bash
cargo run --bin hats -- run -c hats.claude.yml --record-session session.jsonl -p "your prompt"
```

## E2E Testing

```bash
cargo run -p hats-e2e -- claude             # Live API tests
cargo run -p hats-e2e -- --mock             # CI-safe mock mode
cargo run -p hats-e2e -- --mock --filter connect  # Filter scenarios
cargo run -p hats-e2e -- --list             # List scenarios
```

Reports generated in `.e2e-tests/`.

## RObot (Human-in-the-Loop)

Hats supports human interaction during orchestration via Telegram. Agents can ask questions and humans can send proactive guidance.

### Configuration

```yaml
# hats.yml
RObot:
  enabled: true
  timeout_seconds: 300    # How long to block waiting for a response
  telegram:
    bot_token: "your-token"  # Or set HATS_TELEGRAM_BOT_TOKEN env var
```

### Event Types

| Event / Command | Direction | Purpose |
|-------|-----------|---------|
| `human.interact` | Agent to Human | Agent asks a question; loop blocks until response or timeout |
| `human.response` | Human to Agent | Reply to a `human.interact` question |
| `human.guidance` | Human to Agent | Proactive guidance injected as `## ROBOT GUIDANCE` in prompt |
| `hats tools interact progress` | Agent to Human | Non-blocking progress notification via Telegram (no event, direct send) |

### How It Works

- The Telegram bot starts only on the **primary loop** (the one holding `.hats/loop.lock`)
- When an agent emits `human.interact`, the event loop sends the question via Telegram and **blocks**
- Responses are published as `human.response` events on the bus
- Proactive messages become `human.guidance` events, squashed into a numbered list in the prompt
- Send failures retry with exponential backoff (3 attempts); if all fail, treated as timeout
- Parallel loops route messages via reply-to, `@loop-id` prefix, or default to primary

See `crates/hats-telegram/README.md` for setup instructions.

## Diagnostics

TUI mode always logs to `.hats/diagnostics/logs/hats-{timestamp}.log` (last 5 kept automatically).

```bash
HATS_DIAGNOSTICS=1 hats run -p "your prompt"
```

Output in `.hats/diagnostics/<timestamp>/`:
- `agent-output.jsonl` — Agent text, tool calls, results
- `orchestration.jsonl` — Hat selection, events, backpressure
- `errors.jsonl` — Parse errors, validation failures

```bash
jq 'select(.type == "tool_call")' .hats/diagnostics/*/agent-output.jsonl
hats clean --diagnostics
```

## IMPORTANT

- Run `cargo test` before declaring any task done
- Backwards compatibility doesn't matter — it adds clutter for no reason
- Prefer replay-based smoke tests over live API calls for CI
- Run python tests using a .venv
- You MUST not commit ephemeral files
- When I ask you to view something that means to use playwright/chrome tools to go view it.
- When adding or changing `hats tools` subcommands, update `crates/hats-core/data/hats-tools.md` — this is the single source of truth for the hats-tools skill (`.claude/skills/hats-tools/SKILL.md` is a symlink to it)
- Design docs and specs go in `.hats/specs` and one-off code tasks and bug fixes go in `.hats/tasks`
