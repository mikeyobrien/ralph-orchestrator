# ralph-core

The orchestration engine — the heart of Ralph.

## Overview

`ralph-core` provides:

- Configuration loading and validation
- The main event loop
- Memory and task storage
- Event parsing
- Instruction assembly

## Key Components

### Config

Configuration loading from YAML.

```rust
use ralph_core::config::Config;

// Load from file
let config = Config::load("ralph.yml")?;

// Load with defaults
let config = Config::default();
```

**Configuration structure:**

```rust
pub struct Config {
    pub cli: CliConfig,
    pub event_loop: EventLoopConfig,
    pub core: CoreConfig,
    pub memories: MemoryConfig,
    pub tasks: TaskConfig,
    pub hats: HashMap<String, HatConfig>,
}

pub struct EventLoopConfig {
    pub completion_promise: String,
    pub max_iterations: usize,
    pub max_runtime_seconds: u64,
    pub idle_timeout_secs: u64,
    pub starting_event: Option<String>,
    pub checkpoint_interval: usize,
    pub prompt_file: Option<String>,
}

pub struct CliConfig {
    pub backend: String,
    pub prompt_mode: PromptMode,
}

pub struct MemoryConfig {
    pub enabled: bool,
    pub inject: InjectMode,
    pub budget: usize,
    pub filter: MemoryFilter,
}

pub struct TaskConfig {
    pub enabled: bool,
}
```

### EventLoop

The main orchestration loop.

```rust
use ralph_core::EventLoop;

// Create with config
let event_loop = EventLoop::new(config);

// Run orchestration
let result = event_loop.run().await?;
```

**EventLoop lifecycle:**

1. Load configuration
2. Initialize EventBus with hats
3. Publish starting event (if configured)
4. Loop:
   - Get next event
   - Find matching hat
   - Inject instructions
   - Execute backend
   - Parse output for events
   - Check for completion
5. Return result

### MemoryStore

Persistent memory management.

```rust
use ralph_core::memory_store::MemoryStore;

let store = MemoryStore::new(".agent/memories.md");

// Add memory
store.add(Memory {
    content: "Uses barrel exports".to_string(),
    memory_type: MemoryType::Pattern,
    tags: vec!["structure".to_string()],
})?;

// Search
let results = store.search("exports")?;

// List by type
let patterns = store.list_by_type(MemoryType::Pattern)?;
```

### TaskStore

Runtime task tracking.

```rust
use ralph_core::task_store::TaskStore;

let store = TaskStore::new(".agent/tasks.jsonl");

// Add task
let id = store.add(Task {
    title: "Implement auth".to_string(),
    priority: 2,
    blocked_by: vec![],
})?;

// Get ready tasks
let ready = store.ready()?;

// Close task
store.close(&id)?;
```

### EventParser

Parse agent output for events.

```rust
use ralph_core::event_parser::EventParser;

let parser = EventParser::new();

// Parse output
let events = parser.parse(agent_output)?;

// Check for completion
let complete = parser.is_complete(agent_output, "LOOP_COMPLETE");
```

**Event formats recognized:**

```bash
# CLI command
ralph emit "build.done" "tests: pass, lint: pass, typecheck: pass, audit: pass, coverage: pass"

# JSON
{"event": "build.done", "payload": "tests: pass, lint: pass, typecheck: pass, audit: pass, coverage: pass"}
```

### Instructions

Hat instruction assembly.

```rust
use ralph_core::instructions::InstructionBuilder;

let builder = InstructionBuilder::new();

// Build instructions for a hat
let instructions = builder
    .with_base_prompt(&config.prompt_file)
    .with_guardrails(&config.guardrails)
    .with_memories(&memories)
    .with_hat_instructions(&hat.instructions)
    .build()?;
```

## Testing Support

### Smoke Runner

Replay-based testing with JSONL fixtures.

```rust
use ralph_core::testing::smoke_runner::SmokeRunner;

let runner = SmokeRunner::new("tests/fixtures/basic.jsonl");
let result = runner.run().await?;
assert!(result.completed);
```

### Session Recorder

Record sessions for replay.

```rust
use ralph_core::session_recorder::SessionRecorder;

let recorder = SessionRecorder::new("session.jsonl");
recorder.record_output("Hello")?;
recorder.record_tool_call("read_file", args)?;
recorder.finish()?;
```

## Error Types

```rust
pub enum CoreError {
    ConfigError(String),
    IoError(std::io::Error),
    ParseError(String),
    MemoryError(String),
    TaskError(String),
}
```

## Feature Flags

| Flag | Description |
|------|-------------|
| `default` | Standard features |
| `testing` | Test utilities |

## Example: Custom Event Loop

```rust
use ralph_core::{Config, EventLoop};
use ralph_proto::{EventBus, Event};

#[tokio::main]
async fn main() -> Result<()> {
    // Load config
    let config = Config::load("ralph.yml")?;

    // Create event loop
    let mut event_loop = EventLoop::new(config);

    // Optional: Add custom event listener
    event_loop.on_event(|event| {
        println!("Event: {:?}", event.topic);
    });

    // Run
    let result = event_loop.run().await?;

    println!("Completed in {} iterations", result.iterations);
    Ok(())
}
```
