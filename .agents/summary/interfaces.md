# Interfaces

## Core Traits

### `RobotService` (ralph-proto)
Human-in-the-loop abstraction. Implemented by `TelegramRobotService`.

```rust
#[async_trait]
pub trait RobotService: Send + Sync {
    async fn ask(&self, question: &str, context: CheckinContext) -> Result<String>;
    async fn notify(&self, message: &str) -> Result<()>;
    async fn checkin(&self, context: CheckinContext) -> Option<String>;
}
```

### `StreamHandler` (ralph-adapters)
Handles streaming output from agent backends. Multiple implementations for different display modes.

```rust
pub trait StreamHandler: Send {
    fn on_text(&mut self, text: &str);
    fn on_tool_use(&mut self, name: &str, input: &Value);
    fn on_tool_result(&mut self, result: &str);
    fn on_error(&mut self, error: &str);
    fn on_complete(&mut self, result: &SessionResult);
}
```

Implementations: `ConsoleStreamHandler`, `PrettyStreamHandler`, `QuietStreamHandler`, `TuiStreamHandler`

### `HookExecutorContract` (ralph-core)
Abstraction for running lifecycle hooks.

```rust
#[async_trait]
pub trait HookExecutorContract: Send + Sync {
    async fn run(&self, request: HookRunRequest) -> Result<HookRunResult, HookExecutorError>;
}
```

### `DaemonAdapter` (ralph-proto)
Abstraction for daemon-mode loop spawning.

```rust
pub trait DaemonAdapter: Send + Sync {
    fn start_loop(&self, config: Value) -> Result<String>;
}
```

### `Authenticator` (ralph-api)
API authentication abstraction.

```rust
pub trait Authenticator: Send + Sync {
    fn authenticate(&self, headers: &HeaderMap) -> Result<String, ApiError>;
}
```

## RPC API (ralph-api)

JSON-RPC v1 API exposed over HTTP/WebSocket and MCP stdio.

### System Methods
| Method | Description |
|--------|-------------|
| `system.health` | Health check |
| `system.version` | API version info |
| `system.capabilities` | Available capabilities |

### Task Methods
| Method | Description |
|--------|-------------|
| `task.list` | List tasks with optional filters |
| `task.get` | Get task by ID |
| `task.ready` | Get next ready task |
| `task.create` | Create new task |
| `task.update` | Update task fields |
| `task.close` | Close task as completed |
| `task.archive` / `task.unarchive` | Archive management |
| `task.delete` / `task.clear` | Deletion |
| `task.run` | Run a specific task |
| `task.run_all` | Run all ready tasks |
| `task.retry` / `task.cancel` | Retry/cancel running task |
| `task.status` | Get task execution status |

### Loop Methods
| Method | Description |
|--------|-------------|
| `loop.list` | List all loops |
| `loop.status` | Get loop status |
| `loop.process` | Process merge queue |
| `loop.prune` | Prune stale loops |
| `loop.retry` / `loop.discard` | Retry/discard failed loop |
| `loop.stop` | Stop running loop |
| `loop.merge` | Merge completed loop |
| `loop.merge_button_state` | Get merge UI state |
| `loop.trigger_merge_task` | Trigger merge task |

### Planning Methods
| Method | Description |
|--------|-------------|
| `planning.list` | List planning sessions |
| `planning.get` | Get session details |
| `planning.start` | Start new planning session |
| `planning.respond` | Send user response |
| `planning.resume` | Resume paused session |
| `planning.delete` | Delete session |
| `planning.get_artifact` | Get generated artifact |

### Config / Collection / Preset / Stream Methods
| Method | Description |
|--------|-------------|
| `config.get` / `config.update` | Configuration management |
| `collection.*` | Hat collection CRUD + import/export |
| `preset.list` | List built-in presets |
| `stream.subscribe` / `stream.unsubscribe` / `stream.ack` | WebSocket event streaming |

## CLI Interface (`ralph` binary)

### Primary Commands
| Command | Description |
|---------|-------------|
| `ralph run` | Start orchestration loop (`-p` prompt, `-c` config, `--max-iterations`, `--tui`) |
| `ralph plan` | Interactive PDD planning session |
| `ralph init` | Initialize Ralph in a project (`--backend`) |
| `ralph web` | Launch web dashboard |
| `ralph mcp serve` | Start MCP server over stdio |

### Management Commands
| Command | Description |
|---------|-------------|
| `ralph task` | Task CRUD (list, add, update, close, delete) |
| `ralph loops` | Monitor parallel loops |
| `ralph memory` | Memory management (list, add, remove) |
| `ralph hats` | Inspect hat configurations |
| `ralph hooks validate` | Validate hook configuration |
| `ralph skills` | List available skills |
| `ralph presets` | List/show presets |
| `ralph tools` | Agent tool subcommands |
| `ralph bot` | Telegram bot management (onboard, status, test) |
| `ralph doctor` | System diagnostics |
| `ralph clean` | Clean diagnostics/artifacts |
| `ralph events` | View event history |
| `ralph completions` | Shell completion generation |

## Event Topics (Pub/Sub)

Standard event topics used in the default hat collection:

| Topic | Publisher | Subscriber |
|-------|----------|------------|
| `work.start` | EventLoop (starting event) | Planner |
| `subtask.ready` | Planner | Builder |
| `subtask.done` | Builder | Planner |
| `all_steps.done` | Planner | Reviewer |
| `implementation.done` | Builder | Reviewer |
| `review.approved` | Reviewer | Finalizer |
| `review.changes_requested` | Reviewer | Builder |
| `LOOP_COMPLETE` | Finalizer | EventLoop (termination) |
| `human.interact` | Any hat | RObot (blocks for response) |
| `human.response` | RObot | EventLoop |
| `human.guidance` | RObot | EventLoop (injected into prompt) |
| `plan.*` | Planning hats | Planning system |
| `loop.cancel` | External | EventLoop (graceful cancel) |

## Backpressure Gates

Configured in `ralph.yml` under `backpressure.gates`. Each gate runs a command and blocks progress on failure.

```yaml
backpressure:
  gates:
    - name: fmt
      command: cargo fmt --all -- --check
      on_fail: "Formatting failed."
    - name: clippy
      command: cargo clippy --all-targets --all-features -- -D warnings
      on_fail: "Clippy lint failures."
    - name: test
      command: cargo test --all
      on_fail: "Tests failed."
```

## Built-in Presets

Located in `crates/ralph-cli/presets/`:

| Preset | Purpose |
|--------|---------|
| `code-assist.yml` | Single-hat code implementation |
| `debug.yml` | Debugging workflow |
| `research.yml` | Research and analysis |
| `review.yml` | Code review |
| `pdd-to-code-assist.yml` | PDD planning → code implementation |
| `hatless-baseline.yml` | Minimal hatless configuration |
| `merge-loop.yml` | Merge queue processing |
| `minimal/` | Minimal preset directory |
