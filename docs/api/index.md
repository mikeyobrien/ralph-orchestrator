# API Reference

Technical reference documentation for Ralph's crates.

## Crate Overview

| Crate | Purpose | Documentation |
|-------|---------|---------------|
| [ralph-proto](ralph-proto.md) | Shared protocol definitions | Events, hats, topics |
| [ralph-core](ralph-core.md) | Shared configuration and coordination state | Config, memories, tasks, registry, merge queue |
| [ralph-adapters](ralph-adapters.md) | Autoloop process and artifact integration | Journal, event-stream, and summary adapters |
| [ralph-tui](ralph-tui.md) | Terminal observation UI | TUI components |
| [ralph-cli](ralph-cli.md) | CLI frontend and autoloop launcher | Commands, completion, and merge coordination |

## Quick Links

### Core Types

```rust
// Events
use ralph_proto::{Event, Topic, EventBus};

// Hats
use ralph_proto::{Hat, HatId};

// Configuration and coordination
use ralph_core::{CliConfig, EventLoopConfig, RalphConfig};
```

### Common Operations

```rust
// Construct the public configuration type
use ralph_core::RalphConfig;

let config = RalphConfig::default();
assert_eq!(config.core.specs_dir, ".ralph/specs/");
```

Application execution starts through `ralph-cli`, which translates supported
configuration into an autoloop preset and launches autoloop. Autoloop owns role
dispatch and completion judgment; Ralph observes its journal, event stream, and
summary and coordinates the TUI, registry, worktrees, and merge queue.

## Rust Documentation

Generate and view Rust docs:

```bash
# Generate docs
cargo doc --no-deps --open

# Generate with dependencies
cargo doc --open
```

## Stability

| Crate | Status |
|-------|--------|
| ralph-proto | Stable |
| ralph-core | Stable |
| ralph-adapters | Stable |
| ralph-tui | Experimental |
| ralph-cli | Stable |
| ralph-e2e | Internal |
| ralph-bench | Internal |

"Stable" means the public API is unlikely to change in breaking ways.
"Experimental" means the API may change.
"Internal" means the crate is not intended for external use.
