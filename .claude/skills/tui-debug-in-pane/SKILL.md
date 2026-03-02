---
name: tui-debug-in-pane
description: Use when you need to reproduce or debug TUI rendering issues (garbled output, broken streaming, layout corruption) by running ralph in a tmux split pane and capturing live output.
---

# tui-debug-in-pane

Debug TUI rendering bugs by launching ralph in a tmux split pane and capturing live output. The split keeps your main pane free for inspection commands.

## When to Use

- Reproducing garbled or corrupted TUI output from a specific provider
- Investigating streaming rendering issues that only appear live
- Capturing TUI state for bug reports or diagnostics

## Quick Reference

| Task | Command |
|------|---------|
| Identify current pane | `tmux display-message -p '#{session_name}:#{window_index}.#{pane_index}'` |
| Create split pane | `tmux split-window -h -t SESSION:WINDOW -c /path/to/repo` |
| List panes | `tmux list-panes -t SESSION:WINDOW` |
| Fix shell in pane | `tmux send-keys -t PANE "exec /path/to/fish" Enter` |
| Send command | `tmux send-keys -t PANE "command" Enter` |
| Capture output | `tmux capture-pane -t PANE -p -S -60` |
| Quit TUI | `tmux send-keys -t PANE q` |
| Force stop | `tmux send-keys -t PANE C-c` |
| Kill split pane | `tmux kill-pane -t PANE` |

## Procedure

### 1. Create the Split Pane

```bash
tmux display-message -p '#{session_name}:#{window_index}.#{pane_index}'
tmux split-window -h -t SESSION:WINDOW -c /path/to/repo
tmux list-panes -t SESSION:WINDOW
```

### 2. Fix Shell Environment

Split panes inherit the tmux server's default shell (bash), not the parent's. Tools like `pi` need fish (mise/Node).

```bash
tmux send-keys -t PANE "exec /nix/store/km9w9r1p4nl92y5fp4vfwsjymjig4axl-fish-3.7.1/bin/fish" Enter
tmux send-keys -t PANE "which pi && pi --version" Enter
```

### 3. Clean Up Before Running

```bash
rm -f .ralph/loop.lock
git worktree prune
```

### 4. Launch Ralph

```bash
# Prefer release binary (faster startup)
tmux send-keys -t PANE "target/release/ralph run -c CONFIG.yml -p 'prompt' --max-iterations N" Enter

# Or with cargo (must specify --bin for this workspace)
tmux send-keys -t PANE "cargo run --bin ralph -- run -c CONFIG.yml -p 'prompt' --max-iterations N" Enter
```

### 5. Capture and Analyze

```bash
sleep 20  # Pi/Kiro take 15-30s to start streaming
tmux capture-pane -t PANE -p -S -60
ls -lt .ralph/diagnostics/logs/ | head -5
```

### 6. Clean Up

```bash
tmux send-keys -t PANE q
tmux send-keys -t PANE C-c
tmux kill-pane -t PANE
```

## Common Mistakes

- **Shell mismatch**: Split panes get bash by default. If tools fail with "command not found", switch to fish with `exec /path/to/fish`.
- **Stale loop lock**: If `.ralph/loop.lock` exists, ralph spawns worktree loops instead of running normally. Always delete it first.
- **Wrong backend in TUI header**: Without `cli.backend: pi` in the config, ralph uses claude regardless of hat settings.
- **Missing hat `name` field**: HatConfig requires `name`; omitting it causes a config parse error.
- **Premature capture**: Pi and Kiro take 15-30s before streaming text appears. Capture too early and you see an empty content area.
- **Ghost keystrokes**: If the TUI already exited, pressing `q` prepends it to your next command. Check if the TUI is still running before sending quit keys.
