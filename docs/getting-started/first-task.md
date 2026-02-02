# Your First Task

Let's walk through creating and running a complete task with Ralph.

## Choose Your Mode

Ralph offers two modes. Choose based on your task complexity:

| Mode | When to Use |
|------|-------------|
| **Traditional** | Simple tasks, quick automation, getting started |
| **Hat-Based** | Complex workflows, multi-step processes, role separation |

For this guide, we'll use traditional mode first, then show hat-based mode.

## Traditional Mode Example

### 1. Initialize

```bash
mkdir my-first-ralph-task
cd my-first-ralph-task
git init  # Ralph works best with git

ralph init --backend claude
```

### 2. Create Your Prompt

Create `PROMPT.md`:

```markdown
# Task: Build a Simple Calculator (Rust)

Create a Rust calculator module with:

## Requirements
- Functions: add, subtract, multiply, divide
- Handle division by zero gracefully
- Include unit tests

## Acceptance Criteria
- All functions work correctly
- Tests pass with `cargo test`
- Code is formatted with `cargo fmt`
```

### 3. Run Ralph

```bash
ralph run
```

Ralph will:

1. Read your prompt
2. Start the AI agent
3. Iterate until `LOOP_COMPLETE` is output
4. Show progress in the TUI

### 4. Review Results

When Ralph completes, check your directory:

```bash
ls -la
# src/lib.rs
# tests/calculator.rs
# etc.

# Run the tests
cargo test
```

## Hat-Based Mode Example

For more complex tasks, use hats to separate concerns.

### 1. Initialize with a Preset

```bash
ralph init --preset tdd-red-green
```

This creates a configuration with specialized hats:

- **Tester** - Writes failing tests first
- **Implementer** - Makes tests pass
- **Refactorer** - Cleans up the code

### 2. Create Your Prompt

```markdown
# Task: Build a URL Shortener

Create a URL shortening service with:

## Requirements
- Generate short codes for URLs
- Retrieve original URLs from short codes
- Handle invalid inputs gracefully
- Persist mappings to SQLite

## Constraints
- Short codes: 6 alphanumeric characters
- No duplicate short codes
```

### 3. Run with Hat Coordination

```bash
ralph run
```

The TUI shows which hat is active:

```
[iter 3] 00:02:15 Tester
```

### 4. View Event History

```bash
ralph events
```

Shows the event flow between hats:

```
task.start -> Tester
test.written -> Implementer
test.passed -> Refactorer
refactor.done -> Tester
...
```

## Tips for Good Prompts

### Be Specific

```markdown
# Bad
Make a web app.

# Good
Create an Axum web app with:
- GET /health endpoint returning {"status": "ok"}
- POST /users accepting JSON {name, email}
- SQLite database for persistence
```

### Include Acceptance Criteria

```markdown
## Acceptance Criteria
- [ ] All endpoints respond correctly
- [ ] Invalid JSON returns 400 error
- [ ] Database persists across restarts
```

### Specify Constraints

```markdown
## Constraints
- Use Axum (not Actix)
- Rust 1.75+
- No external API calls
```

## Monitoring and Control

### View Progress

The TUI shows real-time progress. Key information:

- **Iteration count** - How many cycles Ralph has run
- **Elapsed time** - Total runtime
- **Active hat** - Which persona is working (hat-based mode)
- **Agent output** - What the AI is doing

### Stop Early

Press `q` in the TUI to quit gracefully.

### Resume Interrupted Sessions

```bash
ralph run --continue
```

### Check Metrics

After completion, check `.agent/` for:

- `scratchpad.md` - Shared memory (legacy mode)
- `memories.md` - Persistent learning
- `tasks.jsonl` - Task tracking

## Common Issues

### Task Not Completing

If Ralph runs forever:

1. Check your prompt has clear completion criteria
2. Ensure `LOOP_COMPLETE` can be reasonably output
3. Set a lower `--max-iterations` for testing

### Wrong Backend

```bash
# Explicitly specify backend
ralph run --backend kiro
```

### Agent Errors

Check the agent is installed and authenticated:

```bash
# Test Claude directly
claude -p "Hello"

# Test Kiro
kiro -p "Hello"
```

## Next Steps

- Learn about [Hats & Events](../concepts/hats-and-events.md)
- Explore [Presets](../guide/presets.md) for your workflow
- Master [Writing Prompts](../guide/prompts.md)
