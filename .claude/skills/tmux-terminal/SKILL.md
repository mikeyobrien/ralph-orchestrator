---
name: tmux-terminal
description: Interactive terminal control via tmux for TUI apps, prompts, and long-running CLI workflows.
hats: [developer, qa_tester]
---

# tmux-terminal

## Overview

Use tmux to drive interactive terminal sessions, including TUI workflows like `ralph-tui`. tmux lets you send keystrokes, capture screen output, and keep processes running between steps.

## When to Use

- Testing `ralph-tui` or any interactive CLI prompts
- Managing long-running processes (web server, loops, watch mode)
- Capturing live terminal output for QA reports
- Interacting with applications that redraw the screen

## Prerequisites

- `tmux` installed (pre-installed on macOS)

Verify:
```bash
tmux -V
```

## Core Commands

Create a detached session:
```bash
tmux new-session -d -s <name>
```

Send commands (append Enter to execute):
```bash
tmux send-keys -t <name> "<command>" Enter
```

Capture screen output:
```bash
tmux capture-pane -t <name> -p
```

Kill session when done:
```bash
tmux kill-session -t <name>
```

## Special Keys

Use `send-keys` with key names:
- `Enter`
- `C-c` (Ctrl-C)
- `C-d` (Ctrl-D)
- `Tab`
- `Escape`
- `Up`, `Down`, `Left`, `Right`

Examples:
```bash
tmux send-keys -t <name> Up
tmux send-keys -t <name> C-c
```

## TUI Interaction Patterns

### Start ralph-tui
```bash
tmux new-session -d -s ralph-tui
tmux send-keys -t ralph-tui "cargo run -p ralph-tui" Enter
```

### Navigate in TUI
```bash
tmux send-keys -t ralph-tui Down
tmux send-keys -t ralph-tui Enter
```

### Capture and parse the screen
```bash
tmux capture-pane -t ralph-tui -p -S -200
```

Use `-S -200` to capture the last 200 lines when the screen is noisy.

## Long-Running Process Management

- Start servers or loops in a tmux session to keep them alive.
- Use `capture-pane` to confirm health (look for "listening" or "ready" text).
- Stop cleanly with `C-c` then `kill-session`.

Example:
```bash
tmux new-session -d -s ralph-web
tmux send-keys -t ralph-web "cargo run -p ralph-cli -- web" Enter
tmux capture-pane -t ralph-web -p | rg -n "listening|ready"
tmux send-keys -t ralph-web C-c
tmux kill-session -t ralph-web
```

## Notes

- Keep session names short and unique.
- Always clean up sessions to avoid leaking background processes.
- If output looks empty, wait briefly and capture again.
