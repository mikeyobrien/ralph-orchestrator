# Configuration

Complete reference for Ralph's YAML configuration.

## Configuration File

Ralph uses `ralph.yml` by default. Override with:

```bash
ralph run -c custom-config.yml
```

## CLI Config Overrides

You can override specific core fields from the command line without creating a separate config file. This is useful for:

- Running parallel Ralph instances with isolated scratchpads
- Testing with different specs directories
- CI/CD pipelines with dynamic paths

**Syntax:** `-c core.field=value`

**Supported fields:**

| Field | Description |
|-------|-------------|
| `core.scratchpad` | Path to scratchpad file |
| `core.specs_dir` | Path to specs directory |

**Examples:**

```bash
# Override scratchpad (loads ralph.yml + applies override)
ralph run -c core.scratchpad=.agent/feature-auth/scratchpad.md

# Explicit config + override
ralph run -c ralph.yml -c core.scratchpad=.agent/feature-auth/scratchpad.md

# Multiple overrides
ralph run -c core.scratchpad=.runs/task-1/scratchpad.md -c core.specs_dir=./custom-specs/
```

Overrides are applied after `ralph.yml` is loaded, so they take precedence. The scratchpad directory is auto-created if it doesn't exist.

## Full Configuration Reference

```yaml
# Event loop settings
event_loop:
  completion_promise: "LOOP_COMPLETE"  # Output that signals completion
  max_iterations: 100                   # Maximum orchestration loops
  max_runtime_seconds: 14400            # 4 hours max runtime
  idle_timeout_secs: 1800               # 30 min idle timeout
  starting_event: "task.start"          # First event published (hat mode)
  checkpoint_interval: 5                # Git checkpoint frequency
  prompt_file: "PROMPT.md"              # Default prompt file

# CLI backend settings
cli:
  backend: "claude"                     # Backend name
  prompt_mode: "arg"                    # arg or stdin

# Core behaviors
core:
  specs_dir: "./specs/"                 # Specifications directory
  guardrails:                           # Rules injected into every prompt
    - "Fresh context each iteration"
    - "Backpressure is law"

# Memories — persistent learning
memories:
  enabled: true                         # Enable memory system
  inject: auto                          # auto, manual, none
  budget: 2000                          # Max tokens to inject
  filter:
    types: []                           # Filter by memory type
    tags: []                            # Filter by tags
    recent: 0                           # Days limit (0 = no limit)

# Tasks — runtime work tracking
tasks:
  enabled: true                         # Enable task system

# Hats — specialized personas
hats:
  my_hat:
    name: "My Hat"                      # Display name
    description: "Purpose"              # Optional description
    triggers: ["event.*"]               # Subscription patterns
    publishes: ["event.done"]           # Allowed event types
    default_publishes: "event.done"     # Default when no explicit
    max_activations: 10                 # Activation limit
    backend: "claude"                   # Backend override
    instructions: |
      Hat-specific instructions...
```

## Section Details

### event_loop

Controls the orchestration loop behavior.

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `completion_promise` | string | `"LOOP_COMPLETE"` | Output text that ends the loop |
| `max_iterations` | integer | `100` | Maximum iterations before stopping |
| `max_runtime_seconds` | integer | `14400` | Maximum runtime (4 hours) |
| `idle_timeout_secs` | integer | `1800` | Idle timeout (30 minutes) |
| `starting_event` | string | `null` | First event (enables hat mode) |
| `checkpoint_interval` | integer | `5` | Git checkpoint frequency |
| `prompt_file` | string | `"PROMPT.md"` | Default prompt file |

### cli

Backend configuration.

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `backend` | string | auto-detect | Backend name |
| `prompt_mode` | string | `"arg"` | How prompt is passed |

**Backend values:**
- `claude` — Claude Code
- `kiro` — Kiro
- `gemini` — Gemini CLI
- `codex` — Codex
- `amp` — Amp
- `copilot` — Copilot CLI
- `opencode` — OpenCode

**Prompt mode values:**
- `arg` — Pass as CLI argument: `cli -p "prompt"`
- `stdin` — Pass via stdin: `echo "prompt" | cli`

### core

Core behaviors and guardrails.

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `specs_dir` | string | `"./specs/"` | Specifications directory |
| `guardrails` | list | `[]` | Rules injected into every prompt |

### memories

Persistent learning across sessions.

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `enabled` | boolean | `true` | Enable memory system |
| `inject` | string | `"auto"` | Injection mode |
| `budget` | integer | `2000` | Max tokens to inject |
| `filter.types` | list | `[]` | Filter by memory type |
| `filter.tags` | list | `[]` | Filter by tags |
| `filter.recent` | integer | `0` | Days limit |

**Injection modes:**
- `auto` — Automatically inject at iteration start
- `manual` — Agent must call `ralph tools memory prime`
- `none` — No injection

### tasks

Runtime work tracking.

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `enabled` | boolean | `true` | Enable task system |

### hats

Specialized personas for hat-based mode.

| Option | Type | Required | Description |
|--------|------|----------|-------------|
| `name` | string | Yes | Display name |
| `description` | string | No | Purpose description |
| `triggers` | list | Yes | Event subscription patterns |
| `publishes` | list | Yes | Allowed event types |
| `default_publishes` | string | No | Default event if none explicit |
| `max_activations` | integer | No | Limit activations |
| `backend` | string | No | Backend override |
| `instructions` | string | Yes | Hat-specific prompt |

## Example Configurations

### Traditional Mode (Minimal)

```yaml
cli:
  backend: "claude"

event_loop:
  completion_promise: "LOOP_COMPLETE"
  max_iterations: 100
```

### Hat-Based Mode

```yaml
cli:
  backend: "claude"

event_loop:
  completion_promise: "LOOP_COMPLETE"
  max_iterations: 100
  starting_event: "task.start"

hats:
  planner:
    name: "Planner"
    triggers: ["task.start"]
    publishes: ["plan.ready"]
    instructions: |
      Create an implementation plan.

  builder:
    name: "Builder"
    triggers: ["plan.ready"]
    publishes: ["build.done"]
    instructions: |
      Implement the plan.
      Evidence required: tests pass.
```

### With Memories Disabled

```yaml
cli:
  backend: "claude"

event_loop:
  completion_promise: "LOOP_COMPLETE"

memories:
  enabled: false

tasks:
  enabled: false
```

### With Custom Guardrails

```yaml
cli:
  backend: "claude"

event_loop:
  completion_promise: "LOOP_COMPLETE"

core:
  guardrails:
    - "Always run tests before declaring done"
    - "Never modify production database"
    - "Follow existing code patterns"
```

## Environment Variables

| Variable | Description |
|----------|-------------|
| `RALPH_CONFIG` | Default config file path |
| `RALPH_DIAGNOSTICS` | Enable diagnostics (`1`) |
| `NO_COLOR` | Disable color output |

## Next Steps

- Explore [Presets](presets.md) for pre-configured workflows
- Learn about [CLI Reference](cli-reference.md)
- Understand [Backends](backends.md)
