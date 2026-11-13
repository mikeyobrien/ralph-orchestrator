# Configuration

Complete reference for Ralph's YAML configuration.

## Configuration File

Ralph composes configuration from up to three layers:

1. `~/.ralph/config.yml` when present — user-level defaults loaded automatically
2. `ralph.yml` in the current workspace (or `$RALPH_CONFIG` / `-c <file>`) — project-level overrides
3. `-c core.field=value` overrides — applied last

Project config overlays on top of the user config via deep merge. Mappings are merged recursively and scalar values or arrays from the project config replace the user-level value.

```bash
# Use the workspace config (and automatically merge ~/.ralph/config.yml if present)
ralph run

# Override the project config path
RALPH_CONFIG=/path/to/config.yml ralph run ...
ralph run -c custom-config.yml
```

### User-level config (`~/.ralph/config.yml`)

Use `~/.ralph/config.yml` for defaults you want everywhere, such as shared backend settings, global lifecycle hooks, or organization-wide guardrails.

A common pattern is keeping notification hooks global while leaving project-specific automation in the repo-local `ralph.yml`:

```yaml
# ~/.ralph/config.yml
hooks:
  enabled: true
  events:
    post.loop.complete:
      - name: notify-success
        command: ["./scripts/notify.sh", "complete"]
        on_error: warn
    post.loop.error:
      - name: notify-failure
        command: ["./scripts/notify.sh", "error"]
        on_error: warn
```

```yaml
# ./ralph.yml
hooks:
  events:
    pre.loop.start:
      - name: env-guard
        command: ["./scripts/hooks/env-guard.sh"]
        on_error: block
```

With those two files, Ralph loads both and deep-merges them before validation and execution.

## MCP Workspace Resolution

`ralph mcp serve` resolves its workspace root in this order:

1. `--workspace-root <path>`
2. `RALPH_API_WORKSPACE_ROOT`
3. current working directory

Use one MCP server instance per workspace/repo. Ralph's current control-plane APIs are
workspace-scoped: `config.*`, `task.*`, `loop.*`, `planning.*`, and `collection.*` all
read or persist state under a single root.

## CLI Config Overrides

You can override specific core fields from the command line without creating a separate config file. This is useful for:

- Running parallel Ralph instances with isolated scratchpads
- Testing with different specs directories
- CI/CD pipelines with dynamic paths

**Syntax:** `-c core.field=value`

**Supported fields:**

| Field | Description |
|-------|-------------|
| `core.scratchpad` | Path to scratchpad file (string shorthand for `scratchpad.path`) |
| `core.specs_dir` | Path to specs directory |

**Examples:**

```bash
# Override scratchpad (loads ralph.yml + applies override)
ralph run -c core.scratchpad=.ralph/agent/feature-auth/scratchpad.md

# Explicit config + override
ralph run -c ralph.yml -c core.scratchpad=.ralph/agent/feature-auth/scratchpad.md

# Multiple overrides
ralph run -c core.scratchpad=.runs/task-1/scratchpad.md -c core.specs_dir=./custom-specs/
```

Overrides are applied after `ralph.yml` is loaded, so they take precedence. The scratchpad directory is auto-created if it doesn't exist.

## Combined Config Compatibility (`-c` + `-H`)

Ralph supports both styles:
- **Single-file combined config**: `-c ralph.yml` with core + hats in one file
- **Split config**: `-c <core>` plus `-H <hats source>`

If both are used (`-c` contains hats and `-H` is provided), `-H` wins for workflow sections:
- `hats` and `events` from `-H` replace `hats`/`events` from `-c`
- `event_loop` values from `-H` override matching `event_loop` keys from `-c`
- `-c core.*=...` overrides still apply last

## Full Configuration Reference

