---
status: review
gap_analysis: 2026-01-13
related:
  - cli-adapters.spec.md
  - interactive-mode.spec.md
  - terminal-ui.spec.md
---

# Event Loop Specification

## Overview

Event-driven orchestration loop with pub/sub messaging. Ralph works autonomously: planning work, implementing tasks, and validating completion. The observer pattern enables extensibility—custom hats, logging, metrics, and other behaviors can subscribe to events without modifying core logic.

## Architecture

There's only one Ralph. Ralph wears different hats.

```
┌─────────────────────────────────────────────────────────────────────┐
│                           Ralph                                      │
│                                                                      │
│  ┌─────────────────────────────────────────────────────────────┐    │
│  │                     CORE BEHAVIORS                           │    │
│  │  • Fresh context each iteration                              │    │
│  │  • Scratchpad access (.agent/scratchpad.md)                  │    │
│  │  • Search-first guardrail                                    │    │
│  │  • Backpressure compliance                                   │    │
│  └─────────────────────────────────────────────────────────────┘    │
│                              │                                       │
│                    ┌─────────┴─────────┐                            │
│                    ▼                   ▼                            │
│           ┌──────────────┐    ┌──────────────┐                      │
│           │  🎩 Planner  │    │  🔨 Builder  │    ... custom hats   │
│           │  (adds plan  │    │  (adds build │                      │
│           │  instructions)│    │  instructions)│                      │
│           └──────────────┘    └──────────────┘                      │
│                                                                      │
└──────────────────────────────┬──────────────────────────────────────┘
                               │
                               ▼
                    ┌─────────────────────┐
                    │     Event Bus       │◀─── Observers (logging,
                    │     (pub/sub)       │     metrics, custom)
                    └─────────────────────┘
                               │
              ┌────────────────┼────────────────┐
              ▼                ▼                ▼
         task.start      build.task       build.done
         build.blocked   review.request   (etc.)
```

**Key insight:** Hats are instruction layers, not separate agents. The orchestrator routes events to trigger hat changes. Core behaviors are always present—hats add to them, never replace.

### There Is No "Single-Hat Mode" vs "Multi-Hat Mode"

This is a common misconception. There is only **one Ralph** who wears **one or more hats**. The orchestrator always:

1. Registers hats (default: planner + builder)
2. Routes events to trigger hat changes
3. Runs one hat per iteration

What varies is **which hats are registered**, not the "mode":

| Configuration | Hats Registered | Behavior |
|---------------|-----------------|----------|
| Default (no custom hats) | planner, builder | Ralph alternates between planning and building |
| Custom hats defined | planner, builder, reviewer, ... | Ralph wears whichever hat the event triggers |
| Single hat override | just builder (or custom) | Ralph always wears the same hat (rare, for simple tasks) |

**Anti-pattern:** Logs or code that reference "single-hat mode" or "multi-hat mode" should instead log which hats are registered:
- ✅ `"Ralph ready with hats: planner, builder"`
- ✅ `"Ralph ready with hats: planner, builder, reviewer"`
- ❌ `"Starting in multi-hat mode"`
- ❌ `"Starting in single-hat mode"`

## Core Behaviors (All Hats)

Every Ralph invocation includes these behaviors, regardless of which hat is active:

| Behavior | Description |
|----------|-------------|
| **Fresh context** | Each iteration starts with cleared context. The scratchpad is memory. |
| **Scratchpad access** | `.agent/scratchpad.md` is always readable and writable. |
| **Specs awareness** | `./specs/` directory is the source of truth for requirements. |
| **Search-first guardrail** | Never assume "not implemented"—search the codebase first. |
| **Backpressure compliance** | Tests, typecheck, lint must pass before claiming completion. |

**Hats add instructions on top of core—they never replace core behaviors.**

## Hats

Hats define two things:
1. **Triggers** — Which events cause Ralph to wear this hat
2. **Instructions** — What additional guidance this hat provides

### 🎩 Planner Hat

Ralph puts on the planner hat to figure out what needs doing.

| Property | Value |
|----------|-------|
| ID | `planner` |
| Triggers | `task.start`, `task.resume`, `build.done`, `build.blocked` |
| Publishes | `build.task` |

**Instructions added:**
- Gap analysis from `./specs/` to identify work
- Create and maintain `.agent/scratchpad.md`
- Dispatch `build.task` events one at a time
- Validate completion claims
- Output completion promise when ALL work is done
- ❌ Don't implement code
- ❌ Don't make commits

### 🔨 Builder Hat

Ralph puts on the builder hat to get stuff done.

| Property | Value |
|----------|-------|
| ID | `builder` |
| Triggers | `build.task` |
| Publishes | `build.done`, `build.blocked` |

**Instructions added:**
- Pick highest priority `[ ]` task from scratchpad
- Implement it following existing patterns
- Run backpressure (tests, typecheck, lint)
- Commit and publish `build.done`
- Report `build.blocked` if stuck
- Mark task `[x]` in scratchpad when done
- ❌ Don't create the scratchpad
- ❌ Don't output completion promise

## Event Flow

### Happy Path

```
1. [Loop]    → task.start  → [🎩 Planner]
2. [🎩]      creates scratchpad, dispatches first task
3. [🎩]      → build.task  → [🔨 Builder]
4. [🔨]      implements, validates, commits
5. [🔨]      → build.done  → [🎩]
6. [🎩]      verifies, dispatches next task (or completes)
7. ... Ralph keeps switching hats until done ...
8. [🎩]      outputs LOOP_COMPLETE
```

### Blocked Path

```
1. [🔨] → build.blocked → [🎩]
   "Can't proceed: missing X dependency"
2. [🎩] adds task to scratchpad, reprioritizes
3. [🎩] → build.task    → [🔨]
   "Install X dependency"
4. ... Ralph figures it out ...
```

### Escape Hatches (Avoiding Stuck States)

Ralph has several ways to avoid getting stuck:

1. **Cancel impossible tasks.** Planner can mark tasks `[~]` with explanation if they're genuinely unresolvable. Move on.

2. **Fresh context resets perspective.** Each iteration starts fresh. What seemed impossible last time might click this time.

