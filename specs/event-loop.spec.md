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

### Default

```yaml
# ralph.yml
event_loop:
  prompt_file: "PROMPT.md"
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

## CLI Backend

Supports any headless CLI tool:

| Backend | Invocation |
|---------|------------|
| `claude` | `claude -p "prompt"` |
| `kiro` | `kiro-cli chat --no-interactive --trust-all-tools "prompt"` |
| `gemini` | Prompt via stdin |
| `custom` | User-defined command |

Output streams to stdout in real-time while being accumulated for event parsing.

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

### Loop Termination

- **Given** Planner hat outputs `LOOP_COMPLETE`
- **When** orchestrator parses output
- **Then** loop enters TERMINATING state and exits with code 0

- **Given** Builder hat outputs `LOOP_COMPLETE`
- **When** orchestrator parses output
- **Then** completion promise is ignored (only Planner can terminate)

- **Given** `LOOP_COMPLETE` appears inside an event payload
- **When** orchestrator parses output
- **Then** completion promise is not detected (must be in final output, not event content)

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
| **Completion promise** | Output contains `LOOP_COMPLETE` (or configured `completion_promise`) | 0 (success) |
| **Max iterations** | `iteration >= max_iterations` | 2 (limit) |
| **Max runtime** | `elapsed >= max_runtime_seconds` | 2 (limit) |
| **Consecutive failures** | `consecutive_failures >= max_consecutive_failures` | 1 (failure) |
| **User interrupt** | SIGINT/SIGTERM received | 130 (interrupt) |
| **Unrecoverable error** | CLI crash, config error, system failure | 1 (failure) |

**Note:** Cost tracking is the CLI tool's responsibility, not the orchestrator's. Claude, Kiro, etc. track their own usage.

### Completion Detection

The orchestrator parses agent output after each iteration looking for:

1. **Completion promise** — The exact string from `completion_promise` config (default: `LOOP_COMPLETE`)
2. **Event tags** — `<event topic="...">` blocks for routing

Completion promise detection:
- Must appear in the agent's final output (not inside an event payload)
- Can be on its own line or inline: `All tasks done. LOOP_COMPLETE`
- Case-sensitive match against configured `completion_promise`
- Only valid when Planner hat is active (Builder cannot terminate the loop)

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
          ┌────────►│    RUNNING      │◄────────┐
          │         └────────┬────────┘         │
          │                  │                  │
          │    ┌─────────────┼─────────────┐    │
          │    ▼             ▼             ▼    │
     build.done      build.blocked    completion
          │                  │         promise
          │                  │             │
          │                  ▼             ▼
          │         ┌─────────────┐  ┌──────────┐
          └─────────│  RUNNING    │  │TERMINATING│
                    └─────────────┘  └─────┬────┘
                                           │
                    ┌─────────────────┐    │
                    │  SAFEGUARD HIT  │────┤
                    └─────────────────┘    │
                             │             │
                             ▼             ▼
                    ┌─────────────────────────┐
                    │        EXITED           │
                    │  (summary, exit code)   │
                    └─────────────────────────┘
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
