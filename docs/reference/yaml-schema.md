# Configuration Reference (ralph.yml)

Complete reference for Ralph Orchestrator YAML configuration files.

## File Location

Ralph looks for `ralph.yml` in the current directory by default.
Override with the `-c` flag:

```bash
ralph run -c /path/to/config.yml
```

## Format Versions

Ralph supports two YAML configuration formats:

### V2 Nested Format (Recommended)

```yaml
cli:
  backend: claude
event_loop:
  max_iterations: 100
core:
  scratchpad: .agent/scratchpad.md
hats:
  builder:
    name: "Builder"
    # ...
```

### V1 Flat Format (Legacy)

Still supported for backwards compatibility:

```yaml
agent: claude
max_iterations: 100
prompt_file: PROMPT.md
```

V1 fields are automatically normalized to V2 structure at load time.

---

## Top-Level Structure

```yaml
# V2 sections (recommended)
cli: { ... }           # CLI backend configuration
event_loop: { ... }    # Loop control and limits
core: { ... }          # Shared paths and guardrails
hats: { ... }          # Custom hat definitions
events: { ... }        # Event metadata (optional)
adapters: { ... }      # Per-backend adapter settings
tui: { ... }           # TUI configuration

# Feature flags
verbose: false
archive_prompts: false  # Deferred feature
enable_metrics: false   # Deferred feature

# Warning control
_suppress_warnings: false  # Suppress all config warnings
```

---

## cli Section

Controls CLI backend selection and execution behavior.

```yaml
cli:
  backend: claude
  default_mode: autonomous
  idle_timeout_secs: 30
  experimental_tui: false
```

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `backend` | string | `"claude"` | Backend to use: `claude`, `kiro`, `gemini`, `codex`, `amp`, `auto`, or `custom` |
| `command` | string | null | Custom command (required when `backend: custom`) |
| `args` | string[] | `[]` | Additional CLI args for custom backend |
| `prompt_mode` | string | `"arg"` | How to pass prompt: `arg` or `stdin` |
| `prompt_flag` | string | null | Custom prompt flag (default: `-p`) |
| `default_mode` | string | `"autonomous"` | Default execution mode: `autonomous` or `interactive` |
| `idle_timeout_secs` | u32 | `30` | Idle timeout for interactive mode (0 = disabled) |
| `experimental_tui` | bool | `false` | Enable TUI mode |
| `dry_run` | bool | `false` | Show what would execute without running |
| `quiet` | bool | `false` | Suppress streaming output (for CI/scripting) |
| `color_mode` | string | `"auto"` | Color output: `auto`, `always`, or `never` |
| `record_session` | string | null | Record session to JSONL file path |

### Backend Selection

```yaml
# Named backends
cli:
  backend: claude    # Claude Code CLI
  backend: kiro      # Kiro CLI
  backend: gemini    # Gemini CLI
  backend: codex     # Codex CLI
  backend: amp       # Amp CLI
  backend: auto      # Auto-detect first available

# Custom backend
cli:
  backend: custom
  command: /usr/local/bin/my-agent
  args: ["--mode", "batch"]
  prompt_mode: stdin
```

---

## event_loop Section

Controls the orchestration loop behavior and limits.

```yaml
event_loop:
  prompt_file: PROMPT.md
  completion_promise: LOOP_COMPLETE
  max_iterations: 100
  max_runtime_seconds: 14400
  max_cost_usd: 50.0
  max_consecutive_failures: 5
```

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `prompt` | string | null | Inline prompt text (mutually exclusive with `prompt_file`) |
| `prompt_file` | string | `"PROMPT.md"` | Path to prompt file |
| `completion_promise` | string | `"LOOP_COMPLETE"` | String that signals loop completion |
| `max_iterations` | u32 | `100` | Maximum loop iterations |
| `max_runtime_seconds` | u64 | `14400` | Maximum runtime (4 hours) |
| `max_cost_usd` | f64 | null | Maximum cost in USD |
| `max_consecutive_failures` | u32 | `5` | Stop after N consecutive failures |
| `starting_event` | string | null | Initial event for hat workflows |

### Inline vs File Prompts

```yaml
# File-based prompt (default)
event_loop:
  prompt_file: PROMPT.md

# Inline prompt
event_loop:
  prompt: |
    Analyze the codebase and identify performance bottlenecks.
    Focus on database queries and API response times.
```

Note: `prompt` and `prompt_file` are mutually exclusive. You cannot specify both.

---

## core Section

Shared paths and guardrails injected into every prompt.

```yaml
core:
  scratchpad: .agent/scratchpad.md
  specs_dir: ./specs/
  guardrails:
    - "Fresh context each iteration - scratchpad is memory"
    - "Don't assume 'not implemented' - search first"
    - "Backpressure is law - tests/typecheck/lint must pass"
```

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `scratchpad` | string | `".agent/scratchpad.md"` | Shared state file between iterations |
| `specs_dir` | string | `"./specs/"` | Specs directory (source of truth) |
| `guardrails` | string[] | (see below) | Core behaviors injected into every prompt |

### Default Guardrails

```yaml
guardrails:
  - "Fresh context each iteration - scratchpad is memory"
  - "Don't assume 'not implemented' - search first"
  - "Backpressure is law - tests/typecheck/lint must pass"
```

---

## hats Section

Define custom hats for specialized workflows.

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

### Hat Configuration

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `name` | string | Yes | Human-readable name |
| `description` | string | Yes | Short purpose description (required for all hats) |
| `triggers` | string[] | No | Events that activate this hat |
| `publishes` | string[] | No | Events this hat can emit |
| `instructions` | string | No | Instructions prepended to prompts |
| `backend` | string or object | No | Backend override (inherits from `cli.backend`) |
| `default_publishes` | string | No | Fallback event if hat forgets to write one |