```yaml
# Event loop settings
event_loop:
  completion_promise: "LOOP_COMPLETE"  # Output that signals completion
  max_iterations: 100                   # Maximum model iterations
  max_script_runs: 500                  # Maximum script hat runs
  max_runtime_seconds: 14400            # 4 hours max runtime
  idle_timeout_secs: 1800               # 30 min idle timeout
  starting_event: "task.start"          # First event published (hat mode)
  checkpoint_interval: 5                # Git checkpoint frequency
  prompt_file: "PROMPT.md"              # Default prompt file

# CLI backend settings
cli:
  backend: "claude"                     # Backend name
  prompt_mode: "arg"                    # arg or stdin
  effort: "medium"                      # Default reasoning effort for hats without their own
  model: "claude-opus-5-5"              # Default model ID for hats without their own

# Core behaviors
core:
  scratchpad:                            # Scratchpad configuration
    enabled: true                        # Enable scratchpad (default: true)
    path: .ralph/agent/scratchpad.md     # Scratchpad file path
  specs_dir: ".ralph/specs/"             # Committed specifications directory
  guardrails:                            # Rules injected into every prompt
    - "Fresh context each iteration"
    - "Never modify production database"

# Memories — persistent learning
memories:
  enabled: true                         # Enable memory system
  inject: auto                          # auto, manual, none
  budget: 2000                          # Max tokens to inject
  filter:
    types: []                           # Filter by memory type
    tags: []                            # Filter by memory tags
    recent: 0                           # Days limit (0 = no limit)

# Tasks — runtime work tracking
tasks:
  enabled: true                         # Enable task system

# Optional features
features:
  parallel: true                        # Allow worktree loops when primary lock is held
  auto_merge: false                     # Auto-merge worktree loops on completion
  preflight:
    enabled: false                      # Run preflight automatically on `ralph run`
    strict: false                       # Treat warnings as failures
    skip: []                            # Skip checks by name (for example: ["hooks"])

# Lifecycle hooks (v1)
hooks:
  enabled: false
  defaults:
    timeout_seconds: 30
    max_output_bytes: 8192
    suspend_mode: wait_for_resume
  events:
    pre.loop.start:
      - name: env-guard
        command: ["./scripts/hooks/env-guard.sh"]
        on_error: block
        mutate:
          enabled: false

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
    effort: "high"                      # Reasoning effort (low|medium|high|xhigh|max)
    model: "claude-fable-5-2"           # Model ID (inherits cli.model)
    scratchpad:                         # Per-hat scratchpad override
      enabled: true                     #   Enable scratchpad (default: true)
      path: .ralph/agent/my-hat.md      #   Scratchpad file path. Inherits from core if omitted.
    instructions: |
      Hat-specific instructions...
  my_script:
    name: "My Script"                   # A script hat: runs a command, no model
    description: "Checks the work"
    triggers: ["event.done"]
    publishes: ["check.done"]
    run: ["sh", "scripts/check.sh"]     # argv list, or a string run via `sh -c`
    timeout: 120                        # Seconds (default: the backend's adapter timeout)

# Gates — evidence an event must carry before it is routed
gates:
  - event: "check.done"
    requires:
      - field: commit
        verify: commit                  # Ralph checks the commit exists and is new
    blocked: "check.blocked"            # Default: "<prefix>.blocked"

# Fragments — instruction text for hats on matching models
fragments:
  - models: ["claude-fable-*"]          # Glob patterns (* and ?) on the full model ID
    hats: [my_hat]                      # Optional; default: every model hat + the coordinator
    text: |
      Model-specific guidance...
```

## Section Details

### event_loop

Controls the orchestration loop behavior.

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `completion_promise` | string | `"LOOP_COMPLETE"` | Output text that ends the loop |
| `max_iterations` | integer | `100` | Maximum model iterations before stopping. Script hat runs don't count |
| `max_script_runs` | integer | `500` | Maximum script hat runs before stopping (exit code 2). Must be at least 1 |
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
- `kiro` / `kiro-acp` — Kiro (both invoke the `kiro-cli` binary)
- `gemini` — Gemini CLI
- `codex` — Codex
- `forge` — Forge
- `amp` — Amp
- `copilot` — Copilot CLI
- `opencode` — OpenCode
- `pi` — Pi
- `roo` — Roo
- `omp` — OMP (oh-my-pi); Pi-family stream, OMP owns auth/providers/models
- `custom` — Custom adapter/backend (requires `cli.command`)
- `auto` — Pick the first available backend by priority (the default detection order)

**Prompt mode values:**
- `arg` — Pass as CLI argument: `cli -p "prompt"`
- `stdin` — Pass via stdin: `echo "prompt" | cli`

### adapters

Per-backend execution settings. A shared `default` applies to every backend; a
per-backend key overrides it for that backend only.

