# Quick Start

Run your first Hats orchestration in about 10 minutes.

## 1. Install Hats

If you haven't installed Hats yet, follow the full [Installation](installation.md) guide.

Quick install (npm):

```bash
npm install -g @hats/hats-cli
```

## 2. Install a Backend CLI (Claude Recommended)

Hats needs at least one AI CLI tool available on your PATH.

```bash
# Claude Code
npm install -g @anthropic-ai/claude-code

# Verify the CLI is available
claude --version
```

If the backend requires authentication, complete its login flow per the provider's instructions.

## 3. Verify Setup with `hats doctor`

Run the doctor command to validate your environment:

```bash
hats doctor
```

Fix any **WARN** or **FAIL** items before continuing. If you see auth warnings, verify your backend CLI is logged in.

## 4. Initialize a Project

```bash
mkdir my-hats-project
cd my-hats-project
git init  # Hats works best with git

# Create a default config
hats init --backend claude
```

This creates `hats.yml` in your project.

## 5. Create a Minimal Hat Collection

Hats can run with hats (role-based personas) for more structured workflows. Create a minimal hat collection file:

```yaml
# hats.yml
event_loop:
  starting_event: "task.start"

hats:
  builder:
    name: "Builder"
    triggers: ["task.start"]
    publishes: ["task.done"]
    instructions: |
      Implement the task from PROMPT.md.
      Run any relevant tests.
      When finished, emit task.done and print LOOP_COMPLETE.
```

## 6. Define Your Task

Create a `PROMPT.md` file with your task:

```markdown
# Task: Create a Todo List CLI (Rust)

Build a Rust command-line todo list with:
- Add tasks
- List tasks
- Mark tasks complete
- Save to a JSON file

Include error handling and unit tests.
```

## 7. Run Hats

```bash
# Traditional mode (uses hats.yml)
hats run

# Hat-based mode (uses hats.yml)
hats run --config hats.yml

# Inline prompt example
hats run -p "Add input validation to the user API endpoints"
```

## 8. Understand the Output

While running, Hats shows a TUI with:

- Current iteration number
- Elapsed time
- Active hat (if hat-based)
- Recent agent output

Hats stops when one of these occurs:

- `LOOP_COMPLETE` is output (success)
- Maximum iterations reached (default: 100)
- Maximum runtime exceeded (default: 4 hours)
- You quit the TUI

When it finishes, review the generated files in your project directory and `.agent/` run logs.

## Command-Line Options

```bash
# Limit iterations
hats run --max-iterations 50

# Use different config file
hats run -c custom-hats.yml

# Resume interrupted session
hats run --continue

# Quiet mode for CI
hats run -q
```

## Example Tasks

### Simple Function

```markdown
Write a TypeScript function that validates email addresses.
Include unit tests.
```

### Web Scraper

```markdown
Create a web scraper that:
1. Fetches the Hacker News homepage
2. Extracts the top 10 stories
3. Saves them to JSON

Use Node.js with a simple HTML parser.
```

### CLI Tool

```markdown
Build a markdown to HTML converter:
- Accept input/output file arguments
- Support basic markdown syntax
- Add --watch mode
```

## Next Steps

- Read [Your First Task](first-task.md) for a detailed walkthrough
- Understand [Concepts](../concepts/index.md) like hats and events
- Explore [Presets](../guide/presets.md) for common workflows
- Learn about [Configuration](../guide/configuration.md) options