### Reserved Triggers

The following triggers are reserved for Ralph (the coordinator):
- `task.start`
- `task.resume`

Custom hats cannot use these triggers. Use semantic events like `work.start` instead.

### Hat Backend Formats

```yaml
hats:
  # Named backend (string)
  reviewer:
    backend: gemini

  # Kiro agent (object)
  builder:
    backend:
      type: kiro
      agent: builder

  # Custom backend (object)
  analyzer:
    backend:
      command: /usr/local/bin/custom-agent
      args: ["--mode", "analyze"]
```

---

## events Section

Define metadata for custom events (optional).

```yaml
events:
  deploy.start:
    description: "Deployment has been requested"
    on_trigger: "Prepare artifacts, validate config, check dependencies"
    on_publish: "Signal that deployment should begin"
```

| Field | Type | Description |
|-------|------|-------------|
| `description` | string | Brief description of the event |
| `on_trigger` | string | Instructions for hats receiving this event |
| `on_publish` | string | Instructions for hats emitting this event |

---

## adapters Section

Per-backend adapter settings.

```yaml
adapters:
  claude:
    timeout: 300
    enabled: true
  gemini:
    timeout: 300
    enabled: false
  kiro:
    timeout: 300
    enabled: true
  codex:
    timeout: 300
    enabled: true
  amp:
    timeout: 300
    enabled: true
```

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `timeout` | u64 | `300` | CLI execution timeout in seconds |
| `enabled` | bool | `true` | Include in auto-detection |

---

## tui Section

TUI (Terminal User Interface) configuration.

```yaml
tui:
  prefix_key: ctrl-a
```

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `prefix_key` | string | `"ctrl-a"` | Prefix key combination (e.g., `ctrl-a`, `ctrl-b`) |

---

## V1 Compatibility Fields

These flat fields are automatically normalized to V2 structure:

| V1 Field | Maps To | Description |
|----------|---------|-------------|
| `agent` | `cli.backend` | Backend selection |
| `agent_priority` | (internal) | Fallback order for auto-detection |
| `prompt_file` | `event_loop.prompt_file` | Prompt file path |
| `completion_promise` | `event_loop.completion_promise` | Completion marker |
| `max_iterations` | `event_loop.max_iterations` | Iteration limit |
| `max_runtime` | `event_loop.max_runtime_seconds` | Runtime limit |
| `max_cost` | `event_loop.max_cost_usd` | Cost limit |

### Dropped Fields

These fields are accepted but ignored (with warnings):

| Field | Reason |
|-------|--------|
| `max_tokens` | Token limits controlled by CLI tool |
| `retry_delay` | Retry logic handled differently in v2 |
| `adapters.*.tool_permissions` | CLI tool manages its own permissions |

---

## Complete Examples

### Minimal Configuration

```yaml
cli:
  backend: claude
```

### Solo Mode (No Hats)

```yaml
cli:
  backend: claude

event_loop:
  prompt_file: TASK.md
  max_iterations: 50
  max_cost_usd: 10.0
```

### Multi-Hat Workflow

```yaml
cli:
  backend: claude

event_loop:
  prompt_file: PROMPT.md
  max_iterations: 100
  starting_event: tdd.start

core:
  scratchpad: .agent/scratchpad.md
  specs_dir: ./specs/

hats:
  planner:
    name: "Planner"
    description: "Plans implementation approach"
    triggers:
      - planning.start
      - build.done
    publishes:
      - build.task
      - planning.complete
    instructions: |
      You are the planner. Create detailed implementation plans.
    backend: claude

  builder:
    name: "Builder"
    description: "Implements code from plans"
    triggers:
      - build.task
    publishes:
      - build.done
      - build.blocked
    instructions: |
      You are the builder. Write clean, tested code.
    backend: gemini
    default_publishes: build.done

  reviewer:
    name: "Reviewer"
    description: "Reviews code for quality"
    triggers:
      - review.request
    publishes:
      - review.approved
      - review.changes_requested
    instructions: |
      You are the reviewer. Focus on correctness and maintainability.
    backend:
      type: kiro
      agent: reviewer
```

### CI/CD Configuration

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

---

## CLI Flag Overrides

Many config options can be overridden via CLI flags:

| Config Field | CLI Flag |
|-------------|----------|
| `cli.backend` | `--backend`, `-b` |
| `event_loop.prompt_file` | `--prompt-file` |
| `event_loop.prompt` | `--prompt`, `-p` |
| `event_loop.max_iterations` | `--max-iterations`, `-n` |
| `event_loop.max_runtime_seconds` | `--max-runtime` |
| `event_loop.max_cost_usd` | `--max-cost` |
| `cli.dry_run` | `--dry-run` |
| `cli.quiet` | `--quiet`, `-q` |
| `cli.color_mode` | `--color` |
| `cli.record_session` | `--record-session` |

CLI flags take precedence over config file values.

---

## Validation

Ralph validates configuration and reports:

- **Errors** (fatal): Ambiguous routing, reserved triggers, missing required fields
- **Warnings** (non-fatal): Dropped fields, deferred features

Use `_suppress_warnings: true` to silence warnings in CI environments.

### Common Validation Errors

```yaml
# ERROR: Ambiguous routing - same trigger on multiple hats
hats:
  hat1:
    triggers: [build.done]
  hat2:
    triggers: [build.done]  # Error: ambiguous

# ERROR: Reserved trigger
hats:
  my_hat:
    triggers: [task.start]  # Error: reserved for Ralph

# ERROR: Missing description
hats:
  my_hat:
    name: "My Hat"
    triggers: [work.start]
    # Error: description is required
```