```yaml
adapters:
  default:
    timeout: 300        # seconds; applied to every backend without its own override
    enabled: true       # included in auto-detection
  claude:
    timeout: 600        # claude-specific override
  pi:
    timeout: 120        # pi-specific override
```

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `default.timeout` | integer | `300` | Inactivity timeout (seconds) for any backend without an override |
| `default.enabled` | boolean | `true` | Whether a backend participates in auto-detection |
| `<backend>.timeout` | integer | `default.timeout` | Per-backend timeout override |
| `<backend>.enabled` | boolean | `default.enabled` | Per-backend auto-detection override |

Override keys must be a catalogued backend name or `custom`; an unknown key is a
validation error. `enabled: false` skips a backend only during auto-detection —
selecting it explicitly (`cli.backend: <name>`) still works.

> **Migration notice — `adapters.claude` no longer inherits.** Previously Pi, Roo,
> Copilot, and OpenCode silently inherited the `adapters.claude` settings through a
> catch-all fallback. They now resolve to `adapters.default` (300 s) unless given
> their own override. If you raised `adapters.claude.timeout` to tune one of those
> backends, move the setting to its own key (for example `adapters.pi.timeout`) to
> preserve it. Ralph emits a one-time `ClaudeFallbackMigration` warning when
> `adapters.claude` is set and none of those backends has its own override.

### core

Core behaviors, scratchpad, and guardrails.

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `scratchpad` | string or object | `{ enabled: true, path: ".ralph/agent/scratchpad.md" }` | Scratchpad configuration (see below) |
| `scratchpad.enabled` | boolean | `true` | Enable the scratchpad |
| `scratchpad.path` | string | `".ralph/agent/scratchpad.md"` | Scratchpad file path |
| `specs_dir` | string | `".ralph/specs/"` | Committed specifications directory |
| `guardrails` | list | `[]` | Rules injected into every prompt |

The `scratchpad` field accepts a plain string (shorthand for setting `path` with `enabled: true`) or a structured object with `enabled` and `path`:

```yaml
# String shorthand — sets path, enabled defaults to true
core:
  scratchpad: ".workspace/plan.md"

# Structured object — full control
core:
  scratchpad:
    enabled: true
    path: .ralph/agent/scratchpad.md
```

> **Solo mode safety:** If scratchpad is disabled (`enabled: false`) but no hats are defined, Ralph force-enables it with a warning. Scratchpad is the only continuity mechanism in solo mode.

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

### features

Optional runtime capabilities.

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `parallel` | boolean | `true` | Spawn worktree loops when another loop holds the primary lock |
| `auto_merge` | boolean | `false` | Auto-merge completed worktree loops |
| `preflight.enabled` | boolean | `false` | Run `ralph preflight` checks automatically before `ralph run` |
| `preflight.strict` | boolean | `false` | Treat preflight warnings as failures |
| `preflight.skip` | list | `[]` | Skip checks by name (for example `hooks`, `git`) |

When `features.preflight.enabled: true`, `ralph run` uses the default preflight suite:
`config`, `hooks`, `backend`, `telegram`, `git`, `paths`, `tools`, and `specs`.

### hooks

Lifecycle hooks for orchestrator phase-events (v1).

Hooks can be defined in either the user-level `~/.ralph/config.yml` or the workspace `ralph.yml`. Ralph loads the user config first, then overlays the project config on top. That means hooks in the user config apply globally unless the project config replaces the same event mapping.

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `enabled` | boolean | `false` | Enable hook dispatch for lifecycle events |
| `defaults.timeout_seconds` | integer | `30` | Default per-hook timeout in seconds |
| `defaults.max_output_bytes` | integer | `8192` | Default stdout/stderr cap per stream |
| `defaults.suspend_mode` | enum | `wait_for_resume` | Default suspend mode for `on_error: suspend` |
| `events` | map | `{}` | Mapping from lifecycle phase-event key to list of hook specs |

Supported v1 lifecycle phase-event keys under `hooks.events`:

- `pre.loop.start`, `post.loop.start`
- `pre.iteration.start`, `post.iteration.start`
- `pre.plan.created`, `post.plan.created`
- `pre.human.interact`, `post.human.interact`
- `pre.loop.complete`, `post.loop.complete`
- `pre.loop.error`, `post.loop.error`

Hook spec (`HookSpec`) fields:

| Field | Required | Description |
|-------|----------|-------------|
| `name` | Yes | Stable identifier used in telemetry/diagnostics |
| `command` | Yes | Command argv array (`command[0]` must resolve to an executable) |
| `cwd` | No | Working directory override (absolute or workspace-relative) |
| `env` | No | Environment variable overrides for the hook process |
| `timeout_seconds` | No | Per-hook timeout override (must be > 0) |
| `max_output_bytes` | No | Per-hook output cap override per stream (must be > 0) |
| `on_error` | Yes | Failure disposition: `warn`, `block`, or `suspend` |
| `suspend_mode` | No | Suspend strategy override (`wait_for_resume`, `retry_backoff`, `wait_then_retry`) |
| `mutate.enabled` | No | Opt-in hook stdout mutation parsing (default `false`) |
| `mutate.format` | No | Optional format guardrail; only `json` is allowed in v1 |

Mutation scope in v1 is intentionally narrow:

- Mutation parsing only happens when `mutate.enabled: true`.
- Hook stdout must be JSON using the v1 contract: `{"metadata": { ... }}`.
- Only metadata namespace updates are allowed (`metadata.accumulated.hook_metadata.<hook_name>`).
- Prompt/event/config mutation is out of scope for v1.

Minimal runnable example:

- Config: [`examples/hooks/minimal/ralph.hooks.yml`](https://github.com/mikeyobrien/ralph-orchestrator/blob/main/examples/hooks/minimal/ralph.hooks.yml)
- Scripts: [`examples/hooks/scripts/env-guard.sh`](https://github.com/mikeyobrien/ralph-orchestrator/blob/main/examples/hooks/scripts/env-guard.sh), [`examples/hooks/scripts/notify.sh`](https://github.com/mikeyobrien/ralph-orchestrator/blob/main/examples/hooks/scripts/notify.sh)
- Validate: `ralph hooks validate -c examples/hooks/minimal/ralph.hooks.yml`

### hats

Specialized personas for hat-based mode.

| Option | Type | Required | Description |
|--------|------|----------|-------------|
| `name` | string | Yes | Display name |
| `description` | string | Yes | Purpose description |
| `triggers` | list | Yes | Event subscription patterns |
| `publishes` | list | Yes | Allowed event types |
| `default_publishes` | string | No | Event published when an iteration emits none |
| `max_activations` | integer | No | Limit activations (script runs count) |
| `run` | string or list | No | Run this command instead of a model. See [Script hats](#script-hats) |
| `timeout` | integer | No | Seconds before the hat's backend or script is stopped |
| `backend` | string | No | Backend override |
| `effort` | string | No | Reasoning effort for this hat: `low`, `medium`, `high`, `xhigh`, or `max`. Inherits `cli.effort`. Passed as `--effort` to Claude; other backends ignore it |
| `model` | string | No | Model ID for this hat. Inherits `cli.model`. Passed as `--model <id>` to Claude and Kiro and `-m <id>` to Codex; other backends ignore it. A model flag already in the hat's backend args wins |
| `scratchpad` | string or object | No | Per-hat scratchpad override (inherits `core.scratchpad` if omitted) |
| `instructions` | string | Yes, unless `run` is set | Hat-specific prompt. Not allowed with `run` |

Each hat can override the global scratchpad with its own `scratchpad` field. Like the core-level setting, it accepts a plain string or a structured object:

```yaml
hats:
  planner:
    scratchpad: .ralph/agent/planner.md       # String shorthand
    # ...
  builder:
    scratchpad:
      path: .ralph/agent/builder.md           # Structured with custom path
    # ...
  validator:
    scratchpad:
      enabled: false                          # Disable scratchpad entirely
    # ...
  reviewer:                                   # No scratchpad key = inherits global
    # ...
```

**Resolution order:** hat override → `core.scratchpad` → defaults.

### Script hats

A hat with `run` is a script hat. It is triggered by events and publishes
events like any hat, but each activation runs a command instead of a model.
Use it for deterministic steps (advancing a queue, merging a branch, running a
check) that would otherwise cost a full model iteration.

```yaml
hats:
  advance:
    name: "Queue keeper"
    description: "Dispatches the next source, or finishes"
    triggers: ["queue.start", "merge.done"]
    publishes: ["queue.advance", "queue.done"]
    run: ["sh", "scripts/advance.sh"]     # argv list
    timeout: 120
    default_publishes: "queue.advance"    # used when the command exits 0 without emitting
  merger:
    name: "Merger"
    description: "Merges the finished source branch"
    triggers: ["source.done"]
    publishes: ["merge.done"]
    run: 'git merge --no-edit "$RALPH_TRIGGER_PAYLOAD" && "$RALPH_BIN" emit merge.done "merged=$(git rev-parse HEAD)"'
```

`run` is either an argv list (`command[0]` must be an executable) or a string,
which runs as `sh -c "<string>"`. The command runs in the workspace root and
publishes with `"$RALPH_BIN" emit <topic> <payload>`, exactly as an agent does.
Its events then go through the normal path: gates, completion handling, and
routing.

**Validation:**
- `run` can't be combined with `instructions` or `extra_instructions`. The error names the hat.
- `run` can't be empty or contain a NUL byte, and `concurrency` must be 1 (script hats don't run as waves).
- `default_publishes` can't be the completion promise. Ralph never completes the loop from a default event, so the script would rerun until the loop goes stale. Have the script emit the promise itself.
- `backend`, `backend_args`, `effort`, `model`, and `disallowed_tools` are ignored on a script hat, with a warning.

**Environment.** The command inherits Ralph's environment, plus:

| Variable | Value |
|---|---|
| `RALPH_BIN` | The running Ralph executable. Its directory is also first on `PATH` |
| `RALPH_EVENTS_FILE` | The events file `ralph emit` writes to |
| `RALPH_WORKSPACE_ROOT` | Workspace root (also the working directory) |
| `RALPH_HAT` | Hat id |
| `RALPH_ITERATION`, `RALPH_LOOP_ID` | Iteration number and loop id |
| `RALPH_TRIGGER_TOPIC` | Topic of the first triggering event |
| `RALPH_TRIGGER_PAYLOAD` | Its payload, truncated to 64 KiB, with NUL bytes removed |
| `RALPH_TRIGGER_EVENTS_FILE` | JSONL file with every consumed event (`{"topic", "payload", "source"}`), untruncated. Deleted after the run |

stdin carries `{"hat", "iteration", "trigger_topic"}` as JSON. A command
doesn't have to read it.

**Outcomes:**

| Command result | What Ralph does |
|---|---|
| Exit 0, emitted at least one event (the completion or cancellation promise counts) | Normal iteration |
| Exit 0, emitted nothing | Publishes `default_publishes` if set, otherwise `<hat>.failed` with `command exited 0 without emitting` |
| Non-zero exit, timeout, or could not start | Discards anything it emitted, and publishes `<hat>.failed`. Counts toward `max_consecutive_failures` |

The `<hat>.failed` payload's first line is the cause (`command exited 3`,
`command timed out after 120s`, `command was killed by a signal`,
`could not run command: ...`). Then comes how many emitted events were
discarded, if any, and the last 4 KiB of stderr. Like any event,
`<hat>.failed` goes to a hat that triggers on it, or to Ralph.

**Scope.** Scope enforcement is always on for script hats, whatever
`enforce_hat_scope` says. An emitted topic outside `publishes` is dropped and
`<hat>.scope_violation` is published instead.

**Accounting.** A script run gets an iteration number. It shows in the TUI
(`<hat> (script)`), diagnostics, and `iteration.summary` (backend `script`,
cost 0), and fires the lifecycle hooks like any iteration. It counts toward the
hat's `max_activations`, stale-loop detection, and `event_loop.max_script_runs`,
but not toward `max_iterations`, which bounds model work. Two script hats that
trigger each other stop at `max_script_runs`, or sooner through stale-loop
detection.

**Limits.**
- The hat's `timeout` stops the command. If the hat sets none, the global backend's adapter timeout (`adapters.<backend>.timeout`, default 300) applies. The command runs in its own process group, so a timeout or Ctrl+C stops its children too.
- Each output stream is captured up to `hooks.defaults.max_output_bytes` (8192 by default). Anything past that is dropped without blocking the command.
- A child left running in the background (`sleep 60 &`) keeps the output pipes open, so the run lasts until that child exits or the timeout fires. Redirect a background child's output (`cmd >/dev/null 2>&1 &`), or don't leave one running.

## Fragments

`fragments:` adds instruction text to a hat only while the hat runs on a
matching model. Swap the hat's model and its fragments follow with no other
edit, so model-specific prompting advice does not linger in `instructions`
after a model change.

```yaml
hats:
  planner:
    model: "claude-fable-5-2"
    # ...
  builder:
    model: "claude-opus-5-5"
    # ...

fragments:
  - models: ["claude-fable-*"]
    hats: [planner, ralph]
    text: |
      You are the owner and long-term maintainer of this codebase, and its design is up to you.
```

| Field | Type | Required | Description |
|-------|------|----------|-------------|
| `models` | list | Yes | Glob patterns matched against the hat's model ID. `*` matches any run of characters, `?` one character. Matching is case-sensitive and covers the whole ID, so `claude-fable` does not match `claude-fable-5-2` but `claude-fable*` does |
| `hats` | list | No | Hat IDs to target. `ralph` targets the coordinator prompt (the prompt that runs when no hat subscribes to a topic). Omit to target every model hat plus the coordinator |
| `text` | string | Yes | Text appended to the hat's instructions |

**Which model a hat runs on.** Ralph reads it the way it builds the backend
command line. A model flag already in the hat's backend args (`--model <id>`,
`--model=<id>`, or `-m <id>`) wins, because the backend runs that model. The
args checked are the hat backend's own `args` (or `cli.args` when the hat uses
the global backend), then `backend_args`. Otherwise the hat's `model`, else
`cli.model`. The coordinator uses a model flag in `cli.args`, else `cli.model`.
A hat with no model anywhere matches no fragment. So does a hat whose model
flag has no value.

