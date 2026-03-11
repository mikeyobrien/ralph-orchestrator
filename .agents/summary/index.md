# Ralph Orchestrator — Documentation Index

> This file is the primary entry point for AI assistants working with the Ralph Orchestrator codebase. Read this file first to understand what documentation is available and where to find detailed information.

## How to Use This Documentation

1. Start here (`index.md`) to understand the project and locate relevant files
2. For architecture questions → `architecture.md`
3. For "where is X implemented?" → `components.md`
4. For API/CLI/event details → `interfaces.md`
5. For type definitions and storage formats → `data_models.md`
6. For "how does X work end-to-end?" → `workflows.md`
7. For dependency questions → `dependencies.md`
8. For project metadata → `codebase_info.md`

## Project Summary

Ralph Orchestrator is a Rust-based framework (v2.8.0) that runs AI agents in iterative loops using a hat-based coordination system. Agents wear "hats" (specialized personas) that communicate via pub/sub events. The loop continues until the task is complete or a termination condition is met. Supports Claude, Kiro, Gemini, Codex, Amp, Pi, and custom backends.

## Documentation Files

| File | Content | When to Read |
|------|---------|-------------|
| [`codebase_info.md`](codebase_info.md) | Project metadata, language stack, crate overview, distribution info | Quick orientation, version/stack questions |
| [`architecture.md`](architecture.md) | System architecture diagrams, orchestration flow, hat system design, event system, parallel loops, API architecture | Understanding how components fit together, design decisions |
| [`components.md`](components.md) | Detailed module-by-module breakdown of all 9 Rust crates, web frontend, and legacy backend | Finding where specific functionality lives, understanding module responsibilities |
| [`interfaces.md`](interfaces.md) | Core traits, RPC API methods, CLI commands, event topics, backpressure gates, built-in presets | Understanding APIs, adding new commands/methods, event routing |
| [`data_models.md`](data_models.md) | All major types (Event, Hat, Task, Memory, Config, etc.), persistence formats | Working with data structures, understanding storage |
| [`workflows.md`](workflows.md) | End-to-end flows: orchestration loop, planning, task lifecycle, parallel loops, human-in-the-loop, CI | Understanding complete processes, debugging flow issues |
| [`dependencies.md`](dependencies.md) | Rust crate dependencies, Node.js packages, build tools, internal crate dependency graph | Adding dependencies, understanding the build, version questions |
| [`review_notes.md`](review_notes.md) | Documentation gaps, consistency issues, improvement recommendations | Meta: improving the documentation itself |

## Key Entry Points for Common Tasks

| Task | Start Here |
|------|-----------|
| Add a new CLI command | `components.md` → ralph-cli section, `interfaces.md` → CLI Interface |
| Add a new RPC method | `components.md` → ralph-api section, `interfaces.md` → RPC API |
| Add a new agent backend | `components.md` → ralph-adapters section, `architecture.md` → Execution Modes |
| Modify the event loop | `architecture.md` → Core Orchestration Flow, `components.md` → ralph-core Event Loop |
| Add a new hat | `data_models.md` → HatConfig, `interfaces.md` → Event Topics |
| Work with tasks/memories | `data_models.md` → Task/Memory types, `components.md` → State Management |
| Add lifecycle hooks | `components.md` → Hooks section, `data_models.md` → Hook Types, `workflows.md` → Hook Lifecycle |
| Work on the web dashboard | `components.md` → Web Frontend / ralph-api sections |
| Add parallel loop features | `architecture.md` → Parallel Loops, `workflows.md` → Parallel Loop Workflow |
| Add Telegram features | `components.md` → ralph-telegram, `workflows.md` → Human-in-the-Loop |
| Understand testing | `workflows.md` → Session Recording & Replay, `components.md` → ralph-e2e |
