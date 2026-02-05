# CLI Reference

Complete reference for Hats's command-line interface.

## Global Options

These options work with all commands:

| Option | Description |
|--------|-------------|
| `-c, --config <SOURCE>` | Config source (can be specified multiple times) |
| `-v, --verbose` | Verbose output |
| `--color <MODE>` | Color output: `auto`, `always`, `never` |
| `-h, --help` | Show help |
| `-V, --version` | Show version |

### Config Sources (`-c`)

The `-c` flag specifies where to load configuration from. If not provided, `hats.yml` is loaded by default.

**Config source types:**

| Format | Description |
|--------|-------------|
| `hats.yml` | Local file path |
| `builtin:preset-name` | Embedded preset |
| `https://example.com/config.yml` | Remote URL |
| `core.field=value` | Override a core config field |

Only one config file/preset/URL is used (the first one specified). Overrides can be specified multiple times and layer on top.

**Supported override fields:**

| Field | Description |
|-------|-------------|
| `core.scratchpad` | Path to scratchpad file |
| `core.specs_dir` | Path to specs directory |

**Examples:**

```bash
# Use custom config file
hats run -c production.yml

# Use embedded preset
hats run -c builtin:tdd-red-green

# Override scratchpad (loads hats.yml + applies override)
hats run -c core.scratchpad=.agent/feature-x/scratchpad.md

# Explicit config + override
hats run -c hats.yml -c core.scratchpad=.agent/feature-x/scratchpad.md

# Multiple overrides
hats run -c core.scratchpad=.runs/task-123/scratchpad.md -c core.specs_dir=./my-specs/
```

Overrides are applied after config file loading, so they take precedence.

## Commands

### hats run

Run the orchestration loop.

```bash
hats run [OPTIONS]
```

**Options:**

| Option | Description |
|--------|-------------|
| `-p, --prompt <TEXT>` | Inline prompt text |
| `-P, --prompt-file <FILE>` | Prompt file path |
| `--max-iterations <N>` | Override max iterations |
| `--completion-promise <TEXT>` | Override completion trigger |
| `--dry-run` | Show what would execute |
| `--no-tui` | Disable TUI mode |
| `-a, --autonomous` | Force headless mode |
| `--idle-timeout <SECS>` | TUI idle timeout (default: 30) |
| `--record-session <FILE>` | Record session to JSONL |
| `-q, --quiet` | Suppress output (for CI) |
| `--continue` | Resume from existing state |

**Examples:**

```bash
# Basic run with TUI
hats run

# With inline prompt
hats run -p "Implement user authentication"

# Use custom config
hats run -c production.yml

# Use builtin preset
hats run -c builtin:tdd-red-green

# Override scratchpad for parallel runs
hats run -c hats.yml -c core.scratchpad=.agent/feature-x/scratchpad.md

# Dry run
hats run --dry-run

# CI mode (quiet, no TUI)
hats run -q --no-tui

# Limit iterations
hats run --max-iterations 50

# Record session for debugging
hats run --record-session debug.jsonl
```

### hats init

Initialize configuration file.

```bash
hats init [OPTIONS]
```

**Options:**

| Option | Description |
|--------|-------------|
| `--backend <NAME>` | Backend: `claude`, `kiro`, `gemini`, `codex`, `amp`, `copilot`, `opencode` |
| `--preset <NAME>` | Use preset configuration |
| `--list-presets` | List available presets |
| `--force` | Overwrite existing config |

**Examples:**

```bash
# Traditional mode with Claude
hats init --backend claude

# Use TDD preset
hats init --preset tdd-red-green

# List all presets
hats init --list-presets

# Force overwrite
hats init --preset debug --force
```

### hats plan

Start an interactive PDD planning session.

```bash
hats plan [OPTIONS] [IDEA]
```

**Options:**

| Option | Description |
|--------|-------------|
| `<IDEA>` | Optional rough idea to develop |
| `-b, --backend <BACKEND>` | Backend to use |

**Examples:**

```bash
# Interactive planning
hats plan

# Plan with idea
hats plan "build a REST API"

# Use specific backend
hats plan --backend kiro "my idea"
```

### hats task

Generate code task files.

```bash
hats task [OPTIONS] [INPUT]
```

**Options:**

| Option | Description |
|--------|-------------|
| `<INPUT>` | Description text or path to PDD plan file |
| `-b, --backend <BACKEND>` | Backend to use |

**Examples:**