3. **Safeguards terminate runaway loops.** `max_consecutive_failures` catches spiraling. Better to stop and ask a human than burn tokens.

4. **Planner owns the plan.** If Builder keeps blocking on the same thing, Planner can reframe the work, split tasks differently, or cancel and try another approach.

**The key insight:** Ralph doesn't need to solve everything. Ralph needs to make progress or recognize when to stop. Blocking is information—use it to adjust the plan, not spiral.

## ⚠️ CRITICAL: Self-Routing Is Allowed

**This is a key design decision that affects system resilience.**

Events **CAN** route back to the hat that emitted them. There is NO source-based filtering that prevents a hat from receiving events it published.

### Why This Matters

LLMs are non-deterministic. Even with perfect prompts, an agent might emit the "wrong" event:
- Planner might emit `build.done` instead of dispatching `build.task`
- Builder might emit `build.task` instead of `build.done`

If source-based filtering blocked self-routing, this would cause **premature termination**:

```
❌ WITH source filtering (broken):
   planner → build.done → (blocked: source=planner) → NO RECIPIENTS → terminate

✅ WITHOUT source filtering (resilient):
   planner → build.done → planner receives it → continues planning
```

### Design Rationale

Per **Tenet 2: Backpressure Over Prescription**:
> Don't prescribe how—create gates that reject bad work... Backpressure is enforceable; instructions are suggestions.

Telling the LLM "only emit build.task" is a suggestion it might ignore. The system must be resilient to this.

### Loop Prevention

Self-routing does NOT cause infinite loops because:

1. **Subscription patterns prevent cycles** — Planner triggers on `build.done`, not `build.task`. Builder triggers on `build.task`, not `build.done`. Even if a hat emits an event it subscribes to, the workflow naturally progresses.

2. **Loop thrashing detection** — If the same hat emits 3+ consecutive `build.blocked` events, the orchestrator terminates with `LoopThrashing`. This is the real safety net.

3. **Fresh context breaks cycles** — Each iteration starts fresh. Even if the agent makes the same mistake, fresh context often self-corrects.

### Acceptance Criteria

- **Given** planner hat emits `build.done` (even though builder "should" emit it)
- **When** event is routed
- **Then** planner receives the event and continues (self-routing allowed)

- **Given** any hat emits an event matching its own triggers
- **When** event is routed
- **Then** hat receives the event (no source-based blocking)

- **Given** same hat emits 3+ consecutive `build.blocked` events
- **When** thrashing detection runs
- **Then** loop terminates with `LoopThrashing` reason (this is the safety net, not source filtering)

## Event Payloads

### build.task

```
<event topic="build.task">
## Task
Add refresh token support

## Acceptance Criteria
- [ ] Generate refresh token on login
- [ ] Add POST /auth/refresh endpoint

## Context
JWT auth exists in src/auth.rs. Follow that pattern.
</event>
```

### build.done

```
<event topic="build.done">
## Completed
Add refresh token support

## Changes
- src/auth.rs: Added refresh token generation
- src/routes/auth.rs: Added POST /auth/refresh

## Validation
- cargo check: PASS
- cargo test: PASS
- cargo clippy: PASS

## Commit
abc1234: feat(auth): add refresh token support
</event>
```

### build.blocked

```
<event topic="build.blocked">
## Task
Add database migration

## Blocker
No migration system exists. Need sqlx-migrate or similar.

## Recommendation
Add sqlx-migrate before continuing.
</event>
```

### loop.terminate (System Event)

Published by the orchestrator (not agents) when the loop exits:

```
<event topic="loop.terminate">
## Reason
completed | max_iterations | max_runtime | consecutive_failures | interrupted | error

## Status
All tasks completed successfully.

## Summary
- Iterations: 12
- Duration: 23m 45s
- Exit code: 0
</event>
```

**Note:** This is an observer-only event. Hats cannot trigger on `loop.terminate`—it signals that the loop is ending, not that work should continue.

## Agent Instructions

Prompts are built by combining **core behaviors** with **hat-specific instructions**:

```
┌─────────────────────────────────────┐
│ CORE BEHAVIORS (always injected)   │
│ - Scratchpad access                │
│ - Specs awareness                  │
│ - Search-first guardrail           │
│ - Backpressure compliance          │
├─────────────────────────────────────┤
│ HAT INSTRUCTIONS (mode-specific)   │
│ - What this hat does               │
│ - What this hat doesn't do         │
│ - Completion criteria              │
├─────────────────────────────────────┤
│ EVENT CONTEXT                      │
│ - Triggering event payload         │
│ - Relevant prior events (optional) │
└─────────────────────────────────────┘
```

### Core Prompt (Always Present)

```
You are Ralph. Fresh context each iteration—the scratchpad is your memory.

## ALWAYS

- **Scratchpad:** `.agent/scratchpad.md` is shared state. Read it. Update it.
- **Specs:** `./specs/` is the source of truth. Implementations must match.
- **Search first:** Never assume "not implemented." Search the codebase first.
- **Backpressure:** Tests, typecheck, lint must pass. No exceptions.
```

### 🎩 Planner Hat (Added Instructions)

```
## PLANNER MODE

You're planning, not building.

1. **Gap analysis.** Compare `./specs/` against codebase. What's missing? Broken?

2. **Own the scratchpad.** Create or update with prioritized tasks.
   - `[ ]` pending
   - `[x]` done
   - `[~]` cancelled (with reason)

3. **Dispatch work.** Publish `build.task` ONE AT A TIME. Clear acceptance criteria.

4. **Validate.** When build reports done, verify it satisfies the spec.

## DON'T

- ❌ Write implementation code
- ❌ Run tests or make commits
- ❌ Pick tasks to implement yourself

## DONE

When ALL tasks are `[x]` or `[~]` and specs are satisfied, output: {completion_promise}
```

### 🔨 Builder Hat (Added Instructions)

```
## BUILDER MODE

You're building, not planning. One task, then exit.

1. **Pick ONE task.** Highest priority `[ ]` from scratchpad.

2. **Implement.** Write the code. Follow existing patterns.

3. **Validate.** Run backpressure. Must pass.

4. **Commit.** One task, one commit. Mark `[x]` in scratchpad.

5. **Exit.** Publish `build.done`. The loop continues.

## DON'T

- ❌ Create the scratchpad
- ❌ Decide what tasks to add
- ❌ Output the completion promise

## STUCK?

Can't finish? Publish `build.blocked` with:
- What you tried
- Why it failed
- What would unblock you
```

