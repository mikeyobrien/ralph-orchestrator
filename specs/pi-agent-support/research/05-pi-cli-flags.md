# Research: Pi CLI Flags and Permission Model

## Summary

Pi has a simpler permission model than Claude CLI. In print mode (`-p`), all tools are auto-approved. The key flags for Ralph integration are `-p`, `--mode json`, `--no-session`, and the optional `--provider`/`--model`/`--thinking` flags.

## Pi CLI Flags (Ralph-relevant)

### Execution Mode
| Flag | Purpose | Ralph usage |
|------|---------|-------------|
| `-p, --print` | Non-interactive headless mode | **Required** for `pi()` headless backend |
| `--mode json` | NDJSON event stream output | **Required** for structured streaming |
| `--mode text` | Plain text output (default) | Fallback if JSON parsing fails |
| `--no-session` | Disable session persistence | **Required** — Ralph manages its own state |

### Model Configuration
| Flag | Purpose | Ralph usage |
|------|---------|-------------|
| `--provider <name>` | LLM provider | Optional, configurable per hat |
| `--model <id>` | Model ID | Optional, configurable per hat |
| `--thinking <level>` | Reasoning level (off/minimal/low/medium/high/xhigh) | Optional, configurable per hat |

### Tool Control
| Flag | Purpose | Ralph usage |
|------|---------|-------------|
| `--tools <list>` | Restrict available tools (read,bash,edit,write,grep,find,ls) | Optional, for restricted hats |
| `--no-tools` | Disable all tools | Unlikely to use |

### Extension/Skill Control
| Flag | Purpose | Ralph usage |
|------|---------|-------------|
| `-e, --extension <path>` | Load specific extension | Optional, advanced config |
| `--no-extensions` | Disable extension discovery | Optional, for clean runs |
| `--skill <path>` | Load specific skill | Optional, advanced config |
| `--no-skills` | Disable skill discovery | Optional, for clean runs |
| `--no-prompt-templates` | Disable prompt template discovery | Optional |

### Context
| Flag | Purpose | Ralph usage |
|------|---------|-------------|
| `--system-prompt <text>` | Override system prompt | Could be used by hat system |
| `--append-system-prompt <text>` | Append to system prompt | Better for hat-specific additions |
| `-c, --continue` | Continue previous session | Not for Ralph (uses --no-session) |

## Permission Model

Pi does NOT have a `--dangerously-skip-permissions` flag because:
- In print mode (`-p`), all tools are auto-approved by default
- There's no interactive approval flow in headless mode
- Extensions can add approval gates, but these are opt-in

This means Ralph's pi backend needs fewer flags than the Claude backend.

## Headless Pi Backend Command

Minimal:
```bash
pi -p --mode json --no-session "prompt text"
```

Full:
```bash
pi -p --mode json --no-session \
  --provider anthropic --model claude-sonnet-4 \
  --thinking medium \
  --no-extensions --no-skills \
  "prompt text"
```

## Interactive Pi Backend Command

For `ralph plan` (interactive mode):
```bash
pi --no-session "initial prompt text"
```

No `-p` flag, no `--mode json` — runs pi's TUI with the initial prompt.

## Large Prompt Handling

Claude CLI has a 7000-char prompt limit that Ralph works around with temp files. Pi doesn't appear to have this limitation (prompts are passed via args or stdin). However, the OS `ARG_MAX` limit still applies. For very large prompts, Ralph should use the same temp file strategy:

```bash
pi -p --mode json --no-session "Please read and execute the task in /tmp/ralph-prompt-xxx"
```

## Pi Detection

Binary name: `pi`

Detection command: `pi --version` — outputs version string and exits 0.

**Potential conflict**: `pi` could collide with other binaries (e.g., Raspberry Pi tools). The auto-detection should verify the output contains something pi-specific (e.g., check for `@mariozechner/pi-coding-agent` in the version output).

## DisallowedTools Equivalent

Claude backend uses `--disallowedTools=TodoWrite,TaskCreate,...` to prevent the agent from using Claude's built-in task management (which conflicts with Ralph's task system). 

Pi doesn't have TodoWrite/TaskCreate tools. Its built-in tools are: read, bash, edit, write, grep, find, ls. No equivalent restriction is needed.

However, if pi has extensions loaded that register conflicting tools, `--no-extensions` could be used to disable them.