Fragments match the literal configured string. A Claude Code alias such as
`opus` does not match `claude-opus-*`; use full model IDs.

**Order.** Matching fragments are appended to the hat's instructions after
`extra_instructions`, in the order they appear in `fragments:`. This happens
once, when the config loads.

**Script hats.** Fragments never apply to script hats (`run:`). A fragment
whose `hats` names a script hat fails config load.

**The coordinator.** Fragments that target `ralph` (or omit `hats`) are added to
the coordinator prompt, after its workflow section, only on iterations where
Ralph coordinates. A hat iteration carries only that hat's own fragments.

**Sharing text.** YAML anchors let one block serve several fragments:

```yaml
_fable_reports: &fable_reports |
  # Communicating clearly
  Write for an intelligent but low-context reader. ...

fragments:
  - models: ["claude-fable-*"]
    hats: [finalizer, critic]
    text: *fable_reports
```

**Hats files and presets.** A hats file (`-H`) or a builtin preset can carry
`fragments:`. They are appended after any `fragments:` in the core config, so
both apply.

**Validation.** Config load fails, naming the fragment as `fragments[<index>]`, when:

- `models` is empty or has an empty pattern
- `text` is empty
- `hats` is an empty list or has an empty ID
- `hats` names `ralph` while a hat is also named `ralph`
- `hats` names a script hat
- a key is missing or misspelled (`hat:` for `hats:` would otherwise widen the fragment to every hat)