## Configuration

### Prompt Configuration

The task prompt can be specified via CLI flags or config file. This enables self-contained presets (single-file workflows) while preserving flexibility for external prompt files.

| Source | Config Key / Flag | Description |
|--------|-------------------|-------------|
| CLI inline | `-p "text"` | Inline prompt text |
| CLI file | `-P path` | Path to prompt file |
| Config inline | `event_loop.prompt` | Inline prompt in YAML |
| Config file | `event_loop.prompt_file` | Path to prompt file |
| Default | — | `PROMPT.md` in working directory |

**Precedence (highest to lowest):**

```
1. CLI -p "text"           # Explicit inline (wins over everything)
2. CLI -P path             # Explicit file path
3. Config prompt:          # Config inline text
4. Config prompt_file:     # Config file path
5. Default PROMPT.md       # Fallback
```

**Rules:**
- `-p` and `-P` are mutually exclusive (error if both specified)
- `prompt` and `prompt_file` are mutually exclusive (error if both specified)
- CLI flags override config (more specific wins)
- If none specified and `PROMPT.md` doesn't exist, error with helpful message

**Example — Self-contained preset:**
```yaml
# presets/gap-analysis.yml
event_loop:
  prompt: |
    Perform a deep gap analysis between specs and implementation.

    ## Process
    1. Read each spec in ./specs/
    2. Compare acceptance criteria against implementation
    3. Document gaps in ISSUES.md

    ## Output
    Write findings to ISSUES.md with structured format.
  completion_promise: "GAP_ANALYSIS_COMPLETE"
```

**Example — External prompt file:**
```yaml
# ralph.yml
event_loop:
  prompt_file: "prompts/feature-request.md"
```

**Example — CLI override:**
```bash
# Override config prompt with quick task
ralph run --config presets/feature.yml -p "Fix the failing auth tests"
```

### Default

```yaml
# ralph.yml
event_loop:
  # Prompt: specify ONE of these (or use CLI flags, or default to PROMPT.md)
  # prompt: "Inline task description..."  # Inline prompt text
  prompt_file: "PROMPT.md"                 # Path to prompt file (default)

  completion_promise: "LOOP_COMPLETE"
  max_iterations: 100
  max_runtime_seconds: 14400
  max_consecutive_failures: 5

cli:
  backend: "claude"

# Core behaviors (always injected, can customize paths):
core:
  scratchpad: ".agent/scratchpad.md"
  specs_dir: "./specs/"

# Default hats (created automatically if not specified):
# hats:
#   planner:
#     triggers: ["task.start", "task.resume", "build.done", "build.blocked"]
#     publishes: ["build.task"]
#   builder:
#     triggers: ["build.task"]
#     publishes: ["build.done", "build.blocked"]
```

### Custom Hats (Extended Teams)

Custom hats **extend** the default planner/builder—they don't replace them unless explicitly overridden.

```yaml
# ralph.yml - Add a reviewer to the team
event_loop:
  prompt_file: "PROMPT.md"
  completion_promise: "LOOP_COMPLETE"

cli:
  backend: "claude"

core:
  scratchpad: ".agent/scratchpad.md"
  specs_dir: "./specs/"
  guardrails:
    - "Fresh context each iteration - scratchpad is memory"
    - "Don't assume 'not implemented' - search first"
    - "Backpressure is law - tests/typecheck/lint must pass"

hats:
  planner:
    triggers: ["task.start", "build.done", "build.blocked", "review.done", "review.rejected"]
    publishes: ["build.task", "review.request"]
    instructions: |
      Planning mode. After build.done, request review before marking complete.
      When review.done received, verify and dispatch next task (or complete).

  builder:
    triggers: ["build.task"]
    publishes: ["build.done", "build.blocked"]
    instructions: |
      Building mode. Pick ONE task, implement, commit.
      Mark [x] when done. Exit.

  reviewer:
    triggers: ["review.request"]
    publishes: ["review.done", "review.rejected"]
    instructions: |
      Review mode. Check the implementation for:
      - Correctness against spec
      - Code style consistency
      - Security issues

      If good: publish review.done
      If problems: publish review.rejected with specific feedback
```

### Validation Rules

The orchestrator validates hat configurations:

| Rule | Purpose |
|------|---------|
| Every trigger maps to exactly one hat | No ambiguous routing |
| `publishes` events are parseable | Catch typos early |
| Custom hats can't suppress core behaviors | Safety guardrail |
| At least one hat can output completion promise | Ensure loop can terminate |

## Preflight Validation

Before the loop starts, the orchestrator runs preflight checks to catch configuration issues early. This prevents wasted iterations on broken configurations.

### Preflight Errors (Fatal)

| Error | Description |
|-------|-------------|
| `NoHatsConfigured` | No hats defined - loop has nothing to execute |
| `UnhandledEvent` | A published event has no subscriber (dead end) |
| `NoInitialHandler` | No hat triggers on `task.start` |
| `GitNotAvailable` | Git checkpointing enabled but `git` not in PATH |
| `PathNotFound` | Required path (scratchpad parent) doesn't exist |

### Preflight Warnings (Informational)

| Warning | Description |
|---------|-------------|
| `HatNeverPublishes` | A hat has no publish events (may be intentional) |
| `UndefinedEventMetadata` | Custom event has no metadata defined |

### Example Output

```
❌ Preflight check failed:
   • Event 'deploy.start' published by 'deployer' has no subscriber.
     This would cause the loop to terminate unexpectedly.

Fix these issues before running the loop.
```

## Event Metadata

Event metadata defines what each event topic means, enabling auto-derived instructions for custom hats. This allows users to define custom workflows without writing verbose instruction blocks.

### Schema

```yaml
events:
  <topic>:
    description: "Brief description of what this event represents"
    on_trigger: "What a hat should do when it receives this event"
    on_publish: "When/how a hat should emit this event"
```

### Example

