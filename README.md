# Ralph Orchestrator

[![License](https://img.shields.io/badge/license-MIT-blue)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.75+-orange)](https://www.rust-lang.org/)
[![Build](https://img.shields.io/github/actions/workflow/status/mikeyobrien/ralph-orchestrator/ci.yml?branch=main&label=CI)](https://github.com/mikeyobrien/ralph-orchestrator/actions)
[![Coverage](https://img.shields.io/badge/coverage-65%25-yellowgreen)](coverage/index.html)
[![Mentioned in Awesome Claude Code](https://awesome.re/mentioned-badge.svg)](https://github.com/hesreallyhim/awesome-claude-code)
[![Docs](https://img.shields.io/badge/docs-mkdocs-blue)](https://mikeyobrien.github.io/ralph-orchestrator/)


A hat-based orchestration framework that keeps Ralph in a loop until the task is done.

> "Me fail English? That's unpossible!" - Ralph Wiggum

**Notice:** Ralph Orchestrator is under active development. It works today, but expect rough edges and breaking changes between releases.

v1.0.0 was ralphed into existence with little oversight and guidance. v2.0.0 is a simpler, more-structured implementation. Looking for the old version? See [v1.2.3](https://github.com/mikeyobrien/ralph-orchestrator/tree/v1.2.3). 

<img width="912" height="712" alt="Screenshot 2026-01-20 at 10 27 57 AM" src="https://github.com/user-attachments/assets/91b08b47-8b0a-4e2c-b66e-88551c2c5cc6" />

## Table of Contents

- [What is Ralph?](#what-is-ralph)
- [Features](#features)
- [Installation](#installation)
- [Quick Start](#quick-start)
- [Configuration](#configuration)
- [Custom Backends and Per-Hat Configuration](#custom-backends-and-per-hat-configuration)
- [Presets](#presets)
- [Key Concepts](#key-concepts)
- [Orchestration and Coordination Patterns](#orchestration-and-coordination-patterns)
- [Multi-Loop Concurrency](#multi-loop-concurrency)
- [CLI Reference](#cli-reference)
- [Architecture](#architecture)
- [Building & Testing](#building--testing)
- [Contributing](#contributing)
- [License](#license)
- [Acknowledgments](#acknowledgments)

## What is Ralph?

Ralph implements the [Ralph Wiggum technique](https://ghuntley.com/ralph/) — autonomous task completion through continuous iteration.

> "The orchestrator is a thin coordination layer, not a platform. Ralph is smart; let Ralph do the work."

### Two Modes of Operation

Ralph supports two orchestration styles:

| Mode | Description | Best For |
|------|-------------|----------|
| **Traditional** | Simple loop — Ralph iterates until done | Quick tasks, simple automation, minimal config |
| **Hat-Based** | Ralph can wear many hats — specialized personas coordinate through events | Complex workflows, multi-step processes, role separation |

**Traditional mode** is the original Ralph Wiggum approach: start a loop, let it run until it outputs the completion promise. No roles, no events, just iteration.

**Hat-based mode** adds structure: specialized personas coordinate through events. You define the roles that fit your workflow — reviewers, testers, documenters, whatever makes sense. Presets provide ready-made patterns like TDD or spec-driven development.

### The Ralph Tenets

1. **Fresh Context Is Reliability** — Each iteration clears context. Re-read specs, plan, code every cycle.
2. **Backpressure Over Prescription** — Don't prescribe how; create gates that reject bad work.
3. **The Plan Is Disposable** — Regeneration costs one planning loop. Cheap.
4. **Disk Is State, Git Is Memory** — Files are the handoff mechanism.
5. **Steer With Signals, Not Scripts** — Add signs, not scripts.
6. **Let Ralph Ralph** — Sit *on* the loop, not *in* it.

See [AGENTS.md](AGENTS.md) for the full philosophy.

## Features

- **Multi-Backend Support** — Works with Claude Code, Kiro, Gemini CLI, Codex, Amp, Copilot CLI, and OpenCode
- **Hat System** — Specialized Ralph personas with distinct behaviors
- **Event-Driven Coordination** — Hats communicate through typed events with glob pattern matching
- **Backpressure Enforcement** — Gates that reject incomplete work (tests, lint, typecheck)
- **Presets Library** — 20+ pre-configured workflows for common development patterns
- **Interactive TUI** — Real-time terminal UI for monitoring Ralph's activity (enabled by default)
- **Memories** — Persistent learning across sessions stored in `.agent/memories.md`
- **Tasks** — Runtime work tracking stored in `.agent/tasks.jsonl`
- **Multi-Loop Concurrency** — Run parallel loops in git worktrees with automatic merge workflow
- **Session Recording** — Record and replay sessions for debugging and testing (experimental)

## Installation

### Prerequisites

- [Rust](https://rustup.rs/) 1.75+
- At least one AI CLI:
  - [Claude Code](https://github.com/anthropics/claude-code) (recommended)
  - [Kiro](https://kiro.dev/)
  - [Gemini CLI](https://github.com/google-gemini/gemini-cli)
  - [Codex](https://github.com/openai/codex)
  - [Amp](https://github.com/sourcegraph/amp)
  - [Copilot CLI](https://docs.github.com/copilot) (`npm install -g @github/copilot`)
  - [OpenCode](https://opencode.ai/) (`curl -fsSL https://opencode.ai/install | bash`)

### Via npm (Recommended)

```bash
# Install globally
npm install -g @ralph-orchestrator/ralph-cli

# Or run directly with npx
npx @ralph-orchestrator/ralph-cli --version
```

### Via Homebrew (macOS)

```bash
brew install ralph-orchestrator
```

### Via Cargo

```bash
cargo install ralph-cli
```

### From Source

```bash
git clone https://github.com/mikeyobrien/ralph-orchestrator.git
cd ralph-orchestrator
cargo build --release

# Add to PATH
export PATH="$PATH:$(pwd)/target/release"

# Or create symlink
sudo ln -s $(pwd)/target/release/ralph /usr/local/bin/ralph
```

### Verify Installation

```bash
ralph --version
ralph --help
```

### Migrating from v1 (Python)

If you have the old Python-based Ralph v1 installed, uninstall it first to avoid conflicts:

```bash
# If installed via pip
pip uninstall ralph-orchestrator

# If installed via pipx
pipx uninstall ralph-orchestrator

# If installed via uv
uv tool uninstall ralph-orchestrator

# Verify removal
which ralph  # Should return nothing or point to new Rust version
```

The v1 Python version is no longer maintained. See [v1.2.3](https://github.com/mikeyobrien/ralph-orchestrator/tree/v1.2.3) for historical reference.

## Quick Start

### 1. Initialize a Project

```bash
# Traditional mode — simple loop, no hats (recommended for getting started)
ralph init --backend claude

# Hat-based mode — use a preset workflow with specialized personas
ralph init --preset tdd-red-green

# Combine preset with different backend
ralph init --preset spec-driven --backend kiro

# See all available presets
ralph init --list-presets
```

This creates `ralph.yml` in your current directory. Without a preset, you get traditional mode (no hats). With a preset, you get hat-based orchestration.

### 2. Define Your Task

**Option A:** Create a `PROMPT.md` file:

```bash
cat > PROMPT.md << 'EOF'
Build a REST API with the following endpoints:
- POST /users - Create a new user
- GET /users/:id - Get user by ID
- PUT /users/:id - Update user
- DELETE /users/:id - Delete user

Use Express.js with TypeScript. Include input validation
and proper error handling.
EOF
```

**Option B:** Pass inline prompt when running:

```bash
ralph run -p "Add input validation to the user API endpoints"
```

### 3. Run Ralph

```bash
# TUI mode (default) — real-time terminal UI for monitoring
ralph run

# With inline prompt
ralph run -p "Implement the login endpoint with JWT authentication"

# Headless mode (no TUI)
ralph run --no-tui

# Resume interrupted session
ralph run --continue

# Dry run (show what would execute)
ralph run --dry-run
```

### Alternative: SOP-Driven Sessions

For standalone planning and task generation (without Ralph's event loop), use these commands:

```bash
# Start an interactive PDD planning session
ralph plan                           # SOP prompts for input
ralph plan "build a REST API"        # Provide idea inline
ralph plan --backend kiro "my idea"  # Use specific backend

# Generate code task files from descriptions
ralph task                           # SOP prompts for input
ralph task "add authentication"      # From description
ralph task specs/feature/plan.md     # From PDD plan file
```

These commands spawn an interactive AI session with bundled SOPs — perfect for one-off planning without configuring a full workflow.

## Configuration

Ralph uses a YAML configuration file (`ralph.yml` by default).

### Traditional Mode (No Hats)

The simplest configuration — just a loop that runs until completion:

```yaml
# ralph.yml — Traditional mode
cli:
  backend: "claude"

event_loop:
  completion_promise: "LOOP_COMPLETE"
  max_iterations: 100
```

This runs Ralph in a loop. No hats, no events, no role switching. Ralph iterates until it outputs `LOOP_COMPLETE` or hits the iteration limit.

### Hat-Based Mode (Specialized Personas)

> Ralph can wear many hats.

Add a `hats` section to enable role-based orchestration. Hats subscribe to events (triggers) and publish events when done:

```yaml
# ralph.yml — Hat-based mode (example structure)
cli:
  backend: "claude"

event_loop:
  completion_promise: "LOOP_COMPLETE"
  max_iterations: 100
  starting_event: "task.start"

hats:
  my_hat:
    name: "🎯 My Hat"
    triggers: ["task.start"]        # Events that activate this hat
    publishes: ["work.done"]        # Events this hat can emit
    instructions: |
      Your instructions here...
```

With hats, Ralph publishes a starting event, which triggers the matching hat. That hat does its work and publishes an event, potentially triggering other hats. This event-driven handoff continues until completion.

> **Tip:** Use `ralph init --preset <name>` to get pre-configured hat workflows. See [Presets](#presets) for ready-made patterns like TDD, spec-driven development, and more.

### Full Configuration Reference

```yaml
# Event loop settings
event_loop:
  completion_promise: "LOOP_COMPLETE"  # Output that signals completion
  max_iterations: 100                   # Maximum orchestration loops
  max_runtime_seconds: 14400            # 4 hours max runtime
  idle_timeout_secs: 1800               # 30 min idle timeout
  starting_event: "task.start"          # First event published

# CLI backend settings
cli:
  backend: "claude"                     # claude, kiro, gemini, codex, amp, copilot, opencode, custom
  prompt_mode: "arg"                    # arg (CLI argument) or stdin

# Core behaviors (always injected into prompts)
core:
  scratchpad: ".agent/scratchpad.md"    # Shared memory across iterations
  specs_dir: "./specs/"                 # Directory for specifications
  guardrails:                           # Rules injected into every prompt
    - "Fresh context each iteration - scratchpad is memory"
    - "Don't assume 'not implemented' - search first"
    - "Backpressure is law - tests/typecheck/lint must pass"

# Memories — persistent learning across sessions (enabled by default)
memories:
  enabled: true                         # Set false to disable
  inject: auto                          # auto, manual, or none

# Tasks — runtime work tracking (enabled by default)
tasks:
  enabled: true                         # Set false to use scratchpad-only mode

# Custom hats (omit to use default planner/builder)
hats:
  my_hat:
    name: "My Hat Name"                 # Display name
    triggers: ["some.event"]            # Events that activate this hat
    publishes: ["other.event"]          # Events this hat can emit
    instructions: |                     # Prompt instructions
      What this hat should do...
```


## Custom Backends and Per-Hat Configuration

### Custom Backends

Beyond the built-in backends (Claude, Kiro, Gemini, Codex, Amp, Copilot, OpenCode), you can define custom backends to integrate any CLI-based AI agent:

```yaml
cli:
  backend: "custom"
  command: "my-agent"
  args: ["--headless", "--auto-approve"]
  prompt_mode: "arg"        # "arg" or "stdin"
  prompt_flag: "-p"         # Optional: flag for prompt argument
```

| Field | Description |
|-------|-------------|
| `command` | The CLI command to execute |
| `args` | Arguments inserted before the prompt |
| `prompt_mode` | How to pass the prompt: `arg` (command-line argument) or `stdin` |
| `prompt_flag` | Flag preceding the prompt (e.g., `-p`, `--prompt`). If omitted, prompt is positional. |

### Per-Hat Backend Configuration

Different hats can use different backends, enabling specialized tools for specialized tasks:

```yaml
cli:
  backend: "claude"  # Default for Ralph and hats without explicit backend

hats:
  builder:
    name: "🔨 Builder"
    description: "Implements code"
    triggers: ["build.task"]
    publishes: ["build.done"]
    backend: "claude"        # Explicit: Claude for coding

  researcher:
    name: "🔍 Researcher"
    description: "Researches technical questions"
    triggers: ["research.task"]
    publishes: ["research.done"]
    backend:                 # Kiro with custom agent (has MCP tools)
      type: "kiro"
      agent: "researcher"

  reviewer:
    name: "👀 Reviewer"
    description: "Reviews code changes"
    triggers: ["review.task"]
    publishes: ["review.done"]
    backend: "gemini"        # Different model for fresh perspective
```

**Backend Types:**

| Type | Syntax | Invocation |
|------|--------|------------|
| Named | `backend: "claude"` | Uses standard backend configuration |
| Kiro Agent | `backend: { type: "kiro", agent: "builder" }` | `kiro-cli --agent builder ...` |
| Custom | `backend: { command: "...", args: [...] }` | Your custom command |

**When to mix backends:**

| Scenario | Recommended Backend |
|----------|---------------------|
| Complex coding | Claude (best reasoning) |
| AWS/cloud tasks | Kiro with agent (MCP tools) |
| Code review | Different model (fresh perspective) |
| Internal tools | Custom backend |
| Cost optimization | Faster/cheaper model for simple tasks |

Hats without explicit `backend` inherit from `cli.backend`.

## Presets

Presets are pre-configured workflows for common development patterns.

### Development Workflows

| Preset | Pattern | Description |
|--------|---------|-------------|
| `feature` | Planner-Builder | Standard feature development |
| `feature-minimal` | Single hat | Minimal feature development |
| `tdd-red-green` | Test-Implement-Refactor | TDD with red-green-refactor cycle |
| `spec-driven` | Spec-Build-Verify | Specification-first development |
| `refactor` | Analyze-Plan-Execute | Code refactoring workflow |

### Debugging & Investigation

| Preset | Pattern | Description |
|--------|---------|-------------|
| `debug` | Investigate-Fix-Verify | Bug investigation and fixing |
| `incident-response` | Triage-Fix-Postmortem | Production incident handling |
| `code-archaeology` | Explore-Document-Present | Legacy code understanding |

### Review & Quality

| Preset | Pattern | Description |
|--------|---------|-------------|
| `review` | Analyze-Critique-Suggest | Code review workflow |
| `pr-review` | Multi-Perspective | PR review with specialized reviewers |
| `adversarial-review` | Critic-Defender | Devil's advocate review style |

### Documentation

| Preset | Pattern | Description |
|--------|---------|-------------|
| `docs` | Write-Review-Publish | Documentation writing |
| `documentation-first` | Doc-Implement-Sync | Doc-first development |

### Specialized

| Preset | Pattern | Description |
|--------|---------|-------------|
| `api-design` | Design-Implement-Document | API-first development |
| `migration-safety` | Analyze-Migrate-Verify | Safe code migrations |
| `performance-optimization` | Profile-Optimize-Benchmark | Performance tuning |
| `scientific-method` | Hypothesis-Experiment-Conclude | Experimental approach |
| `mob-programming` | Rotate roles | Simulated mob programming |
| `socratic-learning` | Question-Answer-Synthesize | Learning through dialogue |
| `research` | Gather-Analyze-Synthesize | Research and analysis |
| `gap-analysis` | Current-Target-Plan | Gap identification |

### Using Presets

```bash
# List all available presets
ralph init --list-presets

# Initialize with a preset
ralph init --preset tdd-red-green

# Use preset with different backend
ralph init --preset spec-driven --backend gemini

# Override existing config
ralph init --preset debug --force
```

## Key Concepts

### Hats

Hats are specialized Ralph personas. Each hat has:

- **Triggers**: Events that activate this hat
- **Publishes**: Events this hat can emit
- **Instructions**: Prompt injected when hat is active

View event history:

```bash
ralph events
```

## Orchestration and Coordination Patterns

Ralph's hat system enables sophisticated multi-agent workflows through event-driven coordination. This section covers the architectural patterns, event routing mechanics, and built-in workflow templates.

### How Hat-Based Orchestration Works

#### The Event-Driven Model

Hats communicate through a **pub/sub event system**:

1. **Ralph publishes a starting event** (e.g., `task.start`)
2. **The matching hat activates** — the hat subscribed to that event takes over
3. **The hat does its work** and publishes an event when done
4. **The next hat activates** — triggered by the new event
5. **The cycle continues** until a termination event or `LOOP_COMPLETE`

```
┌─────────────────────────────────────────────────────────────────┐
│  task.start → [Test Writer] → test.written → [Implementer] →   │
│  test.passing → [Refactorer] → refactor.done ──┐                │
│                                                │                │
│  ┌─────────────────────────────────────────────┘                │
│  └──→ (loops back to Test Writer for next test)                 │
└─────────────────────────────────────────────────────────────────┘
```

#### Ralph as the Constant Coordinator

In hat-based mode, **Ralph is always present**:

- Ralph cannot be removed or replaced
- Custom hats define the **topology** (who triggers whom)
- Ralph executes with **topology awareness** — knowing which hats exist and their relationships
- Ralph serves as the **universal fallback** — orphaned events automatically route to Ralph

This means custom hats don't execute directly. Instead, Ralph reads all pending events across all hats and decides what to do based on the defined topology. Ralph then either:
- Delegates to the appropriate hat by publishing an event
- Handles the work directly if no hat is suited

#### Event Routing and Topic Matching

Events route to hats using **glob-style pattern matching**:

| Pattern | Matches |
|---------|---------|
| `task.start` | Exactly `task.start` |
| `build.*` | `build.done`, `build.blocked`, `build.task`, etc. |
| `*.done` | `build.done`, `review.done`, `test.done`, etc. |
| `*` | Everything (global wildcard — used by Ralph as fallback) |

**Priority Rules:**
- Specific patterns take precedence over wildcards
- If multiple hats have specific subscriptions, that's an error (ambiguous routing)
- Global wildcard (`*`) only triggers if no specific handler exists

### Coordination Patterns

Ralph presets implement several proven coordination patterns:

#### 1. Linear Pipeline

The simplest pattern — work flows through a sequence of specialists.

```
Input → Hat A → Event → Hat B → Event → Hat C → Output
```

**Example: TDD Red-Green-Refactor** (`tdd-red-green.yml`)

```yaml
hats:
  test_writer:
    triggers: ["tdd.start", "refactor.done"]
    publishes: ["test.written"]

  implementer:
    triggers: ["test.written"]
    publishes: ["test.passing"]

  refactorer:
    triggers: ["test.passing"]
    publishes: ["refactor.done", "cycle.complete"]
```

```
tdd.start → 🔴 Test Writer → test.written → 🟢 Implementer →
test.passing → 🔵 Refactorer → refactor.done ─┐
                                              │
              ┌───────────────────────────────┘
              └──→ (back to Test Writer)
```

**When to use:** Workflows with clear sequential phases where each step builds on the previous.

#### 2. Contract-First Pipeline

A variant where work must pass validation gates before proceeding.

**Example: Spec-Driven Development** (`spec-driven.yml`)

```yaml
hats:
  spec_writer:
    triggers: ["spec.start", "spec.rejected"]
    publishes: ["spec.ready"]

  spec_reviewer:
    triggers: ["spec.ready"]
    publishes: ["spec.approved", "spec.rejected"]

  implementer:
    triggers: ["spec.approved", "spec.violated"]
    publishes: ["implementation.done"]

  verifier:
    triggers: ["implementation.done"]
    publishes: ["task.complete", "spec.violated"]
```

```
spec.start → 📋 Spec Writer ──→ spec.ready ──→ 🔎 Spec Critic
                 ↑                                   │
                 └────── spec.rejected ──────────────┤
                                                     ↓
                                               spec.approved
                                                     │
                                                     ↓
task.complete ←── ✅ Verifier ←── impl.done ←── ⚙️ Implementer
                       │                              ↑
                       └──── spec.violated ───────────┘
```

**When to use:** High-stakes changes where the spec must be rock-solid before implementation begins.

#### 3. Cyclic Rotation

Multiple roles take turns, each bringing a different perspective.

**Example: Mob Programming** (`mob-programming.yml`)

```yaml
hats:
  navigator:
    triggers: ["mob.start", "observation.noted"]
    publishes: ["direction.set", "mob.complete"]

  driver:
    triggers: ["direction.set"]
    publishes: ["code.written"]

  observer:
    triggers: ["code.written"]
    publishes: ["observation.noted"]
```

```
mob.start → 🧭 Navigator → direction.set → ⌨️ Driver →
code.written → 👁️ Observer → observation.noted ─┐
                                                │
              ┌─────────────────────────────────┘
              └──→ (back to Navigator)
```

**When to use:** Complex features that benefit from multiple perspectives and continuous feedback.

#### 4. Adversarial Review

Two roles with opposing objectives ensure robustness.

**Example: Red Team / Blue Team** (`adversarial-review.yml`)

```yaml
hats:
  builder:
    name: "🔵 Blue Team (Builder)"
    triggers: ["security.review", "fix.applied"]
    publishes: ["build.ready"]

  red_team:
    name: "🔴 Red Team (Attacker)"
    triggers: ["build.ready"]
    publishes: ["vulnerability.found", "security.approved"]

  fixer:
    triggers: ["vulnerability.found"]
    publishes: ["fix.applied"]
```

```
security.review → 🔵 Blue Team → build.ready → 🔴 Red Team
                      ↑                            │
                      │                            ├─→ security.approved ✓
                      │                            │
                      │                            └─→ vulnerability.found
                      │                                        │
                      └────── fix.applied ←── 🛡️ Fixer ←──────┘
```

**When to use:** Security-sensitive code, authentication systems, or any code where adversarial thinking improves quality.

#### 5. Hypothesis-Driven Investigation

The scientific method applied to debugging.

**Example: Scientific Method** (`scientific-method.yml`)

```yaml
hats:
  observer:
    triggers: ["science.start", "hypothesis.rejected"]
    publishes: ["observation.made"]

  theorist:
    triggers: ["observation.made"]
    publishes: ["hypothesis.formed"]

  experimenter:
    triggers: ["hypothesis.formed"]
    publishes: ["hypothesis.confirmed", "hypothesis.rejected"]

  fixer:
    triggers: ["hypothesis.confirmed"]
    publishes: ["fix.applied"]
```

```
science.start → 🔬 Observer → observation.made → 🧠 Theorist →
hypothesis.formed → 🧪 Experimenter ──┬─→ hypothesis.confirmed → 🔧 Fixer
                                      │
                                      └─→ hypothesis.rejected ─┐
                                                               │
              ┌────────────────────────────────────────────────┘
              └──→ (back to Observer with new data)
```

**When to use:** Complex bugs where the root cause isn't obvious. Forces systematic investigation over random fixes.

#### 6. Coordinator-Specialist (Fan-Out)

A coordinator delegates to specialists based on the work type.

**Example: Gap Analysis** (`gap-analysis.yml`)

```yaml
hats:
  analyzer:
    triggers: ["gap.start", "verify.complete", "report.complete"]
    publishes: ["analyze.spec", "verify.request", "report.request"]

  verifier:
    triggers: ["analyze.spec", "verify.request"]
    publishes: ["verify.complete"]

  reporter:
    triggers: ["report.request"]
    publishes: ["report.complete"]
```

```
                    ┌─→ analyze.spec ──→ 🔍 Verifier ──┐
                    │                                  │
gap.start → 📊 Analyzer ←── verify.complete ──────────┘
                    │
                    └─→ report.request ──→ 📝 Reporter ──→ report.complete
```

**When to use:** Work that naturally decomposes into independent specialist tasks (analysis, verification, reporting).

#### 7. Adaptive Entry Point

A bootstrapping hat detects input type and routes to the appropriate workflow.

**Example: Code-Assist** (`code-assist.yml`)

```yaml
hats:
  planner:
    triggers: ["build.start"]
    publishes: ["tasks.ready"]
    # Detects: PDD directory vs. code task file vs. description

  builder:
    triggers: ["tasks.ready", "validation.failed", "task.complete"]
    publishes: ["implementation.ready", "task.complete"]

  validator:
    triggers: ["implementation.ready"]
    publishes: ["validation.passed", "validation.failed"]

  committer:
    triggers: ["validation.passed"]
    publishes: ["commit.complete"]
```

```
build.start → 📋 Planner ─── (detects input type) ───→ tasks.ready
                                                            │
    ┌───────────────────────────────────────────────────────┘
    │
    ↓
⚙️ Builder ←─────────────── validation.failed ←─────┐
    │                                               │
    ├── task.complete ──→ (loop for PDD mode) ──────┤
    │                                               │
    └── implementation.ready ──→ ✅ Validator ──────┤
                                      │             │
                                      └─→ validation.passed
                                              │
                                              ↓
                                        📦 Committer → commit.complete
```

**When to use:** Workflows that need to handle multiple input formats or adapt their behavior based on context.

### Designing Custom Hat Collections

#### Hat Configuration Schema

```yaml
hats:
  my_hat:
    name: "🎯 Display Name"      # Shown in TUI and logs
    description: "What this hat does"  # REQUIRED — Ralph uses this for delegation
    triggers: ["event.a", "event.b"]   # Events that activate this hat
    publishes: ["event.c", "event.d"]  # Events this hat can emit
    default_publishes: "event.c"       # Fallback if hat forgets to emit
    max_activations: 10                # Optional cap on activations
    backend: "claude"                  # Optional backend override
    instructions: |
      Prompt injected when this hat is active.
      Tell the hat what to do, not how to do it.
```

#### Design Principles

1. **Description is critical** — Ralph uses hat descriptions to decide when to delegate. Make them clear and specific.

2. **One hat, one responsibility** — Each hat should have a clear, focused purpose. If you're writing "and" in the description, consider splitting.

3. **Events are routing signals, not data** — Keep payloads brief. Store detailed output in files and reference them in events.

4. **Design for recovery** — If a hat fails or forgets to publish, Ralph catches the orphaned event. Your topology should handle unexpected states gracefully.

5. **Test with simple prompts first** — Complex topologies can have emergent behavior. Start simple, validate the flow, then add complexity.

#### Validation Rules

Ralph validates hat configurations:

- **Required description**: Every hat must have a description (Ralph needs it for delegation context)
- **Reserved triggers**: `task.start` and `task.resume` are reserved for Ralph
- **No ambiguous routing**: Each trigger pattern must map to exactly one hat

```
ERROR: Ambiguous routing for trigger 'build.done'.
Both 'planner' and 'reviewer' trigger on 'build.done'.
```

### Event Emission

Hats emit events to signal completion or hand off work:

```bash
# Simple event with payload
ralph emit "build.done" "tests: pass, lint: pass"

# Event with JSON payload
ralph emit "review.done" --json '{"status": "approved", "issues": 0}'

# Direct handoff to specific hat (bypasses routing)
ralph emit "handoff" --target reviewer "Please review the changes"
```

**In agent output**, events are embedded as XML tags:

```xml
<event topic="impl.done">Implementation complete</event>
<event topic="handoff" target="reviewer">Please review</event>
```

### Choosing a Pattern

| Scenario | Recommended Pattern | Preset |
|----------|---------------------|--------|
| Sequential workflow with clear phases | Linear Pipeline | `tdd-red-green` |
| Spec must be approved before coding | Contract-First | `spec-driven` |
| Need multiple perspectives | Cyclic Rotation | `mob-programming` |
| Security review required | Adversarial | `adversarial-review` |
| Debugging complex issues | Hypothesis-Driven | `scientific-method` |
| Work decomposes into specialist tasks | Coordinator-Specialist | `gap-analysis` |
| Multiple input formats | Adaptive Entry | `code-assist` |
| Standard feature development | Basic delegation | `feature` |

### When Not to Use Hats

Hat-based orchestration adds complexity. Use **traditional mode** (no hats) when:

- The task is straightforward and single-focused
- You don't need role separation or handoffs
- You're prototyping and want minimal configuration
- The work doesn't naturally decompose into distinct phases

Traditional mode is just Ralph in a loop until completion — simpler, faster to set up, and often sufficient.

### Memories and Tasks

Ralph uses two complementary systems for persistent state (both enabled by default):

**Memories** (`.agent/memories.md`) — Accumulated wisdom across sessions:
- Codebase patterns and conventions discovered
- Architectural decisions and rationale
- Recurring problem solutions (fixes)
- Project-specific context

**Tasks** (`.agent/tasks.jsonl`) — Runtime work tracking:
- Create, list, and close tasks during orchestration
- Track dependencies between tasks
- Used for loop completion verification

When memories and tasks are enabled, they replace the scratchpad for state management. Set `memories.enabled: false` and `tasks.enabled: false` to use the legacy scratchpad-only mode.

### Scratchpad (Legacy Mode)

When memories/tasks are disabled, all hats share `.agent/scratchpad.md` — persistent memory across iterations. This enables hats to build on previous work rather than starting fresh.

The scratchpad is the primary mechanism for:
- Task tracking (with `[ ]`, `[x]`, `[~]` markers)
- Context preservation between iterations
- Handoff between hats

### Backpressure

Ralph enforces quality gates through backpressure. When a builder publishes `build.done`, it must include evidence:

```
tests: pass, lint: pass, typecheck: pass
```

## Multi-Loop Concurrency

Ralph supports running multiple orchestration loops in parallel using git worktrees for filesystem isolation. This enables working on multiple tasks simultaneously without conflicts.

### How It Works

When you start a Ralph loop:

1. **First loop** acquires `.ralph/loop.lock` and runs in-place (the primary loop)
2. **Additional loops** automatically spawn into `.worktrees/<loop-id>/`
3. **Each loop** has isolated events, tasks, and scratchpad
4. **Memories are shared** — symlinked back to the main repo's `.agent/memories.md`
5. **On completion**, worktree loops automatically spawn a merge-ralph to integrate changes

```
┌─────────────────────────────────────────────────────────────────────┐
│  Terminal 1                    │  Terminal 2                       │
│  ralph run -p "Add auth"       │  ralph run -p "Add logging"       │
│  [acquires lock, runs in-place]│  [spawns to worktree]             │
│           ↓                    │           ↓                       │
│     Primary loop               │  .worktrees/ralph-20250124-a3f2/  │
│           ↓                    │           ↓                       │
│     LOOP_COMPLETE              │     LOOP_COMPLETE → auto-merge    │
└─────────────────────────────────────────────────────────────────────┘
```

### Usage

```bash
# First loop acquires lock, runs in-place
ralph run -p "Add authentication"

# In another terminal — automatically spawns to worktree
ralph run -p "Add logging"

# Check running loops
ralph loops

# View logs from a specific loop
ralph loops logs <loop-id>
ralph loops logs <loop-id> --follow  # Real-time streaming

# Force sequential execution (wait for lock)
ralph run --exclusive -p "Task that needs main workspace"

# Skip auto-merge (keep worktree for manual handling)
ralph run --no-auto-merge -p "Experimental feature"
```

### Loop States

| State | Description |
|-------|-------------|
| `running` | Loop is actively executing |
| `queued` | Completed, waiting for merge |
| `merging` | Merge operation in progress |
| `merged` | Successfully merged to main |
| `needs-review` | Merge failed, requires manual resolution |
| `crashed` | Process died unexpectedly |
| `orphan` | Worktree exists but not tracked |
| `discarded` | Explicitly abandoned by user |

### File Structure

```
project/
├── .ralph/
│   ├── loop.lock          # Primary loop indicator
│   ├── loops.json         # Loop registry
│   ├── merge-queue.jsonl  # Merge event log
│   └── events.jsonl       # Primary loop events
├── .agent/
│   └── memories.md        # Shared across all loops
└── .worktrees/
    └── ralph-20250124-a3f2/
        ├── .ralph/events.jsonl    # Loop-isolated
        ├── .agent/
        │   ├── memories.md → ../../.agent/memories.md  # Symlink
        │   └── scratchpad.md      # Loop-isolated
        └── [project files]
```

### Managing Loops

```bash
# List all loops with status
ralph loops list

# View loop output
ralph loops logs <id>              # Full output
ralph loops logs <id> --follow     # Stream real-time

# View event history
ralph loops history <id>           # Formatted table
ralph loops history <id> --json    # Raw JSONL

# Show changes from merge-base
ralph loops diff <id>              # Full diff
ralph loops diff <id> --stat       # Summary only

# Open shell in worktree
ralph loops attach <id>

# Re-run merge for failed loop
ralph loops retry <id>

# Stop a running loop
ralph loops stop <id>              # SIGTERM
ralph loops stop <id> --force      # SIGKILL

# Abandon loop and cleanup
ralph loops discard <id>           # With confirmation
ralph loops discard <id> -y        # Skip confirmation

# Clean up stale loops (crashed processes)
ralph loops prune
```

### Auto-Merge Workflow

When a worktree loop completes, it queues itself for merge. The primary loop processes this queue when it finishes:

```
┌──────────────────────────────────────────────────────────────────────┐
│  Worktree Loop                         Primary Loop                  │
│  ─────────────                         ─────────────                 │
│  LOOP_COMPLETE                                                       │
│       ↓                                                              │
│  Queue for merge ─────────────────────→ [continues working]         │
│       ↓                                       ↓                      │
│  Exit cleanly                          LOOP_COMPLETE                 │
│                                              ↓                       │
│                                        Process merge queue           │
│                                              ↓                       │
│                                        Spawn merge-ralph             │
└──────────────────────────────────────────────────────────────────────┘
```

The merge-ralph process uses a **hat collection** with specialized roles:

| Hat | Trigger | Purpose |
|-----|---------|---------|
| `merger` | `merge.start` | Performs `git merge`, runs tests |
| `resolver` | `conflict.detected` | Resolves merge conflicts by understanding intent |
| `tester` | `conflict.resolved` | Verifies tests pass after conflict resolution |
| `cleaner` | `merge.done` | Removes worktree and branch |
| `failure_handler` | `*failed`, `unresolvable` | Marks loop for manual review |

The workflow handles conflicts intelligently:
1. **No conflicts**: Merge → Run tests → Clean up → Done
2. **With conflicts**: Detect → AI resolves → Run tests → Clean up → Done
3. **Unresolvable**: Abort → Mark for review → Keep worktree for manual fix

### Conflict Resolution

When merge conflicts occur, the AI resolver:

1. Examines conflict markers (`<<<<<<<`, `=======`, `>>>>>>>`)
2. Understands the **intent** of both sides (not just the code)
3. Resolves by preserving both intents when possible
4. Prefers the loop's changes when directly contradictory (newer work)

**Conflicts marked `needs-review`:**
- Major architectural changes on both sides
- Complex refactoring that can't be automatically reconciled
- Business logic contradictions requiring human judgment

To manually resolve:
```bash
# Enter the worktree
ralph loops attach <loop-id>

# Fix the issue, commit
git add . && git commit -m "Manual conflict resolution"

# Retry the merge
ralph loops retry <loop-id>

# Or discard if unneeded
ralph loops discard <loop-id>
```

### Best Practices

**When to use parallel loops:**
- ✅ Independent features with minimal file overlap
- ✅ Bug fixes while feature work continues
- ✅ Documentation updates parallel to code changes
- ✅ Test additions that don't conflict with active development

**When to use `--exclusive` (sequential):**
- ⚠️ Large refactoring touching many files
- ⚠️ Database migrations or schema changes
- ⚠️ Tasks that modify shared configuration files
- ⚠️ Work that depends on changes from another in-progress loop

**Tips for reducing conflicts:**
- Keep loops focused on distinct areas of the codebase
- Use separate files when adding new features
- Avoid modifying the same functions in parallel loops
- Let one loop complete before starting conflicting work

### Troubleshooting

**Loop stuck in `queued` state:**
```bash
# Check if primary loop is still running
ralph loops

# If primary finished but merge didn't start, manually trigger
ralph loops retry <loop-id>
```

**Merge keeps failing:**
```bash
# View merge-ralph logs
ralph loops logs <loop-id>

# Check what changes conflict
ralph loops diff <loop-id>

# Manually resolve in worktree
ralph loops attach <loop-id>
```

**Orphaned worktrees:**
```bash
# List and clean up orphans
ralph loops prune

# Force cleanup of specific worktree
git worktree remove .worktrees/<loop-id> --force
git branch -D ralph/<loop-id>
```

**Lock file issues:**
```bash
# Check who holds the lock
cat .ralph/loop.lock

# If process is dead, remove stale lock
rm .ralph/loop.lock
```

### Environment Variables

| Variable | Description |
|----------|-------------|
| `RALPH_MERGE_LOOP_ID` | Set by auto-merge to identify which loop to merge |
| `RALPH_DIAGNOSTICS=1` | Enable detailed diagnostic logging |
| `RALPH_VERBOSE=1` | Verbose output mode |
| `RALPH_DEBUG_LOG=1` | Write debug logs to `.agent/ralph.log` |

## CLI Reference

### Commands

| Command | Description |
|---------|-------------|
| `ralph run` | Run the orchestration loop (default) |
| `ralph resume` | Resume from existing scratchpad |
| `ralph plan` | Start an interactive PDD planning session |
| `ralph task` | Start an interactive code-task-generator session |
| `ralph events` | View event history |
| `ralph init` | Initialize configuration file |
| `ralph clean` | Clean up `.agent/` directory |
| `ralph emit` | Emit an event to the event log |
| `ralph tools` | Runtime tools for memories and tasks (agent-facing) |
| `ralph loops` | Manage parallel loops (list, logs, stop, merge) |

### Global Options

| Option | Description |
|--------|-------------|
| `-c, --config <FILE>` | Config file path (default: `ralph.yml`) |
| `-v, --verbose` | Verbose output |
| `--color <MODE>` | Color output: `auto`, `always`, `never` |

### `ralph run` Options

| Option | Description |
|--------|-------------|
| `-p, --prompt <TEXT>` | Inline prompt text |
| `-P, --prompt-file <FILE>` | Prompt file path |
| `--max-iterations <N>` | Override max iterations |
| `--completion-promise <TEXT>` | Override completion trigger |
| `--dry-run` | Show what would execute |
| `--no-tui` | Disable TUI mode (TUI is enabled by default) |
| `-a, --autonomous` | Force headless mode |
| `--idle-timeout <SECS>` | TUI idle timeout (default: 30) |
| `--record-session <FILE>` | Record session to JSONL |
| `-q, --quiet` | Suppress output (for CI) |
| `--continue` | Resume from existing scratchpad |
| `--exclusive` | Wait for primary loop slot instead of spawning worktree |
| `--no-auto-merge` | Skip automatic merge after worktree loop completes |

### `ralph init` Options

| Option | Description |
|--------|-------------|
| `--backend <NAME>` | Backend: `claude`, `kiro`, `gemini`, `codex`, `amp`, `copilot`, `opencode` |
| `--preset <NAME>` | Use preset configuration |
| `--list-presets` | List available presets |
| `--force` | Overwrite existing config |

### `ralph plan` Options

| Option | Description |
|--------|-------------|
| `<IDEA>` | Optional rough idea to develop (SOP prompts if not provided) |
| `-b, --backend <BACKEND>` | Backend to use (overrides config and auto-detection) |

### `ralph task` Options

| Option | Description |
|--------|-------------|
| `<INPUT>` | Optional description text or path to PDD plan file |
| `-b, --backend <BACKEND>` | Backend to use (overrides config and auto-detection) |

### `ralph tools` Subcommands

The `tools` command provides agent-facing utilities for runtime state management:

```bash
# Memory management (persistent learning)
ralph tools memory add "content" -t pattern --tags tag1,tag2
ralph tools memory search "query"
ralph tools memory list
ralph tools memory show <id>
ralph tools memory delete <id>

# Task management (runtime tracking)
ralph tools task add "Title" -p 2              # Create task (priority 1-5)
ralph tools task add "X" --blocked-by Y        # With dependency
ralph tools task list                           # All tasks
ralph tools task ready                          # Unblocked tasks only
ralph tools task close <id>                     # Mark complete
```

### `ralph loops` Subcommands

Manage parallel loops running in git worktrees:

| Subcommand | Description |
|------------|-------------|
| `list` | Show all loops with status (default) |
| `logs <ID>` | View loop output (`--follow` for real-time) |
| `history <ID>` | Show event history (`--json` for raw JSONL) |
| `retry <ID>` | Re-run merge for `needs-review` loop |
| `discard <ID>` | Abandon loop and cleanup worktree (`-y` to skip confirm) |
| `stop <ID>` | Terminate running loop (`--force` for SIGKILL) |
| `prune` | Clean up stale loops (crashed processes) |
| `attach <ID>` | Open shell in loop's worktree |
| `diff <ID>` | Show changes from merge-base (`--stat` for summary) |

## Architecture

Ralph is organized as a Cargo workspace with seven crates:

| Crate | Purpose |
|-------|---------|
| `ralph-proto` | Protocol types: Event, Hat, Topic, Error |
| `ralph-core` | Business logic: EventLoop, HatRegistry, Config |
| `ralph-adapters` | CLI backend integrations (Claude, Kiro, Gemini, etc.) |
| `ralph-tui` | Terminal UI with ratatui |
| `ralph-cli` | Binary entry point and CLI parsing |
| `ralph-e2e` | End-to-end test harness for backend validation |
| `ralph-bench` | Benchmarking harness (dev-only) |

## Building & Testing

### Build

```bash
cargo build           # Debug build
cargo build --release # Release build
```

### Test

```bash
# Run all tests (includes smoke tests with JSONL replay)
cargo test

# Run smoke tests specifically
cargo test -p ralph-core smoke_runner

# Run Kiro-specific smoke tests
cargo test -p ralph-core kiro
```

### Smoke Tests

Smoke tests use recorded JSONL fixtures instead of live API calls — fast, free, and deterministic.

**Fixture locations:**
- `crates/ralph-core/tests/fixtures/basic_session.jsonl` — Claude CLI session
- `crates/ralph-core/tests/fixtures/kiro/` — Kiro CLI sessions

**Recording new fixtures:**

```bash
# Record a session
ralph run -c ralph.yml --record-session session.jsonl -p "your prompt"

# Or capture raw CLI output
claude -p "your prompt" 2>&1 | tee output.txt
```

### Linting

```bash
cargo clippy --all-targets --all-features
cargo fmt --check
```

## Contributing

Contributions are welcome! Please:

1. Fork the repository
2. Create a feature branch (`git checkout -b feature/amazing-feature`)
3. Write tests for new functionality
4. Ensure `cargo test` passes
5. Run `cargo clippy` and `cargo fmt`
6. Commit your changes (`git commit -m 'Add amazing feature'`)
7. Push to the branch (`git push origin feature/amazing-feature`)
8. Open a Pull Request

See [AGENTS.md](AGENTS.md) for development philosophy and conventions.

## License

MIT License — See [LICENSE](LICENSE) for details.

## Acknowledgments

- **[Geoffrey Huntley](https://ghuntley.com/ralph/)** — Creator of the Ralph Wiggum technique
- **[Harper Reed](https://harper.blog/)** — Spec-driven development methodology
- **[Strands Agent SOPs](https://github.com/strands-agents/agent-sop)** — Natural language workflows that enable AI agents to perform complex, multi-step tasks with consistency and reliability. 
- **[ratatui](https://ratatui.rs/)** — Terminal UI framework
- **[portable-pty](https://crates.io/crates/portable-pty)** — Cross-platform PTY support

---

*"I'm learnding!" - Ralph Wiggum*

---

[![Star History Chart](https://api.star-history.com/svg?repos=mikeyobrien/ralph-orchestrator&type=date&legend=top-left)](https://www.star-history.com/#mikeyobrien/ralph-orchestrator&type=date&legend=top-left)
