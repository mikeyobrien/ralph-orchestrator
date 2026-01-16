# Ralph Orchestrator

[![License](https://img.shields.io/badge/license-MIT-blue)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.75+-orange)](https://www.rust-lang.org/)
[![Alpha](https://img.shields.io/badge/status-alpha-yellow)]()
[![Build](https://img.shields.io/github/actions/workflow/status/mikeyobrien/ralph-orchestrator/ci.yml?branch=main&label=CI)](https://github.com/mikeyobrien/ralph-orchestrator/actions)

A hat-based multi-agent orchestration framework that keeps AI agents in a loop until the task is done.

> "Me fail English? That's unpossible!" - Ralph Wiggum

**Alpha Notice:** Ralph Orchestrator is under active development. It works today, but expect rough edges and breaking changes between releases.

v1.0.0 was ralphed into existence with little oversight and guidance. v2.0.0 is a simpler, more-structured implementation. Looking for the old version? See [v1.2.3](https://github.com/mikeyobrien/ralph-orchestrator/tree/v1.2.3). 

## Table of Contents

- [What is Ralph?](#what-is-ralph)
- [Features](#features)
- [Installation](#installation)
- [Quick Start](#quick-start)
- [Configuration](#configuration)
- [Presets](#presets)
- [Key Concepts](#key-concepts)
- [CLI Reference](#cli-reference)
- [Architecture](#architecture)
- [Building & Testing](#building--testing)
- [Contributing](#contributing)
- [License](#license)
- [Acknowledgments](#acknowledgments)

## What is Ralph?

Ralph implements the [Ralph Wiggum technique](https://ghuntley.com/ralph/) — autonomous task completion through continuous AI agent iteration. Unlike simple loops, Ralph v2 introduces **hat-based orchestration**: specialized agent roles that coordinate through events.

> "The orchestrator is a thin coordination layer, not a platform. Agents are smart; let them do the work."

### The Ralph Tenets

1. **Fresh Context Is Reliability** — Each iteration clears context. Re-read specs, plan, code every cycle.
2. **Backpressure Over Prescription** — Don't prescribe how; create gates that reject bad work.
3. **The Plan Is Disposable** — Regeneration costs one planning loop. Cheap.
4. **Disk Is State, Git Is Memory** — Files are the handoff mechanism.
5. **Steer With Signals, Not Scripts** — Add signs, not scripts.
6. **Let Ralph Ralph** — Sit *on* the loop, not *in* it.

See [AGENTS.md](AGENTS.md) for the full philosophy.

## Features

- **Multi-Backend Support** — Works with Claude Code, Kiro, Gemini CLI, Codex, and Amp
- **Hat System** — Specialized agent personas with distinct behaviors
- **Event-Driven Coordination** — Hats communicate through typed events with glob pattern matching
- **Backpressure Enforcement** — Gates that reject incomplete work (tests, lint, typecheck)
- **Presets Library** — 20+ pre-configured workflows for common development patterns
- **Interactive TUI** — Real-time terminal UI for monitoring agent activity (experimental)
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

## Quick Start

### 1. Initialize a Project

```bash
# Minimal config for Claude (recommended)
ralph init --backend claude

# Use a preset workflow
ralph init --preset tdd-red-green

# Combine preset with different backend
ralph init --preset spec-driven --backend kiro

# See all available presets
ralph init --list-presets
```

This creates `ralph.yml` in your current directory.

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
# Autonomous mode (headless, default)
ralph run

# With inline prompt
ralph run -p "Implement the login endpoint with JWT authentication"

# Interactive TUI mode (experimental)
ralph run -i

# Resume interrupted session
ralph resume

# Dry run (show what would execute)
ralph run --dry-run
```

## Configuration

Ralph uses a YAML configuration file (`ralph.yml` by default).

### Minimal Configuration

```yaml
# ralph.yml
cli:
  backend: "claude"

event_loop:
  completion_promise: "LOOP_COMPLETE"
  max_iterations: 100
```

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
  backend: "claude"                     # claude, kiro, gemini, codex, amp, custom
  prompt_mode: "arg"                    # arg (CLI argument) or stdin
  experimental_tui: false               # Enable TUI mode support

# Core behaviors (always injected into prompts)
core:
  scratchpad: ".agent/scratchpad.md"    # Shared memory across iterations
  specs_dir: "./specs/"                 # Directory for specifications
  guardrails:                           # Rules injected into every prompt
    - "Fresh context each iteration - scratchpad is memory"
    - "Don't assume 'not implemented' - search first"
    - "Backpressure is law - tests/typecheck/lint must pass"

# Custom hats (omit to use default planner/builder)
hats:
  my_hat:
    name: "My Hat Name"                 # Display name
    triggers: ["some.event"]            # Events that activate this hat
    publishes: ["other.event"]          # Events this hat can emit
    instructions: |                     # Prompt instructions
      What this hat should do...
```


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

Hats are specialized agent personas. Each hat has:

- **Triggers**: Events that activate this hat
- **Publishes**: Events this hat can emit
- **Instructions**: Prompt injected when hat is active

View event history:

```bash
ralph events
```

### Scratchpad

All hats share `.agent/scratchpad.md` — persistent memory across iterations. This enables hats to build on previous work rather than starting fresh.

The scratchpad is the primary mechanism for:
- Task tracking (with `[ ]`, `[x]`, `[~]` markers)
- Context preservation between iterations
- Handoff between hats

### Backpressure

Ralph enforces quality gates through backpressure. When a builder publishes `build.done`, it must include evidence:

```
tests: pass, lint: pass, typecheck: pass
```

## CLI Reference

### Commands

| Command | Description |
|---------|-------------|
| `ralph run` | Run the orchestration loop (default) |
| `ralph resume` | Resume from existing scratchpad |
| `ralph events` | View event history |
| `ralph init` | Initialize configuration file |
| `ralph clean` | Clean up `.agent/` directory |
| `ralph emit` | Emit an event to the event log |

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
| `-i, --interactive` | Enable TUI mode (experimental) |
| `-a, --autonomous` | Force headless mode |
| `--idle-timeout <SECS>` | TUI idle timeout (default: 30) |
| `--record-session <FILE>` | Record session to JSONL |
| `-q, --quiet` | Suppress output (for CI) |

### `ralph init` Options

| Option | Description |
|--------|-------------|
| `--backend <NAME>` | Backend: `claude`, `kiro`, `gemini`, `codex`, `amp` |
| `--preset <NAME>` | Use preset configuration |
| `--list-presets` | List available presets |
| `--force` | Overwrite existing config |

## Architecture

Ralph is organized as a Cargo workspace with six crates:

| Crate | Purpose |
|-------|---------|
| `ralph-proto` | Protocol types: Event, Hat, Topic, Error |
| `ralph-core` | Business logic: EventLoop, HatRegistry, Config |
| `ralph-adapters` | CLI backend integrations (Claude, Kiro, Gemini, etc.) |
| `ralph-tui` | Terminal UI with ratatui |
| `ralph-cli` | Binary entry point and CLI parsing |
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
