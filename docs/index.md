# Ralph Orchestrator

<div align="center" markdown>

**Hat-based orchestration framework that keeps AI agents in a loop until the task is done.**

[![License](https://img.shields.io/badge/license-MIT-blue)](https://github.com/mikeyobrien/ralph-orchestrator/blob/main/LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.75+-orange)](https://www.rust-lang.org/)
[![Build](https://img.shields.io/github/actions/workflow/status/mikeyobrien/ralph-orchestrator/ci.yml?branch=main&label=CI)](https://github.com/mikeyobrien/ralph-orchestrator/actions)

> "Me fail English? That's unpossible!" - Ralph Wiggum

</div>

---

## What is Ralph?

Ralph implements the [Ralph Wiggum technique](https://ghuntley.com/ralph/) — autonomous task completion through continuous iteration. Give Ralph a task, and it will keep working until it's done.

> "The orchestrator is a thin coordination layer, not a platform. Ralph is smart; let Ralph do the work."

### Two Modes of Operation

| Mode | Description | Best For |
|------|-------------|----------|
| **Traditional** | Simple loop — Ralph iterates until done | Quick tasks, simple automation |
| **Hat-Based** | Specialized personas coordinate through events | Complex workflows, multi-step processes |

## Key Features

<div class="grid cards" markdown>

-   :material-robot: **Multi-Backend Support**

    Works with Claude Code, Kiro, Gemini CLI, Codex, Amp, Copilot CLI, and OpenCode

-   :material-hat-fedora: **Hat System**

    Specialized Ralph personas with distinct behaviors coordinating through typed events

-   :material-shield-check: **Backpressure Enforcement**

    Gates that reject incomplete work — tests, lint, typecheck must pass

-   :material-brain: **Memories & Tasks**

    Persistent learning across sessions and runtime work tracking

-   :material-monitor: **Interactive TUI**

    Real-time terminal UI for monitoring Ralph's activity

-   :material-cog: **31 Presets**

    A small set of supported built-in workflows plus a larger catalog of documented examples

</div>

## Quick Example

```bash
# Initialize with traditional mode
ralph init --backend claude

# Create a task
cat > PROMPT.md << 'EOF'
Build a REST API with these endpoints:
- POST /users - Create user
- GET /users/:id - Get user by ID
- PUT /users/:id - Update user

Use Express.js with TypeScript.
EOF

# Run Ralph
ralph run
```

Ralph iterates until it outputs `LOOP_COMPLETE` or hits the iteration limit.

## The Ralph Tenets

1. **Fresh Context Is Reliability** — Each iteration clears context. Re-read specs, plan, code every cycle.
2. **Backpressure Over Prescription** — Don't prescribe how; create gates that reject bad work.
3. **The Plan Is Disposable** — Regeneration costs one planning loop. Cheap.
4. **Disk Is State, Git Is Memory** — Files are the handoff mechanism.
5. **Steer With Signals, Not Scripts** — Add signs, not scripts.
6. **Let Ralph Ralph** — Sit *on* the loop, not *in* it.

## Getting Started

<div class="grid cards" markdown>

-   :material-download: **[Installation](getting-started/installation.md)**

    Install Ralph via npm, Homebrew, or Cargo

-   :material-rocket-launch: **[Quick Start](getting-started/quick-start.md)**

    Get up and running in 5 minutes

-   :material-book-open: **[Concepts](concepts/index.md)**

    Understand hats, events, memories, and backpressure

-   :material-cog: **[Configuration](guide/configuration.md)**

    Configure Ralph for your workflow

</div>

## Architecture

Ralph is organized as a Cargo workspace with seven crates:

| Crate | Purpose |
|-------|---------|
| `ralph-proto` | Protocol types: Event, Hat, Topic |
| `ralph-core` | Business logic: EventLoop, Config |
| `ralph-adapters` | CLI backend integrations |
| `ralph-tui` | Terminal UI with ratatui |
| `ralph-cli` | Binary entry point |
| `ralph-e2e` | End-to-end testing |
| `ralph-bench` | Benchmarking |

## Community

- [GitHub Issues](https://github.com/mikeyobrien/ralph-orchestrator/issues) — Report bugs and request features
- [GitHub Discussions](https://github.com/mikeyobrien/ralph-orchestrator/discussions) — Ask questions and share ideas
- [Contributing Guide](contributing/index.md) — Help improve Ralph

## License

Ralph Orchestrator is open source software licensed under the [MIT License](https://github.com/mikeyobrien/ralph-orchestrator/blob/main/LICENSE).

---

<div align="center" markdown>

*"I'm learnding!" - Ralph Wiggum*

</div>
