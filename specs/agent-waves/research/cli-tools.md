# CLI Tools System Research

## Command Structure

**File:** `crates/ralph-cli/src/main.rs:467-528`

Top-level commands:
```
ralph run | preflight | doctor | tutorial | resume | events | init | clean
ralph emit <topic> [payload]         ← event emission
ralph plan | code-task | task
ralph tools <subcommand>             ← agent-facing tools
ralph loops | hats | tui | web | bot
ralph completions
```

## `ralph tools` Namespace

**File:** `crates/ralph-cli/src/tools.rs`

Agent-facing tools grouped under `ralph tools`:
- `ralph tools memory` — persistent learning
- `ralph tools task` — work item tracking
- `ralph tools skill` — skill loading
- `ralph tools interact` — human-in-the-loop

Pattern: Each subcommand has its own module (e.g., `memory.rs`, `task_cli.rs`) with `Args` struct + `execute()` function.

## `ralph emit` Implementation

**File:** `crates/ralph-cli/src/main.rs:737-758, 2249-2317`

```rust
struct EmitArgs {
    pub topic: String,
    pub payload: String,
    pub json: bool,
    pub ts: Option<String>,
    pub file: PathBuf,       // default: .ralph/events.jsonl
}
```

Flow: Build JSON → resolve events file (`.ralph/current-events` marker) → append JSONL line.

## Tool Documentation

**Source of truth:** `crates/ralph-core/data/ralph-tools.md`
**Symlink:** `.claude/skills/ralph-tools/SKILL.md` → above file

CLAUDE.md instruction: "When adding or changing `ralph tools` subcommands, update `crates/ralph-core/data/ralph-tools.md`"

## Where `ralph wave` Would Fit

### Option A: Top-level command (`ralph wave`)
- Consistent with `ralph emit`, `ralph hats`, `ralph loops`
- More discoverable for users
- Best if wave management is both user-facing and agent-facing

### Option B: Under tools (`ralph tools wave`)
- Consistent with agent-facing tool pattern (memory, task, skill)
- Groups all agent tools together
- Best if wave commands are primarily for agents during hat execution

### Recommendation
Given that `ralph wave start/emit/end` is called from within hat instructions (agent-facing), **Option B (`ralph tools wave`)** aligns with the pattern. However, the issue proposes `ralph wave` as top-level, which may be more intuitive for preset authors.

Consider: `ralph wave` at top level (like `ralph emit`), since both are event-related primitives used by agents. The `ralph tools` namespace is more for state management (memory, tasks).

## Implementation Pattern

New command module: `crates/ralph-cli/src/wave.rs`

```rust
pub struct WaveArgs {
    #[command(subcommand)]
    pub command: WaveCommands,
}

pub enum WaveCommands {
    Start(StartArgs),   // ralph wave start --expect N
    End(EndArgs),       // ralph wave end
    Emit(EmitArgs),     // ralph wave emit <topic> --payloads [...]
}
```

Would write wave metadata to events file alongside regular events, or to a separate `.ralph/waves.json` state file.
