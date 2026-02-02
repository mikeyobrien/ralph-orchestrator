# CLI Reference

Complete reference for Ralph's command-line interface.

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

The `-c` flag specifies where to load configuration from. If not provided, `ralph.yml` is loaded by default.

**Config source types:**

| Format | Description |
|--------|-------------|
| `ralph.yml` | Local file path |
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
ralph run -c production.yml

# Use embedded preset
ralph run -c builtin:tdd-red-green

# Override scratchpad (loads ralph.yml + applies override)
ralph run -c core.scratchpad=.agent/feature-x/scratchpad.md

# Explicit config + override
ralph run -c ralph.yml -c core.scratchpad=.agent/feature-x/scratchpad.md

# Multiple overrides
ralph run -c core.scratchpad=.runs/task-123/scratchpad.md -c core.specs_dir=./my-specs/
```

Overrides are applied after config file loading, so they take precedence.

## Commands

### ralph run

Run the orchestration loop.

```bash
ralph run [OPTIONS]
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
ralph run

# With inline prompt
ralph run -p "Implement user authentication"

# Use custom config
ralph run -c production.yml

# Use builtin preset
ralph run -c builtin:tdd-red-green

# Override scratchpad for parallel runs
ralph run -c ralph.yml -c core.scratchpad=.agent/feature-x/scratchpad.md

# Dry run
ralph run --dry-run

# CI mode (quiet, no TUI)
ralph run -q --no-tui

# Limit iterations
ralph run --max-iterations 50

# Record session for debugging
ralph run --record-session debug.jsonl
```

### ralph init

Initialize configuration file.

```bash
ralph init [OPTIONS]
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
ralph init --backend claude

# Use TDD preset
ralph init --preset tdd-red-green

# List all presets
ralph init --list-presets

# Force overwrite
ralph init --preset debug --force
```

### ralph plan

Start an interactive PDD planning session.

```bash
ralph plan [OPTIONS] [IDEA]
```

**Options:**

| Option | Description |
|--------|-------------|
| `<IDEA>` | Optional rough idea to develop |
| `-b, --backend <BACKEND>` | Backend to use |

**Examples:**

```bash
# Interactive planning
ralph plan

# Plan with idea
ralph plan "build a REST API"

# Use specific backend
ralph plan --backend kiro "my idea"
```

### ralph task

Generate code task files.

```bash
ralph task [OPTIONS] [INPUT]
```

**Options:**

| Option | Description |
|--------|-------------|
| `<INPUT>` | Description text or path to PDD plan file |
| `-b, --backend <BACKEND>` | Backend to use |

**Examples:**

```bash
# Interactive task creation
ralph task

# From description
ralph task "add authentication"

# From PDD plan
ralph task specs/feature/plan.md
```

### ralph events

View event history.

```bash
ralph events [OPTIONS]
```

**Examples:**

```bash
# View all events
ralph events

# Output:
# 2024-01-21 10:30:00 task.start → planner
# 2024-01-21 10:32:15 plan.ready → builder
# 2024-01-21 10:35:42 build.done → reviewer
```

### ralph emit

Emit an event to the event log.

```bash
ralph emit <TOPIC> [PAYLOAD] [OPTIONS]
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
ralph emit "build.done" "tests: pass, lint: pass, typecheck: pass, audit: pass, coverage: pass"

# JSON payload
ralph emit "review.done" --json '{"status": "approved", "issues": 0}'
```

### ralph clean

Clean up `.agent/` directory.

```bash
ralph clean [OPTIONS]
```

**Options:**

| Option | Description |
|--------|-------------|
| `--diagnostics` | Clean diagnostics directory |
| `--all` | Clean everything |

**Examples:**

```bash
# Clean agent state
ralph clean

# Clean diagnostics
ralph clean --diagnostics
```

### ralph tools

Runtime tools for memories and tasks.

#### ralph tools memory

Manage persistent memories.

```bash
ralph tools memory <SUBCOMMAND>
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
ralph tools memory add "Uses barrel exports" -t pattern --tags structure

# Search for fixes
ralph tools memory search -t fix "database"

# List recent memories
ralph tools memory list --last 10

# Show memory details
ralph tools memory show mem-1737372000-a1b2

# Delete a memory
ralph tools memory delete mem-1737372000-a1b2
```

#### ralph tools task

Manage runtime tasks.

```bash
ralph tools task <SUBCOMMAND>
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
ralph tools task add "Implement authentication"

# Add with priority
ralph tools task add "Fix critical bug" -p 1

# Add with dependency
ralph tools task add "Deploy" --blocked-by setup-infra

# List all tasks
ralph tools task list

# List ready tasks
ralph tools task ready

# Close a task
ralph tools task close task-123
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
| `RALPH_DIAGNOSTICS` | Set to `1` to enable diagnostics |
| `RALPH_CONFIG` | Default config file path |
| `NO_COLOR` | Disable color output |

## Shell Completion

Generate shell completions:

```bash
# Bash
ralph completions bash > ~/.local/share/bash-completion/completions/ralph

# Zsh
ralph completions zsh > ~/.zfunc/_ralph

# Fish
ralph completions fish > ~/.config/fish/completions/ralph.fish
```
