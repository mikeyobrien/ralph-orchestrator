# CLI Reference

Complete reference for Ralph's command-line interface.

## Global Options

These options are accepted by all commands.

| Option | Description |
|--------|-------------|
| `-c, --config <SOURCE>` | Primary config source (can be specified multiple times). Defaults to `ralph.yml`, or `$RALPH_CONFIG` when set. |
| `-H, --hats <SOURCE>` | Hat collection source (`file`, `builtin:<name>`, or URL). |
| `-v, --verbose` | Verbose output |
| `--color <MODE>` | Color output: `auto`, `always`, `never` |
| `-h, --help` | Show help |
| `-V, --version` | Show version |

### Core Config Sources (`-c`)

The `-c` flag specifies where to load **core** configuration from. If not provided, `ralph` falls back to:

1. `$RALPH_CONFIG` when present
2. `ralph.yml`

**Core source types:**

| Format | Description |
|--------|-------------|
| `ralph.yml` | Local file path |
| `https://example.com/ralph.core.yml` | Remote URL |
| `core.field=value` | Core config override |

> `-c builtin:<name>` is no longer supported. Use `-H builtin:<name>` for hat collections.

The first non-override core source is used as the base config. Later core overrides replace earlier values.

Backward compatibility: a `-c` config file may still contain `hats`/`events` (single-file combined config).

If `-H/--hats` is provided, it takes precedence over hats in `-c`:
- `hats` and `events` from `-H` replace `hats`/`events` from `-c`
- `event_loop` values from `-H` override matching `event_loop` keys from `-c`
- `-c core.*=...` overrides are still applied last

**Supported override fields:**

| Field | Description |
|-------|-------------|
| `core.scratchpad` | Path to scratchpad file |
| `core.specs_dir` | Path to specs directory |

### Hat Collection Sources (`-H`)

The `-H` flag specifies where to load hat collections from.

| Format | Description |
|--------|-------------|
| `hats/feature.yml` | Local hats file |
| `builtin:feature` | Built-in hat collection |
| `https://example.com/hats.yml` | Remote hats file |

**Examples:**

```bash
# Core only (hatless)
ralph run -c ralph.yml

# Core + built-in hat collection
ralph run -c ralph.yml -H builtin:feature

# Core + file hat collection
ralph run -c ralph.yml -H hats/review.yml

# Core override + hats
ralph run -c ralph.yml -c core.specs_dir=./my-specs -H builtin:debug
```

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
| `--idle-timeout <SECS>` | TUI idle timeout |
| `--exclusive` | Wait for primary loop slot |
| `--no-auto-merge` | Skip automatic merge after worktree loops complete |
| `--skip-preflight` | Skip preflight checks |
| `--record-session <FILE>` | Record session JSONL |
| `-q, --quiet` | Suppress streaming output |
| `--continue` | Resume from existing state |

### ralph init

Initialize `ralph.yml`.

```bash
ralph init [OPTIONS]
```

**Options:**

| Option | Description |
|--------|-------------|
| `--backend <NAME>` | Backend: `claude`, `kiro`, `gemini`, `codex`, `amp`, `copilot`, `opencode`, `pi`, `custom` |
| `--preset <NAME>` | Removed (monolithic presets no longer supported) |
| `--list-presets` | List available built-in hat collections |
| `--force` | Overwrite existing config |

### ralph preflight

Run the preflight check suite.

```bash
ralph preflight [OPTIONS]
```

**Options:**

| Option | Description |
|--------|-------------|
| `--format <human|json>` | Output format |
| `--strict` | Treat warnings as failures |
| `--check <NAME>` | Run one or more checks |

### ralph doctor

Run environment and first-run diagnostic checks.

```bash
ralph doctor [OPTIONS]
```

### ralph tutorial

Run interactive intro walkthrough.

```bash
ralph tutorial [OPTIONS]
```

### ralph plan

Start an interactive PDD planning session.

```bash
ralph plan [OPTIONS] [IDEA]
```

**Options:**

| Option | Description |
|--------|-------------|
| `<IDEA>` | Optional rough idea |
| `-b, --backend <BACKEND>` | Backend override |
| `--teams` | Enable Claude Code agent teams mode |
| `-- <ARGUMENTS>` | Custom backend arguments |

### ralph code-task

Generate code task files from a description or PDD plan.

```bash
ralph code-task [OPTIONS] [INPUT]
```

### ralph task

Deprecated legacy alias for `ralph code-task`.

