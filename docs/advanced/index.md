# Advanced Topics

Deep dives into Ralph's internals and advanced usage patterns.

## In This Section

| Topic | Description |
|-------|-------------|
| [Architecture](architecture.md) | System design and crate structure |
| [Creating Custom Hats](custom-hats.md) | Design and implement custom hats |
| [Event System Design](event-system.md) | How events route between hats |
| [Memory System](memory-system.md) | Persistent learning mechanics |
| [Task System](task-system.md) | Runtime work tracking |
| [Testing & Validation](testing.md) | Smoke tests, E2E tests, TUI validation |
| [Diagnostics](diagnostics.md) | Debug with full visibility |
| [Parallel Loops](parallel-loops.md) | Run multiple loops concurrently with worktrees |

## When to Read This

These guides are for you if:

- You're building complex multi-hat workflows
- You want to understand how Ralph works internally
- You're contributing to Ralph development
- You need to debug tricky issues
- You're extending Ralph with custom backends

## Key Concepts

### Crate Architecture

Ralph is organized as a Cargo workspace:

```
ralph-orchestrator/
├── crates/
│   ├── ralph-proto/     # Protocol types
│   ├── ralph-core/      # Orchestration engine
│   ├── ralph-adapters/  # CLI backends
│   ├── ralph-telegram/  # Telegram bot for human-in-the-loop
│   ├── ralph-tui/       # Terminal UI
│   ├── ralph-cli/       # Binary entry point
│   ├── ralph-e2e/       # End-to-end testing
│   └── ralph-bench/     # Benchmarking
```

### Event Flow

Events are the nervous system of hat-based Ralph:

```mermaid
flowchart LR
    A[starting_event] --> B[EventBus]
    B --> C[Hat Selection]
    C --> D[Hat Execution]
    D --> E[Event Emission]
    E --> B
```

### State Management

Ralph uses files for all persistent state:

| File | Purpose |
|------|---------|
| `.agent/memories.md` | Cross-session learning |
| `.agent/tasks.jsonl` | Runtime work tracking |
| `.agent/event_history.jsonl` | Event audit log |
| `.agent/scratchpad.md` | Legacy state (deprecated) |

## Quick Reference

### Enable Diagnostics

```bash
RALPH_DIAGNOSTICS=1 ralph run
```

### Run E2E Tests

```bash
cargo run -p ralph-e2e -- claude
```

### Record a Session

```bash
ralph run --record-session debug.jsonl -p "your prompt"
```

### Validate TUI

```bash
# See TUI Validation in Testing guide
/tui-validate file:output.txt criteria:ralph-header
```

## Next Steps

Start with [Architecture](architecture.md) for the big picture.
