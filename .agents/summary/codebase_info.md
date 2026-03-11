# Codebase Information

## Project

- **Name**: Ralph Orchestrator
- **Version**: 2.8.0
- **License**: MIT
- **Repository**: https://github.com/mikeyobrien/ralph-orchestrator
- **Rust Edition**: 2024
- **Node.js**: >= 22.0.0

## Description

A hat-based orchestration framework that keeps AI agents in a loop until the task is done. Implements the [Ralph Wiggum technique](https://ghuntley.com/ralph/) — autonomous task completion through continuous iteration with fresh context per cycle.

## Languages & Stack

| Layer | Technology |
|-------|-----------|
| Core orchestration | Rust (9 crates in workspace) |
| Web backend (primary) | Rust (axum, ralph-api crate) |
| Web backend (legacy) | TypeScript (Fastify + tRPC + SQLite) |
| Web frontend | TypeScript (React + Vite + TailwindCSS) |
| Terminal UI | Rust (ratatui + crossterm) |
| CI/CD | GitHub Actions (ci.yml, lint.yml, release.yml, docs.yml) |
| Documentation site | MkDocs |
| Task runner | Just (Justfile) |
| Toolchain management | mise (mise.toml) |
| Package distribution | cargo-dist (npm + shell installers) |

## Workspace Crates

| Crate | Purpose |
|-------|---------|
| `ralph-proto` | Shared types: Event, EventBus, Hat, HatId, Topic, JSON-RPC, UX events |
| `ralph-core` | Orchestration engine: event loop, hats, memories, tasks, hooks, skills, config, worktrees |
| `ralph-adapters` | Backend integrations: Claude, Kiro, Gemini, Codex, Amp, Pi, custom CLI, PTY executor |
| `ralph-cli` | Binary entry point (`ralph`): all CLI commands (run, plan, task, loops, web, mcp, init, etc.) |
| `ralph-tui` | Terminal UI: ratatui-based observation dashboard with iteration navigation |
| `ralph-telegram` | Telegram bot: human-in-the-loop via questions, guidance, and commands |
| `ralph-api` | Rust-native RPC v1 API + MCP server: task/loop/planning/collection/config/stream domains |
| `ralph-e2e` | End-to-end test framework with mock and live modes |
| `ralph-bench` | Benchmarking harness |

## Web Layer

| Component | Location | Stack |
|-----------|----------|-------|
| Frontend | `frontend/ralph-web/` | React, Vite, TailwindCSS, shadcn/ui components |
| Legacy backend | `backend/ralph-web-server/` | Fastify, tRPC, SQLite, TypeScript |
| Primary API | `crates/ralph-api/` | Axum, JSON-RPC, WebSocket streams |

## Supported Agent Backends

Claude Code, Kiro, Gemini CLI, Codex, Amp, Copilot CLI, OpenCode, Pi, and custom commands. Auto-detection available via `agent: auto`.

## Distribution

- npm: `@ralph-orchestrator/ralph-cli`
- Homebrew: `ralph-orchestrator`
- Cargo: `ralph-cli`
- Targets: aarch64-apple-darwin, aarch64-unknown-linux-gnu, x86_64-apple-darwin, x86_64-unknown-linux-gnu
