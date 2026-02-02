# Migration from v1

Guide for migrating from the Python-based Ralph v1 to the Rust-based v2.

## Overview

Ralph v2 is a complete rewrite in Rust with significant changes:

| Aspect | v1 (Python) | v2 (Rust) |
|--------|-------------|-----------|
| Language | Python | Rust |
| Installation | pip/pipx | npm/cargo |
| Config format | Python dict | YAML |
| Hat system | Not present | Core feature |
| Event system | Not present | Core feature |
| Memories | Not present | Built-in |
| Tasks | Not present | Built-in |
| TUI | Basic | Full ratatui |

## Uninstalling v1

Remove the old Python version first:

```bash
# If installed via pip
pip uninstall ralph-orchestrator

# If installed via pipx
pipx uninstall ralph-orchestrator

# If installed via uv
uv tool uninstall ralph-orchestrator

# Verify removal
which ralph  # Should return nothing
```

## Installing v2

```bash
# Via npm (recommended)
npm install -g @ralph-orchestrator/ralph-cli

# Via Homebrew
brew install ralph-orchestrator

# Via Cargo
cargo install ralph-cli
```

## Configuration Changes

### v1 Configuration (Python)

```python
# ralph_config.py
config = {
    "max_iterations": 100,
    "agent": "claude",
    "cost_limit": 10.0,
    "checkpoint_interval": 10,
}
```

### v2 Configuration (YAML)

```yaml
# ralph.yml
cli:
  backend: "claude"

event_loop:
  completion_promise: "LOOP_COMPLETE"
  max_iterations: 100
  checkpoint_interval: 10
```

## Command Changes

| v1 Command | v2 Command |
|------------|------------|
| `python ralph_orchestrator.py --prompt PROMPT.md` | `ralph run` |
| `python ralph_orchestrator.py --agent claude` | `ralph run --backend claude` |
| `python ralph_orchestrator.py --max-iterations 50` | `ralph run --max-iterations 50` |
| `python ralph_orchestrator.py --dry-run` | `ralph run --dry-run` |

## New Features in v2

### Hat System

Specialized personas that didn't exist in v1:

```yaml
hats:
  planner:
    triggers: ["task.start"]
    publishes: ["plan.ready"]
    instructions: "Create a plan..."
```

### Events

Typed communication between hats:

```bash
ralph emit "build.done" "tests: pass, lint: pass, typecheck: pass, audit: pass, coverage: pass"
ralph events  # View history
```

### Memories

Persistent learning:

```bash
ralph tools memory add "Pattern discovered" -t pattern
ralph tools memory search "pattern"
```

### Tasks

Runtime tracking:

```bash
ralph tools task add "Implement feature"
ralph tools task list
ralph tools task close task-123
```

### Presets

Pre-configured workflows:

```bash
ralph init --preset tdd-red-green
```

### TUI

Rich terminal interface (enabled by default):

```bash
ralph run  # TUI mode
ralph run --no-tui  # Headless mode
```

## Removed Features

Some v1 features are handled differently in v2:

| v1 Feature | v2 Equivalent |
|------------|---------------|
| Cost tracking | Not built-in (use backend's tracking) |
| Loop detection | Simplified (max iterations) |
| ACP protocol | Not supported (direct CLI only) |
| Metrics export | Diagnostics system |

## PROMPT.md Compatibility

The prompt file format is mostly compatible:

```markdown
# Task: My Task

Description here.

## Requirements
- Requirement 1
- Requirement 2
```

**Changes:**

- `- [x] TASK_COMPLETE` marker is no longer used
- Use `LOOP_COMPLETE` in output instead
- Acceptance criteria still work the same

## State Directory

| v1 Location | v2 Location |
|-------------|-------------|
| `.agent/metrics/` | (removed) |
| `.agent/checkpoints/` | Git-based |
| `.agent/prompts/` | (removed) |
| `.agent/plans/` | (removed) |
| (none) | `.agent/memories.md` |
| (none) | `.agent/tasks.jsonl` |
| (none) | `.agent/event_history.jsonl` |

## Migration Steps

### 1. Uninstall v1

```bash
pip uninstall ralph-orchestrator
```

### 2. Install v2

```bash
npm install -g @ralph-orchestrator/ralph-cli
```

### 3. Convert Configuration

Create `ralph.yml` from your old config:

```yaml
cli:
  backend: "claude"  # was "agent"

event_loop:
  completion_promise: "LOOP_COMPLETE"
  max_iterations: 100  # same as before
```

### 4. Update Prompts

Change completion markers:

```markdown
# Before (v1)
- [x] TASK_COMPLETE

# After (v2)
Output: LOOP_COMPLETE
```

### 5. Clean Old State

```bash
rm -rf .agent/metrics .agent/checkpoints .agent/prompts .agent/plans
```

### 6. Test

```bash
ralph run --dry-run
ralph run
```

## Getting Help

If you encounter migration issues:

- Check [Troubleshooting](troubleshooting.md)
- [Open an issue](https://github.com/mikeyobrien/ralph-orchestrator/issues)
- Reference v1 code at [v1.2.3](https://github.com/mikeyobrien/ralph-orchestrator/tree/v1.2.3)
