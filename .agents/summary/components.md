# Components

## Rust Crates

### ralph-proto (`crates/ralph-proto/`)

The foundational protocol crate providing shared types, error definitions, and traits used across all Ralph crates.

| Module | Purpose |
|--------|---------|
| [`event.rs`](crates/ralph-proto/src/event.rs) | `Event` struct with topic, payload, source, and target fields |
| [`event_bus.rs`](crates/ralph-proto/src/event_bus.rs) | `EventBus` — central pub/sub hub routing events between hats |
| [`hat.rs`](crates/ralph-proto/src/hat.rs) | `Hat` and `HatId` — agent persona definitions with subscriptions/publishes |
| [`topic.rs`](crates/ralph-proto/src/topic.rs) | `Topic` — routing keys with glob-style pattern matching |
| [`json_rpc.rs`](crates/ralph-proto/src/json_rpc.rs) | `RpcCommand`/`RpcEvent` — JSON-lines IPC protocol types |
| [`robot.rs`](crates/ralph-proto/src/robot.rs) | `RobotService` trait — human-in-the-loop communication abstraction |
| [`daemon.rs`](crates/ralph-proto/src/daemon.rs) | `DaemonAdapter` trait — persistent bot daemon abstraction |
| [`ux_event.rs`](crates/ralph-proto/src/ux_event.rs) | `UxEvent` — terminal/TUI capture events for recording/replay |
| [`error.rs`](crates/ralph-proto/src/error.rs) | Common error types |

---

### ralph-core (`crates/ralph-core/`)

The core orchestration engine containing the event loop, hat system, memory/task management, hooks, and all coordination logic.

**Event Loop & Orchestration:**

| Module | Purpose |
|--------|---------|
| [`event_loop/mod.rs`](crates/ralph-core/src/event_loop/mod.rs) | `EventLoop` — main orchestration loop coordinating hat execution |
| [`event_loop/loop_state.rs`](crates/ralph-core/src/event_loop/loop_state.rs) | `LoopState` — mutable state tracked across iterations |
| [`hatless_ralph.rs`](crates/ralph-core/src/hatless_ralph.rs) | `HatlessRalph` — constant coordinator, prompt builder |
| [`hat_registry.rs`](crates/ralph-core/src/hat_registry.rs) | `HatRegistry` — converts config hats to proto hats |
| [`event_parser.rs`](crates/ralph-core/src/event_parser.rs) | `EventParser` — parses JSONL events from agent output |
| [`event_reader.rs`](crates/ralph-core/src/event_reader.rs) | `EventReader` — reads and validates event files |
| [`event_logger.rs`](crates/ralph-core/src/event_logger.rs) | `EventLogger` — records event history for debugging |
| [`handoff.rs`](crates/ralph-core/src/handoff.rs) | `HandoffWriter` — writes handoff events between hats |

**Configuration:**

| Module | Purpose |
|--------|---------|
| [`config.rs`](crates/ralph-core/src/config.rs) | `RalphConfig` — full configuration model with v1/v2 format support |
| [`instructions.rs`](crates/ralph-core/src/instructions.rs) | `InstructionBuilder` — assembles prompts from guardrails, hat instructions, memories |

**Memory & Task Systems:**

| Module | Purpose |
|--------|---------|
| [`memory.rs`](crates/ralph-core/src/memory.rs) | `Memory`, `MemoryType` — persistent learning data structures |
| [`memory_store.rs`](crates/ralph-core/src/memory_store.rs) | `MarkdownMemoryStore` — markdown-based memory persistence |
| [`memory_parser.rs`](crates/ralph-core/src/memory_parser.rs) | Parser for structured markdown memory format |
| [`task.rs`](crates/ralph-core/src/task.rs) | `Task`, `TaskStatus` — runtime work tracking data structures |
| [`task_store.rs`](crates/ralph-core/src/task_store.rs) | `TaskStore` — JSONL-based task persistence |
| [`task_definition.rs`](crates/ralph-core/src/task_definition.rs) | `TaskDefinition` — code task file parsing |

**Parallel Loops & Coordination:**

| Module | Purpose |
|--------|---------|
| [`loop_lock.rs`](crates/ralph-core/src/loop_lock.rs) | `LoopLock` — file-based PID lock for primary loop |
| [`loop_registry.rs`](crates/ralph-core/src/loop_registry.rs) | `LoopRegistry` — registry of all tracked loops |
| [`loop_context.rs`](crates/ralph-core/src/loop_context.rs) | `LoopContext` — primary vs worktree loop context |
| [`worktree.rs`](crates/ralph-core/src/worktree.rs) | Worktree management (create, remove, sync, symlink) |
| [`merge_queue.rs`](crates/ralph-core/src/merge_queue.rs) | `MergeQueue` — event-sourced merge queue for worktree branches |
| [`loop_name.rs`](crates/ralph-core/src/loop_name.rs) | `LoopNameGenerator` — human-readable loop ID generation |
| [`loop_completion.rs`](crates/ralph-core/src/loop_completion.rs) | `LoopCompletionHandler` — landing/merge on loop completion |
| [`loop_history.rs`](crates/ralph-core/src/loop_history.rs) | `LoopHistory` — persistent loop run history |