```yaml
events:
  deploy.start:
    description: "Deployment has been initiated"
    on_trigger: "Execute deployment steps, monitor progress, handle failures"
    on_publish: "Signal that deployment should begin after validation"

  deploy.done:
    description: "Deployment completed successfully"
    on_trigger: "Verify deployment, run smoke tests, check health endpoints"
    on_publish: "Signal successful deployment with evidence"

  deploy.failed:
    description: "Deployment failed"
    on_trigger: "Analyze failure, decide on rollback or retry strategy"
    on_publish: "Signal deployment failure with error details"

hats:
  deployer:
    name: "Deployer"
    triggers: ["deploy.start"]
    publishes: ["deploy.done", "deploy.failed"]
    # Instructions auto-derived from event metadata!
```

### How Auto-Derivation Works

When a hat has no explicit `instructions`, the orchestrator derives them from:

1. **Triggers** → Look up `events.<topic>.on_trigger` for each trigger
2. **Publishes** → Look up `events.<topic>.on_publish` for each publish
3. **Built-in defaults** → Fall back for well-known events (`task.start`, `build.task`, etc.)
4. **Must-publish rule** → Always appended: "Every iteration MUST publish one of your allowed events"

### Well-Known Events (Built-in Defaults)

These events have built-in default behaviors and don't require metadata:

| Event | Default Trigger Behavior | Default Publish Behavior |
|-------|-------------------------|-------------------------|
| `task.start` | Analyze task, create plan in scratchpad | — |
| `task.resume` | Continue from existing scratchpad | — |
| `build.task` | Implement task, run backpressure, commit | Dispatch ONE AT A TIME |
| `build.done` | Review completed work, decide next steps | When tests pass |
| `build.blocked` | Analyze blocker, decide how to unblock | When stuck |
| `review.request` | Review for correctness, tests, patterns | After build completion |
| `review.approved` | Mark task complete, proceed to next | If changes look good |
| `review.changes_requested` | Add fix tasks, dispatch | If issues found |

### Priority Order

When building instructions:

```
1. Explicit instructions (if provided) → Use as-is
2. Event metadata on_trigger/on_publish → Use if defined
3. Built-in defaults → Use for well-known events
4. Warning → Emitted if custom event has no metadata
```

### Validation

Preflight validation warns if a custom event lacks metadata:

```
WARN Preflight: Custom event 'security.scan' used as trigger by 'scanner'
     has no metadata. Add to 'events:' config or provide explicit instructions.
```

This warning is skipped if the hat has explicit `instructions` defined.

## CLI Usage

### Commands

| Command | Description |
|---------|-------------|
| `ralph run` | Start orchestration loop (default) |
| `ralph resume` | Resume from existing scratchpad |
| `ralph events` | View event history |

### Prompt Options

| Flag | Description | Example |
|------|-------------|---------|
| `-p <text>` | Inline prompt text | `ralph run -p "Add login feature"` |
| `-P <path>` | Prompt file path | `ralph run -P prompts/spec-sync.md` |

