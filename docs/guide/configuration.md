# Configuration Guide

Ralph Orchestrator provides flexible configuration through YAML files and CLI flags to control execution, manage costs, and customize workflows.

## Configuration File

Ralph uses YAML configuration files. Create a `ralph.yml` in your project root:

```yaml
cli:
  backend: claude

event_loop:
  prompt_file: PROMPT.md
  max_iterations: 100
  max_cost_usd: 50.0
```

Use a different config file with the `-c` flag:

```bash
ralph run -c custom-config.yml
```

## Quick Start Examples

### Basic Usage

```yaml
# ralph.yml - minimal config
cli:
  backend: claude
```

Run with:

```bash
ralph run -p "Implement the feature described in PROMPT.md"
```

### With Limits

```yaml
cli:
  backend: claude

event_loop:
  max_iterations: 50
  max_runtime_seconds: 3600  # 1 hour
  max_cost_usd: 25.0
```

### Using a Preset

```bash
# Use a pre-configured workflow
ralph run -c presets/research.yml -p "How does auth work in this codebase?"
```

## Configuration Sections

### cli - Backend Selection

Controls which AI CLI tool to use:

```yaml
cli:
  backend: claude          # claude, kiro, gemini, codex, amp, auto, custom
  default_mode: autonomous # autonomous or interactive
  idle_timeout_secs: 30    # For interactive mode
```

**Available backends:**

| Backend | Description |
|---------|-------------|
| `claude` | Claude Code CLI (default) |
| `kiro` | Kiro CLI |
| `gemini` | Gemini CLI |
| `codex` | Codex CLI |
| `amp` | Amp CLI |
| `auto` | Auto-detect first available |
| `custom` | Custom command (requires `command` field) |

**Custom backend example:**

```yaml
cli:
  backend: custom
  command: /usr/local/bin/my-agent
  args: ["--mode", "batch"]
  prompt_mode: stdin
```

### event_loop - Execution Limits

Controls the orchestration loop:

```yaml
event_loop:
  prompt_file: PROMPT.md        # Or use inline prompt
  completion_promise: LOOP_COMPLETE
  max_iterations: 100
  max_runtime_seconds: 14400    # 4 hours
  max_cost_usd: 50.0
  max_consecutive_failures: 5
```

**Inline prompt (alternative to file):**

```yaml
event_loop:
  prompt: |
    Analyze performance bottlenecks in the database layer.
    Focus on query optimization.
```

Note: `prompt` and `prompt_file` are mutually exclusive.

### core - Shared Settings

Paths and guardrails shared across all iterations:

```yaml
core:
  scratchpad: .agent/scratchpad.md
  specs_dir: ./specs/
  guardrails:
    - "Fresh context each iteration - scratchpad is memory"
    - "Don't assume 'not implemented' - search first"
    - "Backpressure is law - tests/typecheck/lint must pass"
```

### hats - Custom Workflows

Define specialized hats for complex workflows:

```yaml
hats:
  builder:
    name: "Builder"
    description: "Implements features from spec"
    triggers:
      - build.task
    publishes:
      - build.done
      - build.blocked
    instructions: |
      You are the builder. Implement the feature as specified.
    backend: claude
    default_publishes: build.done
```

**Important:** Every hat requires a `description` field.

See [Hat System Guide](hat-system.md) for detailed hat configuration.

## CLI Flags

Common flags that override config values:

```bash
# Backend selection
ralph run -b gemini -p "Your prompt"
ralph run --backend kiro -p "Your prompt"

# Limits
ralph run -n 50 -p "Your prompt"          # Max iterations
ralph run --max-runtime 3600 -p "Your prompt"  # 1 hour limit
ralph run --max-cost 25.0 -p "Your prompt"

# Output control
ralph run -q -p "Your prompt"             # Quiet mode
ralph run --color never -p "Your prompt"  # Disable colors
ralph run --dry-run -p "Your prompt"      # Show what would run

# Session recording
ralph run --record-session session.jsonl -p "Your prompt"
```

Run `ralph run --help` for the complete list.

## Configuration Profiles

### Development

```yaml
cli:
  backend: claude
  experimental_tui: true

event_loop:
  max_iterations: 20
  max_cost_usd: 5.0
```

