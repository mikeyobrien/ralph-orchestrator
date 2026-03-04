# Ralph Orchestrator — Documentation Index

> **For AI Assistants**: This file is the primary entry point for understanding the Ralph Orchestrator codebase. Read this file first, then consult specific files as needed based on the summaries below.

## How to Use This Documentation

1. **Start here** — this index provides enough context to answer most questions
2. **Drill down** — each file below covers a specific aspect in depth
3. **Cross-reference** — files reference each other where topics overlap

## Documentation Files

| File | Purpose | When to Consult |
|------|---------|-----------------|
| [`codebase_info.md`](.agents/summary/codebase_info.md) | Project identity, tech stack, workspace structure, platforms | Understanding project basics, technology choices, file organization |
| [`architecture.md`](.agents/summary/architecture.md) | System architecture, design patterns, core principles | Understanding how the system works, why design decisions were made |
| [`components.md`](.agents/summary/components.md) | Detailed breakdown of every crate, module, and web package | Finding specific code, understanding module responsibilities |
| [`interfaces.md`](.agents/summary/interfaces.md) | Rust traits, JSON-RPC protocol, CLI commands, REST API, EventBus | Understanding APIs, integration points, communication protocols |
| [`data_models.md`](.agents/summary/data_models.md) | All data structures, configuration types, file formats | Understanding data flow, configuration options, state management |
| [`workflows.md`](.agents/summary/workflows.md) | End-to-end workflows with Mermaid diagrams | Understanding execution flows, debugging, process tracing |
| [`dependencies.md`](.agents/summary/dependencies.md) | External dependencies, internal crate graph, build tools | Understanding dependency choices, version constraints |
| [`review_notes.md`](.agents/summary/review_notes.md) | Documentation consistency and completeness review | Identifying gaps, planning documentation improvements |

## Quick Reference

### Project Structure
- **9 Rust crates** in `crates/` (proto → core → adapters → cli/tui/telegram/api/e2e/bench)
- **1 Node.js backend** in `backend/ralph-web-server/` (Fastify + tRPC + SQLite)
- **1 React frontend** in `frontend/ralph-web/` (Vite + TailwindCSS)
- **Hat presets** in `presets/` (YAML configuration files)

### Key Concepts
- **Event Loop**: Core orchestration via pub/sub EventBus with topic-based routing
- **Hats**: Agent personas (Planner, Builder, custom) with subscriptions and publications
- **HatlessRalph**: Constant coordinator that builds prompts and delegates
- **Memories**: Persistent learning stored in markdown (`.ralph/agent/memories.md`)
- **Tasks**: Runtime work tracking in JSONL (`.ralph/agent/tasks.jsonl`)
- **Hooks**: Lifecycle event handlers (pre/post loop, iteration, completion)
- **Parallel Loops**: Concurrent execution via git worktrees with merge queue
- **RObot**: Human-in-the-loop via Telegram (blocking questions + proactive guidance)

### Backends Supported
Claude, Kiro, Gemini, Codex, Amp, Pi, and custom commands.

### Execution Modes
1. **Subprocess TUI** (default): Two-process model with ratatui frontend
2. **Autonomous** (`--no-tui`): Headless CLI execution
3. **RPC** (`--rpc`): JSON-lines protocol for IDE integration
4. **Web Dashboard**: React UI via ralph-api server
5. **Telegram Daemon**: Persistent bot for remote interaction

### Configuration
- Core config: `ralph.yml` (supports v1 flat and v2 nested formats)
- Hat collections: `presets/*.yml` or custom YAML via `-H` flag
- Split model: core config + hat collection merged at runtime
