---
status: review
gap_analysis: 2026-01-14
last_updated: 2026-01-14
related:
  - tui-mode.spec.md
  - event-loop.spec.md
  - adapters/claude.spec.md
  - adapters/gemini.spec.md
  - adapters/kiro.spec.md
  - adapters/codex.spec.md
  - adapters/amp.spec.md
---

# CLI Adapters Specification

## Overview

Ralph orchestrates AI agents via CLI adapters—thin wrappers that invoke headless coding assistants. Users can choose from built-in adapters or define custom adapters for any CLI tool that accepts prompts and produces text output.

## Problem Statement

Different teams use different AI assistants. A project using Claude shouldn't be locked out of Ralph if they want to experiment with Gemini. A company with internal tooling needs to plug in proprietary agents.

**Goals:**
1. Zero-friction switching between supported backends
2. First-class custom adapter support for any headless CLI agent
3. Clear guidance on when to use built-in vs custom adapters
4. Single point of extension for adding new built-in adapters

## Built-in vs Custom Adapters

### Why Use Built-in Adapters?

Built-in adapters are pre-configured with battle-tested defaults:

| Feature | Built-in | Custom |
|---------|----------|--------|
| **Auto-detection** | ✅ Discovered in PATH automatically | ❌ Must explicitly configure |
| **Headless flags** | ✅ Pre-configured for non-interactive mode | ⚠️ User must know correct flags |
| **Prompt delivery** | ✅ Optimal method pre-selected (arg vs stdin) | ⚠️ User must test and configure |
| **PTY compatibility** | ✅ Tested with Ralph's PTY mode | ⚠️ May not work with PTY |

**Recommendation:** Use built-in adapters when available. They handle edge cases you don't want to discover in production.

### When to Use Custom Adapters

Custom adapters are appropriate when:
- Using a proprietary or internal AI CLI tool
- Testing experimental or beta AI tools not yet supported
- Wrapping a tool with organization-specific flags or authentication
- Running a local model via a CLI interface (e.g., Ollama, llama.cpp)

## Supported Backends

| Backend | Command | Spec |
|---------|---------|------|
| **Claude** (default) | `claude` | [adapters/claude.spec.md](adapters/claude.spec.md) |
| **Gemini** | `gemini` | [adapters/gemini.spec.md](adapters/gemini.spec.md) |
| **Kiro** | `kiro-cli chat` | [adapters/kiro.spec.md](adapters/kiro.spec.md) |
| **Codex** | `codex exec` | [adapters/codex.spec.md](adapters/codex.spec.md) |
| **Amp** | `amp` | [adapters/amp.spec.md](adapters/amp.spec.md) |

### Quick Reference

| Backend | Auto-Approve Flag | Prompt Delivery | TTY Required | Structured Output |
|---------|-------------------|-----------------|--------------|-------------------|
| Claude | `--dangerously-skip-permissions` | `-p "prompt"` | **Yes** | `--output-format stream-json` |
| Gemini | `--yolo` | `-p "prompt"` | No | — |
| Kiro | `--trust-all-tools` | positional arg | No | — |
| Codex | `--full-auto` | positional arg | No | `--json` |
| Amp | `--dangerously-allow-all` | `-x "prompt"` | No | — |

### Custom Adapters

User-defined backend for any CLI tool that accepts prompts and produces text output.

| Property | Value |
|----------|-------|
| Command | User-specified |
| Headless flags | User-specified via `args` |
| Prompt mode | User-specified (`arg` or `stdin`) |
| TTY required | Depends on tool |
| Auto-detect | Not supported |

**Example — Ollama with a local model:**
```yaml
cli:
  backend: "custom"
  command: "ollama"
  args: ["run", "codellama"]
  prompt_mode: "stdin"
```

**Example — Internal corporate agent:**
```yaml
cli:
  backend: "custom"
  command: "/opt/corp-ai/agent"
  args: ["--headless", "--no-confirm", "--json"]
  prompt_mode: "arg"
  prompt_flag: "--prompt"
```

**Tips for custom adapters:**
1. Test the command manually first: `echo "hello" | your-tool` or `your-tool --prompt "hello"`
2. Ensure the tool exits cleanly after producing output (no interactive prompts)
3. If the tool has approval prompts, find the flag to disable them
4. For tools that require TTY, use TUI mode (`ralph run --tui`)

## Configuration

### Authentication

Ralph does not manage backend authentication. Ensure the appropriate environment variable is set before running:

| Backend | Environment Variable |
|---------|---------------------|
| Claude | Authenticated via `claude` CLI login |
| Gemini | `GEMINI_API_KEY` |
| Kiro | Authenticated via `kiro-cli` login |
| Codex | `CODEX_API_KEY` |
| Amp | `AMP_API_KEY` |

### ralph.yml Schema

