# Components

## ralph-proto — Protocol Definitions

Foundation crate with zero orchestration logic. Defines shared abstractions used across all other crates.

| Module | Responsibility |
|--------|---------------|
| `event.rs` | `Event` struct: topic, payload, source/target hat |
| `event_bus.rs` | `EventBus`: pub/sub routing, observer pattern, pending event queues |
| `hat.rs` | `Hat`, `HatId`: persona definitions with subscriptions/publishes |
| `topic.rs` | `Topic`: pattern-based event routing |
| `json_rpc.rs` | `RpcCommand`, `RpcEvent`, `RpcState`: JSON-RPC protocol for TUI/API communication |
| `robot.rs` | `RobotService` trait, `CheckinContext`: human-in-the-loop abstraction |
| `ux_event.rs` | `UxEvent`, `TerminalWrite`, `FrameCapture`: TUI rendering events |
| `daemon.rs` | `DaemonAdapter`, `StartLoopFn`: daemon mode abstractions |

## ralph-core — Orchestration Engine

The largest crate. Contains all orchestration logic, state management, and coordination primitives.

### Event Loop (`event_loop/`)
- `mod.rs`: Main `EventLoop` struct — coordinates hat selection, prompt building, agent execution, event parsing, termination detection
- `loop_state.rs`: `LoopState` — iteration counter, cost tracking, timing, consecutive failure count
- `tests.rs`: Unit tests for event loop logic

### Hat System
- `hatless_ralph.rs`: `HatlessRalph` — the constant coordinator. Builds prompts with hat instructions, memories, skills, robot guidance. Always present as fallback
- `hat_registry.rs`: `HatRegistry` — stores hat definitions, resolves subscribers by topic
- `instructions.rs`: `InstructionBuilder` — assembles prompt sections (objective, hat instructions, skills, memories, tasks, guardrails)

### Configuration
- `config.rs`: `RalphConfig` — top-level config with v1/v2 format compatibility. Includes `EventLoopConfig`, `CliConfig`, `CoreConfig`, `HatConfig`, `HooksConfig`, `SkillsConfig`, `MemoriesConfig`, `FeaturesConfig`

### State Management
- `memory.rs` / `memory_store.rs`: `Memory`, `MarkdownMemoryStore` — persistent learning in `.ralph/agent/memories.md`. Types: Pattern, Decision, Fix, Context
- `task.rs` / `task_store.rs`: `Task`, `TaskStore` — JSONL-based task tracking with status (Open, InProgress, Closed, Failed), priorities, dependencies
- `task_definition.rs`: Code task file parsing (`.code-task.md`)
- `planning_session.rs`: `PlanningSession` — chat-style collaborative planning with conversation JSONL and artifacts

### Event Processing
- `event_parser.rs`: `EventParser` — extracts structured events from agent output, detects mutations
- `event_reader.rs`: `EventReader` — reads JSONL event files, handles malformed lines
- `event_logger.rs`: `EventLogger`, `EventHistory` — records and queries event history

### Parallel Execution
- `worktree.rs`: Git worktree management — create, remove, list, symlink shared state
- `merge_queue.rs`: `MergeQueue` — event-sourced merge queue for completed worktree loops
- `loop_lock.rs`: `LoopLock` — PID-based lock for primary loop coordination
- `loop_registry.rs`: `LoopRegistry` — tracks all active/completed loops in `loops.json`

### Hooks
- `hooks/engine.rs`: `HookEngine` — resolves hooks by phase-event, builds JSON payloads
- `hooks/executor.rs`: `HookExecutor` — runs hook commands, streams output, handles errors
- `hooks/suspend_state.rs`: `SuspendStateStore` — persists hook suspend/resume state

### Skills
- `skill.rs`: `SkillEntry`, `SkillFrontmatter` — skill data types with YAML frontmatter parsing
- `skill_registry.rs`: `SkillRegistry` — discovers, loads, and indexes skills from directories and built-ins

