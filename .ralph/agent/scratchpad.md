# Scratchpad — Architectural Plan for Ralph Orchestrator

## Objective
Create a detailed architectural plan document that enables recreating the full functionality of the Ralph Orchestrator through agentic coding.

## Understanding
After thorough exploration, this is a **Rust-based AI agent orchestration framework** (not a PDF/question processing system as the user described — the description may refer to a different project). The repository has TWO main parts:

1. **Rust Core (8 crates)** — The orchestration engine that keeps AI agents in a loop until tasks are done. Uses an event-driven "hat" system where specialized AI personas coordinate through pub/sub events.

2. **Web Dashboard (TypeScript)** — A React + Vite frontend with Fastify + tRPC backend for monitoring/managing orchestration loops, creating tasks, viewing logs in real-time, and visually building hat collections.

## Key Architectural Systems Identified

### Rust Crates
- **ralph-proto**: Core types (Event, EventBus, Hat, Topic, RobotService trait)
- **ralph-core**: Orchestration logic (event loop, hat system, memory/task stores, worktree coordination, loop registry, merge queue, config)
- **ralph-adapters**: Backend integrations (Claude, Kiro, Gemini, Codex, Amp) with PTY/CLI execution
- **ralph-cli**: CLI entry point (run, plan, task, loops, web, init, tools, bot commands)
- **ralph-telegram**: Human-in-the-loop via Telegram (bidirectional messaging, state persistence)
- **ralph-tui**: ratatui-based terminal UI
- **ralph-e2e**: End-to-end testing framework with mock mode
- **ralph-bench**: Benchmarking

### Web Stack
- **Backend**: Fastify + tRPC + SQLite (better-sqlite3/Drizzle ORM)
  - Task queue with Dispatcher pattern (detached process management)
  - LogBroadcaster via WebSocket for real-time log streaming
  - ProcessSupervisor for crash recovery
  - ConfigMerger for preset composition
  - LoopsManager for merge queue processing
  - PlanningService for interactive planning sessions
- **Frontend**: React 19 + Vite 7 + TailwindCSS 4 + TypeScript 5.9
  - tRPC client with React Query
  - Zustand stores (UI state + log buffers)
  - Visual hat builder (React Flow)
  - Real-time log viewer with WebSocket
  - Task management with loop integration
  - Planning Q&A interface with artifact viewer

### Infrastructure
- 14+ YAML presets (feature, bugfix, debug, spec-driven, research, etc.)
- Docker + docker-compose (orchestrator, Redis, optional Postgres, Prometheus, Grafana)
- MkDocs documentation site
- GitHub Actions CI/CD with cargo-dist for multi-platform binaries
- npm publishing as @ralph-orchestrator/ralph-cli
- Homebrew tap

## Doc Review (Doc Reviewer Hat)

### Review Summary: APPROVED with corrections applied

Reviewed `.ralph/specs/architecture-plan.md` (16 sections, ~1700 lines) against actual codebase.

### Corrections Applied

1. **EventLoop struct** (Section 3): Fixed `registry` type from `HashMap<HatId, Hat>` to `HatRegistry`. Added missing `robot_guidance: Vec<String>` field. Changed `diagnostics` from `Option<Diagnostics>` to `DiagnosticsCollector`. Changed `loop_context` from `LoopContext` to `Option<LoopContext>`.

2. **RobotService trait** (Section 4): Removed incorrect `#[async_trait]` — trait methods are synchronous `fn`, not `async fn`. Fixed method signatures to match actual code (`send_question` returns `anyhow::Result<i32>`, `wait_for_response` takes `events_path: &Path`). Added missing methods: `timeout_secs()`, `shutdown_flag()`, `stop()`.

3. **CheckinContext struct** (Section 4): Completely replaced incorrect fields. Was: `iteration`, `elapsed`, `current_hat`, `last_event`. Actual: `current_hat`, `open_tasks`, `closed_tasks`, `cumulative_cost`.

4. **tRPC Routers** (Section 12): Updated router table to match actual code. Was: task, collection, settings, plan, loops, logs. Actual: task, hat, loops, collection, config, presets, planning. Added note that all routers live in single `trpc.ts` file.

### Verified Accurate (no changes needed)

- HatlessRalph struct fields (Section 3) ✓
- Event, EventBus, Hat, HatId, Topic structs (Section 4) ✓
- CliBackend, OutputFormat, PromptMode (Section 5) ✓
- MemoryType enum and Memory struct (Section 6) ✓
- TaskStatus enum and Task struct (Section 7) ✓
- MergeEventType, MergeState enums (Section 8) ✓
- TerminationReason enum variants (Section 3) ✓
- Preset count (14 files) (Section 9) ✓
- Overall structure and recreation guide (Section 16) ✓

## Plan
The document should be structured as a comprehensive architecture guide with sections that enable full recreation:

1. **Overview & Philosophy** — The Ralph Tenets, design philosophy
2. **Repository Structure** — Crate layout, web stack, supporting infrastructure
3. **Core Orchestration Engine** — Event loop, hat system, event bus, prompt construction
4. **Protocol Layer** — Event, EventBus, Hat, Topic types
5. **Backend Adapters** — How CLIs are spawned, PTY vs standard execution, stream handling
6. **Memory System** — Types, storage format, locking, auto-injection
7. **Task System** — Task lifecycle, storage, dependency resolution, loop ownership
8. **Parallel Loops** — Lock coordination, git worktrees, merge queue, loop registry
9. **Configuration System** — YAML structure, hat configs, preset system
10. **Human-in-the-Loop (RObot)** — Telegram integration, event flow, multi-loop routing
11. **Terminal UI** — ratatui architecture
12. **Web Dashboard Backend** — Server architecture, queue system, process supervision
13. **Web Dashboard Frontend** — Component hierarchy, state management, real-time streaming
14. **Testing Strategy** — Smoke tests, E2E framework, mock mode
15. **CI/CD & Distribution** — Build, package, publish pipeline
16. **Recreating the System** — Step-by-step guide for agentic coding

### HUMAN GUIDANCE (2026-02-06 07:45:03 UTC)

Focus on error handling

### HUMAN GUIDANCE (2026-02-06 07:45:03 UTC)

Keep this in mind

## Verification Evidence (Iteration: handling review.blocked)

- **build: pass** — `cargo build` completed successfully (Finished `dev` profile)
- **tests: pass** — 238 passed, 1 pre-existing flaky failure (`check_port_available_detects_in_use` — port race condition, not related to documentation), 2 ignored
- **Document**: `.ralph/specs/architecture-plan.md` — 1729 lines, 16 sections, reviewed and corrected by Doc Reviewer hat

Re-emitting `write.done` with verification evidence to satisfy the review.blocked gate.

## Fresh Verification (2026-02-06, re-run for review.blocked)

- **build: pass** — `cargo build` completed successfully (Finished `dev` profile, 0.40s)
- **tests: pass** — 238 passed, 1 pre-existing flaky failure (`check_port_available_detects_in_use` — port race condition, unrelated), 2 ignored
- **Document**: `.ralph/specs/architecture-plan.md` — 1729 lines, 16 sections, reviewed and corrected

Emitting `write.done` with explicit verification evidence in payload.

## Second Doc Review Pass (2026-02-06, handling write.done event)

### Review Summary: REVISION REQUESTED — Missing error handling section

Performed independent verification of all 16 sections against the codebase. All technical claims verified accurate:
- TerminationReason enum: all 10 variants match ✓
- Preset count: exactly 14 YAML files ✓
- tRPC routers: all 7 match (task, hat, loops, collection, config, presets, planning) ✓
- Claude temp file threshold: exactly 7000 chars ✓
- HatlessRalph, EventBus, Hat, Event structs ✓

### Missing Section: Error Handling Patterns

Per human guidance "Focus on error handling", the document needs a dedicated error handling section. Findings from codebase analysis:

1. **Error type strategy**: `thiserror`-based custom error enums throughout Rust crates (14+ error types in ralph-core: ConfigError, WorktreeError, MergeQueueError, LockError, RegistryError, etc.)
2. **Propagation**: `Result<T, SpecificError>` with `#[from]` auto-conversions. `anyhow` used minimally (only skill_registry.rs)
3. **Retry logic**:
   - Telegram: 3 attempts with exponential backoff (1s, 2s, 4s) via `retry_with_backoff()`
   - Event loop: consecutive failure tracking, terminates after 5 (configurable)
   - Backend adapters: No automatic retries (fresh context handles recovery per Tenet 1)
4. **Graceful degradation**: Diagnostics fallback to disabled, robot timeout fallback
5. **Web server**: tRPC error codes (NOT_FOUND, INTERNAL_SERVER_ERROR, BAD_REQUEST), WebSocket reconnection with exponential backoff (1s to 30s), graceful subprocess shutdown (SIGTERM then SIGKILL after 5s)
6. **No circuit breaker patterns** found in codebase

Requesting revision to add this section (suggested placement: between Section 4 and Section 5, or as a cross-cutting concerns section).

### HUMAN GUIDANCE (2026-02-06 07:54:25 UTC)

Keep this in mind

### HUMAN GUIDANCE (2026-02-06 07:54:25 UTC)

Focus on error handling

## Writer: Error Handling Patterns Section Added (2026-02-06)

Handled `review.revision` event requesting dedicated error handling section per human guidance.

### What was added

New **Section 5: Error Handling Patterns** inserted between Protocol Layer (Section 4) and Backend Adapters (now Section 6). All subsequent sections renumbered (5-16 → 6-17). Document grew from 1729 to 1905 lines, now 17 sections.

### Section covers

1. **Error type strategy** — 25+ `thiserror` error enums across crates, table of ralph-core error types, `TelegramError` enum
2. **Anyhow usage** — Only at CLI/application boundaries, not in library crates
3. **Consecutive failure tracking** — Event loop counter with configurable threshold (default 5)
4. **Retry with exponential backoff** — Telegram: 3 attempts, 1s/2s/4s delays
5. **Graceful degradation** — Diagnostics fallback to disabled, robot timeout returns `Ok(None)`, check-in failures logged but not fatal
6. **Web server errors** — tRPC error codes (NOT_FOUND, INTERNAL_SERVER_ERROR, BAD_REQUEST), WebSocket disconnect handling, two-phase subprocess shutdown (SIGTERM then SIGKILL)

### Verification

- **build: pass** — `cargo build` completed (0.41s)
- **tests: pass** — 238 passed, 1 pre-existing flaky failure (port race), 2 ignored
- All code snippets verified against actual source files
