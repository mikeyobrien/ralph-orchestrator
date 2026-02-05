# User Guide

Practical guides for using Hats effectively.

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

- Initialize a project: `hats init --backend claude`
- Run with a preset: `hats init --preset tdd-red-green`
- List presets: `hats init --list-presets`

### Running Hats

- Basic run: `hats run`
- With inline prompt: `hats run -p "Implement feature X"`
- Headless mode: `hats run --no-tui`
- Resume session: `hats run --continue`

### Monitoring

- View event history: `hats events`
- Check memories: `hats tools memory list`
- Check tasks: `hats tools task list`

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
hats init --preset feature
# Edit PROMPT.md with your feature spec
hats run
```

### Debug an Issue

```bash
hats init --preset debug
hats run -p "Investigate why user authentication fails on mobile"
```

### Run TDD Workflow

```bash
hats init --preset tdd-red-green
hats run -p "Add email validation to user registration"
```

### Review Code

```bash
hats init --preset review
hats run -p "Review the changes in src/api/"
```

## Next Steps

Start with [Configuration](configuration.md) to understand all options.