### CI/CD

```yaml
cli:
  backend: claude
  quiet: true
  color_mode: never

event_loop:
  max_iterations: 50
  max_runtime_seconds: 3600
  max_cost_usd: 25.0

_suppress_warnings: true
```

### Production

```yaml
cli:
  backend: claude

event_loop:
  max_iterations: 100
  max_runtime_seconds: 14400
  max_cost_usd: 100.0
  max_consecutive_failures: 3
```

## Hat Configuration (v2.0+)

Ralph v2.0 introduces "Hatless Ralph" - a constant coordinator with optional, configurable hats.

For comprehensive hat system documentation, see the [Hat System Guide](hat-system.md).

### Hat Backends

Each hat can specify its own backend:

```yaml
cli:
  backend: claude  # Default backend for Ralph

hats:
  builder:
    name: "Builder"
    description: "Implements code changes"
    backend: gemini  # This hat uses Gemini
    triggers: ["build.task"]
    publishes: ["build.done"]

  reviewer:
    name: "Reviewer"
    description: "Reviews code for quality"
    backend:
      type: kiro
      agent: codex  # Kiro with custom agent
    triggers: ["review.request"]
    publishes: ["review.done"]
```

**Backend types:**

| Type | Format | Example |
|------|--------|---------|
| Named | String | `backend: claude` |
| Kiro Agent | Object | `backend: {type: kiro, agent: codex}` |
| Custom | Object | `backend: {command: ./my-agent, args: [--flag]}` |

### Default Publishes

Hats can specify a fallback event if they forget to write one:

```yaml
hats:
  builder:
    name: "Builder"
    description: "Implements code changes"
    triggers: ["build.task"]
    publishes: ["build.done", "build.blocked"]
    default_publishes: "build.done"
```

If the builder completes without writing events to `.agent/events.jsonl`, Ralph automatically injects `build.done`.

### Solo Mode vs Multi-Hat Mode

**Solo mode** (no hats):
```yaml
cli:
  backend: claude

event_loop:
  prompt_file: PROMPT.md
  completion_promise: "LOOP_COMPLETE"
# No hats section - Ralph handles everything
```

**Multi-hat mode**:
```yaml
cli:
  backend: claude

event_loop:
  starting_event: "task.start"
  completion_promise: "LOOP_COMPLETE"

hats:
  builder:
    name: "Builder"
    description: "Implements code changes"
    triggers: ["build.task"]
    publishes: ["build.done"]
    backend: claude

  tester:
    name: "Tester"
    description: "Runs tests and reports results"
    triggers: ["test.request"]
    publishes: ["test.pass", "test.fail"]
    backend: gemini
```

### Using Presets

Ralph ships with 23 pre-configured hat collections:

```bash
# List available presets
ralph init --list-presets

# Initialize with a preset
ralph init --preset tdd-red-green
```

See the [Preset Reference](../reference/presets.md) for all available presets.

## V1 Compatibility

Ralph still supports the flat V1 configuration format:

```yaml
# V1 format (still works)
agent: claude
prompt_file: PROMPT.md
max_iterations: 100
max_runtime: 14400
max_cost: 50.0
```

These fields are automatically normalized to V2 structure. You can mix formats:

```yaml
# Mixed format
agent: claude              # V1 field
event_loop:                # V2 section
  max_iterations: 50
```

## Dropped Fields

These V1 fields are accepted but ignored:

| Field | Reason |
|-------|--------|
| `max_tokens` | Token limits controlled by CLI tool |
| `retry_delay` | Retry logic handled differently |
| `tool_permissions` | CLI tool manages its own permissions |

Ralph warns about dropped fields unless `_suppress_warnings: true` is set.

## Best Practices

1. **Start with a preset** - Use `presets/` as starting points
2. **Set cost limits** - Always configure `max_cost_usd` for safety
3. **Use scratchpad** - Let Ralph persist state between iterations
4. **Run tests** - Ralph's default guardrails enforce test passing
5. **Version control config** - Commit your `ralph.yml` files

## Next Steps

- [Hat System Guide](hat-system.md) - Comprehensive hat documentation
- [Preset Reference](../reference/presets.md) - 23 pre-configured workflows
- [Cost Management](cost-management.md) - Budget control strategies