```bash
# Interactive task creation
hats task

# From description
hats task "add authentication"

# From PDD plan
hats task specs/feature/plan.md
```

### hats events

View event history.

```bash
hats events [OPTIONS]
```

**Examples:**

```bash
# View all events
hats events

# Output:
# 2024-01-21 10:30:00 task.start → planner
# 2024-01-21 10:32:15 plan.ready → builder
# 2024-01-21 10:35:42 build.done → reviewer
```

### hats emit

Emit an event to the event log.

```bash
hats emit <TOPIC> [PAYLOAD] [OPTIONS]
```

**Options:**

| Option | Description |
|--------|-------------|
| `<TOPIC>` | Event topic (e.g., `build.done`) |
| `[PAYLOAD]` | Optional text payload |
| `--json <DATA>` | JSON payload |

**Examples:**

```bash
# Simple event
hats emit "build.done" "tests: pass, lint: pass, typecheck: pass, audit: pass, coverage: pass"

# JSON payload
hats emit "review.done" --json '{"status": "approved", "issues": 0}'
```

### hats clean

Clean up `.agent/` directory.

```bash
hats clean [OPTIONS]
```

**Options:**

| Option | Description |
|--------|-------------|
| `--diagnostics` | Clean diagnostics directory |
| `--all` | Clean everything |

**Examples:**

```bash
# Clean agent state
hats clean

# Clean diagnostics
hats clean --diagnostics
```

### hats tools

Runtime tools for memories and tasks.

#### hats tools memory

Manage persistent memories.

```bash
hats tools memory <SUBCOMMAND>
```

**Subcommands:**

| Command | Description |
|---------|-------------|
| `add <CONTENT>` | Add a new memory |
| `search <QUERY>` | Search memories |
| `list` | List all memories |
| `show <ID>` | Show memory details |
| `delete <ID>` | Delete a memory |
| `prime` | Prime memories for injection |

**Add Options:**

| Option | Description |
|--------|-------------|
| `-t, --type <TYPE>` | Memory type: `pattern`, `decision`, `fix`, `context` |
| `--tags <TAGS>` | Comma-separated tags |

**Search Options:**

| Option | Description |
|--------|-------------|
| `-t, --type <TYPE>` | Filter by type |
| `--tags <TAGS>` | Filter by tags |

**List Options:**

| Option | Description |
|--------|-------------|
| `-t, --type <TYPE>` | Filter by type |
| `--last <N>` | Show last N memories |

**Prime Options:**

| Option | Description |
|--------|-------------|
| `--budget <N>` | Max tokens to inject |
| `--tags <TAGS>` | Filter by tags |
| `--recent <DAYS>` | Only last N days |

**Examples:**

```bash
# Add a pattern memory
hats tools memory add "Uses barrel exports" -t pattern --tags structure

# Search for fixes
hats tools memory search -t fix "database"

# List recent memories
hats tools memory list --last 10

# Show memory details
hats tools memory show mem-1737372000-a1b2

# Delete a memory
hats tools memory delete mem-1737372000-a1b2
```

#### hats tools task

Manage runtime tasks.

```bash
hats tools task <SUBCOMMAND>
```

**Subcommands:**

| Command | Description |
|---------|-------------|
| `add <TITLE>` | Add a new task |
| `list` | List all tasks |
| `ready` | List unblocked tasks |
| `close <ID>` | Close a task |

**Add Options:**

| Option | Description |
|--------|-------------|
| `-p, --priority <N>` | Priority 1-5 (1 = highest) |
| `--blocked-by <ID>` | Task ID this is blocked by |

**Examples:**

```bash
# Add a task
hats tools task add "Implement authentication"

# Add with priority
hats tools task add "Fix critical bug" -p 1

# Add with dependency
hats tools task add "Deploy" --blocked-by setup-infra

# List all tasks
hats tools task list

# List ready tasks
hats tools task ready

# Close a task
hats tools task close task-123
```

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | General error |
| 2 | Configuration error |
| 3 | Backend not found |
| 4 | Interrupted |

## Environment Variables

| Variable | Description |
|----------|-------------|
| `HATS_DIAGNOSTICS` | Set to `1` to enable diagnostics |
| `HATS_CONFIG` | Default config file path |
| `NO_COLOR` | Disable color output |

## Shell Completion

Generate shell completions:

```bash
# Bash
hats completions bash > ~/.local/share/bash-completion/completions/hats

# Zsh
hats completions zsh > ~/.zfunc/_hats

# Fish
hats completions fish > ~/.config/fish/completions/hats.fish
```
