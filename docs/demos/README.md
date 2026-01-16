# PR Demo Recordings

This directory contains animated GIF demos for pull requests and documentation.

## Recording Demos

Use the `/pr-demo` skill to create new demos. Quick reference:

```bash
# 1. Script your demo (20-30 seconds, show ONE thing)
# 2. Record
asciinema rec demo.cast --cols 100 --rows 24

# 3. Convert
agg demo.cast demo.gif

# 4. Validate
ls -lh demo.gif  # Should be < 5MB
```

## Current Demos

| Demo | Feature | Duration |
|------|---------|----------|
| `ralph-plan.gif` | `ralph plan` command | ~20s |
| `ralph-task.gif` | `ralph task` command | ~20s |

## Demo Scripts

### ralph plan

```markdown
## Demo: ralph plan
Duration: ~20 seconds

1. [0-3s] Type: ralph plan "Build a CLI tool for managing dotfiles"
2. [3-8s] Show backend detection and session starting
3. [8-18s] Show PDD SOP guiding the planning conversation
4. [18-20s] Exit cleanly (Ctrl+C)
```

### ralph task

```markdown
## Demo: ralph task
Duration: ~20 seconds

1. [0-3s] Type: ralph task "Add user authentication to the API"
2. [3-8s] Show backend detection and session starting
3. [8-18s] Show code-task-generator SOP creating structured task
4. [18-20s] Exit cleanly (Ctrl+C)
```

## Environment Setup

Before recording:

```bash
clear
export PS1='$ '
export TERM=xterm-256color
# Terminal size: 100x24
```