**Hooks System:**

| Module | Purpose |
|--------|---------|
| [`hooks/engine.rs`](crates/ralph-core/src/hooks/engine.rs) | `HookEngine` — lifecycle hook orchestration engine |
| [`hooks/executor.rs`](crates/ralph-core/src/hooks/executor.rs) | `HookExecutor` — runs hook commands with timeouts and output limits |
| [`hooks/suspend_state.rs`](crates/ralph-core/src/hooks/suspend_state.rs) | `SuspendStateStore` — persistent state for suspended hooks |

**Skills System:**

| Module | Purpose |
|--------|---------|
| [`skill.rs`](crates/ralph-core/src/skill.rs) | `SkillEntry` — skill definition with frontmatter parsing |
| [`skill_registry.rs`](crates/ralph-core/src/skill_registry.rs) | `SkillRegistry` — discovers and indexes skills from directories |

**Other:**

| Module | Purpose |
|--------|---------|
| [`git_ops.rs`](crates/ralph-core/src/git_ops.rs) | Git operations (auto-commit, branch detection, stash management) |
| [`preflight.rs`](crates/ralph-core/src/preflight.rs) | `PreflightRunner` — pre-loop environment validation checks |
| [`planning_session.rs`](crates/ralph-core/src/planning_session.rs) | `PlanningSession` — PDD interactive planning sessions |
| [`diagnostics/`](crates/ralph-core/src/diagnostics/) | Diagnostic collectors (agent output, orchestration, errors, performance) |
| [`session_recorder.rs`](crates/ralph-core/src/session_recorder.rs) | `SessionRecorder` — records sessions to JSONL for replay |
| [`session_player.rs`](crates/ralph-core/src/session_player.rs) | `SessionPlayer` — replays recorded sessions |
| [`workspace.rs`](crates/ralph-core/src/workspace.rs) | `WorkspaceManager` — isolated workspaces for benchmarks/E2E |
| [`summary_writer.rs`](crates/ralph-core/src/summary_writer.rs) | `SummaryWriter` — generates loop completion summaries |

**Testing Support:**

| Module | Purpose |
|--------|---------|
| [`testing/mock_backend.rs`](crates/ralph-core/src/testing/mock_backend.rs) | Mock AI backend for unit tests |
| [`testing/replay_backend.rs`](crates/ralph-core/src/testing/replay_backend.rs) | Replay backend using recorded JSONL fixtures |
| [`testing/smoke_runner.rs`](crates/ralph-core/src/testing/smoke_runner.rs) | Smoke test runner using replay fixtures |
| [`testing/scenario.rs`](crates/ralph-core/src/testing/scenario.rs) | Test scenario definitions |

---

### ralph-adapters (`crates/ralph-adapters/`)

Backend integrations for various AI coding assistant CLIs.

| Module | Purpose |
|--------|---------|
| [`cli_backend.rs`](crates/ralph-adapters/src/cli_backend.rs) | `CliBackend` — backend selection and command construction |
| [`cli_executor.rs`](crates/ralph-adapters/src/cli_executor.rs) | `CliExecutor` — executes CLI commands and captures output |
| [`pty_executor.rs`](crates/ralph-adapters/src/pty_executor.rs) | `PtyExecutor` — PTY-based execution preserving terminal UI |
| [`pty_handle.rs`](crates/ralph-adapters/src/pty_handle.rs) | `PtyHandle` — control interface for PTY sessions |
| [`auto_detect.rs`](crates/ralph-adapters/src/auto_detect.rs) | Auto-detection of available backends in system PATH |
| [`claude_stream.rs`](crates/ralph-adapters/src/claude_stream.rs) | `ClaudeStreamParser` — streaming parser for Claude CLI output |
| [`pi_stream.rs`](crates/ralph-adapters/src/pi_stream.rs) | `PiStreamParser` — streaming parser for Pi CLI output |
| [`json_rpc_handler.rs`](crates/ralph-adapters/src/json_rpc_handler.rs) | `JsonRpcStreamHandler` — JSON-RPC protocol handler |
| [`acp_executor.rs`](crates/ralph-adapters/src/acp_executor.rs) | `AcpExecutor` — Agent Communication Protocol executor |
| [`stream_handler.rs`](crates/ralph-adapters/src/stream_handler.rs) | Stream handler variants (Console, Pretty, Quiet, TUI) |