**Checking what applies.** `ralph hats validate` lists, for each hat and the
coordinator, its model and the `fragments[<index>]` entries that apply
(`none` for a hat with no model). It warns about any fragment that applies to
no hat and not to the coordinator, naming its index, its `models` patterns, and
its `hats` list, which usually means a typo in a glob or a hat ID. It also fails
on any config error that `ralph run` would reject at load.

```text
Checks:
  [warn] fragments[2] matches no hat or the coordinator (models: ["claude-fabel-*"], hats: ["planner"])

Fragments:
  builder              claude-opus-5-5          fragments[1]
  planner              claude-fable-5-2         fragments[0]
  ralph                claude-opus-5-5          none  (coordinator)
```

## Gates

`gates:` declares the evidence an event must carry before Ralph routes it. An
event that fails its gate is replaced by a blocked event. Gates let a preset
reject unverified work instead of asking the agent in prose to verify it.

```yaml
gates:
  - event: "foundation.done"
    requires:
      - field: commit
        verify: commit          # a real commit, reachable from HEAD, created this iteration
  - event: "merge.done"
    requires:
      - field: merged
        verify: { commit: { reachable_from: "HEAD", new: false } }
  - event: "review.ready"
    requires:
      - field: tests
        status: passed
      - field: coverage
        min: 80
      - command: ["python3", "-m", "unittest", "-q"]   # Ralph runs it; exit 0 passes
        timeout: 300
    blocked: "review.blocked"
```

| Option | Required | Description |
|---|---|---|
| `event` | Yes | Exact topic the gate applies to (no wildcards) |
| `requires` | No | Requirements, checked in order. All must pass |
| `blocked` | No | Topic published when the gate fails. Default: the event's prefix plus `.blocked` (`merge.done` → `merge.blocked`) |

### Requirement kinds