```bash
ralph task [OPTIONS] [INPUT]
```

### ralph events

View event history for the current or selected run.

```bash
ralph events [OPTIONS]
```

**Options:**

| Option | Description |
|--------|-------------|
| `--file <PATH>` | Use a specific events file |
| `--clear` | Clear event history |

### ralph emit

Emit an event to the current run's events file.

```bash
ralph emit <TOPIC> [PAYLOAD] [OPTIONS]
```

**Options:**

| Option | Description |
|--------|-------------|
| `<TOPIC>` | Event topic (e.g., `build.done`) |
| `[PAYLOAD]` | Optional payload (string or JSON when `--json` is set) |
| `-j, --json` | Parse payload as JSON object |
| `--ts <TIMESTAMP>` | Override event timestamp |
| `--file <PATH>` | Events file path (`.ralph/events.jsonl`) |

### ralph clean

Clean `.ralph/agent` scratchpad and memory state.

```bash
ralph clean [OPTIONS]
```

**Options:**

| Option | Description |
|--------|-------------|
| `--diagnostics` | Clean diagnostics directory |
| `--dry-run` | Preview deletions |

### ralph loops

Manage parallel loops and worktree loop lifecycle.

```bash
ralph loops [OPTIONS] [COMMAND]
```

**Subcommands:**

- `list [--json] [--all]`
- `logs <loop-id> [--follow]`
- `history <loop-id> [--json]`
- `retry <loop-id>`
- `discard <loop-id> [--yes]`
- `stop [loop-id] [--force]`
- `prune`
- `attach <loop-id>`
- `diff <loop-id> [--stat]`
- `merge <loop-id> [--force]`
- `process`
- `merge-button-state <loop-id>`

### ralph hats

Manage and inspect configured hats.

```bash
ralph hats [OPTIONS] [COMMAND]
```

**Subcommands:**

- `list [--format table|json]`
- `show <name>`
- `validate`
- `graph [--format unicode|ascii|compact|mermaid] [--backend <backend>]`

### ralph web

Run the web dashboard.

```bash
ralph web [OPTIONS]
```

**Options:**

| Option | Description |
|--------|-------------|
| `--backend-port <BACKEND_PORT>` | RPC API port (default: 3000) |
| `--frontend-port <FRONTEND_PORT>` | Frontend port (default: 5173) |
| `--workspace <WORKSPACE>` | Workspace root |
| `--legacy-node-api` | Run deprecated Node tRPC backend instead of Rust RPC API |
| `--no-open` | Do not open browser |

### ralph bot

Manage Telegram bot setup and testing.

```bash
ralph bot [OPTIONS] <COMMAND>
```

**Subcommands:**

- `onboard [--token <TOKEN>] [--chat-id <CHAT_ID>] [--timeout <SECONDS>]`
- `status`
- `test [MESSAGE]`
- `token set <TOKEN> [--config <path>]`
- `daemon`

### ralph tools

Runtime tools for memories, tasks, and skills.

#### ralph tools memory

```bash
ralph tools memory <SUBCOMMAND>
```

**Subcommands:**

| Command | Description |
|---------|-------------|
| `init` | Initialize memory file |
| `add <CONTENT>` | Store a new memory |
| `search <QUERY>` | Search memories |
| `list` | List memories |
| `show <ID>` | Show a memory |
| `delete <ID>` | Delete a memory |
| `prime` | Prime context memory output |

#### ralph tools task

```bash
ralph tools task <SUBCOMMAND>
```

**Subcommands:**

| Command | Description |
|---------|-------------|
| `add <TITLE>` | Create a task |
| `list` | List all tasks |
| `ready` | List unblocked tasks |
| `close <ID>` | Mark task complete |
| `fail <ID>` | Mark task failed |
| `show <ID>` | Show task details |

#### ralph tools skill

```bash
ralph tools skill <SUBCOMMAND>
```

#### ralph tools interact

Interact with human via Telegram progress/proactiveness hooks.

### ralph completions

Generate shell completions.

```bash
ralph completions <SHELL>
```

Supported shells: `bash`, `elvish`, `fish`, `powershell`, `zsh`.

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Completion promise reached (`LOOP_COMPLETE`) |
| 1 | Failure or stop condition (failure/cancelled/throttled state) |
| 2 | Runtime limits reached (`max-iterations`, `max-runtime`, or `max-cost`) |
| 3 | Loop requested restart |
| 130 | Interrupted by signal (Ctrl-C / SIGINT) |

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
