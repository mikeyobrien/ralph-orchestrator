# Implementation Context — Ralph Memories

## Summary

This document provides implementation context for the `ralph memories` feature. The design uses markdown storage with orchestrator-controlled injection.

## Key Files to Create/Modify

### New Files (ralph-core)

| File | Purpose |
|------|---------|
| `crates/ralph-core/src/memory.rs` | `Memory` struct, `MemoryType` enum |
| `crates/ralph-core/src/memory_parser.rs` | Markdown parsing logic |
| `crates/ralph-core/src/memory_store.rs` | `MarkdownMemoryStore` for read/write operations |

### Modified Files

| File | Changes |
|------|---------|
| `crates/ralph-core/src/lib.rs` | Add module exports |
| `crates/ralph-core/Cargo.toml` | Add `rand`, `once_cell`, `regex` dependencies |
| `crates/ralph-cli/src/main.rs` | Add `memory` subcommand with nested commands |
| `crates/ralph-core/src/config.rs` | Add `memories` config section |
| `crates/ralph-core/src/event_loop.rs` | Add memory injection at iteration start |

### New Test Files

| File | Purpose |
|------|---------|
| `crates/ralph-core/src/memory_parser.rs` | Unit tests (inline `#[cfg(test)]`) |
| `crates/ralph-core/src/memory_store.rs` | Unit tests (inline `#[cfg(test)]`) |
| `crates/ralph-cli/tests/integration_memory.rs` | Integration tests for CLI commands |

## File Structure

```
.agent/
├── memories.md        # NEW: Memory storage (markdown)
├── scratchpad.md      # Existing working memory
├── events.jsonl       # Existing event history
└── ralph.log          # Existing debug logs
```

## Patterns to Follow

### CLI Subcommand Structure

Use clap's subcommand pattern for `ralph memory`:

```rust
#[derive(Subcommand, Debug)]
enum Commands {
    // ... existing commands ...

    /// Memory management commands
    Memory(MemoryCommand),
}

#[derive(Parser, Debug)]
struct MemoryCommand {
    #[command(subcommand)]
    command: MemorySubcommand,
}

#[derive(Subcommand, Debug)]
enum MemorySubcommand {
    /// Store a new memory
    Add(MemoryAddArgs),
    /// Search memories
    Search(MemorySearchArgs),
    /// List all memories
    List(MemoryListArgs),
    /// Show a single memory
    Show(MemoryShowArgs),
    /// Delete a memory
    Delete(MemoryDeleteArgs),
    /// Output memories for context injection
    Prime(MemoryPrimeArgs),
    /// Initialize memories file
    Init(MemoryInitArgs),
}
```

### Markdown Parsing Pattern

Use lazy regex compilation:

```rust
use once_cell::sync::Lazy;
use regex::Regex;

static SECTION_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^## (Patterns|Decisions|Fixes|Context)").unwrap()
});

static MEMORY_ID_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"^### (mem-\d+-[0-9a-f]{4})").unwrap()
});
```

### Config Integration

Add to `RalphConfig`:

```rust
#[derive(Debug, Clone, Deserialize)]
pub struct MemoriesConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_inject_mode")]
    pub inject: InjectMode,
    #[serde(default)]
    pub budget: usize,
    #[serde(default = "default_true")]
    pub skill_injection: bool,
    #[serde(default)]
    pub filter: MemoriesFilter,
}

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum InjectMode {
    Auto,
    #[default]
    Manual,
    None,
}
```

### Color Output Pattern

Match existing codebase pattern:

```rust
if use_colors {
    println!(
        "{}📝{} Memory stored: {}{}{}",
        colors::GREEN, colors::RESET,
        colors::CYAN, id, colors::RESET
    );
} else {
    println!("Memory stored: {}", id);
}
```

## Dependencies

### Workspace Cargo.toml

```toml
[workspace.dependencies]
once_cell = "1.19"
regex = "1.10"
```

### ralph-core Cargo.toml

```toml
[dependencies]
once_cell.workspace = true
regex.workspace = true
rand.workspace = true  # already present
```

## Skill Content Location

Embed the memory usage skill as a const in ralph-core:

```rust
// crates/ralph-core/src/memory_skill.rs
pub const MEMORY_SKILL: &str = r#"
## Using Project Memories

This project uses Ralph's memory system for persistent learnings...
"#;
```

## Research Files

Background research is available in:
- `specs/ralph-memories/research/existing-patterns.md` — Beads project analysis
- `specs/ralph-memories/research/technologies.md` — Storage format decisions
- `specs/ralph-memories/research/broken-windows.md` — Opportunistic fixes
