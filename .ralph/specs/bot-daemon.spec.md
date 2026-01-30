# Design: `ralph bot daemon`

## Problem

When no Ralph loop is running, Telegram messages go nowhere. Users must SSH in or open a terminal to start `ralph run`. The daemon bridges this gap — a persistent process that listens on the configured communication adapter and starts loops on demand.

## Architecture

The daemon is **adapter-agnostic**. The CLI command resolves which communication adapter the user configured and delegates entirely to it. Telegram is the first adapter; others (Slack, Discord) can follow the same trait.

```
ralph bot daemon                      ← CLI: reads config, picks adapter
    │
    ▼
DaemonAdapter trait (ralph-core)      ← interface: run_daemon(start_loop_fn)
    │
    ├── TelegramDaemon (ralph-telegram)   ← polls getUpdates, sends messages
    ├── SlackDaemon (future)              ← WebSocket listener
    └── ...
```

**Key constraint**: The adapter crate cannot depend on `ralph-cli` (that would be circular). Instead, the CLI passes a **callback** — `start_loop_fn` — that the adapter calls when it wants to start a loop. The adapter owns everything else: polling, commands, greeting, farewell, and handing off to the loop's `TelegramService` during a run.

## Behavior

### State Machine

```
          ┌─────────────┐
          │    Idle      │◄──────────────────┐
          │  (polling)   │                   │
          └──────┬───────┘                   │
                 │ message received          │ loop finishes
                 ▼                           │
          ┌──────────────┐    ┌──────────────┴──┐
          │ Check lock   │───▶│  Loop Running    │
          │ locked?  ────│──▶ │  (hand off)      │
          │ unlocked? ───│──▶ │  (start loop)    │
          └──────────────┘    └─────────────────┘
```

**Idle (no loop running):**
- Adapter polls for messages (Telegram: `getUpdates` with 30s long-poll)
- On message: checks `LoopLock::is_locked()`
- If unlocked: calls `start_loop_fn(prompt)` to start a new loop
- If locked (e.g., loop started externally): informs user the loop will receive messages directly

**Loop running (turn-taking model):**
- Daemon **stops polling**. The loop's own `TelegramService` takes over the `getUpdates` stream.
- The loop's `TelegramService` handles all interaction: commands, guidance, responses, check-ins, reactions, multimedia, markdown.
- Daemon simply awaits the loop task's completion.
- One Telegram integration, not two.

**Loop finishes:**
- Loop's `TelegramService` has already stopped and sent its farewell.
- Daemon sends completion notification (success/error).
- Daemon resumes polling, returns to idle.

### Loop Startup

- Daemon sends ack: "Starting loop: *{prompt}*"
- Calls `start_loop_fn(prompt)` which spawns the loop as a tokio task
- Loop runs in **main workspace** (not a worktree)
- Config loaded from `ralph.yml` (no passthrough flags)
- Lock acquired normally via `LoopLock::try_acquire()`

### Commands (Idle Only)

| Command | Behavior |
|---------|----------|
| `/status` | Reports: idle or loop running (externally) |

During a loop, the loop's own `TelegramService` handles all commands (`/status`, `/tasks`, `/memories`, `/tail`, `/restart`).

### Startup & Shutdown

- `ralph bot daemon` starts the daemon
- Sends greeting: "Ralph daemon online"
- On Ctrl+C / SIGTERM: sends farewell, cleans up, exits

## Implementation

### Step 1: Define `DaemonAdapter` trait in `ralph-core`

```rust
/// Callback the adapter calls to start an orchestration loop.
/// Returns the termination reason on completion.
pub type StartLoopFn = Box<
    dyn Fn(String) -> std::pin::Pin<Box<dyn std::future::Future<Output = anyhow::Result<TerminationReason>> + Send>>
        + Send
        + Sync,
>;

/// A communication adapter that can run in daemon mode.
#[async_trait]
pub trait DaemonAdapter: Send + Sync {
    /// Run the daemon loop. Blocks until shutdown.
    async fn run_daemon(
        &self,
        workspace_root: PathBuf,
        start_loop: StartLoopFn,
    ) -> anyhow::Result<()>;
}
```

### Step 2: Implement `TelegramDaemon` in `ralph-telegram`

In `crates/ralph-telegram/src/daemon.rs`:

- Implements `DaemonAdapter` for `TelegramDaemon`
- Resolves token and chat_id
- Sends greeting
- Idle polling loop (reuse existing `getUpdates` pattern)
- On message + unlocked: call `start_loop(prompt)`, await completion
- On message + locked: inform user
- `/status` command handling
- Signal handlers for graceful shutdown
- Sends farewell on exit

### Step 3: Extract `start_loop()` in `ralph-cli`

In `crates/ralph-cli/src/loop_runner.rs`:

```rust
pub async fn start_loop(
    prompt: String,
    workspace_root: PathBuf,
    config_path: Option<PathBuf>,
) -> Result<TerminationReason>
```

Handles: load config, apply prompt, acquire lock, run event loop, release lock.

### Step 4: Wire `ralph bot daemon` CLI command

In `crates/ralph-cli/src/bot.rs`:

- Add `Daemon` variant to `BotCommands` enum
- `run_daemon()`:
  - Reads config to determine adapter type
  - Creates `TelegramDaemon` (or future adapters)
  - Wraps `start_loop()` as a `StartLoopFn` callback
  - Calls `adapter.run_daemon(workspace_root, start_loop_fn)`

## Non-Goals

- Worktree isolation for daemon-spawned loops
- CLI flags passthrough to spawned loops
- Queuing multiple loops
- Confirmation before starting a loop
- Non-Telegram adapters (future work, but trait is ready)

## Files Changed

| File | Change |
|------|--------|
| `crates/ralph-core/src/daemon.rs` | New: `DaemonAdapter` trait, `StartLoopFn` type |
| `crates/ralph-telegram/src/daemon.rs` | New: `TelegramDaemon` implementing `DaemonAdapter` |
| `crates/ralph-cli/src/loop_runner.rs` | Add `start_loop()` extracted from `run_command()` |
| `crates/ralph-cli/src/bot.rs` | `Daemon` subcommand, delegates to adapter |