See [Prompt Configuration](#prompt-configuration) for full precedence rules.

**Quick tasks** — Use `-p` for one-off commands:
```bash
ralph run -p "Fix the broken tests in auth module"
ralph run -p "Add input validation to the signup form"
```

**Self-contained presets** — Use `event_loop.prompt` in config (no separate file needed):
```bash
ralph run --config presets/gap-analysis.yml
```

**Override preset prompt** — CLI flags win over config:
```bash
ralph run --config presets/feature.yml -p "Just fix the auth bug"
```

### Other Options

| Flag | Description |
|------|-------------|
| `-c, --config <path>` | Config file (default: `ralph.yml`) |
| `-v, --verbose` | Enable verbose output (shows stderr, debug logs) |
| `--max-iterations <n>` | Override max iterations |
| `--dry-run` | Show config without running |
| `-i, --interactive` | Enable interactive mode (PTY + user input) |
| `-a, --autonomous` | Force autonomous mode (overrides config) |
| `--idle-timeout <secs>` | Override idle timeout in interactive mode |

See [interactive-mode.spec.md](interactive-mode.spec.md) for details on execution modes.

## CLI Backend

Supports any headless CLI tool:

| Backend | Invocation |
|---------|------------|
| `claude` | `claude -p "prompt"` |
| `kiro` | `kiro-cli chat --no-interactive --trust-all-tools "prompt"` |
| `gemini` | Prompt via stdin |
| `custom` | User-defined command |

Output streams to stdout in real-time while being accumulated for event parsing.

### Output Visibility

| Stream | Default Mode | Verbose Mode (`-v`) |
|--------|--------------|---------------------|
| **stdout** | Streamed to terminal | Streamed to terminal |
| **stderr** | Hidden (accumulated only) | Streamed with `[stderr]` prefix |

**Rationale:** CLI tools often write progress indicators, warnings, and debug info to stderr. Showing this by default creates noise that obscures the actual agent output. In verbose mode, stderr is valuable for debugging CLI tool issues.

## Event Syntax

Agents publish events using XML-style tags:

```
<event topic="build.done">
Completed the task. Tests passing.
</event>
```

With optional target:
```
<event topic="build.task" target="builder">
Implement the login feature.
</event>
```

## Safeguards

| Guard | Behavior |
|-------|----------|
| `max_iterations` | Terminate after N iterations |
| `max_runtime_seconds` | Terminate after N seconds |
| `max_consecutive_failures` | Terminate after N failures in a row |

### Process Management

The orchestrator owns all spawned CLI processes and must ensure no orphaned processes remain:

| Scenario | Behavior |
|----------|----------|
| **Normal exit** | Wait for current CLI process to complete, then exit |
| **SIGINT (Ctrl+C)** | Allow current iteration to finish gracefully, then exit |
| **SIGTERM** | Send SIGTERM to child process, wait up to 5s, then SIGKILL if needed |
| **SIGHUP / terminal close** | Same as SIGTERM—kill child process before exiting |
| **Orchestrator crash** | Child processes inherit SIGKILL (process group leadership) |

**Implementation requirement:** The orchestrator must run as a process group leader. All spawned CLI processes (Claude, Kiro, etc.) belong to this group. On termination, the entire process group receives the signal, preventing orphans.

## Acceptance Criteria

### Core Behaviors

- **Given** any hat is active
- **When** prompt is built
- **Then** core behaviors (scratchpad access, search-first, backpressure) are included

- **Given** custom hat configuration
- **When** hat attempts to suppress core behaviors
- **Then** configuration is rejected with validation error

- **Given** Ralph in any mode
- **When** Ralph assumes something is "not implemented"
- **Then** backpressure fails (search-first guardrail)

### Initialization

- **Given** default config (no custom hats)
- **When** the loop initializes
- **Then** planner and builder hats are registered with their triggers

- **Given** the loop starts
- **When** `task.start` is published
- **Then** planner hat is triggered (it has `task.start` in triggers)

### 🎩 Planner Hat Behavior

- **Given** Ralph has planner hat on, receives `task.start`
- **When** no `.agent/scratchpad.md` exists
- **Then** Ralph creates scratchpad via gap analysis

- **Given** Ralph has planner hat on, receives `build.done`
- **When** tasks remain in scratchpad
- **Then** Ralph publishes next `build.task`

- **Given** Ralph has planner hat on, receives `build.blocked`
- **When** blocker requires new work
- **Then** Ralph updates scratchpad and dispatches appropriate task

- **Given** all tasks are `[x]` or `[~]`
- **When** Ralph verifies specs are satisfied
- **Then** Ralph outputs completion promise

### 🔨 Builder Hat Behavior

- **Given** Ralph has builder hat on, receives `build.task`
- **When** implementation succeeds and backpressure passes
- **Then** Ralph commits and publishes `build.done`

- **Given** Ralph has builder hat on, receives `build.task`
- **When** implementation is blocked
- **Then** Ralph publishes `build.blocked` with explanation

### Event Routing

- **Given** Ralph (planner hat) publishes `build.task`
- **When** event is routed
- **Then** builder hat is triggered (next iteration wears builder hat)

- **Given** Ralph (builder hat) publishes `build.done`
- **When** event is routed
- **Then** planner hat is triggered (next iteration wears planner hat)

- **Given** event topic matches multiple hat triggers
- **When** configuration is validated
- **Then** validation fails with "ambiguous routing" error

### Custom Hats

- **Given** custom hat `reviewer` with triggers `["review.request"]`
- **When** `review.request` event is published
- **Then** reviewer hat is triggered for next iteration

- **Given** custom hat extends default hats
- **When** loop initializes
- **Then** all hats (default + custom) are registered

### Preflight Validation

- **Given** config where hat publishes event with no subscriber
- **When** preflight check runs
- **Then** error is returned with "UnhandledEvent" specifying the dead-end event

- **Given** config where no hat triggers on `task.start`
- **When** preflight check runs
- **Then** error is returned with "NoInitialHandler"

- **Given** git checkpointing enabled but `git` not in PATH
- **When** preflight check runs
- **Then** error is returned with "GitNotAvailable"

- **Given** preflight errors exist
- **When** `ralph run` is executed
- **Then** loop does not start and errors are displayed with fix suggestions

- **Given** config with valid event flow graph
- **When** preflight check runs
- **Then** no errors are returned and loop proceeds

### Event Metadata

- **Given** custom event `deploy.start` with metadata defined
- **When** hat triggers on `deploy.start` without explicit instructions
- **Then** instructions are derived from `events.deploy.start.on_trigger`

- **Given** custom event `deploy.done` with metadata defined
- **When** hat publishes `deploy.done` without explicit instructions
- **Then** instructions include guidance from `events.deploy.done.on_publish`

- **Given** custom event without metadata and hat without instructions
- **When** preflight check runs
- **Then** warning is emitted about undefined event metadata

- **Given** custom event without metadata but hat has explicit instructions
- **When** preflight check runs
- **Then** no warning is emitted (explicit instructions override)

- **Given** well-known event `build.task` without metadata
- **When** instructions are derived
- **Then** built-in default behavior is used (no warning)

- **Given** hat with triggers and publishes but no instructions
- **When** prompt is built
- **Then** derived behaviors are included with "MUST publish" rule appended

### Safeguards

- **Given** `max_consecutive_failures: 5` in config
- **When** 5 iterations fail in a row
- **Then** loop terminates with failure limit reason

### Process Management

- **Given** orchestrator starts
- **When** it spawns CLI processes
- **Then** orchestrator runs as process group leader with children in the same group

- **Given** user sends SIGINT (Ctrl+C)
- **When** CLI process is running
- **Then** current iteration completes, child process terminates, no orphans remain

- **Given** user sends SIGTERM
- **When** CLI process is running
- **Then** SIGTERM is forwarded to child, orchestrator waits up to 5s, then SIGKILL if needed

- **Given** terminal is closed (SIGHUP)
- **When** CLI process is running
- **Then** child process is terminated before orchestrator exits

- **Given** orchestrator crashes or is killed with SIGKILL
- **When** CLI process was running
- **Then** process group signal propagation ensures child is also killed

- **Given** any termination scenario
- **When** `ps aux | grep claude` is run after Ralph exits
- **Then** no Claude/Kiro processes from this session remain running

### Escape Hatches

- **Given** Ralph (builder hat) publishes `build.blocked` repeatedly for same task
- **When** planner hat receives third block on same task
- **Then** Ralph cancels task `[~]` with explanation and moves on

- **Given** task is marked `[~]` cancelled
- **When** Ralph verifies completion
- **Then** cancelled tasks don't block completion (they're acknowledged failures)

- **Given** Ralph cannot make progress on any remaining tasks
- **When** all uncancelled tasks are blocked
- **Then** Ralph outputs completion promise with "partial completion" note

### Event Parsing

- **Given** output contains `<event topic="build.done">content</event>`
- **When** output is parsed
- **Then** event is created with topic `build.done` and content as payload

### Loop Termination (Dual Condition)

#### Task State Verification

- **Given** Planner hat outputs `LOOP_COMPLETE`
- **When** scratchpad contains pending `[ ]` tasks
- **Then** completion is rejected, warning logged, `completion_confirmations` reset to 0

