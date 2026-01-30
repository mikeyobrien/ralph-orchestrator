# User Guide

Practical guides for using Ralph Orchestrator effectively.

## In This Section

| Guide | Description |
|-------|-------------|
| [Configuration](configuration.md) | Full configuration reference |
| [Presets](presets.md) | Pre-configured workflows |
| [CLI Reference](cli-reference.md) | Command-line interface |
| [Backends](backends.md) | Supported AI backends |
| [Writing Prompts](prompts.md) | Prompt engineering tips |
| [Cost Management](cost-management.md) | Controlling API costs |
| [Telegram Integration](telegram.md) | Human-in-the-loop via Telegram |

## Quick Links

### Getting Started

- Initialize a project: `ralph init --backend claude`
- Run with a preset: `ralph init --preset tdd-red-green`
- List presets: `ralph init --list-presets`

### Running Ralph

- Basic run: `ralph run`
- With inline prompt: `ralph run -p "Implement feature X"`
- Headless mode: `ralph run --no-tui`
- Resume session: `ralph run --continue`

### Monitoring

- View event history: `ralph events`
- Check memories: `ralph tools memory list`
- Check tasks: `ralph tools task list`

## Choosing a Workflow

| Your Situation | Recommended Approach |
|----------------|---------------------|
| Simple task | Traditional mode (no hats) |
| Test-first development | `--preset tdd-red-green` |
| Spec-driven work | `--preset spec-driven` |
| Bug investigation | `--preset debug` |
| Code review | `--preset review` |
| Documentation | `--preset docs` |

## Common Tasks

### Start a New Feature

```bash
ralph init --preset feature
# Edit PROMPT.md with your feature spec
ralph run
```

### Debug an Issue

```bash
ralph init --preset debug
ralph run -p "Investigate why user authentication fails on mobile"
```

### Run TDD Workflow

```bash
ralph init --preset tdd-red-green
ralph run -p "Add email validation to user registration"
```

### Review Code

```bash
ralph init --preset review
ralph run -p "Review the changes in src/api/"
```

## Next Steps

Start with [Configuration](configuration.md) to understand all options.