| Requirement | Passes when | Trust |
|---|---|---|
| `field: k` | `k` is in the payload with a non-empty value | Agent's claim |
| `field: k` + `status: passed` | `k` carries that status word | Agent's claim |
| `field: k` + `min: n` / `max: n` | `k` parses as a number within the bounds (inclusive; a trailing `%` is allowed) | Agent's claim |
| `field: k` + `verify: commit` | `k` is a commit, reachable from `HEAD`, and not reachable from the `HEAD` recorded when the iteration started | Checked by Ralph |
| `field: k` + `verify: { commit: { reachable_from: <ref>, new: <bool> } }` | As above, from `<ref>` instead of `HEAD`. `new: false` drops the "created this iteration" check | Checked by Ralph |
| `command: [argv]` (+ `timeout`) | Ralph runs the command and it exits 0 | Checked by Ralph |

Checking stops at the first failure, so put cheap field checks before an
expensive `command`.

**Payloads.** Field requirements read the event payload as a JSON object or as
`key=value` tokens:

```bash
ralph emit review.ready "tests=passed coverage=87% commit=abc1234"
ralph emit review.ready --json '{"tests": "passed", "coverage": 87}'
ralph emit review.ready --json '{"tests": {"value": "42", "status": "passed"}}'
```

A token value may be quoted (`note="two words"`) and may be followed by a
status word (`tests=42 passed`). The status words are `passed`, `passing`,
`failed`, and `failing`. A value that is itself the status word
(`tests=passed`) also counts as a status. Values that are empty, `false`,
`null`, an array, or an object without `value` count as missing.

**Commit checks** run `git` in the workspace. A value that isn't a commit fails
with `not a commit`. If git itself fails (for example, the workspace isn't a
repository), the check fails with `git failed`, never passes.

**Command requirements** get `RALPH_EVENT_TOPIC` and `RALPH_EVENT_PAYLOAD`
(capped at 64 KiB) in their environment, and `{"topic"}` on stdin. They run in
the workspace root, in their own process group. `timeout` defaults to 300
seconds. A timed-out command is stopped with its children.

### Blocked events

A failing event is not routed. Ralph publishes the blocked topic instead. Its
payload names the failed requirement and the reason, then gives details and
the original payload:

```text
Gate on `review.ready` failed: field `coverage`: threshold
value 62 is below min 80

Original payload:
tests=passed coverage=62%
```

Reasons are `missing`, `status`, `threshold`, `not a commit`, `not reachable`,
`not new`, `git failed`, `command exited N` (with the stderr tail),
`command timed out`, and `command failed`. A blocked event routes like any
other: to a hat that triggers on it, or to Ralph. Each block is logged as
`BackpressureTriggered` in diagnostics. Repeated blocks feed the usual
thrashing and stale-loop detection.

Gates apply wherever an event enters the loop: `ralph emit`, wave dispatch,
events that arrive while Ralph waits for a human, and events replayed on
resume.

### Gating completion

A gate on the completion promise stops a run from ending without evidence:

```yaml
gates:
  - event: "LOOP_COMPLETE"
    requires:
      - command: ["cargo", "test", "--quiet"]
```

If the gate fails, the loop goes on and the blocked event (`LOOP_COMPLETE.blocked`
here) is published. A completion promise found only in the agent's output text
is gated too, with an empty payload, so only `command` requirements can pass it.

### Validation

- Two gates on the same event is an error.
- A `command` with an empty argv, or a NUL byte in any argument, is an error.
- The blocked topic can't be the gated event, another gate's event, the completion or cancellation promise, or `build.done`, `review.done`, or `verify.passed`. Each of these would let a failing event through unchecked.
- A gate on an event that no hat publishes is a warning.

The built-in `build.done` and `review.done` payload checks still apply
alongside gates.

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

### With Per-Hat Scratchpads

```yaml
cli:
  backend: "claude"

event_loop:
  completion_promise: "LOOP_COMPLETE"
  starting_event: "task.start"

core:
  scratchpad:
    enabled: true
    path: .ralph/agent/scratchpad.md

hats:
  planner:
    name: "Planner"
    scratchpad:
      path: .ralph/agent/planner.md
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

  reviewer:
    name: "Reviewer"
    scratchpad:
      enabled: false
    triggers: ["build.done"]
    publishes: ["review.done"]
    instructions: |
      Review the implementation. No scratchpad needed.
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
