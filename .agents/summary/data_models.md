# Data Models

## Core Types (ralph-proto)

### Event
```rust
pub struct Event {
    pub topic: Topic,        // Routing topic (pattern-matchable)
    pub payload: String,     // Content/payload
    pub source: Option<HatId>,  // Publishing hat
    pub target: Option<HatId>,  // Direct handoff target
}
```

### Hat
```rust
pub struct Hat {
    pub id: HatId,                    // Unique identifier
    pub name: String,                 // Human-readable name
    pub description: String,          // Purpose description
    pub subscriptions: Vec<Topic>,    // Topics this hat listens to
    pub publishes: Vec<Topic>,        // Topics this hat emits
    pub instructions: String,         // Prompt instructions
}
```

### JSON-RPC Protocol
```rust
pub enum RpcCommand { ... }  // Commands from TUI/API to runtime
pub enum RpcEvent { ... }    // Events from runtime to TUI/API
pub struct RpcState { ... }  // Full loop state snapshot
pub struct RpcIterationInfo { ... }  // Per-iteration metadata
pub struct RpcTaskCounts { ... }     // Task status counts
```

### UX Events
```rust
pub enum UxEvent {
    TerminalWrite(TerminalWrite),   // Raw terminal output
    TerminalResize(TerminalResize), // Terminal size change
    FrameCapture(FrameCapture),     // TUI frame snapshot
}
```

## Configuration (ralph-core)

### RalphConfig (top-level)
```rust
pub struct RalphConfig {
    pub event_loop: EventLoopConfig,  // max_iterations, max_runtime, completion_promise, starting_event
    pub cli: CliConfig,               // backend, custom_command, args, prompt_mode, output_format
    pub core: CoreConfig,             // specs_dir, guardrails, scratchpad settings
    pub hats: HashMap<String, HatConfig>,  // Hat definitions
    pub events: HashMap<String, EventMetadata>,  // Event topic metadata
    pub backpressure: BackpressureConfig,  // Gates (fmt, clippy, test)
    pub hooks: HooksConfig,           // Lifecycle hooks
    pub skills: SkillsConfig,         // Skill directories and overrides
    pub memories: MemoriesConfig,     // Memory system config
    pub features: FeaturesConfig,     // Feature flags (loop naming, etc.)
    pub RObot: RobotConfig,           // Human-in-the-loop config
    // v1 compatibility fields...
}
```

### HatConfig
```rust
pub struct HatConfig {
    pub name: String,
    pub description: String,
    pub triggers: Vec<String>,        // Subscribe topics
    pub publishes: Vec<String>,       // Publish topics
    pub default_publishes: Option<String>,
    pub instructions: String,         // Prompt text
    pub backend: Option<HatBackend>,  // Per-hat backend override
    pub inject: Option<InjectMode>,   // How to inject instructions
    pub disallowed_tools: Vec<String>,
}
```

## State Models (ralph-core)

### Task
```rust
pub struct Task {
    pub id: String,                   // task-{unix_timestamp}-{4_hex}
    pub title: String,
    pub description: Option<String>,
    pub key: Option<String>,          // Idempotent key
    pub status: TaskStatus,           // Open, InProgress, Closed, Failed
    pub priority: u8,                 // 1-5 (1 = highest)
    pub blocked_by: Vec<String>,      // Dependency IDs
    pub loop_id: Option<String>,      // Owning loop
    pub tags: Vec<String>,
    pub created_at: String,           // ISO 8601
    pub updated_at: String,
}

pub enum TaskStatus { Open, InProgress, Closed, Failed }
```

### Memory
```rust
pub struct Memory {
    pub content: String,
    pub memory_type: MemoryType,      // Pattern, Decision, Fix, Context
    pub source: Option<String>,       // Where it came from
    pub created_at: String,
}

pub enum MemoryType { Pattern, Decision, Fix, Context }
```

### LoopState
```rust
pub struct LoopState {
    pub iteration: u32,
    pub total_cost_usd: f64,
    pub consecutive_failures: u32,
    pub start_time: Instant,
    pub last_event_topic: Option<String>,
    pub consecutive_same_topic: u32,
}
```

### TerminationReason
```rust
pub enum TerminationReason {
    CompletionPromise,    // exit 0
    MaxIterations,        // exit 2
    MaxRuntime,           // exit 2
    MaxCost,              // exit 2
    ConsecutiveFailures,  // exit 1
    LoopThrashing,        // exit 1
    LoopStale,            // exit 1
    ValidationFailure,    // exit 1
    Stopped,              // exit 1
    Interrupted,          // exit 130
    RestartRequested,     // exit 3
    WorkspaceGone,        // exit 1
    Cancelled,            // exit 0
}
```

### Skill
```rust
pub struct SkillEntry {
    pub name: String,
    pub description: String,
    pub content: String,          // Markdown body (frontmatter stripped)
    pub source: SkillSource,      // BuiltIn or File(PathBuf)
    pub hats: Vec<String>,        // Restrict to specific hats
    pub backends: Vec<String>,    // Restrict to specific backends
    pub tags: Vec<String>,
    pub auto_inject: bool,        // Inject into every prompt
}
```

### Hook Types
```rust
pub struct HookSpec {
    pub command: String,
    pub timeout_seconds: Option<u64>,
    pub on_error: Option<HookOnError>,
    pub suspend: Option<HookSuspendMode>,
    pub mutation: Option<HookMutationConfig>,
}

pub enum HookPhaseEvent {
    IterationBefore, IterationAfter,
    LoopStart, LoopEnd,
    // ... other phases
}
```

### Planning Session
```rust
pub struct PlanningSession {
    pub id: String,
    pub status: SessionStatus,    // Active, WaitingForInput, Completed, TimedOut, Failed
    pub created_at: String,
    pub updated_at: String,
}

pub enum SessionStatus {
    Active,
    WaitingForInput { prompt_id: String },
    Completed, TimedOut, Failed,
}
```

### Parallel Loop Types
```rust
pub struct LoopEntry {
    pub id: String,
    pub status: String,
    pub worktree_path: Option<PathBuf>,
    pub branch: Option<String>,
    pub prompt: Option<String>,
    pub started_at: String,
}

pub struct MergeEntry {
    pub loop_id: String,
    pub branch: String,
    pub worktree_path: PathBuf,
    pub status: MergeButtonState,
}

pub enum MergeButtonState { Ready, InProgress, Merged, Failed, Discarded }
```

## Persistence Formats

| Data | Format | Location |
|------|--------|----------|
| Memories | Structured Markdown | `.ralph/agent/memories.md` |
| Tasks | JSONL (append-only) | `.ralph/agent/tasks.jsonl` |
| Events | JSONL | `.ralph/events.jsonl` |
| Loop registry | JSON | `.ralph/loops.json` |
| Merge queue | JSONL (event-sourced) | `.ralph/merge-queue.jsonl` |
| Loop lock | Text (PID + prompt) | `.ralph/loop.lock` |
| History | JSONL | `.ralph/history.jsonl` |
| Telegram state | JSON | `.ralph/telegram-state.json` |
| Planning conversations | JSONL | `.ralph/planning/{id}/conversation.jsonl` |
| Planning artifacts | Markdown | `.ralph/planning/{id}/artifacts/` |
| Diagnostics | JSONL | `.ralph/diagnostics/{timestamp}/` |
| Config | YAML | `ralph.yml` (or `-c` override) |
| Scratchpad | Markdown | `.ralph/scratchpad.md` (legacy mode) |