---

### ralph-cli (`crates/ralph-cli/`)

CLI entry point with all user-facing commands.

| Module | Purpose |
|--------|---------|
| [`main.rs`](crates/ralph-cli/src/main.rs) | CLI argument parsing, command dispatch, subprocess TUI mode |
| [`loop_runner.rs`](crates/ralph-cli/src/loop_runner.rs) | `run_loop_impl` — main loop execution orchestration |
| [`tools.rs`](crates/ralph-cli/src/tools.rs) | `ralph tools` — agent-facing runtime tools (emit, task, memory, etc.) |
| [`task_cli.rs`](crates/ralph-cli/src/task_cli.rs) | `ralph task` — code task generation commands |
| [`loops.rs`](crates/ralph-cli/src/loops.rs) | `ralph loops` — parallel loop management |
| [`hats.rs`](crates/ralph-cli/src/hats.rs) | `ralph hats` — hat management and inspection |
| [`bot.rs`](crates/ralph-cli/src/bot.rs) | `ralph bot` — Telegram bot setup and daemon |
| [`hooks.rs`](crates/ralph-cli/src/hooks.rs) | `ralph hooks` — hook configuration validation |
| [`init.rs`](crates/ralph-cli/src/init.rs) | `ralph init` — project initialization |
| [`presets.rs`](crates/ralph-cli/src/presets.rs) | Preset/hat collection loading and merging |
| [`memory.rs`](crates/ralph-cli/src/memory.rs) | `ralph memory` — memory management CLI |
| [`sop_runner.rs`](crates/ralph-cli/src/sop_runner.rs) | SOP-based planning runner |
| [`web.rs`](crates/ralph-cli/src/web.rs) | `ralph web` — web dashboard launcher |
| [`doctor.rs`](crates/ralph-cli/src/doctor.rs) | `ralph doctor` — environment diagnostic checks |
| [`rpc_stdin.rs`](crates/ralph-cli/src/rpc_stdin.rs) | RPC stdin command reader for `--rpc` mode |

---

### ralph-tui (`crates/ralph-tui/`)

Terminal user interface built with ratatui. Operates in three modes: in-process, RPC client (HTTP/WS), and subprocess RPC.

| Module | Purpose |
|--------|---------|
| [`app.rs`](crates/ralph-tui/src/app.rs) | `App` — main TUI application loop and render logic |
| [`state.rs`](crates/ralph-tui/src/state.rs) | `TuiState` — shared state model for the TUI |
| [`state_mutations.rs`](crates/ralph-tui/src/state_mutations.rs) | State update logic for incoming events |
| [`input.rs`](crates/ralph-tui/src/input.rs) | Keyboard input handling and action dispatch |
| [`text_renderer.rs`](crates/ralph-tui/src/text_renderer.rs) | Markdown/ANSI to ratatui `Line` conversion |
| [`rpc_client.rs`](crates/ralph-tui/src/rpc_client.rs) | `RpcClient` — HTTP/WS client for ralph-api |
| [`rpc_bridge.rs`](crates/ralph-tui/src/rpc_bridge.rs) | WebSocket bridge connecting RPC events to TUI state |
| [`rpc_source.rs`](crates/ralph-tui/src/rpc_source.rs) | Subprocess stdout event reader |
| [`rpc_writer.rs`](crates/ralph-tui/src/rpc_writer.rs) | `RpcWriter` — sends commands to subprocess stdin |
| [`widgets/`](crates/ralph-tui/src/widgets/) | UI widgets: header, footer, content area, help overlay |

---

### ralph-telegram (`crates/ralph-telegram/`)

Telegram bot integration for human-in-the-loop communication.

| Module | Purpose |
|--------|---------|
| [`bot.rs`](crates/ralph-telegram/src/bot.rs) | `TelegramBot` — teloxide bot wrapper with message sending |
| [`service.rs`](crates/ralph-telegram/src/service.rs) | `TelegramService` — implements `RobotService` trait |
| [`handler.rs`](crates/ralph-telegram/src/handler.rs) | `MessageHandler` — processes incoming Telegram messages |
| [`state.rs`](crates/ralph-telegram/src/state.rs) | `StateManager` — persists chat ID, pending questions |
| [`daemon.rs`](crates/ralph-telegram/src/daemon.rs) | `TelegramDaemon` — implements `DaemonAdapter` for persistent bot |
| [`commands.rs`](crates/ralph-telegram/src/commands.rs) | Bot commands (/start, /status, /restart, etc.) |
| [`error.rs`](crates/ralph-telegram/src/error.rs) | Telegram-specific error types |