### Utilities
- `git_ops.rs`: Git operations (auto-commit, branch info, stash management)
- `loop_context.rs`: `LoopContext` — workspace root, ralph dir, events file paths
- `loop_completion.rs`: `LoopCompletionHandler` — post-loop actions (merge queue, cleanup)
- `loop_history.rs`: `LoopHistory` — iteration history tracking
- `handoff.rs`: `HandoffWriter` — writes handoff files between iterations
- `preflight.rs`: `PreflightRunner` — pre-loop validation checks
- `summary_writer.rs`: Post-loop summary generation
- `landing.rs`: `LandingHandler` — post-completion landing actions
- `session_recorder.rs` / `session_player.rs`: JSONL session recording/replay (behind `recording` feature flag)
- `diagnostics/`: Agent output, orchestration, error, and performance diagnostics collectors

## ralph-adapters — Backend Integrations

Bridges between Ralph's orchestration engine and various AI agent CLIs.

| Module | Responsibility |
|--------|---------------|
| `cli_executor.rs` | `CliExecutor` — spawns agent CLI as subprocess, captures output |
| `pty_executor.rs` | `PtyExecutor` — PTY-based execution preserving rich terminal UI |
| `acp_executor.rs` | `AcpExecutor` — Agent Communication Protocol executor |
| `cli_backend.rs` | `CliBackend` — backend configuration (command, args, prompt mode, output format) |
| `auto_detect.rs` | `detect_backend` — finds available backends in PATH |
| `claude_stream.rs` | `ClaudeStreamParser` — parses Claude CLI streaming JSON output |
| `pi_stream.rs` | `PiStreamParser` — parses Pi agent streaming output |
| `json_rpc_handler.rs` | `JsonRpcStreamHandler` — handles JSON-RPC output from agents |
| `stream_handler.rs` | `StreamHandler` trait + implementations: Console, Pretty, Quiet, TUI |
| `pty_handle.rs` | `PtyHandle` — low-level PTY management and control commands |

## ralph-cli — Command-Line Interface

Binary crate producing the `ralph` executable. All user-facing commands.

| Module | Commands |
|--------|----------|
| `main.rs` | Clap parser, subcommand dispatch |
| `loop_runner.rs` | `ralph run` — main orchestration loop |
| `init.rs` | `ralph init` — project initialization |
| `sop_runner.rs` | `ralph plan` — SOP-based planning |
| `task_cli.rs` | `ralph task` — task management |
| `loops.rs` | `ralph loops` — loop monitoring |
| `web.rs` | `ralph web` — web dashboard launcher |
| `mcp.rs` | `ralph mcp` — MCP server mode |
| `bot.rs` | `ralph bot` — Telegram bot management |
| `hats.rs` | `ralph hats` — hat inspection |
| `hooks.rs` | `ralph hooks` — hook validation |
| `memory.rs` | `ralph memory` — memory management |
| `skill_cli.rs` | `ralph skills` — skill listing |
| `tools.rs` | `ralph tools` — agent tool subcommands |
| `presets.rs` | `ralph presets` — preset management |
| `doctor.rs` | `ralph doctor` — system diagnostics |
| `interact.rs` | `ralph interact` — human interaction |
| `config_resolution.rs` | Config file discovery and merging |
| `backend_support.rs` | Backend availability checks |
| `preflight.rs` | Pre-run validation |
| `display.rs` | Output formatting |

## ralph-api — RPC API & MCP Server

Rust-native control-plane API with domain-driven design.

| Module | Responsibility |
|--------|---------------|
| `runtime.rs` | `RpcRuntime` — central dispatch, auth, idempotency |
| `transport.rs` | Axum HTTP/WebSocket server |
| `mcp.rs` | `RalphMcpServer` — MCP server over stdio using rmcp |
| `protocol.rs` | Request/response envelopes, method registry, schema validation |
| `task_domain.rs` | task.* methods (CRUD, run, cancel, status) |
| `loop_domain.rs` | loop.* methods (list, status, stop, merge, retry) |
| `planning_domain.rs` | planning.* methods (start, respond, resume, artifacts) |
| `config_domain.rs` | config.* methods (get, update) |
| `collection_domain.rs` | collection.* methods (CRUD, import/export hat collections) |
| `preset_domain.rs` | preset.* methods (list built-in presets) |
| `stream_domain.rs` | stream.* methods (WebSocket event streaming) |
| `auth.rs` | Authentication (local trusted, token-based) |
| `idempotency.rs` | Idempotency store for mutating operations |

