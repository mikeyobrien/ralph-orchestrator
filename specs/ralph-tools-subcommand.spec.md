# Spec: `ralph tools` Subcommand

## Summary

Move `ralph memory` under a new `ralph tools` subcommand. Move beads-lite task tracking to `ralph tools task`. Repurpose `ralph task` as an alias for `ralph code-task`.

## Motivation

- **Clear separation**: `tools` = things Ralph uses during orchestration (agent-facing). Top-level commands = user-facing.
- **Intuitive naming**: `ralph task` feels like "create a task" which maps to `code-task`, not beads-lite tracking.
- **Semantic clarity**: Runtime tools Ralph reads/writes live under `ralph tools`.

## Design

### New Command Structure

```
ralph
├── run             # Core orchestration
├── resume          # Resume orchestration
├── events          # View event history
├── init            # Initialize configuration
├── clean           # Clean artifacts
├── emit            # Emit custom events
├── plan            # PDD planning (user-initiated)
├── code-task       # Task file generation
├── task            # Alias for code-task
└── tools           # Ralph's runtime tools
    ├── memory      # Persistent memories
    └── task        # Work item tracking (beads-lite)
```

### Usage Examples

```bash
# User-facing task creation (both equivalent)
ralph task "Add authentication"
ralph code-task "Add authentication"

# Ralph's memory tool
ralph tools memory add "uses barrel exports" --type pattern
ralph tools memory search "authentication"
ralph tools memory prime --budget 2000
ralph tools memory list
ralph tools memory show mem-1737372000-a1b2
ralph tools memory delete mem-1737372000-a1b2

# Ralph's task tracking tool (beads-lite)
ralph tools task add "Fix auth bug" --priority 1
ralph tools task list --status open
ralph tools task ready
ralph tools task show task-1737372000-a1b2
ralph tools task close task-1737372000-a1b2
```

## Acceptance Criteria

### CLI Implementation

- [ ] Add `Tools` variant to `Commands` enum in `main.rs`
- [ ] Create `tools.rs` module as dispatcher for `memory` and `task`
- [ ] Route `ralph tools memory ...` to existing `memory::execute()`
- [ ] Route `ralph tools task ...` to existing `beads::execute()`
- [ ] Remove top-level `Memory` variant from `Commands`
- [ ] Change top-level `Task` to alias for `CodeTask` (delegates to code-task logic)
- [ ] Update help text to reflect new structure

### Injected Prompts (Critical)

- [ ] Update `crates/ralph-core/src/hatless_ralph.rs` - skill injection uses `ralph tools memory` and `ralph tools task`
- [ ] Update `crates/ralph-core/src/config.rs` if it references commands

### Documentation

- [ ] Update `AGENTS.md` - all `ralph memory` → `ralph tools memory`, `ralph task` → `ralph tools task`
- [ ] Update `README.md` if it references these commands

### Skills

- [ ] Update `.claude/skills/ralph-memories/SKILL.md` - all command examples

### Tests

- [ ] Update `crates/ralph-cli/tests/integration_memory.rs` - CLI invocations
- [ ] Update `crates/ralph-e2e/src/scenarios/memory.rs` - E2E test commands
- [ ] Update `crates/ralph-e2e/src/scenarios/orchestration.rs` if it uses task commands
- [ ] Verify `cargo test` passes

### Smoke Test

- [ ] Run `ralph tools memory --help` and verify subcommands listed
- [ ] Run `ralph tools task --help` and verify subcommands listed
- [ ] Run `ralph tools memory add "test" --type pattern` and verify it works
- [ ] Run `ralph tools task add "test task"` and verify it works
- [ ] Run `ralph task "test"` and verify it creates a code-task file (alias works)
- [ ] Run `ralph memory --help` and verify it errors (no longer exists)

## Files to Modify

| File | Change |
|------|--------|
| `crates/ralph-cli/src/main.rs` | Add `Tools` command, remove `Memory`, change `Task` to alias `CodeTask` |
| `crates/ralph-cli/src/tools.rs` | NEW: dispatcher module routing to `memory` and `beads` |
| `crates/ralph-core/src/hatless_ralph.rs` | Update injected skill instructions |
| `crates/ralph-core/src/config.rs` | Update if references commands |
| `AGENTS.md` | Update all command references |
| `README.md` | Update if references commands |
| `.claude/skills/ralph-memories/SKILL.md` | Update command examples |
| `crates/ralph-cli/tests/integration_memory.rs` | Update CLI invocations |
| `crates/ralph-e2e/src/scenarios/memory.rs` | Update E2E commands |
| `crates/ralph-e2e/src/scenarios/orchestration.rs` | Update if uses task commands |

## No Backwards Compatibility

**`ralph memory` will stop working.** No aliases, no deprecation warnings, no migration period.

- `ralph memory add ...` → error, use `ralph tools memory add ...`
- `ralph task add ...` → now creates a code-task file, use `ralph tools task add ...` for beads-lite

Per CLAUDE.md: "Backwards compatibility doesn't matter — it adds clutter for no reason."

## Out of Scope

- Historical specs in `specs/` don't need updating (they document past decisions)
- Config file format unchanged (memories/tasks storage paths stay the same)

## Risks

- **E2E tests may fail** if they invoke CLI commands directly - must update scenarios
- **Skill injection** is critical path - if `hatless_ralph.rs` isn't updated, Ralph won't know new command syntax