- **Given** Planner hat outputs `LOOP_COMPLETE`
- **When** all scratchpad tasks are `[x]` or `[~]`
- **Then** `completion_confirmations` is incremented

- **Given** scratchpad does not exist
- **When** Planner outputs `LOOP_COMPLETE`
- **Then** completion is rejected (no task state to verify)

#### Consecutive Confirmation

- **Given** Planner outputs `LOOP_COMPLETE` with valid task state (first time)
- **When** `completion_confirmations` becomes 1
- **Then** loop continues to next iteration (does NOT terminate yet)

- **Given** Planner outputs `LOOP_COMPLETE` with valid task state (second consecutive time)
- **When** `completion_confirmations` becomes 2
- **Then** loop enters TERMINATING state and exits with code 0

- **Given** Planner outputs `LOOP_COMPLETE` then does NOT output it next iteration
- **When** confirmation chain is broken
- **Then** `completion_confirmations` resets to 0

#### Other Termination Rules

- **Given** Builder hat outputs `LOOP_COMPLETE`
- **When** orchestrator parses output
- **Then** completion promise is ignored (only Planner can terminate)

- **Given** `LOOP_COMPLETE` appears inside an event payload
- **When** orchestrator parses output
- **Then** completion promise is not detected (must be in final output, not event content)

- **Given** `LOOP_COMPLETE` appears inside an event payload AND in the final output
- **When** orchestrator parses output
- **Then** completion is BLOCKED (event tag presence taints the output)

- **Given** loop terminates for any reason
- **When** termination flow executes
- **Then** `.agent/summary.md` is written with status, iterations, cost, and task summary

- **Given** `max_iterations: 10` and iteration reaches 10
- **When** iteration completes without completion promise
- **Then** loop terminates with exit code 2 (limit)

- **Given** SIGINT received during iteration
- **When** signal is handled
- **Then** current iteration is allowed to finish, then loop exits with code 130

- **Given** loop terminated due to safeguard (not completion promise)
- **When** user runs `ralph resume`
- **Then** loop restarts reading existing scratchpad, continuing from where it left off

### Event History

- **Given** any event is published
- **When** event is routed
- **Then** event is appended to `.agent/events.jsonl` with timestamp and metadata

- **Given** `ralph events` is executed
- **When** event history exists
- **Then** events are displayed in chronological order

- **Given** `ralph events --topic "build.blocked"` is executed
- **When** event history contains blocked events
- **Then** only `build.blocked` events are displayed

- **Given** loop terminates (success or failure)
- **When** event history is examined
- **Then** full event trail is available for debugging

### Log Messages

- **Given** loop initializes with hats [planner, builder]
- **When** startup log is emitted
- **Then** message includes hat list, not "mode" terminology

- **Given** Ralph changes from planner to builder hat
- **When** hat change is logged
- **Then** message is `"Putting on my builder hat."` (not "switching agents")

- **Given** log message references hats
- **When** message is examined
- **Then** it never contains "single-hat mode" or "multi-hat mode"

- **Given** loop terminates
- **When** termination log is emitted
- **Then** message includes iteration count and cost

### Output Visibility

- **Given** CLI backend writes to stderr
- **When** running in default mode (no `-v` flag)
- **Then** stderr output is NOT displayed to terminal

- **Given** CLI backend writes to stderr
- **When** running in verbose mode (`-v` flag)
- **Then** stderr output is displayed with `[stderr]` prefix

- **Given** CLI backend writes to stderr
- **When** output is accumulated for event parsing
- **Then** stderr content is included regardless of verbose mode

### Prompt Options

#### CLI Flags

- **Given** user runs `ralph run -p "Fix the tests"`
- **When** loop initializes
- **Then** "Fix the tests" is used as prompt content (inline text)

- **Given** user runs `ralph run -P prompts/my-prompt.md`
- **When** loop initializes
- **Then** contents of `prompts/my-prompt.md` are used as prompt content

- **Given** user runs `ralph run -p "text" -P file.md`
- **When** arguments are parsed
- **Then** error is returned (mutually exclusive flags)

- **Given** user runs `ralph run -P nonexistent.md`
- **When** file does not exist
- **Then** error is returned with helpful message

#### Config File Prompts

- **Given** config has `event_loop.prompt: "Inline task"`
- **When** user runs `ralph run` without `-p` or `-P`
- **Then** "Inline task" is used as prompt content

- **Given** config has `event_loop.prompt_file: "custom.md"`
- **When** user runs `ralph run` without `-p` or `-P`
- **Then** contents of `custom.md` are used as prompt content

- **Given** config has both `event_loop.prompt` and `event_loop.prompt_file`
- **When** config is validated
- **Then** error is returned (mutually exclusive options)

- **Given** config has `event_loop.prompt: "Task"` and user runs `ralph run -p "Override"`
- **When** loop initializes
- **Then** "Override" is used (CLI wins over config)

- **Given** config has `event_loop.prompt_file: "task.md"` and user runs `ralph run -p "Override"`
- **When** loop initializes
- **Then** "Override" is used (CLI wins over config)

#### Default Fallback

- **Given** user runs `ralph run` without `-p` or `-P`
- **When** config has neither `prompt` nor `prompt_file`
- **Then** contents of `PROMPT.md` from working directory are used

- **Given** no prompt specified anywhere and `PROMPT.md` doesn't exist
- **When** loop attempts to initialize
- **Then** error is returned with message listing all prompt options

### Iteration Demarcation

- **Given** iteration N begins
- **When** orchestrator starts the iteration
- **Then** a visual separator is printed with iteration number, hat, elapsed time, and progress

- **Given** user scrolls through terminal output
- **When** looking for iteration boundaries
- **Then** iteration starts are clearly distinguishable from agent output (box-drawing characters)

- **Given** iteration separator is printed
- **When** separator is examined
- **Then** it includes: iteration number, hat emoji and name, elapsed time, progress (N/max)

## Event History & Debugging

All events are logged to `.agent/events.jsonl` for debugging and post-mortem analysis.

### Event Log Format