```yaml
cli:
  # Backend selection: "auto", "claude", "gemini", "kiro", "codex", "amp", "custom"
  backend: "auto"

  # Custom backend configuration (only used when backend: "custom")
  command: "my-ai-cli"           # Required for custom
  args:                          # Optional: additional arguments
    - "--headless"
    - "--no-confirm"
  prompt_mode: "arg"             # "arg" or "stdin"
  prompt_flag: "--prompt"        # Flag prefix for arg mode (optional)

  # Execution mode (see interactive-mode.spec.md)
  default_mode: "autonomous"     # "autonomous" or "interactive"
  idle_timeout_secs: 30          # Kill after N seconds idle (TUI mode only)

# Per-adapter settings
adapters:
  claude:
    enabled: true                # Include in auto-detection
    timeout: 300                 # Execution timeout in seconds
  gemini:
    enabled: true
    timeout: 300
  kiro:
    enabled: true
    timeout: 300
  codex:
    enabled: true
    timeout: 300
  amp:
    enabled: true
    timeout: 300
  pi:
    enabled: true
    timeout: 300                 # Inactivity outside Pi tools and pending requests
    tool_timeout: 3600           # Pi only: tool or pending model-request ceiling
```

`adapters.pi` uses the same settings defaults as other adapters and never
inherits `adapters.claude`. In Pi JSON CLI execution, the inactivity timer pauses
while a tool is open and resumes after its matching end event (success or error).
Tool updates are not heartbeats, so silent tools are not killed by this timer;
instead a tool open longer than `tool_timeout` (default 3600s) fails the
iteration as stuck. A `turn_start` awaiting its first assistant message also
counts as activity and uses the same ceiling, with a distinct model-request
timeout if exceeded. Pi with neither an open tool nor a pending request still
times out on inactivity. Other backends keep their existing behavior.

### Auto-Detection

When `backend: "auto"` (the default), Ralph checks for available backends in priority order:

1. **claude** — Check `claude --version`
2. **kiro** — Check `kiro-cli --version`
3. **gemini** — Check `gemini --version`
4. **codex** — Check `codex --version`
5. **amp** — Check `amp --version`

The first backend that:
- Is found in PATH (command returns exit code 0)
- Is enabled in `adapters.<name>.enabled` (default: true)

...is selected for the session. The result is cached—detection runs once per Ralph invocation.

**Disabling adapters:**

```yaml
adapters:
  claude:
    enabled: false  # Skip Claude even if installed
```

## Adapter Interface

### CliBackend Structure

Each adapter defines:

```
┌─────────────────────────────────────────────────────────────┐
│                      CliBackend                              │
├─────────────────────────────────────────────────────────────┤
│  command: String        │ Executable name or path           │
│  args: Vec<String>      │ Arguments before the prompt       │
│  prompt_mode: PromptMode│ How to pass the prompt            │
│  prompt_flag: Option    │ Flag for arg mode (e.g., "-p")    │
└─────────────────────────────────────────────────────────────┘
```

### Prompt Modes

| Mode | Behavior | Best For |
|------|----------|----------|
| **Arg** | Prompt passed as command-line argument | Tools with `-p`, `--prompt` flags |
| **Stdin** | Prompt written to process stdin | Tools that read from stdin, large prompts |

**Arg mode example:**
```bash
claude --dangerously-skip-permissions -p "implement feature X"
```

**Stdin mode example:**
```bash
echo "implement feature X" | gemini
```

### Adding a New Built-in Adapter

To add support for a new backend, modify these files:

| File | Change |
|------|--------|
| `cli_backend.rs` | Add static constructor method and match arm in `from_config()` |
| `auto_detect.rs` | Add to `DEFAULT_PRIORITY` list and `NoBackendError` message |
| `config.rs` | Add `AdapterSettings` field to `AdaptersConfig` |

Follow the pattern of existing adapters. Add tests to verify command construction.

## Behavior

### Execution Flow

```
┌──────────────┐     ┌──────────────┐     ┌──────────────┐
│   Ralph      │     │  CliBackend  │     │  Executor    │
│  EventLoop   │     │  (config)    │     │ (CLI/PTY)    │
└──────┬───────┘     └──────┬───────┘     └──────┬───────┘
       │                    │                    │
       │ 1. Get backend     │                    │
       │───────────────────▶│                    │
       │                    │                    │
       │ 2. Build command   │                    │
       │───────────────────▶│                    │
       │                    │                    │
       │    (cmd, args,     │                    │
       │     stdin_input)   │                    │
       │◀───────────────────│                    │
       │                    │                    │
       │ 3. Execute         │                    │
       │───────────────────────────────────────▶│
       │                    │                    │
       │    ExecutionResult │                    │
       │◀───────────────────────────────────────│
       │                    │                    │
```

### Error Handling

| Scenario | Behavior |
|----------|----------|
| Backend not in PATH | Auto-detection skips it, or error if explicitly configured |
| Backend exits non-zero | Iteration marked failed, output captured for debugging |
| Backend times out | SIGTERM sent, then SIGKILL after grace period |
| Prompt too large for arg mode | Use stdin mode instead (custom backends) |
| No backends available | Clear error with installation links |
| Working directory inaccessible | Error before spawn with clear message |

### Working Directory

Agents execute in a specific working directory, which determines where file operations and relative paths resolve.

