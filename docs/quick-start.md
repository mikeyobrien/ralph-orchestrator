# Quick Start Guide

Get Ralph Orchestrator up and running in 5 minutes!

## Prerequisites

Before you begin, ensure you have:

- Git (for version control)
- At least one AI CLI tool installed

## Step 1: Install an AI Agent

Ralph works with multiple AI agents. Install at least one:

=== "Claude (Recommended)"

    ```bash
    npm install -g @anthropic-ai/claude-code
    ```

=== "Kiro"

    ```bash
    npm install -g @anthropic-ai/kiro
    ```

=== "Gemini"

    ```bash
    npm install -g @google/gemini-cli
    ```

## Step 2: Install Ralph

```bash
# Clone the repository
git clone https://github.com/mikeyobrien/ralph-orchestrator.git
cd ralph-orchestrator

# Build from source
cargo build --release

# Add to your PATH
export PATH="$PWD/target/release:$PATH"
```

Or download a pre-built binary from the [releases page](https://github.com/mikeyobrien/ralph-orchestrator/releases).

## Step 3: Create Your First Task

Create a `PROMPT.md` file with your task:

```markdown
# Task: Create a Todo List CLI

Build a Python command-line todo list application with:

- Add tasks
- List tasks
- Mark tasks as complete
- Save tasks to a JSON file

Include proper error handling and a help command.

The orchestrator will continue iterations until all requirements are met or limits reached.
```

## Step 4: Run Ralph

```bash
# Basic execution (uses Claude by default)
ralph run

# Or specify a different agent
ralph run -b gemini

# Or use an inline prompt
ralph run -p "Create a hello world program in Python"
```

## Step 5: Monitor Progress

Ralph will now:

1. Read your prompt file
2. Execute the AI agent
3. Check for completion
4. Iterate until done or limits reached

You'll see output like:

```
[iter 1] Starting iteration...
[iter 1] Agent executing...
[iter 1] Complete (45s)
[iter 2] Starting iteration...
```

## What Happens Next?

Ralph will continue iterating until one of these conditions is met:

- All requirements appear to be satisfied
- Maximum iterations reached (default: 100)
- Maximum runtime exceeded (default: 4 hours)
- Cost limits reached
- Unrecoverable error occurs
- Completion marker detected in output
- Loop detection triggers (repetitive outputs)

## Signaling Completion

Add a completion marker to your output when the task is done:

```markdown
LOOP_COMPLETE
```

Ralph will detect this marker and stop orchestration. You can customize the marker:

```yaml
# ralph.yml
event_loop:
  completion_promise: "TASK_DONE"
```

## Basic Configuration

Create a `ralph.yml` file to control behavior:

```yaml
cli:
  backend: claude

event_loop:
  max_iterations: 50
  max_runtime_seconds: 3600
  max_cost_usd: 10.0
```

Or use command-line flags:

```bash
# Limit iterations
ralph run -n 50

# Dry run (test without executing)
ralph run --dry-run

# Quiet mode
ralph run -q
```

## Example Tasks

### Simple Function

```markdown
Write a Python function that validates email addresses using regex.
Include comprehensive unit tests.
```

### Web Scraper

```markdown
Create a web scraper that:

1. Fetches the HackerNews homepage
2. Extracts the top 10 stories
3. Saves them to a JSON file
   Use requests and BeautifulSoup.
```

### CLI Tool

```markdown
Build a markdown to HTML converter CLI tool:

- Accept input/output file arguments
- Support basic markdown syntax
- Add --watch mode for auto-conversion
```

## Using Presets

Ralph includes pre-configured presets for common workflows:

```bash
# Research mode - exploration without code changes
ralph run -c presets/research.yml -p "How does auth work in this codebase?"

# TDD workflow - test-first development
ralph run -c presets/tdd-red-green.yml
```

See [Presets](../presets/README.md) for the full list.

## Next Steps

Now that you've run your first Ralph task:

- Read the [Configuration Guide](guide/configuration.md) for detailed setup
- Learn about [Presets](../presets/README.md) for specialized workflows
- Understand [Cost Management](guide/cost-management.md)
- Explore [Advanced Architecture](advanced/architecture.md) for hat-based workflows

## Troubleshooting

### Agent Not Found

If Ralph can't find an AI agent:

```
ERROR: No AI agents detected
```

**Solution**: Install one of the supported agents (see Step 1)

### Permission Denied

If you get permission errors:

```bash
chmod +x ralph
```

### Task Not Completing

If your task runs indefinitely:

- Check that your prompt includes clear completion criteria
- Ensure the agent can work towards completion
- Add `LOOP_COMPLETE` marker to signal completion

## Getting Help

- Check the [FAQ](faq.md)
- Read the [Troubleshooting Guide](troubleshooting.md)
- Open an [issue on GitHub](https://github.com/mikeyobrien/ralph-orchestrator/issues)
- Join the [discussions](https://github.com/mikeyobrien/ralph-orchestrator/discussions)