```jsonl
{"ts":"2024-01-15T10:23:45Z","iteration":1,"hat":"loop","topic":"task.start","triggered":"planner","payload":"..."}
{"ts":"2024-01-15T10:24:12Z","iteration":1,"hat":"planner","topic":"build.task","triggered":"builder","payload":"Add auth..."}
{"ts":"2024-01-15T10:25:33Z","iteration":2,"hat":"builder","topic":"build.done","triggered":"planner","payload":"Completed..."}
{"ts":"2024-01-15T10:25:45Z","iteration":2,"hat":"builder","topic":"build.blocked","triggered":"planner","payload":"Missing..."}
```

### What Gets Logged

| Field | Description |
|-------|-------------|
| `ts` | ISO 8601 timestamp |
| `iteration` | Loop iteration number |
| `hat` | Hat that was active when event was published |
| `topic` | Event topic |
| `triggered` | Hat that will be triggered by this event |
| `payload` | Event content (truncated if large) |
| `blocked_count` | (optional) How many times this task has blocked |

### Debug Commands

```bash
# View event history
ralph events

# View last N events
ralph events --last 20

# Filter by topic
ralph events --topic "build.blocked"

# Filter by iteration
ralph events --iteration 5

# Export for analysis
ralph events --format json > debug.json
```

### Debugging Stuck States

When Ralph gets stuck, the event history shows:
- Which tasks keep blocking (pattern detection)
- The back-and-forth between hats
- Where the loop is spending time
- What information is being passed

## Loop Termination

The orchestrator must detect when to exit the loop and report the outcome. Termination can occur for several reasons—successful completion, safeguard limits, or unrecoverable errors.

### Termination Triggers

| Trigger | Detection | Exit Code |
|---------|-----------|-----------|
| **Completion promise** | Dual condition met: (1) all tasks `[x]`/`[~]` AND (2) `LOOP_COMPLETE` on 2 consecutive iterations | 0 (success) |
| **Max iterations** | `iteration >= max_iterations` | 2 (limit) |
| **Max runtime** | `elapsed >= max_runtime_seconds` | 2 (limit) |
| **Consecutive failures** | `consecutive_failures >= max_consecutive_failures` | 1 (failure) |
| **User interrupt** | SIGINT/SIGTERM received | 130 (interrupt) |
| **Unrecoverable error** | CLI crash, config error, system failure | 1 (failure) |

**Note:** Cost tracking is the CLI tool's responsibility, not the orchestrator's. Claude, Kiro, etc. track their own usage.

### Completion Detection (Dual Condition)

**Design rationale:** LLMs can be confident but wrong. A single `LOOP_COMPLETE` signal is just the agent's opinion. Dual condition exits add objective verification to prevent premature termination.

The orchestrator requires **both conditions** to be satisfied before terminating:

| Condition | What It Verifies | Why It Matters |
|-----------|------------------|----------------|
| **Task State** | All scratchpad tasks are `[x]` or `[~]` | Objective proof work is done |
| **Consecutive Confirmation** | Planner outputs `LOOP_COMPLETE` on 2 consecutive iterations | Fresh context on 2nd iteration catches false confidence |

#### Completion Flow

```
Iteration N: Planner outputs LOOP_COMPLETE
    │
    ├─► Check scratchpad: any `[ ]` pending tasks?
    │       │
    │       ├─► YES: Reject completion, log warning, continue loop
    │       │        (completion_confirmations reset to 0)
    │       │
    │       └─► NO: Record confirmation (completion_confirmations = 1)
    │                Continue to iteration N+1
    │
Iteration N+1: Planner outputs LOOP_COMPLETE again (fresh context)
    │
    ├─► Check scratchpad again: any `[ ]` pending tasks?
    │       │
    │       ├─► YES: Reject, reset counter
    │       │
    │       └─► NO: completion_confirmations = 2
    │                ✓ TERMINATE with CompletionPromise
```

#### Detection Rules

1. **Completion promise** — The exact string from `completion_promise` config (default: `LOOP_COMPLETE`)
2. **Event tags** — `<event topic="...">` blocks for routing

Completion promise detection:
- Must appear in the agent's final output (not inside an event payload)
- Can be on its own line or inline: `All tasks done. LOOP_COMPLETE`
- Case-sensitive match against configured `completion_promise`
- Only valid when Planner hat is active (Builder cannot terminate the loop)
- **Requires task state verification** — rejected if pending `[ ]` tasks exist
- **Requires consecutive confirmation** — must pass verification on 2 consecutive iterations

**⚠️ CRITICAL: Event Tag Blocking Rule**

If the completion promise appears inside **any** event tag payload, completion is **blocked entirely** — even if the promise also appears in the final output.

```
❌ WILL NOT COMPLETE (promise in event tag blocks):
<event topic="build.task">Fix LOOP_COMPLETE detection bug</event>
All done! LOOP_COMPLETE

✅ WILL COMPLETE (promise only in final output):
<event topic="build.done">Task finished</event>
All done! LOOP_COMPLETE
```

**Rationale:** Agents may reference the completion promise when describing tasks (e.g., "Fix LOOP_COMPLETE detection"). Without this safeguard, if the agent also happened to output `LOOP_COMPLETE` in their final text, the loop could terminate unexpectedly. The blocking rule ensures explicit, intentional completion.

#### Why Consecutive Confirmation?

Fresh context is Ralph's superpower. On the second iteration:
- The agent re-reads the scratchpad from scratch
- No memory of "I already said I'm done"
- If work is truly complete, the agent will independently conclude the same thing
- If the first completion was premature, fresh eyes often catch what was missed

This adds ~1 iteration overhead but dramatically improves completion confidence.

### Termination Flow

```
1. [Orchestrator] Detects termination trigger
2. [Orchestrator] Publishes `loop.terminate` event to observers
3. [Orchestrator] Writes summary to `.agent/summary.md`
4. [Orchestrator] Exits with appropriate exit code
```

### Exit Summary

On termination, the orchestrator writes `.agent/summary.md`:

```markdown
# Loop Summary

**Status:** Completed successfully
**Iterations:** 12
**Duration:** 23m 45s

## Tasks
- [x] Add refresh token support
- [x] Update login endpoint
- [~] Add rate limiting (cancelled: out of scope)

## Events
- 12 total events
- 6 build.task
- 5 build.done
- 1 build.blocked

## Final Commit
abc1234: feat(auth): complete auth overhaul
```

