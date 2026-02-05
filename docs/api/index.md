# API Reference

Technical reference documentation for Hats's crates.

## Crate Overview

| Crate | Purpose | Documentation |
|-------|---------|---------------|
| [hats-proto](hats-proto.md) | Protocol types: Event, Hat, Topic | Core data structures |
| [hats-core](hats-core.md) | Orchestration engine | EventLoop, Config |
| [hats-adapters](hats-adapters.md) | CLI backends | Backend integrations |
| [hats-tui](hats-tui.md) | Terminal UI | TUI components |
| [hats-cli](hats-cli.md) | Binary entry point | CLI commands |

## Quick Links

### Core Types

```rust
// Events
use hats_proto::{Event, Topic, EventBus};

// Hats
use hats_proto::{Hat, HatId};

// Configuration
use hats_core::config::{Config, EventLoopConfig, CliConfig};
```

### Common Operations

```rust
// Load configuration
let config = Config::load("hats.yml")?;

// Create event loop
let event_loop = EventLoop::new(config);

// Run orchestration
event_loop.run().await?;
```

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
| hats-proto | Stable |
| hats-core | Stable |
| hats-adapters | Stable |
| hats-tui | Experimental |
| hats-cli | Stable |
| hats-e2e | Internal |
| hats-bench | Internal |

"Stable" means the public API is unlikely to change in breaking ways.
"Experimental" means the API may change.
"Internal" means the crate is not intended for external use.