| Scenario | Working Directory |
|----------|-------------------|
| `ralph run` | Directory where `ralph` was invoked |
| `ralph run` with `ralph.yml` in parent | Directory where `ralph` was invoked (not config location) |
| Benchmark harness | Isolated workspace directory (see benchmark spec) |

**Default behavior**: The agent process inherits Ralph's current working directory at spawn time. This is the directory from which the user ran the `ralph` command.

**Why not the project root?** Some agents (like Claude) use their working directory to determine project scope. Running from a subdirectory is intentional—it lets users scope the agent to a specific area of the codebase.

**PTY considerations**: When spawning agents in a PTY (TUI mode), the working directory must be set explicitly on the `CommandBuilder`. The PTY does not automatically inherit the parent process's cwd in all cases. See [tui-mode.spec.md](tui-mode.spec.md) for details.

### Output Processing

All adapters produce text output that Ralph processes for:
1. **Event parsing** — `<event topic="...">` XML tags trigger hat changes
2. **Completion detection** — Configurable promise string (default: `LOOP_COMPLETE`)
3. **Logging** — Full output captured to `.agent/events.jsonl`

#### Structured Output Mode

Some backends support structured JSON output (see Structured Output column in Quick Reference). When enabled:
- Output is NDJSON (newline-delimited JSON) instead of plain text
- Each line is a self-contained event with type, content, and metadata
- Enables real-time TUI updates for tool calls, token usage, and progress
- See [adapters/claude.spec.md](adapters/claude.spec.md) for Claude's JSON stream format

## Acceptance Criteria

### Auto-Detection

**Given** `backend: "auto"` in config
**When** Ralph starts
**Then** it checks backends in priority order and selects the first available enabled backend

**Given** `backend: "auto"` and `adapters.claude.enabled: false`
**When** Claude is installed
**Then** Claude is skipped and the next available backend is selected

**Given** no backends are installed
**When** Ralph starts with `backend: "auto"`
**Then** a clear error message lists all checked backends with installation links

### Explicit Backend Selection

**Given** `backend: "gemini"` in config
**When** Ralph starts
**Then** Gemini is used regardless of auto-detection priority

**Given** `backend: "gemini"` but Gemini is not installed
**When** Ralph starts
**Then** an error indicates Gemini was requested but not found

### Custom Backend

**Given** `backend: "custom"` with `command: "my-ai"` and `prompt_mode: "stdin"`
**When** Ralph executes an iteration
**Then** the prompt is written to `my-ai` stdin

**Given** `backend: "custom"` with `args: ["--headless", "--json"]`
**When** Ralph builds the command
**Then** the args are included before the prompt

**Given** `backend: "custom"` without `command` specified
**When** Ralph starts
**Then** an error indicates custom backend requires a command

### Prompt Delivery

**Given** a backend with `prompt_mode: "arg"` and `prompt_flag: "-p"`
**When** executing with prompt "test"
**Then** the command is invoked as `<cmd> <args> -p "test"`

**Given** a backend with `prompt_mode: "arg"` and no `prompt_flag` (positional)
**When** executing with prompt "test"
**Then** the command is invoked as `<cmd> <args> "test"` (prompt appended without flag)

**Given** a backend with `prompt_mode: "stdin"`
**When** executing with prompt "test"
**Then** "test" is written to the process stdin

### Adapter Timeout

**Given** `adapters.claude.timeout: 60`
**When** Claude runs for more than 60 seconds
**Then** Ralph sends SIGTERM and marks the iteration as timed out

### Working Directory

**Given** user runs `ralph run` from `/home/user/project/src`
**When** Ralph spawns the agent
**Then** the agent's working directory is `/home/user/project/src`

**Given** user runs `ralph run` from `/home/user/project/src` with config at `/home/user/project/ralph.yml`
**When** Ralph spawns the agent
**Then** the agent's working directory is `/home/user/project/src` (not `/home/user/project`)

**Given** user runs `ralph run` from a directory that doesn't exist (race condition, deleted during startup)
**When** Ralph attempts to spawn the agent
**Then** an error is returned indicating the working directory is inaccessible

**Given** TUI mode with PTY spawning
**When** Ralph builds the command
**Then** the working directory is explicitly set on the PTY `CommandBuilder` (not relying on inheritance)

### TUI Mode Compatibility

**Given** `backend: "claude"` in autonomous mode
**When** Ralph executes an iteration
**Then** Ralph spawns Claude in a PTY (required due to Claude's TTY dependency)

**Given** TUI mode (`--tui`) and `backend: "custom"`
**When** the custom backend doesn't render TUI output
**Then** execution still works (PTY is transparent to simple CLI tools)

See [tui-mode.spec.md](tui-mode.spec.md) for details on execution modes.

## Non-Goals

- **Agent-specific features** — Adapters don't expose backend-specific capabilities (tools, MCP, etc.). Ralph treats all backends as prompt-in, text-out.
- **Multi-backend orchestration** — One backend per Ralph session. Switching mid-loop is not supported.
- **Backend authentication** — Adapters assume the CLI is already authenticated. Ralph doesn't manage API keys or OAuth flows.
- **Output format normalization** — Different backends have different output styles. Ralph parses events but doesn't normalize prose.