### Partial Completion

When the loop terminates due to safeguards (not completion promise):

1. Outstanding tasks remain in scratchpad as `[ ]`
2. Summary reflects partial status
3. User can resume with `ralph resume` (reads existing scratchpad)

### The Loop State Machine

```
                    ┌─────────────────┐
                    │     START       │
                    └────────┬────────┘
                             │ publish task.start
                             ▼
                    ┌─────────────────┐
          ┌────────►│    RUNNING      │◄────────────────────┐
          │         └────────┬────────┘                     │
          │                  │                              │
          │    ┌─────────────┼─────────────┐                │
          │    ▼             ▼             ▼                │
     build.done      build.blocked    LOOP_COMPLETE         │
          │                  │             │                │
          │                  │             ▼                │
          │                  │    ┌────────────────┐        │
          │                  │    │ Check Tasks    │        │
          │                  │    │ (any [ ] ?)    │        │
          │                  │    └───────┬────────┘        │
          │                  │            │                 │
          │                  │      ┌─────┴─────┐           │
          │                  │      ▼           ▼           │
          │                  │   pending     all done       │
          │                  │      │           │           │
          │                  │      │     ┌─────┴─────┐     │
          │                  │      │     ▼           ▼     │
          │                  │      │  1st confirm  2nd confirm
          │                  │      │     │           │     │
          │                  ▼      ▼     ▼           ▼     │
          │         ┌─────────────────────┐    ┌──────────┐ │
          └─────────│      RUNNING        │    │TERMINATING│ │
                    │  (reset confirms)   │────┘└─────┬────┘
                    └─────────────────────┘           │
                                                      │
                    ┌─────────────────┐               │
                    │  SAFEGUARD HIT  │───────────────┤
                    └─────────────────┘               │
                             │                        │
                             ▼                        ▼
                    ┌─────────────────────────────────┐
                    │            EXITED               │
                    │     (summary, exit code)        │
                    └─────────────────────────────────┘
```

## Log Messages

The orchestrator emits structured log messages at key points. Logs should be lighthearted and informative—Ralph has personality.

### Iteration Demarcation

Each iteration must be clearly demarcated in the output so users can visually distinguish where one iteration ends and another begins. This is critical for:
- Debugging which iteration caused an issue
- Understanding the flow between hats
- Parsing logs after the fact

**Required visual separator:**

```
═══════════════════════════════════════════════════════════════════════════════
 ITERATION 3 │ 🔨 builder │ 2m 15s elapsed │ 3/100
═══════════════════════════════════════════════════════════════════════════════
```

The separator includes:
- Clear visual break (box-drawing characters)
- Iteration number
- Active hat (with emoji)
- Elapsed time since loop start
- Progress indicator (current/max)

### Lifecycle Messages

| Event | Level | Message Format |
|-------|-------|----------------|
| **Loop start** | INFO | `"I'm Ralph. Got my hats ready: {hats}. Let's do this."` |
| **Hat change** | INFO | `"Putting on my {hat} hat."` |
| **Iteration start** | DEBUG | `"Iteration {n}/{max} — wearing {hat} hat"` |
| **Event published** | DEBUG | `"Published {topic} → triggers {hat}"` |
| **Completion detected** | INFO | `"All done! {completion_promise} detected."` |
| **Loop termination** | INFO | `"Wrapping up: {reason}. {iterations} iterations in {duration}."` |

### Examples

```
INFO  I'm Ralph. Got my hats ready: planner, builder. Let's do this.
INFO  Putting on my planner hat.
DEBUG Iteration 1/100 — wearing planner hat
DEBUG Published build.task → triggers builder
INFO  Putting on my builder hat.
DEBUG Iteration 2/100 — wearing builder hat
DEBUG Published build.done → triggers planner
INFO  Putting on my planner hat.
INFO  All done! LOOP_COMPLETE detected.
INFO  Wrapping up: completed. 3 iterations in 2m 15s.
```

### What NOT to Log

Per the architecture section, avoid these anti-patterns:

| ❌ Don't | ✅ Do Instead |
|----------|---------------|
| `"Starting in single-hat mode"` | `"Got my hats ready: builder"` |
| `"Starting in multi-hat mode"` | `"Got my hats ready: planner, builder"` |
| `"Switching to builder agent"` | `"Putting on my builder hat"` |
| `"Agent completed task"` | `"Published build.done → triggers planner"` |

### Structured Fields

All logs include structured fields for observability:

```rust
info!(
    hats = ?registered_hats,
    max_iterations = config.max_iterations,
    backend = config.cli.backend,
    "I'm Ralph. Got my hats ready: {}. Let's do this.",
    registered_hats.join(", ")
);
```

| Field | Type | Description |
|-------|------|-------------|
| `hats` | `Vec<String>` | Currently registered hat IDs |
| `hat` | `String` | Active hat for this iteration |
| `iteration` | `u32` | Current iteration number |
| `max_iterations` | `u32` | Configured limit |
| `topic` | `String` | Event topic being published |
| `triggered` | `String` | Hat that will handle the event |
| `reason` | `String` | Termination reason |
| `duration` | `String` | Human-readable elapsed time |

## Error Handling

| Error | Behavior |
|-------|----------|
| Unknown topic | Log warning, event dropped |
| CLI failure | Log error, increment failure counter |
| Parse failure | Log warning, treat as raw text |
| Timeout | Kill process, increment failure counter |

## Crate Placement

| Component | Crate |
|-----------|-------|
| Event, EventBus, Hat, Topic types | `ralph-proto` |
| HatRegistry, EventLoopConfig, EventLoop | `ralph-core` |
| LoopState, TerminationReason, ExitCode | `ralph-core` |
| InstructionBuilder (core + hat prompt assembly) | `ralph-core` |
| CoreBehaviors (shared guardrails) | `ralph-core` |
| EventHistory, EventLogger | `ralph-core` |
| SummaryWriter (generates `.agent/summary.md`) | `ralph-core` |
| CliBackend, CliConfig, CliExecutor | `ralph-adapters` |
| CLI entry point, config loading, `events` subcommand | `ralph-cli` |