---

### ralph-api (`crates/ralph-api/`)

REST and WebSocket API server built with Axum for web dashboard and remote TUI integration.

| Module | Purpose |
|--------|---------|
| [`runtime.rs`](crates/ralph-api/src/runtime.rs) | `RpcRuntime` — manages orchestration loop lifecycle |
| [`transport.rs`](crates/ralph-api/src/transport.rs) | Axum router setup, HTTP/WS serving |
| [`protocol.rs`](crates/ralph-api/src/protocol.rs) | Wire protocol types for API communication |
| [`loop_domain.rs`](crates/ralph-api/src/loop_domain.rs) | Loop management API endpoints |
| [`task_domain.rs`](crates/ralph-api/src/task_domain.rs) | Task management API endpoints |
| [`planning_domain.rs`](crates/ralph-api/src/planning_domain.rs) | PDD planning session endpoints |
| [`preset_domain.rs`](crates/ralph-api/src/preset_domain.rs) | Preset/collection browsing endpoints |
| [`config_domain.rs`](crates/ralph-api/src/config_domain.rs) | Configuration management endpoints |
| [`collection_domain.rs`](crates/ralph-api/src/collection_domain.rs) | Hat collection management |
| [`stream_domain/`](crates/ralph-api/src/stream_domain/) | Real-time event streaming via WebSocket |
| [`auth.rs`](crates/ralph-api/src/auth.rs) | Authentication middleware |
| [`config.rs`](crates/ralph-api/src/config.rs) | `ApiConfig` — API server configuration |
| [`idempotency.rs`](crates/ralph-api/src/idempotency.rs) | Idempotency key handling |

---

### ralph-e2e (`crates/ralph-e2e/`)

End-to-end test framework with mock and live API test modes.

| Module | Purpose |
|--------|---------|
| [`runner.rs`](crates/ralph-e2e/src/runner.rs) | E2E test runner |
| [`executor.rs`](crates/ralph-e2e/src/executor.rs) | Test execution engine |
| [`mock.rs`](crates/ralph-e2e/src/mock.rs) | Mock backend for CI-safe testing |
| [`mock_cli.rs`](crates/ralph-e2e/src/mock_cli.rs) | Mock CLI for testing without real backends |
| [`scenarios/`](crates/ralph-e2e/src/scenarios/) | Test scenarios (connectivity, orchestration, tasks, etc.) |
| [`reporter.rs`](crates/ralph-e2e/src/reporter.rs) | Test report generation |
| [`workspace.rs`](crates/ralph-e2e/src/workspace.rs) | Isolated test workspace management |

---

## Web Packages

### @ralph-web/server (`backend/ralph-web-server/`)

Node.js web server providing tRPC API and task management.

| Directory | Purpose |
|-----------|---------|
| [`api/`](backend/ralph-web-server/src/api/) | tRPC routers, REST endpoints, log broadcasting |
| [`runner/`](backend/ralph-web-server/src/runner/) | Ralph process management, event parsing, log streaming |
| [`queue/`](backend/ralph-web-server/src/queue/) | Task queue with persistent storage and event bus |
| [`repositories/`](backend/ralph-web-server/src/repositories/) | SQLite data access (tasks, logs, settings, collections) |
| [`services/`](backend/ralph-web-server/src/services/) | Business logic (config merging, hat management, loops) |
| [`db/`](backend/ralph-web-server/src/db/) | SQLite connection and schema |

### @ralph-web/dashboard (`frontend/ralph-web/`)

React web dashboard for monitoring and managing orchestration loops.

| Directory | Purpose |
|-----------|---------|
| [`components/tasks/`](frontend/ralph-web/src/components/tasks/) | Task management UI (thread list, task detail, status bar) |
| [`components/builder/`](frontend/ralph-web/src/components/builder/) | Visual hat collection builder (React Flow) |
| [`components/plan/`](frontend/ralph-web/src/components/plan/) | PDD planning session UI |
| [`components/layout/`](frontend/ralph-web/src/components/layout/) | Application shell, sidebar, navigation |
| [`pages/`](frontend/ralph-web/src/pages/) | Route pages (tasks, builder, plan, settings) |
| [`hooks/`](frontend/ralph-web/src/hooks/) | React hooks (WebSocket, keyboard shortcuts, notifications) |
| [`stores/`](frontend/ralph-web/src/stores/) | Zustand state stores |
| [`rpc/`](frontend/ralph-web/src/rpc/) | tRPC client configuration |
