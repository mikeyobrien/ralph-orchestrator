# Ralph Orchestrator: Comprehensive Architectural Plan

This document provides a complete architectural reference for the Ralph Orchestrator system. It enables full recreation of the system through agentic coding.

**Version**: 2.4.3 | **Rust Edition**: 2024 | **License**: MIT

---

## Table of Contents

1. [Overview & Philosophy](#1-overview--philosophy)
2. [Repository Structure](#2-repository-structure)
3. [Core Orchestration Engine](#3-core-orchestration-engine)
4. [Protocol Layer](#4-protocol-layer)
5. [Error Handling Patterns](#5-error-handling-patterns)
6. [Backend Adapters](#6-backend-adapters)
7. [Memory System](#7-memory-system)
8. [Task System](#8-task-system)
9. [Parallel Loops](#9-parallel-loops)
10. [Configuration System](#10-configuration-system)
11. [Human-in-the-Loop (RObot)](#11-human-in-the-loop-robot)
12. [Terminal UI](#12-terminal-ui)
13. [Web Dashboard Backend](#13-web-dashboard-backend)
14. [Web Dashboard Frontend](#14-web-dashboard-frontend)
15. [Testing Strategy](#15-testing-strategy)
16. [CI/CD & Distribution](#16-cicd--distribution)
17. [Recreating the System](#17-recreating-the-system)

---

## 1. Overview & Philosophy

Ralph Orchestrator is a multi-agent orchestration framework written in Rust. It keeps AI coding agents (Claude, Kiro, Gemini, Codex, Amp, Copilot, OpenCode) in an iterative loop until a task is complete. The orchestrator coordinates agent work through an event-driven "hat" system where specialized personas publish and subscribe to events.

### The Ralph Tenets

1. **Fresh Context Is Reliability** -- Each iteration clears context. Re-read specs, plan, and code every cycle. Optimize for the "smart zone" (40-60% of ~176K usable tokens).

2. **Backpressure Over Prescription** -- Don't prescribe how; create gates that reject bad work. Tests, typechecks, builds, lints. For subjective criteria, use LLM-as-judge with binary pass/fail.

3. **The Plan Is Disposable** -- Regeneration costs one planning loop. Never fight to save a plan.

4. **Disk Is State, Git Is Memory** -- Memories and Tasks are the handoff mechanisms. No sophisticated coordination needed.

5. **Steer With Signals, Not Scripts** -- The codebase is the instruction manual. When Ralph fails a specific way, add a sign for next time.

6. **Let Ralph Ralph** -- Sit *on* the loop, not *in* it. Tune like a guitar, don't conduct like an orchestra.

### Design Principle

> The orchestrator is a thin coordination layer, not a platform. Agents are smart; let them do the work.

---

## 2. Repository Structure

### Rust Workspace (8 Crates)

```
Cargo.toml                     # Workspace root (resolver = "2")
crates/
  ralph-proto/                 # Protocol types (Event, EventBus, Hat, Topic, RobotService)
  ralph-core/                  # Orchestration engine (event loop, hats, memory, tasks, config)
  ralph-adapters/              # Backend integrations (CLI execution, PTY, stream parsing)
  ralph-cli/                   # CLI entry point (run, plan, task, loops, web, init, tools)
  ralph-telegram/              # Telegram bot for human-in-the-loop communication
  ralph-tui/                   # Terminal UI (ratatui-based observation dashboard)
  ralph-e2e/                   # End-to-end testing framework with mock mode
  ralph-bench/                 # Benchmarking
```

### Dependency Graph

```
ralph-cli
  ├── ralph-core
  │     └── ralph-proto
  ├── ralph-adapters
  │     ├── ralph-core
  │     └── ralph-proto
  ├── ralph-tui
  │     └── ralph-proto
  └── ralph-telegram
        └── ralph-proto
```

### Web Stack

```
backend/
  ralph-web-server/            # Fastify + tRPC + SQLite (better-sqlite3, Drizzle ORM)
    src/
      api/                     # tRPC routers (task, collection, settings, plan, loops, logs)
      db/                      # SQLite schema + Drizzle ORM
      queue/                   # TaskQueueService, Dispatcher, EventBus
      runner/                  # RalphRunner (subprocess), LogStream, ProcessSupervisor
      services/                # TaskBridge, ConfigMerger, LoopsManager, PlanningService
      repositories/            # Data access (Task, TaskLog, QueuedTask, Collection)

frontend/
  ralph-web/                   # React 19 + Vite 7 + TailwindCSS 4 + TypeScript 5.9
    src/
      components/              # UI components (tasks, builder, plan, layout)
      hooks/                   # useTaskWebSocket
      stores/                  # Zustand (useUIStore, useLogStore)
      pages/                   # TasksPage, TaskDetailPage, BuilderPage, PlanPage, SettingsPage
```

### Supporting Infrastructure

```
presets/                       # 14 YAML presets (feature, bugfix, debug, deploy, etc.)
docker-compose.yml             # Orchestrator + Redis + optional Postgres/Prometheus/Grafana
docs/                          # MkDocs documentation site
.github/workflows/
  ci.yml                       # Test + Clippy + Format + E2E Mock + Web Tests + Package Check
  release.yml                  # cargo-dist multi-platform binary builds
  docs.yml                     # MkDocs deployment
  claude.yml                   # Claude AI integration
  claude-code-review.yml       # AI-powered code review
scripts/                       # setup-hooks.sh, sync-embedded-files.sh
```

### Key Runtime Files

| File | Purpose |
|------|---------|
| `.ralph/agent/memories.md` | Persistent learning across sessions (markdown sections) |
| `.ralph/agent/tasks.jsonl` | Runtime work tracking (JSONL append-only) |
| `.ralph/agent/scratchpad.md` | Working memory journal for current objective |
| `.ralph/agent/decisions.md` | Decision journal with confidence scores |
| `.ralph/loop.lock` | Primary loop lock (PID + prompt) via flock() |
| `.ralph/loops.json` | Registry of all tracked loops (JSON) |
| `.ralph/events.jsonl` | Event stream (timestamped JSONL) |
| `.ralph/merge-queue.jsonl` | Event-sourced merge queue (JSONL) |
| `.ralph/telegram-state.json` | Telegram bot state (chat ID, pending questions) |
| `.ralph/diagnostics/` | Trace logs (agent-output, orchestration, errors) |

---

## 3. Core Orchestration Engine

The event loop lives in `crates/ralph-core/src/event_loop/mod.rs`. It drives the entire orchestration cycle.

### EventLoop Struct

```rust
pub struct EventLoop {
    config: RalphConfig,
    registry: HatRegistry,
    bus: EventBus,
    state: LoopState,
    instruction_builder: InstructionBuilder,
    ralph: HatlessRalph,           // Universal coordinator (always registered)
    robot_guidance: Vec<String>,   // Cached human guidance messages across iterations
    event_reader: EventReader,     // Reads events from JSONL
    diagnostics: DiagnosticsCollector,
    loop_context: Option<LoopContext>,
    skill_registry: SkillRegistry,
    robot_service: Option<Box<dyn RobotService>>,
}
```

### Iteration Cycle

Each iteration follows this flow:

```
1. Read new events from .ralph/events.jsonl
2. Publish human guidance (if any RObot messages pending)
3. While events are pending on the bus:
   a. Select hat (specific subscriber match > wildcard fallback > HatlessRalph)
   b. Build prompt (HatlessRalph.build_prompt())
   c. Execute backend (CLI adapter spawns agent process)
   d. Parse output for events (regex: topic="..." payload="...")
   e. Validate events against hat's allowed publishes
   f. Write events to .ralph/events.jsonl
   g. Check termination conditions
4. Return TerminationReason
```

### HatlessRalph (Universal Coordinator)

Defined in `crates/ralph-core/src/hatless_ralph.rs`, HatlessRalph is the constant coordinator that builds prompts for every iteration. It is always registered on the EventBus as a wildcard fallback subscriber.

```rust
pub struct HatlessRalph {
    completion_promise: String,    // Default: "LOOP_COMPLETE"
    core: CoreConfig,
    hat_topology: Option<HatTopology>,
    starting_event: Option<String>,
    memories_enabled: bool,
    objective: Option<String>,
    skill_index: String,
    robot_guidance: Vec<String>,
}
```

The prompt is constructed with these sections (in order):
1. **ORIENTATION** -- Fresh context notice, iteration instructions
2. **SCRATCHPAD** -- Auto-injected contents of scratchpad.md
3. **STATE MANAGEMENT** -- Task/memory/scratchpad rules
4. **GUARDRAILS** -- Always-injected rules (5 default guardrails)
5. **SKILLS** -- Available skill table with load commands
6. **OBJECTIVE** -- The user's original prompt/goal
7. **PENDING EVENTS** -- Events to handle this iteration
8. **ACTIVE HAT** -- Hat-specific instructions (if multi-hat mode)
9. **EVENT WRITING** -- How to publish events (ralph emit syntax)
10. **DONE** -- Completion promise instructions

### Hat Topology

In multi-hat configurations, `HatTopology` generates a routing table:

```rust
pub struct HatTopology {
    hats: Vec<HatInfo>,
}

pub struct HatInfo {
    pub name: String,
    pub description: String,
    pub subscribes_to: Vec<String>,
    pub publishes: Vec<String>,
    pub instructions: String,
    pub event_receivers: HashMap<String, Vec<EventReceiver>>,
}
```

This produces a markdown table showing which hats receive which events, enabling agents to understand the full event flow.

### Termination Conditions

```rust
pub enum TerminationReason {
    CompletionPromise,       // Agent published the completion topic
    MaxIterations,           // Exceeded max_iterations config
    MaxRuntime,              // Exceeded max_runtime_seconds config
    MaxCost,                 // Exceeded max_cost_usd config
    ConsecutiveFailures,     // N consecutive failures (default: 5)
    LoopThrashing,           // Same event repeating without progress
    ValidationFailure,       // Validation gate failed
    Stopped,                 // External stop signal
    Interrupted,             // SIGINT/Ctrl+C
    RestartRequested,        // RObot restart command
}
```

Exit codes: 0 = completion promise, 1 = failure, 2 = limit reached, 130 = user interrupt.

---

## 4. Protocol Layer

The `ralph-proto` crate defines the foundational types used across all crates.

### Event

```rust
pub struct Event {
    pub topic: Topic,              // Pattern-matchable topic string
    pub payload: String,           // Event data (brief, not for data transport)
    pub source: Option<HatId>,     // Which hat published this event
    pub target: Option<HatId>,     // Optional targeted delivery
}

impl Event {
    pub fn new(topic: impl Into<String>, payload: impl Into<String>) -> Self;
}
```

Events are written to `.ralph/events.jsonl` as timestamped JSONL entries. They are append-only and immutable.

### EventBus

```rust
pub struct EventBus {
    hats: HashMap<HatId, Hat>,              // Registered hat subscribers
    pending: HashMap<HatId, Vec<Event>>,    // Per-hat event queues
    human_pending: Vec<Event>,              // Human interaction queue
    observers: Vec<Box<dyn Fn(&Event)>>,    // Observer callbacks (TUI, diagnostics)
}
```

Event routing priority:
1. **Specific subscriptions** -- Hat explicitly subscribes to the event topic
2. **Wildcard fallbacks** -- Hat subscribes to `*` (catches unmatched events)
3. **HatlessRalph** -- Universal fallback, always registered

### Hat

```rust
pub struct Hat {
    pub id: HatId,
    pub name: String,
    pub description: String,
    pub subscriptions: Vec<Topic>,     // Topics this hat receives
    pub publishes: Vec<Topic>,         // Topics this hat can emit
    pub instructions: String,          // Role-specific instructions
}

pub struct HatId(String);

impl Hat {
    pub fn matches(&self, topic: &Topic) -> bool;
    pub fn is_specific_match(&self, topic: &Topic) -> bool;  // Non-wildcard match
    pub fn is_fallback(&self) -> bool;                        // Has wildcard subscription
    pub fn default_planner() -> Self;
    pub fn default_builder() -> Self;
}
```

### Topic

```rust
pub struct Topic(String);

impl Topic {
    pub fn new(pattern: impl Into<String>) -> Self;
    pub fn as_str(&self) -> &str;
    pub fn matches(&self, other: &Topic) -> bool;
    pub fn matches_str(&self, s: &str) -> bool;  // Zero-allocation variant
}
```

Topic matching uses glob-style patterns:
- `*` as a standalone topic matches everything (wildcard)
- `build.*` matches `build.done`, `build.task`, etc. (segment wildcard)
- `build.done` matches only `build.done` (exact match)

### RobotService Trait

```rust
pub trait RobotService: Send + Sync {
    fn send_question(&self, payload: &str) -> anyhow::Result<i32>;
    fn wait_for_response(&self, events_path: &Path) -> anyhow::Result<Option<String>>;
    fn send_checkin(&self, context: &CheckinContext) -> anyhow::Result<i32>;
    fn timeout_secs(&self) -> u64;
    fn shutdown_flag(&self) -> Arc<AtomicBool>;
    fn stop(&self);
}

pub struct CheckinContext {
    pub current_hat: Option<String>,
    pub open_tasks: usize,
    pub closed_tasks: usize,
    pub cumulative_cost: f64,
}
```

### UxEvent

```rust
pub enum UxEvent {
    TerminalWrite(TerminalWrite),
    TerminalResize(TerminalResize),
    FrameCapture(FrameCapture),
    TuiFrame(TuiFrame),
}
```

UxEvents flow from the PTY executor to the TUI for real-time display.

---

## 5. Error Handling Patterns

Error handling follows a layered strategy: structured error types in library crates, graceful degradation for non-critical failures, and retry with backoff for transient network issues. No circuit breaker patterns are used -- the fresh-context-per-iteration design (Tenet 1) naturally handles recovery.

### Error Type Strategy

The Rust crates use `thiserror`-based custom error enums. Each subsystem defines its own error type with `#[from]` auto-conversions for standard error propagation via `?`.

**Protocol layer** (`ralph-proto`):

```rust
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Invalid topic pattern: {0}")]
    InvalidTopic(String),
    #[error("Hat not found: {0}")]
    HatNotFound(String),
    #[error("Event parse error: {0}")]
    EventParse(String),
    #[error("CLI execution error: {0}")]
    CliExecution(String),
    #[error("Configuration error: {0}")]
    Config(String),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("Loop terminated: {0}")]
    LoopTerminated(String),
}
```

**Core crate error types** (`ralph-core`) -- each subsystem has its own enum:

| Error Type | Location | Key Variants |
|------------|----------|--------------|
| `ConfigError` | `config.rs` | `Io`, `Yaml`, `AmbiguousRouting`, `MutuallyExclusive`, `InvalidCompletionPromise`, `CustomBackendRequiresCommand`, `ReservedTrigger`, `MissingDescription` |
| `LockError` | `loop_lock.rs` | `AlreadyLocked(LockMetadata)`, `Io`, `ParseError`, `UnsupportedPlatform` |
| `WorktreeError` | `worktree.rs` | `Io`, `Git`, `AlreadyExists`, `NotFound`, `NotARepo`, `BranchExists` |
| `MergeQueueError` | `merge_queue.rs` | `Io`, `ParseError`, `NotFound`, `InvalidTransition(id, from, to)`, `UnsupportedPlatform` |
| `RegistryError` | `loop_registry.rs` | `Io`, `ParseError`, `NotFound`, `UnsupportedPlatform` |

**Telegram error type** (`ralph-telegram`):

```rust
#[derive(Debug, thiserror::Error)]
pub enum TelegramError {
    #[error("telegram bot token not found: set RALPH_TELEGRAM_BOT_TOKEN or configure human.telegram.bot_token")]
    MissingBotToken,
    #[error("failed to send telegram message after {attempts} attempts: {reason}")]
    Send { attempts: u32, reason: String },
    #[error("timed out waiting for human response after {timeout_secs}s")]
    ResponseTimeout { timeout_secs: u64 },
    #[error("state persistence error: {0}")]
    State(#[from] std::io::Error),
    #[error("state parse error: {0}")]
    StateParse(#[from] serde_json::Error),
    #[error("event write error: {0}")]
    EventWrite(String),
}
```

Across all crates, there are 25+ files defining `thiserror::Error` types. The pattern is consistent: each file owns its error enum, auto-converts from standard errors with `#[from]`, and includes actionable error messages (ConfigError messages include fix suggestions and documentation links).

### Anyhow Usage

`anyhow` is used sparingly and only at application boundaries (CLI commands, bot entry points, benchmarking tools). The core library crates (`ralph-core`, `ralph-proto`) use specific error types exclusively. This keeps library errors inspectable and matchable while allowing CLI code to use `anyhow::Result` for convenience.

### Consecutive Failure Tracking

The event loop tracks consecutive failures as a termination condition:

```rust
// In LoopState:
pub consecutive_failures: u32,  // Initialized to 0

// After each iteration:
if success {
    self.state.consecutive_failures = 0;    // Reset on success
} else {
    self.state.consecutive_failures += 1;   // Increment on failure
}

// Termination check (default threshold: 5):
if self.state.consecutive_failures >= cfg.max_consecutive_failures {
    return Some(TerminationReason::ConsecutiveFailures);
}
```

The threshold is configurable via `event_loop.max_consecutive_failures` in YAML config. This prevents infinite loops when an agent is consistently failing, while allowing occasional failures to self-recover via fresh context (Tenet 1).

### Retry with Exponential Backoff

Telegram message sending uses a retry function with exponential backoff:

```rust
pub const MAX_SEND_RETRIES: u32 = 3;
pub const BASE_RETRY_DELAY: Duration = Duration::from_secs(1);

pub fn retry_with_backoff<F, S>(mut send_fn: F, mut sleep_fn: S) -> TelegramResult<i32>
where
    F: FnMut(u32) -> TelegramResult<i32>,
    S: FnMut(Duration),
{
    for attempt in 1..=MAX_SEND_RETRIES {
        match send_fn(attempt) {
            Ok(msg_id) => return Ok(msg_id),
            Err(e) => {
                if attempt < MAX_SEND_RETRIES {
                    let delay = BASE_RETRY_DELAY * 2u32.pow(attempt - 1); // 1s, 2s, 4s
                    sleep_fn(delay);
                }
            }
        }
    }
    Err(TelegramError::Send { attempts: MAX_SEND_RETRIES, reason: last_error })
}
```

Delays: 1s, 2s, 4s (3 attempts total). If all attempts fail, the error is returned to the caller. Used by `send_with_retry`, `send_document_with_retry`, and `send_photo_with_retry`.

Backend adapters do **not** retry failed agent executions. Per Tenet 1, each iteration starts with fresh context, so retrying the same prompt would be redundant. Instead, the next iteration naturally retries with a rebuilt prompt.

### Graceful Degradation

Non-critical subsystems fall back to disabled/no-op behavior rather than aborting:

**Diagnostics fallback** -- If diagnostics initialization fails, a disabled collector is used:

```rust
let diagnostics = DiagnosticsCollector::new(Path::new("."))
    .unwrap_or_else(|e| {
        debug!("Failed to initialize diagnostics: {}, using disabled collector", e);
        DiagnosticsCollector::disabled()
    });
```

**Robot timeout fallback** -- When waiting for a human response times out, `Ok(None)` is returned instead of an error, allowing the orchestration loop to continue:

```rust
if Instant::now() >= deadline {
    // Remove pending question, continue without response
    return Ok(None);
}
```

**Check-in error handling** -- Failed robot check-ins are logged but don't halt the loop:

```rust
match robot_service.send_checkin(self.state.iteration, elapsed, Some(&context)) {
    Ok(_) => { self.state.last_checkin_at = Some(Instant::now()); }
    Err(e) => { warn!(error = %e, "Failed to send robot check-in"); }
}
```

### Web Server Error Handling

**tRPC errors** use typed error codes for client consumption:

```typescript
throw new TRPCError({ code: "NOT_FOUND", message: `Task with id '${input.id}' not found` });
throw new TRPCError({ code: "INTERNAL_SERVER_ERROR", message: "Task execution is not configured" });
throw new TRPCError({ code: "BAD_REQUEST", message: result.error || "Failed to enqueue task" });
```

**WebSocket disconnection** is handled gracefully by removing clients from subscription maps on `close` or `error` events. No server-side reconnection -- the client handles reconnect with exponential backoff (1s to 30s max, configured in `useTaskWebSocket`).

**Subprocess lifecycle** uses a two-phase shutdown: SIGTERM first, then SIGKILL after a timeout:

- `RalphRunner`: SIGTERM, then SIGKILL after `gracefulTimeoutMs` (default 5s)
- `ProcessSupervisor`: SIGTERM, then SIGKILL after 5s fixed wait
- `gracefulShutdown` (server): SIGTERM with 30s timeout, SIGINT with 10s timeout

**Database connection** uses WAL mode for concurrent read safety. The `closeDatabase()` function is called during graceful shutdown to ensure clean state.

---

## 6. Backend Adapters

The `ralph-adapters` crate handles spawning and communicating with AI agent CLI tools.

### CliBackend

```rust
pub enum OutputFormat {
    Text,           // Plain text output (most backends)
    StreamJson,     // NDJSON stream (Claude with --output-format stream-json)
}

pub enum PromptMode {
    Arg,            // Pass prompt as CLI argument
    Stdin,          // Write prompt to stdin
}

pub struct CliBackend {
    pub command: String,
    pub args: Vec<String>,
    pub prompt_mode: PromptMode,
    pub prompt_flag: Option<String>,
    pub output_format: OutputFormat,
}
```

### Supported Backends

| Backend | Command | Key Flags | Prompt | Output |
|---------|---------|-----------|--------|--------|
| Claude | `claude` | `--dangerously-skip-permissions --verbose --output-format stream-json` | `-p` flag | StreamJson |
| Kiro | `kiro-cli` | `chat --no-interactive --trust-all-tools` | Positional | Text |
| Gemini | `gemini` | `--yolo` | `-p` flag | Text |
| Codex | `codex` | `exec --yolo` | Positional | Text |
| Amp | `amp` | `--dangerously-allow-all` | `-x` flag | Text |
| Copilot | `copilot` | `--allow-all-tools` | `-p` flag | Text |
| OpenCode | `opencode` | `run` | Positional | Text |
| Custom | user-defined | user-defined | Configurable | Text |

Each backend also has an interactive variant (used by `ralph plan`) that removes autonomous-mode flags and adjusts prompt passing.

### Large Prompt Handling

For Claude specifically, prompts exceeding 7,000 characters are written to a temp file. The CLI is then invoked with `"Please read and execute the task in {path}"` instead.

### CliExecutor (Standard Mode)

```rust
pub struct CliExecutor {
    backend: CliBackend,
}

pub struct ExecutionResult {
    pub output: String,
    pub success: bool,
    pub exit_code: Option<i32>,
    pub timed_out: bool,
}
```

Execution flow:
1. Build command from backend config
2. Spawn process with `Stdio::piped()`
3. Read stdout + stderr concurrently (avoids deadlock)
4. If timeout expires, send SIGTERM
5. Return `ExecutionResult`

### PtyExecutor (Interactive/TUI Mode)

```rust
pub struct PtyConfig {
    pub interactive: bool,
    pub idle_timeout_secs: u32,
    pub cols: u16,
    pub rows: u16,
    pub workspace_root: PathBuf,
}

pub struct PtyExecutionResult {
    pub output: String,
    pub stripped_output: String,
    pub extracted_text: String,
    pub success: bool,
    pub exit_code: Option<i32>,
    pub termination: TerminationType,
}

pub enum TerminationType {
    Natural,
    IdleTimeout,
    UserInterrupt,      // Double Ctrl+C
    ForceKill,          // Ctrl+\
}
```

The PTY executor uses `portable-pty` for cross-platform pseudo-terminal support. It preserves ANSI sequences, tracks idle timeout (on both output and input), handles double Ctrl+C detection, and cleans up raw mode on exit/crash.

### Claude Stream Parser

For Claude's NDJSON streaming output:

```rust
pub struct ClaudeStreamParser;

pub struct ClaudeStreamEvent {
    pub message: Option<AssistantMessage>,
    pub delta: Option<ContentBlockDelta>,
    pub usage: Option<Usage>,
}

pub enum ContentBlock {
    Text(String),
    ToolUse { id: String, name: String, input: serde_json::Value },
}
```

### StreamHandler Trait

```rust
pub trait StreamHandler: Send + Sync {
    fn on_text(&self, text: &str);
    fn on_tool_use(&self, name: &str, input: &serde_json::Value);
    fn on_complete(&self, result: &SessionResult);
}
```

Implementations: `ConsoleStreamHandler` (stdout), `PrettyStreamHandler` (formatted), `QuietStreamHandler` (silent), `TuiStreamHandler` (writes to TUI iteration buffer).

### Auto-Detection

When config specifies `agent: auto`, the `auto_detect` module scans the system PATH for available backends and selects one based on priority order.

---

## 7. Memory System

Memories persist learning across iterations and sessions. Defined in `crates/ralph-core/src/memory.rs` and `memory_store.rs`.

### Memory Types

```rust
pub enum MemoryType {
    Pattern,    // Codebase conventions ("Uses barrel exports in each module")
    Decision,   // Architectural choices ("Chose Postgres over SQLite for writes")
    Fix,        // Recurring solutions ("cargo test hangs: kill orphan postgres")
    Context,    // Project knowledge ("The /legacy folder is deprecated")
}
```

### Memory Struct

```rust
pub struct Memory {
    pub id: String,            // Format: mem-{timestamp}-{4hex}
    pub memory_type: MemoryType,
    pub content: String,
    pub tags: Vec<String>,
    pub created: String,       // YYYY-MM-DD
}
```

### Storage Format

Memories are stored in `.ralph/agent/memories.md` as organized markdown sections:

```markdown
## Patterns
### mem-1737372000-a1b2
> The actual memory content
> Can span multiple lines
<!-- tags: tag1, tag2 | created: 2025-01-20 -->

## Decisions
### mem-1737372001-c3d4
> Chose JSONL over SQLite: simpler, git-friendly, append-only
<!-- tags: storage, architecture | created: 2025-01-20 -->

## Fixes
...

## Context
...
```

### MarkdownMemoryStore

```rust
pub struct MarkdownMemoryStore {
    path: PathBuf,
}

impl MarkdownMemoryStore {
    pub fn new(path: impl AsRef<Path>) -> Self;
    pub fn with_default_path(root: impl AsRef<Path>) -> Self;  // .ralph/agent/memories.md
    pub fn init(&self, force: bool) -> io::Result<()>;
    pub fn load(&self) -> io::Result<Vec<Memory>>;      // Shared file lock
    pub fn append(&self, memory: &Memory) -> io::Result<()>;  // Exclusive file lock
    pub fn delete(&self, id: &str) -> io::Result<bool>;
    pub fn search(&self, pattern: &str) -> io::Result<Vec<Memory>>;
}
```

### Multi-Loop Safety

File locking ensures concurrent access safety:
- `load()` acquires a shared lock (multiple readers allowed)
- `append()` / `delete()` acquire an exclusive lock (single writer)

### Injection Modes

```rust
pub enum InjectMode {
    Auto,     // Ralph injects memories into prompt automatically
    Manual,   // Agent must run `ralph tools memory search`
    None,     // Memories disabled
}
```

With `auto` mode, memories are formatted as markdown and truncated to a token budget before injection into the prompt.

---

## 8. Task System

Tasks track work items across iterations. Defined in `crates/ralph-core/src/task.rs` and `task_store.rs`.

### Task Struct

```rust
pub enum TaskStatus {
    Open,         // Not started
    InProgress,   // Being worked on
    Closed,       // Complete
    Failed,       // Failed/abandoned
}

pub struct Task {
    pub id: String,              // Format: task-{timestamp}-{4hex}
    pub title: String,
    pub description: Option<String>,
    pub status: TaskStatus,
    pub priority: u8,            // 1-5 (1 = highest, default 3)
    pub blocked_by: Vec<String>, // Dependency IDs
    pub loop_id: Option<String>, // For multi-loop filtering
    pub created: String,         // ISO 8601
    pub closed: Option<String>,  // If completed
}

impl Task {
    pub fn is_ready(&self, all_tasks: &[Task]) -> bool;
    // Ready = Open + all blockers are in terminal state (Closed/Failed)
}
```

### TaskStore (JSONL Persistence)

```rust
pub struct TaskStore {
    path: PathBuf,
    tasks: Vec<Task>,
    lock: FileLock,
}

impl TaskStore {
    pub fn load(path: &Path) -> io::Result<Self>;         // Shared lock
    pub fn save(&self) -> io::Result<()>;                  // Exclusive lock
    pub fn reload(&mut self) -> io::Result<()>;
    pub fn with_exclusive_lock<F, T>(&mut self, f: F) -> io::Result<T>;
    pub fn add(&mut self, task: Task);
    pub fn get(&self, id: &str) -> Option<&Task>;
    pub fn open_tasks(&self) -> Vec<&Task>;
    pub fn by_priority(&self) -> Vec<&Task>;
    pub fn ready(&self) -> Vec<&Task>;                     // Unblocked tasks
    pub fn has_pending_tasks(&self) -> bool;               // Excludes terminal states
}
```

Storage format (`.ralph/agent/tasks.jsonl`):
```jsonl
{"id":"task-1234567890-a1b2","title":"Implement auth","status":"open","priority":1,...}
{"id":"task-1234567890-c3d4","title":"Add tests","status":"in_progress","priority":2,...}
```

### CLI Commands

```bash
ralph tools task add "Title" -p 2 -d "description" --blocked-by id1,id2
ralph tools task list [--status open|in_progress|closed] [--format table|json|quiet]
ralph tools task ready                    # Show unblocked tasks
ralph tools task close <task-id>
ralph tools task show <task-id>
```

### Loop Completion

The event loop checks task state for completion. When memories and tasks are enabled (default), the loop terminates when:
- No open tasks remain
- Agent publishes `LOOP_COMPLETE` on consecutive iterations

---

## 9. Parallel Loops

Ralph supports multiple orchestration loops running simultaneously using git worktrees.

### Architecture

```
Primary Loop (holds .ralph/loop.lock)
├── Runs in main workspace
├── Processes merge queue on completion
└── Spawns merge-ralph for queued loops

Worktree Loops (.worktrees/<loop-id>/)
├── Isolated filesystem via git worktree
├── Shared .ralph directory (symlinked)
├── Queue for merge on completion
└── Exit cleanly (no spawn)
```

### Loop Lock

```rust
pub struct LoopLock;
pub struct LockGuard;

pub struct LockMetadata {
    pub pid: u32,
    pub started: DateTime<Utc>,
    pub prompt: String,
}

impl LoopLock {
    const LOCK_FILE: &str = ".ralph/loop.lock";
    pub fn try_acquire(workspace_root: impl AsRef<Path>, prompt: &str)
        -> Result<LockGuard, LockError>;
}
```

Uses `flock()` on `.ralph/loop.lock`. If acquired, this is the primary loop. If `AlreadyLocked`, the process spawns a worktree loop instead.

### Worktree Management

Defined in `crates/ralph-core/src/worktree.rs`:

```rust
pub struct WorktreeConfig {
    pub worktree_dir: PathBuf,    // Default: ".worktrees"
}

pub struct Worktree {
    pub path: PathBuf,
    pub branch: String,
    pub is_main: bool,
    pub head: Option<String>,
}

pub fn create_worktree(
    repo_root: impl AsRef<Path>,
    loop_id: &str,
    config: &WorktreeConfig,
) -> Result<Worktree, WorktreeError>;

pub fn remove_worktree(
    repo_root: impl AsRef<Path>,
    worktree_path: &Path,
) -> Result<(), WorktreeError>;
```

`create_worktree()` creates a git worktree at `.worktrees/{loop_id}/` on branch `ralph/{loop_id}`, then syncs untracked and modified files from the main workspace, preserving directory structure and symlinks.

### Loop Registry

```rust
pub struct LoopEntry {
    pub id: String,              // loop-{timestamp}-{4hex}
    pub pid: u32,
    pub started: DateTime<Utc>,
    pub prompt: String,
    pub worktree_path: Option<String>,
    pub workspace: String,
}

pub struct LoopRegistry;

impl LoopRegistry {
    pub fn register(&self, entry: LoopEntry) -> Result<String>;
    pub fn list(&self) -> Result<Vec<LoopEntry>>;
    pub fn deregister(&self, loop_id: &str) -> Result<()>;
}
```

Stored in `.ralph/loops.json`. Uses PID-based stale detection with automatic cleanup of dead processes.

### Merge Queue (Event-Sourced)

```rust
pub enum MergeEventType {
    Queued { prompt: String },
    Merging { pid: u32 },
    Merged { commit: String },
    NeedsReview { reason: String },
    Discarded { reason: Option<String> },
}

pub enum MergeState {
    Queued, Merging, Merged, NeedsReview, Discarded,
}

pub struct MergeQueue;

impl MergeQueue {
    pub fn enqueue(&self, loop_id: &str, prompt: &str) -> Result<()>;
    pub fn next_pending(&self) -> Result<Option<MergeEntry>>;    // FIFO
    pub fn mark_merging(&self, loop_id: &str, pid: u32) -> Result<()>;
    pub fn mark_merged(&self, loop_id: &str, commit: &str) -> Result<()>;
    pub fn mark_needs_review(&self, loop_id: &str, reason: &str) -> Result<()>;
    pub fn mark_discarded(&self, loop_id: &str, reason: Option<&str>) -> Result<()>;
}
```

State is derived from event history (event sourcing). Stored as append-only JSONL in `.ralph/merge-queue.jsonl`. Uses `flock()` for concurrent access safety. Includes `smart_merge_summary()` and `merge_needs_steering()` for conflict detection.

### Parallel Loop Flow

```
1. Primary loop acquires .ralph/loop.lock
2. If second loop starts, lock fails → AlreadyLocked
3. Second loop creates worktree: .worktrees/{loop_id}/
4. Second loop registers in .ralph/loops.json
5. Both loops run independently (isolated filesystems, shared .git)
6. On completion:
   - Worktree loop adds entry to merge queue
   - Primary loop processes merge queue when idle
   - Merge spawns merge-ralph agent to handle conflicts
```

---

## 10. Configuration System

Configuration lives in `crates/ralph-core/src/config.rs`. YAML-based with v1/v2 compatibility.

### Main Config Structure

```rust
pub struct RalphConfig {
    pub event_loop: EventLoopConfig,
    pub cli: CliConfig,
    pub core: CoreConfig,
    pub hats: HashMap<String, HatConfig>,
    pub events: HashMap<String, EventMetadata>,
    pub adapters: AdaptersConfig,
    pub tui: TuiConfig,
    pub memories: MemoriesConfig,
    pub tasks: TasksConfig,
    pub skills: SkillsConfig,
    pub features: FeaturesConfig,
    pub robot: RobotConfig,

    // V1 compatibility fields (mapped to nested V2 fields):
    pub agent: Option<String>,
    pub prompt_file: Option<String>,
    pub completion_promise: Option<String>,
    pub max_iterations: Option<u32>,
    pub max_runtime: Option<u64>,
    pub max_cost: Option<f64>,
}
```

### EventLoopConfig

```rust
pub struct EventLoopConfig {
    pub prompt: Option<String>,             // Inline prompt
    pub prompt_file: String,                // Default: "PROMPT.md"
    pub completion_promise: String,         // Default: "LOOP_COMPLETE"
    pub max_iterations: u32,                // Default: 100
    pub max_runtime_seconds: u64,           // Default: 14400 (4 hours)
    pub max_cost_usd: Option<f64>,
    pub max_consecutive_failures: u32,      // Default: 5
    pub cooldown_delay_seconds: u64,        // Default: 0
    pub starting_event: Option<String>,     // First event to publish
    pub persistent: bool,                   // Default: false
    pub checkpoint_interval: Option<u32>,
    pub mutation_score_warn_threshold: Option<f64>,
}
```

### CliConfig

```rust
pub struct CliConfig {
    pub backend: String,           // "claude", "kiro", "gemini", "codex", "amp", "copilot", "opencode", "custom"
    pub command: Option<String>,   // Override binary path
    pub prompt_mode: String,       // "arg" or "stdin"
    pub default_mode: String,      // "autonomous" or "interactive"
    pub idle_timeout_secs: u32,    // Default: 30
    pub args: Vec<String>,         // Custom args
    pub prompt_flag: Option<String>,
}
```

### HatConfig

```rust
pub struct HatConfig {
    pub name: String,
    pub description: Option<String>,
    pub triggers: Vec<String>,             // Topics that activate this hat
    pub publishes: Vec<String>,            // Topics this hat can emit
    pub instructions: String,              // Role-specific instructions
    pub extra_instructions: Vec<String>,   // Appended to instructions
    pub backend: Option<HatBackend>,       // Hat-specific backend override
    pub default_publishes: Option<String>, // Fallback event if hat forgets to publish
    pub max_activations: Option<u32>,      // Activation limit
}

pub enum HatBackend {
    Named(String),                         // "claude", "kiro", etc.
    NamedWithArgs { backend_type: String, args: Vec<String> },
    KiroAgent { backend_type: String, agent: String, args: Vec<String> },
    Custom { command: String, args: Vec<String> },
}
```

### Preset System

14 YAML preset files in `presets/`:

| Preset | Description | Hat Topology |
|--------|-------------|--------------|
| `feature.yml` | Feature development with code review | Builder + Reviewer |
| `bugfix.yml` | Bug fixing workflow | Builder + Reviewer |
| `debug.yml` | Debugging workflow | Debugger |
| `deploy.yml` | Deployment workflow | Deployer |
| `docs.yml` | Documentation writing | Writer + Doc Reviewer |
| `refactor.yml` | Code refactoring | Builder + Reviewer |
| `research.yml` | Research workflow | Researcher |
| `review.yml` | Code review only | Reviewer |
| `pr-review.yml` | Pull request review | PR Reviewer |
| `spec-driven.yml` | Spec-first development | Planner + Builder + Reviewer |
| `code-assist.yml` | Code assistance | Code Assist |
| `hatless-baseline.yml` | No hats (HatlessRalph only) | None |
| `gap-analysis.yml` | Gap analysis | Analyst |
| `pdd-to-code-assist.yml` | PDD pipeline | PDD + Code Assist |

### Example Preset (feature.yml)

```yaml
event_loop:
  prompt_file: "PROMPT.md"
  completion_promise: "LOOP_COMPLETE"
  max_iterations: 100
  max_runtime_seconds: 14400
  checkpoint_interval: 5

cli:
  backend: "claude"

core:
  specs_dir: "./specs/"

hats:
  builder:
    name: "Builder"
    description: "Implements one task with quality gates."
    triggers: ["build.task"]
    publishes: ["build.done", "build.blocked"]
    default_publishes: "build.done"
    instructions: |
      ## BUILDER MODE
      You're building, not planning. One task, then exit.
      ...

  reviewer:
    name: "Reviewer"
    description: "Reviews implementation for quality."
    triggers: ["review.request"]
    publishes: ["review.approved", "review.changes_requested"]
    default_publishes: "review.approved"
    instructions: |
      ## REVIEWER MODE
      Review the most recent implementation for quality.
      ...
```

### Config Loading

```rust
impl RalphConfig {
    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, ConfigError>;
    pub fn parse_yaml(content: &str) -> Result<Self, ConfigError>;
    pub fn normalize(&mut self);      // Map V1 fields to V2
    pub fn validate(&self) -> Result<Vec<ConfigWarning>, ConfigError>;
}
```

---

## 11. Human-in-the-Loop (RObot)

Ralph supports human interaction during orchestration via Telegram. The implementation spans `crates/ralph-telegram/`.

### Configuration

```yaml
RObot:
  enabled: true
  timeout_seconds: 300
  checkin_interval_seconds: 60
  telegram:
    bot_token: "your-token"  # Or env: RALPH_TELEGRAM_BOT_TOKEN
```

### Event Flow

| Event | Direction | Purpose |
|-------|-----------|---------|
| `human.interact` | Agent -> Human | Agent asks question; loop blocks |
| `human.response` | Human -> Agent | Reply to a pending question |
| `human.guidance` | Human -> Agent | Proactive guidance injected into prompt |

### TelegramService

```rust
pub struct TelegramService {
    workspace_root: PathBuf,
    bot_token: String,
    timeout_secs: u64,
    loop_id: String,
    state_manager: StateManager,
    handler: MessageHandler,
    bot: TelegramBot,
    shutdown: Arc<AtomicBool>,
}

impl TelegramService {
    pub async fn send_question(&self, payload: &str) -> TelegramResult<i32>;
    pub async fn wait_for_response(&self, events_path: &Path) -> TelegramResult<Option<String>>;
    pub async fn send_checkin(&self, context: &CheckinContext) -> TelegramResult<i32>;
}
```

### State Persistence

```rust
pub struct TelegramState {
    pub chat_id: Option<i64>,
    pub pending_questions: Vec<PendingQuestion>,
}
```

Stored in `.ralph/telegram-state.json`. Tracks chat ID registration and pending question routing.

### Message Handler

```rust
pub struct MessageHandler;

impl MessageHandler {
    pub fn process_message(&self, chat_id: i64, text: &str) -> TelegramResult<()>;
}
```

Processing rules:
- `/start` -> Register chat_id
- `/restart` -> Emit restart event
- Reply to pending question -> Emit `human.response` event
- Unprompted message -> Emit `human.guidance` event (squashed into numbered list in prompt)

### Multi-Loop Routing

Messages are routed to the correct loop via:
1. Reply-to the original question message
2. `@loop-id` prefix in message text
3. Default to primary loop

### Retry Strategy

Send failures use exponential backoff: 1s, 2s, 4s (3 attempts max). If all fail, treated as timeout.

The Telegram bot starts only on the primary loop (the one holding `.ralph/loop.lock`).

---

## 12. Terminal UI

The TUI is a ratatui-based observation dashboard in `crates/ralph-tui/`.

### Architecture

```rust
pub struct Tui {
    state: Arc<Mutex<TuiState>>,
    terminated_rx: Option<watch::Receiver<bool>>,
    interrupt_tx: Option<watch::Sender<bool>>,
}

impl Tui {
    pub fn new() -> Self;
    pub fn with_hat_map(self, hat_map: HashMap<String, (HatId, String)>) -> Self;
    pub fn with_termination_signal(mut self, rx: watch::Receiver<bool>) -> Self;
    pub fn with_interrupt_tx(mut self, tx: watch::Sender<bool>) -> Self;
    pub fn observer(&self) -> impl Fn(&Event) + Send + 'static;
    pub async fn run(self) -> Result<()>;
}
```

### TuiState

```rust
pub struct TuiState {
    pub pending_hat: Option<(HatId, String)>,
    pub iteration: u32,
    pub loop_started: Option<Instant>,
    pub iteration_started: Option<Instant>,
    pub last_event: Option<String>,
    pub max_iterations: Option<u32>,
    pub hat_map: HashMap<String, (HatId, String)>,
    pub iterations: Vec<IterationBuffer>,      // Per-iteration content buffers
    pub current_view: usize,                   // Which iteration is being viewed
    pub following_latest: bool,                // Auto-follow new iterations
    pub new_iteration_alert: Option<usize>,    // Alert when viewing history
    pub search_state: SearchState,             // In-content search
    pub loop_completed: bool,                  // Timer freeze on completion
    pub task_counts: TaskCounts,               // Aggregate task stats
    pub active_task: Option<TaskSummary>,
}
```

### IterationBuffer

Each iteration has its own content buffer with independent scroll state:

```rust
pub struct IterationBuffer {
    pub number: u32,
    pub lines: Arc<Mutex<Vec<Line<'static>>>>,   // Shared for real-time streaming
    pub scroll_offset: usize,
    pub following_bottom: bool,                   // Auto-scroll
    pub hat_display: Option<String>,
    pub backend: Option<String>,
    pub started_at: Option<Instant>,
    pub elapsed: Option<Duration>,
}
```

The `lines` field is `Arc<Mutex<>>` to allow stream handlers to write directly during execution, enabling real-time streaming to the TUI.

### Widget Layout

```
┌─────────────────────────────────────────────────────┐
│ Header: hat | iteration N/M | elapsed | status      │
├─────────────────────────────────────────────────────┤
│                                                     │
│ Content: Agent output (iteration-scoped, scrollable)│
│                                                     │
├─────────────────────────────────────────────────────┤
│ Footer: tasks progress | keybinds | search          │
└─────────────────────────────────────────────────────┘
```

### Key Bindings

- `j/k` or arrows: Scroll up/down within iteration
- `h/l` or `[/]`: Navigate between iterations
- `g/G`: Jump to top/bottom
- `/`: Start search, `n/N`: Next/previous match
- `?`: Toggle help overlay
- `Ctrl+C`: Interrupt (double-tap to force)

### Integration

1. EventLoop registers TUI as observer on EventBus
2. TUI receives events and updates state (hat display, iteration tracking)
3. Stream handler writes agent output to latest iteration buffer
4. TUI detects Ctrl+C in raw mode and signals main loop via interrupt channel

---

## 13. Web Dashboard Backend

The backend is a Fastify + tRPC server in `backend/ralph-web-server/`.

### Technology Stack

- **Runtime**: Node.js 22 with TypeScript
- **Server**: Fastify
- **API**: tRPC (type-safe RPC)
- **Database**: SQLite via better-sqlite3 + Drizzle ORM
- **WebSocket**: Native Fastify WebSocket for log streaming

### Database Schema

```sql
-- Tasks table
tasks (id TEXT PK, title TEXT, description TEXT, status TEXT, priority INT,
       prompt TEXT, preset TEXT, autoExecute BOOL, archived BOOL,
       errorMessage TEXT, executionSummary TEXT, exitCode INT, durationMs INT,
       createdAt TEXT, updatedAt TEXT)

-- Task execution logs
task_logs (id INTEGER PK, taskId TEXT FK, line TEXT, timestamp TEXT, source TEXT)

-- Queued tasks for dispatcher
queued_tasks (id TEXT PK, taskType TEXT, payload TEXT, priority INT,
             state TEXT, result TEXT, error TEXT, createdAt TEXT, updatedAt TEXT)

-- Hat collections (visual builder)
collections (id TEXT PK, name TEXT, description TEXT, graphData TEXT,
            createdAt TEXT, updatedAt TEXT)
```

### tRPC Routers

All routers are defined in a single `trpc.ts` file as separate sub-routers merged into `appRouter`:

| Router | Endpoints | Purpose |
|--------|-----------|---------|
| `task` | create, list, get, update, delete, close, archive, run, retry, cancel | Task CRUD + execution |
| `hat` | list, get, update | Hat definition management |
| `loops` | list, merge, retry, discard, stop, diff, getMergeButtonState | Parallel loop management |
| `collection` | create, list, get, update, delete, exportYaml | Hat collection management |
| `config` | get, update | Configuration management |
| `presets` | list, get | Preset browsing |
| `planning` | list, start, submitResponse, getSession, getArtifact | Planning Q&A sessions (inline router) |

### Request Routing

```
GET/POST /trpc/*  → tRPC JSON-RPC
POST /api/v1/*    → REST API
WS /ws/logs       → WebSocket log streaming
```

### WebSocket Protocol

```typescript
// Client -> Server
{ type: "subscribe" | "unsubscribe", taskId: string, sinceId?: number }

// Server -> Client
{ type: "log" | "status" | "error" | "event", taskId: string, data: ..., timestamp: ISO8601 }
```

### LogBroadcaster

Singleton WebSocket message broker:
- Manages client subscriptions per task
- Automatic backlog replay on subscribe (from `sinceId`)
- Broadcasts log entries, status updates, errors, and events

### Task Execution Pipeline

```
User creates task (tRPC) → TaskRepository.create() → DB
    ↓
TaskBridge.enqueueTask() → TaskQueueService.enqueue() → queued_tasks
    ↓
Dispatcher polls queue (100ms) → Dequeues task
    ↓
RalphRunnerHandler.execute() → RalphRunner spawns subprocess
    ↓
LogStream captures stdout/stderr → LogBroadcaster → WebSocket clients
    ↓
On completion: TaskBridge syncs result to DB (status, exit code, duration, summary)
```

### RalphRunner

```typescript
interface RalphRunnerOptions {
    command?: string               // Default: "ralph"
    baseArgs?: string[]            // Default: ["run"]
    cwd?: string
    env?: Record<string, string>
    gracefulTimeoutMs?: number     // 5000ms before SIGKILL
    onOutput?: LogCallback
    supervisor?: ProcessSupervisor
    taskId?: string
}
```

### Dispatcher

Task execution engine with configurable concurrency:
- Polls queue every `pollIntervalMs` (default 100ms)
- `maxConcurrent` tasks (1-10, default 3)
- State machine: PENDING -> RUNNING -> COMPLETED/FAILED
- Graceful shutdown: waits for running tasks (30s SIGTERM, 10s SIGINT)

### ProcessSupervisor

Tracks detached processes for crash recovery:
- Stores state in `~/.ralph/web/runs/{taskId}/`
- Enables reconnection to running processes on server restart
- Reads `status.json`, `stdout.log`, `stderr.log`

### ConfigMerger

Merges base `ralph.yml` with preset hat configurations:
- `"default"` -> Use base config unchanged
- `"builtin:name"` -> Load from `presets/{name}.yml`
- `"directory:name"` -> Load from `.ralph/hats/{name}.yml`
- UUID -> Export from CollectionService
- Merging: Keeps all base settings, replaces only hats and events sections

### LoopsManager

Periodic merge queue processing (every 30s):
- Spawns `ralph loops process` to merge completed worktree branches
- Spawns `ralph loops list --json` for status updates
- Spawns `ralph loops prune` to clean stale loops

### PlanningService

Interactive planning sessions with Q&A:
1. `startSession(prompt)` -> Spawns `ralph run` with planning preset
2. `submitResponse(sessionId, promptId, response)` -> Writes to conversation JSONL
3. `resumeSession(id)` -> Unpauses process
4. `getArtifact(sessionId, filename)` -> Reads generated spec files

Session state stored in `.ralph/planning-sessions/{sessionId}/`.

### Startup Sequence

1. Initialize SQLite database (create tables if needed)
2. Configure LogBroadcaster with log repository
3. Create task queue + event bus
4. Create dispatcher with handlers
5. Create TaskBridge (DB <-> queue bridge)
6. Create LoopsManager
7. Create PlanningService
8. Start Fastify server
9. Hydrate queue from database (restore pending tasks)
10. Reconnect to running processes (Phase 5 recovery)
11. Start dispatcher polling loop
12. Start LoopsManager timer

---

## 14. Web Dashboard Frontend

The frontend is a React + Vite SPA in `frontend/ralph-web/`.

### Technology Stack

- **Framework**: React 19.1.0 + TypeScript 5.9
- **Build**: Vite 7.0.0
- **Styling**: TailwindCSS 4.1
- **State**: Zustand 5.0 (logs + UI) + React Query 5.80 (server state)
- **API**: tRPC React Query integration
- **Routing**: React Router 7.13
- **Visual Builder**: React Flow 12.10
- **UI Primitives**: Radix UI

### Application Routes

```
/tasks          → TasksPage (task list + creation)
/tasks/:id      → TaskDetailPage (detail + logs + actions)
/builder        → BuilderPage (visual hat collection editor)
/plan           → PlanPage (planning Q&A sessions)
/settings       → SettingsPage (hat definitions + config)
```

### Layout

```
AppShell
├── Sidebar (fixed left, navigation with active highlighting)
└── <Outlet> (active route page)
```

### State Management

**useUIStore** (Zustand, persisted to localStorage):
```typescript
{
    sidebarOpen: boolean,
    expandedTasks: Set<string>,
    toggleSidebar(): void,
    toggleTaskExpanded(taskId: string): void,
}
```

**useLogStore** (Zustand, in-memory):
```typescript
{
    taskLogs: Record<string, LogEntry[]>,     // Persistent across component unmount
    taskLogMeta: Record<string, { lastId?: number }>,
    appendLog(taskId, entry): void,
    appendLogs(taskId, entries): void,        // Batch (50ms debounce)
    clearLogs(taskId): void,
    getLogs(taskId): LogEntry[],
    getLastLogId(taskId): number | null,
}
```

Zustand is used for logs because they must survive component unmount (task card collapse/expand) and enable real-time updates across multiple viewers.

### tRPC Client

```typescript
const trpc = createTRPCReact<AppRouter>();

// Vite proxy configuration:
proxy: {
    "/trpc": { target: "http://localhost:3000", changeOrigin: true },
    "/ws": { target: "http://localhost:3000", ws: true, changeOrigin: true },
}
```

### WebSocket Hook (useTaskWebSocket)

```typescript
interface UseTaskWebSocketReturn {
    entries: LogEntry[];
    latestEntry: LogEntry | null;
    events: RalphEvent[];
    connectionState: "connecting" | "connected" | "disconnected" | "error";
    taskStatus: string;
    connect(): void;
    disconnect(): void;
    clearEntries(): void;
}
```

Features:
- Auto-connect when taskId is set
- Exponential backoff reconnection (1s -> 30s max)
- Backlog replay from sinceId (resume from last seen)
- 50ms batch buffering to reduce renders
- Stores logs in Zustand for persistence

### Key Components

**Task Components**:
- `TaskInput` -- Form to create new tasks with prompt
- `ThreadList` -- Displays all tasks with 5s polling
- `TaskThread` -- Collapsible task card (header + metadata + log viewer)
- `EnhancedLogViewer` -- Real-time log display with line numbers, color coding (stdout=green, stderr=red), filter toggles, auto-scroll, copy buttons
- `LiveStatus` -- Task execution status indicator
- `LoopActions` -- Merge, Retry, Discard, Stop buttons

**Builder Components** (React Flow):
- `CollectionBuilder` -- Main canvas (drag, zoom, pan, minimap)
- `HatNode` -- Custom node type displaying hat name/description
- `HatPalette` -- Draggable hat definitions list
- `PropertiesPanel` -- Edit selected node properties
- `OffsetEdge` -- Custom curved edge with label

**Planning Components**:
- `PlanLanding` -- Start new session, view previous sessions
- `PlanSession` -- Active Q&A interface with artifact viewer

### Visual Hat Builder Flow

```
User drags hat nodes onto canvas → connects with edges (event flow)
    ↓
Save: trpc.collection.update({graph: {nodes, edges}})
    ↓
Export: CollectionService derives triggers/publishes from edges
    ↓
Generates Ralph-compatible YAML preset
    ↓
Use with: ralph run -c exported-preset.yml
```

---

## 15. Testing Strategy

### Unit Tests (Rust)

```bash
cargo test                                   # All tests
cargo test -p ralph-core test_name           # Single test
cargo test -p ralph-core smoke_runner        # Smoke tests
```

Each crate contains `#[cfg(test)] mod tests` with extensive coverage. Key test areas:
- Event routing and topic matching (`ralph-proto`)
- Hat selection priority (`ralph-proto`)
- Config parsing and validation (`ralph-core`)
- Memory CRUD and file locking (`ralph-core`)
- Task lifecycle and dependency resolution (`ralph-core`)
- Merge queue state transitions (`ralph-core`)
- Backend command construction (`ralph-adapters`)
- TUI state management (`ralph-tui`)

### Smoke Tests (Replay-Based)

Smoke tests use recorded JSONL fixtures instead of live API calls:

```bash
cargo test -p ralph-core smoke_runner        # All smoke tests
cargo test -p ralph-core kiro                # Backend-specific
```

Fixtures in `crates/ralph-core/tests/fixtures/`. Record new fixtures:
```bash
ralph run -c config.yml --record-session session.jsonl -p "your prompt"
```

### E2E Testing Framework

The `ralph-e2e` crate provides a comprehensive end-to-end test harness.

```bash
cargo run -p ralph-e2e -- claude             # Live API tests
cargo run -p ralph-e2e -- --mock             # CI-safe mock mode
cargo run -p ralph-e2e -- --mock --filter connect  # Filter scenarios
cargo run -p ralph-e2e -- --list             # List scenarios
```

**Scenario Tiers**:

| Tier | Category | Scenarios |
|------|----------|-----------|
| 1 | Connectivity | `ConnectivityScenario` |
| 2 | Orchestration | `SingleIterScenario`, `MultiIterScenario`, `CompletionScenario` |
| 3 | Events | `EventsScenario`, `BackpressureScenario` |
| 4 | Capabilities | `StreamingScenario`, `ToolUseScenario` |
| 5 | Hats | `HatSingleScenario`, `HatEventRoutingScenario`, `HatInstructionsScenario`, `HatMultiWorkflowScenario`, `HatBackendOverrideScenario` |
| 6 | Memory | `MemoryAddScenario`, `MemorySearchScenario`, `MemoryInjectionScenario`, `MemoryPersistenceScenario`, `MemoryCorruptedFileScenario`, `MemoryMissingFileScenario`, `MemoryLargeContentScenario`, `MemoryRapidWriteScenario` |
| 6 | Tasks | `TaskAddScenario`, `TaskCloseScenario`, `TaskReadyScenario`, `TaskCompletionScenario` |
| 7 | Incremental | `ChainedLoopScenario`, `IncrementalFeatureScenario` |
| 8 | Errors | `AuthFailureScenario`, `BackendUnavailableScenario`, `MaxIterationsScenario`, `TimeoutScenario` |

**TestScenario Trait**:
```rust
#[async_trait]
pub trait TestScenario: Send + Sync {
    fn id(&self) -> &str;
    fn description(&self) -> &str;
    fn tier(&self) -> &str;
    fn supported_backends(&self) -> Vec<Backend>;
    fn setup(&self, workspace: &Path, backend: Backend) -> Result<ScenarioConfig>;
    async fn run(&self, executor: &RalphExecutor, config: &ScenarioConfig) -> Result<TestResult>;
    fn cleanup(&self, workspace: &Path) -> Result<()>;
}
```

Reports generated in `.e2e-tests/` as markdown and JSON.

### Web Tests

```bash
npm run test:server             # Backend tests (Vitest)
npm run test -w @ralph-web/dashboard  # Frontend tests (Vitest + jsdom)
```

### Quality Gates

The CI pipeline enforces:
1. `cargo test` -- All Rust unit tests pass
2. `cargo clippy --all-targets --all-features -- -D warnings` -- No lint warnings
3. `cargo fmt --all -- --check` -- Consistent formatting
4. E2E mock tests -- Deterministic replay scenarios
5. `npm run build` -- Web dashboard builds successfully
6. `npm run test:server` -- Backend tests pass
7. `cargo package -p ralph-cli --allow-dirty --list` -- Package check for embedded files

---

## 16. CI/CD & Distribution

### GitHub Actions Workflows

**CI** (`.github/workflows/ci.yml`):
- Triggers: push to main, pull requests to main
- Jobs: check-embedded-files -> test + clippy + fmt (parallel) -> e2e-mock + web-tests + package-check
- Ubuntu-latest runners with system deps (`libdbus-1-dev`, `pkg-config`)

**Release** (`.github/workflows/release.yml`):
- Triggers: version tags matching `**[0-9]+.[0-9]+.[0-9]+*`
- Uses cargo-dist v0.30.3 for multi-platform binary builds
- Targets: `aarch64-apple-darwin`, `x86_64-apple-darwin`, `aarch64-unknown-linux-gnu`, `x86_64-unknown-linux-gnu`
- Windows excluded (requires Unix PTY and signal handling)
- Installers: shell script, npm package
- Publishes to npm as `@ralph-orchestrator/ralph-cli`

**Docs** (`.github/workflows/docs.yml`):
- MkDocs documentation site deployment

### Distribution Channels

1. **GitHub Releases** -- Multi-platform binaries via cargo-dist
2. **npm** -- `@ralph-orchestrator/ralph-cli` package
3. **Cargo** -- `cargo install ralph-cli`
4. **Homebrew** -- Via homebrew tap

### Docker

```yaml
# docker-compose.yml services:
ralph-orchestrator:     # Main app container
redis:                  # Caching & state (port 6379)
postgres:               # Optional persistent DB (profile: with-db)
prometheus:             # Metrics collection (profile: monitoring)
grafana:                # Visualization (port 3000, profile: monitoring)
docs:                   # MkDocs (port 8000, profile: development)
```

Multi-stage Dockerfile:
1. Build stage: Python 3.11-slim + uv package manager
2. Runtime stage: Python 3.11-slim + Node.js + npm, non-root user

### Build Profile

```toml
[profile.dist]
inherits = "release"
lto = "thin"
```

---

## 17. Recreating the System

This section provides a step-by-step guide for recreating the Ralph Orchestrator through agentic coding.

### Phase 1: Protocol Layer (`ralph-proto`)

Create the foundational types first since all other crates depend on them.

1. **Topic** -- Glob-style pattern matching with `*` wildcard support
2. **Event** -- Struct with topic, payload, source, target
3. **Hat** -- Struct with id, name, description, subscriptions, publishes, instructions
4. **HatId** -- Newtype wrapper over String
5. **EventBus** -- HashMap-based pub/sub with specific > wildcard priority routing
6. **RobotService** -- Async trait for human-in-the-loop integration
7. **UxEvent** -- Terminal event types for TUI integration
8. **DaemonAdapter** -- Trait for daemon/loop management

### Phase 2: Core Engine (`ralph-core`)

Build the orchestration engine.

1. **Config** (`config.rs`) -- YAML parsing with serde, v1/v2 compatibility, validation
2. **Memory** (`memory.rs`, `memory_store.rs`) -- Memory types, markdown persistence, file locking
3. **Task** (`task.rs`, `task_store.rs`) -- Task types, JSONL persistence, dependency resolution
4. **HatlessRalph** (`hatless_ralph.rs`) -- Prompt builder with section composition, hat topology
5. **EventLoop** (`event_loop/mod.rs`) -- Main iteration cycle, hat selection, termination conditions
6. **Worktree** (`worktree.rs`) -- Git worktree creation/removal, file syncing
7. **Loop Registry** (`loop_registry.rs`) -- JSON persistence, PID-based stale detection
8. **Merge Queue** (`merge_queue.rs`) -- Event-sourced JSONL, state machine, file locking
9. **Skill Registry** -- Skill loading and index generation
10. **Event Reader** -- JSONL event parsing with timestamp tracking

### Phase 3: Backend Adapters (`ralph-adapters`)

Implement the agent execution layer.

1. **CliBackend** (`cli_backend.rs`) -- Backend definitions for all 7+ agents, command building
2. **CliExecutor** (`cli_executor.rs`) -- Standard subprocess execution with timeout
3. **PtyExecutor** (`pty_executor.rs`) -- PTY-based execution with portable-pty, idle detection
4. **ClaudeStreamParser** (`claude_stream.rs`) -- NDJSON parsing for Claude's stream-json output
5. **StreamHandler** (`stream_handler.rs`) -- Console, Pretty, Quiet, and TUI stream handlers
6. **AutoDetect** (`auto_detect.rs`) -- PATH scanning for available backends

### Phase 4: CLI (`ralph-cli`)

Wire up the command-line interface.

1. **run** -- Main orchestration command (config loading, loop lock, event loop execution)
2. **plan** -- Interactive planning sessions with backends
3. **task** -- Code task management (`.code-task.md` files)
4. **loops** -- Parallel loop management (list, merge, stop, prune, attach, diff)
5. **memory** -- Memory CRUD commands
6. **web** -- Launch web dashboard (backend + frontend)
7. **init** -- Initialize `.ralph/` directory structure
8. **tools** -- Runtime tools (task, memory, interact, skill, emit)
9. **events** -- Event history and inspection
10. **hats** -- Hat configuration inspection

### Phase 5: Telegram Integration (`ralph-telegram`)

Implement human-in-the-loop.

1. **StateManager** -- JSON persistence for chat ID and pending questions
2. **TelegramBot** -- teloxide-based bot with command/message handlers
3. **MessageHandler** -- Event dispatch (human.response, human.guidance, restart)
4. **TelegramService** -- RobotService implementation with retry/backoff

### Phase 6: Terminal UI (`ralph-tui`)

Build the observation dashboard.

1. **TuiState** -- Observable state with iteration buffers, search, task tracking
2. **IterationBuffer** -- Per-iteration content with shared lines (Arc<Mutex<>>)
3. **App** -- Main ratatui loop with crossterm events
4. **Widgets** -- Header (hat + iteration + timer), Content (scrollable output), Footer (tasks + keybinds)
5. **Input** -- Key binding dispatch (scroll, navigate, search, help, interrupt)

### Phase 7: Web Dashboard Backend

Build the server.

1. **Database** -- SQLite schema with Drizzle ORM migrations
2. **Repositories** -- Task, TaskLog, QueuedTask, Collection CRUD
3. **Queue** -- TaskQueueService (in-memory + persistent), EventBus, Dispatcher
4. **Runner** -- RalphRunner (subprocess), LogStream, ProcessSupervisor, FileOutputStreamer
5. **Services** -- TaskBridge, ConfigMerger, LoopsManager, PlanningService, CollectionService
6. **API** -- tRPC routers (task, collection, settings, plan, loops, logs)
7. **WebSocket** -- LogBroadcaster for real-time log streaming
8. **Server** -- Fastify setup, startup sequence, graceful shutdown

### Phase 8: Web Dashboard Frontend

Build the React SPA.

1. **tRPC Client** -- Type-safe API client with React Query
2. **Zustand Stores** -- useUIStore (persisted), useLogStore (in-memory)
3. **WebSocket Hook** -- useTaskWebSocket with reconnection and batch buffering
4. **Layout** -- AppShell + Sidebar + routing
5. **Task Pages** -- TasksPage (list + create), TaskDetailPage (detail + logs)
6. **Builder Page** -- CollectionBuilder with React Flow (nodes, edges, palette, properties)
7. **Plan Page** -- PlanLanding + PlanSession (Q&A interface)
8. **Settings Page** -- Hat definitions + config management
9. **UI Components** -- Button, Card, Badge, Input, Textarea (Radix UI + Tailwind)

### Phase 9: Presets & Configuration

Create the preset library.

1. Write 14 YAML presets covering common workflows
2. Each preset defines hats with triggers, publishes, instructions
3. Ensure presets compose cleanly with the ConfigMerger

### Phase 10: Testing & CI/CD

Set up the quality pipeline.

1. **Unit Tests** -- Comprehensive tests for each crate
2. **Smoke Tests** -- JSONL replay fixtures for deterministic testing
3. **E2E Framework** -- TestScenario trait with tiered scenarios and mock mode
4. **CI Pipeline** -- GitHub Actions for test, clippy, fmt, e2e-mock, web-tests
5. **Release Pipeline** -- cargo-dist for multi-platform builds, npm publishing
6. **Docker** -- Multi-stage Dockerfile + docker-compose with supporting services

### Phase 11: Infrastructure

Final integration.

1. **Scripts** -- setup-hooks.sh, sync-embedded-files.sh
2. **Documentation** -- MkDocs site, CLAUDE.md project guide
3. **Diagnostics** -- JSONL logging (agent-output, orchestration, errors)
4. **Skills** -- Skill definitions and loading system

### Critical Implementation Notes

- **File locking is essential** -- Memory, task, merge queue, and loop registry all use `flock()` for concurrent access safety across parallel loops
- **Events are append-only** -- Never modify existing JSONL entries
- **HatlessRalph is always registered** -- Universal fallback ensures no orphaned events
- **Fresh context per iteration** -- Prompts are rebuilt from scratch each cycle
- **Large prompts use temp files** -- Claude backend writes prompts >7K chars to temp files
- **PTY cleanup is critical** -- Raw terminal mode must be restored on crash/exit
- **Backpressure over prescription** -- Quality gates (tests, lint, typecheck) enforce correctness
- **Workspace edition 2024** -- Uses latest Rust edition features
- **unsafe_code = "forbid"** -- No unsafe code allowed in the workspace