## ralph-tui — Terminal UI

Read-only observation dashboard for monitoring orchestration loops.

| Module | Responsibility |
|--------|---------------|
| `app.rs` | Main TUI loop, event handling, rendering |
| `state.rs` / `state_mutations.rs` | `TuiState` — iteration data, scroll position, search |
| `rpc_bridge.rs` / `rpc_client.rs` | WebSocket connection to ralph-api |
| `rpc_source.rs` / `rpc_writer.rs` | RPC data source and command writer |
| `text_renderer.rs` | Markdown-to-terminal rendering |
| `input.rs` | Keyboard/mouse input mapping |
| `widgets/` | Header, content pane, footer, help overlay |
| `update_check.rs` | Version update notifications |

## ralph-telegram — Telegram Integration

Human-in-the-loop via Telegram bot (teloxide framework).

| Module | Responsibility |
|--------|---------------|
| `bot.rs` | Bot lifecycle, message sending with retry |
| `handler.rs` | Incoming message routing |
| `commands.rs` | `/status`, `/tasks`, `/restart` command handlers |
| `service.rs` | `TelegramRobotService` — implements `RobotService` trait |
| `state.rs` | Persistent bot state (chat ID, pending questions) |
| `daemon.rs` | Background bot polling |
| `loop_lock.rs` | Primary loop detection for bot ownership |

## ralph-e2e — End-to-End Tests

Test framework with mock and live modes.

| Module | Responsibility |
|--------|---------------|
| `scenarios/` | Test scenarios: connectivity, events, hats, memory, tasks, orchestration, capabilities, errors, incremental |
| `executor.rs` | Scenario execution engine |
| `runner.rs` | Test runner with filtering |
| `mock.rs` / `mock_cli.rs` | Mock backend for CI-safe testing |
| `reporter.rs` | Markdown report generation |
| `analyzer.rs` | Result analysis |

## Web Frontend (`frontend/ralph-web/`)

React + Vite + TailwindCSS dashboard.

| Area | Key Files |
|------|-----------|
| Pages | `TasksPage`, `TaskDetailPage`, `BuilderPage`, `PlanPage`, `SettingsPage` |
| Task components | `TaskThread`, `ThreadList`, `TaskInput`, `LoopDetail`, `LoopActions`, `LiveStatus` |
| Builder | `CollectionBuilder` (visual hat collection editor with ReactFlow) |
| Planning | `PlanLanding`, `PlanSession` |
| Layout | `AppShell`, `Sidebar`, `NavItem` |
| State | `store.ts` (Zustand), `logStore.ts` |
| RPC | `rpc/client.ts` (WebSocket to ralph-api) |
| Hooks | `useTaskWebSocket`, `useKeyboardShortcuts`, `useNotifications` |

## Legacy Web Backend (`backend/ralph-web-server/`)

Node.js Fastify + tRPC server (being superseded by ralph-api).

| Area | Key Files |
|------|-----------|
| API | `api/trpc.ts`, `api/rest.ts`, `api/server.ts` |
| Runner | `RalphRunner`, `ProcessSupervisor`, `RalphEventParser`, `LogStream` |
| Queue | `Dispatcher`, `PersistentTaskQueueService`, `TaskState` |
| Services | `CollectionService`, `ConfigMerger`, `HatManager`, `LoopsManager`, `PlanningService`, `TaskBridge` |
| DB | `db/connection.ts`, `db/schema.ts` (SQLite) |
| Repositories | `TaskRepository`, `QueuedTaskRepository`, `TaskLogRepository`, `CollectionRepository` |
