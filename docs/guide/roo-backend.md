# Using Ralph Orchestrator with Roo Code CLI

## Quick Start

### Prerequisites

1. **Ralph** installed (`cargo build` from this repo)
2. **Roo CLI** installed (`roo --version` should return 0.1.15+)
3. **AWS Bedrock** access configured (or another supported provider)

### Run Your First Loop

```bash
# Simple one-iteration test
ralph run -b roo --max-iterations 1 \
  -- --provider bedrock --aws-profile roo-bedrock --aws-region us-east-1 \
     --model anthropic.claude-sonnet-4-6 --max-tokens 64000 \
  -p "Create a hello.txt file with 'Hello World'"
```

## Configuration

### Option 1: CLI Flags (Quick)

Pass roo-specific flags after `--`:

```bash
ralph run -b roo -- \
  --provider bedrock \
  --aws-profile roo-bedrock \
  --aws-region us-east-1 \
  --model anthropic.claude-sonnet-4-6 \
  --max-tokens 64000
```

### Option 2: Config File (Recommended)

Create a `ralph.roo.yml`:

```yaml
# Ralph + Roo Configuration
event_loop:
  completion_promise: "LOOP_COMPLETE"
  max_iterations: 100
  max_runtime_seconds: 14400      # 4 hours
  max_consecutive_failures: 5

cli:
  backend: "roo"
  prompt_mode: "arg"
  pty_mode: false
  idle_timeout_secs: 30
  args:
    - "--provider"
    - "bedrock"
    - "--aws-profile"
    - "roo-bedrock"
    - "--aws-region"
    - "us-east-1"
    - "--model"
    - "anthropic.claude-sonnet-4-6"
    - "--max-tokens"
    - "100000"
    - "--reasoning-effort"
    - "medium"

core:
  specs_dir: "./specs/"
  guardrails:
    - "Fresh context each iteration - save learnings to memories for next time"
    - "Don't assume 'not implemented' - search first"
    - "Verification is mandatory - tests/typecheck/lint/audit must pass"
    - "Confidence protocol: score decisions 0-100. >80 proceed autonomously; 50-80 proceed + document; <50 choose safe default + document."

hats:
  builder:
    name: "Builder"
    description: "Implements code, creates files, runs tests. Does the actual work."
    triggers: ["build.task"]
    publishes: ["build.done", "build.blocked"]
    instructions: |
      ## WORKFLOW
      You are Builder. Your job is to IMPLEMENT - write code, create files, run tests.
      1. Read the build.task event payload - that's your task
      2. IMPLEMENT: Create files, write code, run commands
      3. VERIFY: Run tests/builds to confirm it works
      4. COMPLETE: Emit build.done when verified, or build.blocked if stuck
      RULES:
      - Do the actual work - don't just plan or delegate
      - Never emit build.task (that's for coordination, not you)
```

Then run:

```bash
ralph run -c ralph.roo.yml -p "Build feature X"
```

### Option 3: PDD-to-Code-Assist with Roo

For the full PDD → Code Assist workflow using Roo with Claude Opus 4.6:

Create `ralph.roo.pdd.yml`:

```yaml
# PDD-to-Code-Assist with Roo Code CLI
# Uses Claude Opus 4.6 via Bedrock with medium reasoning effort

event_loop:
  prompt_file: "PROMPT.md"
  completion_promise: "LOOP_COMPLETE"
  starting_event: "design.start"
  max_iterations: 150
  max_runtime_seconds: 14400
  checkpoint_interval: 5

cli:
  backend: "roo"
  prompt_mode: "arg"
  pty_mode: false
  idle_timeout_secs: 60
  args:
    - "--provider"
    - "bedrock"
    - "--aws-profile"
    - "roo-bedrock"
    - "--aws-region"
    - "us-east-1"
    - "--model"
    - "anthropic.claude-opus-4-6"
    - "--max-tokens"
    - "100000"
    - "--reasoning-effort"
    - "medium"

core:
  specs_dir: "./specs/"
  guardrails:
    - "Fresh context each iteration — save learnings to memories for next time"
    - "Verification is mandatory — tests/typecheck/lint/audit must pass"
    - "YAGNI ruthlessly — no speculative features"
    - "KISS always — simplest solution that works"
    - "Preserve primary sources — all referenced files, research findings, code snippets, and external docs must be captured with source attribution"
    - "Confidence protocol: score decisions 0-100. >80 proceed autonomously; 50-80 proceed + document in .ralph/agent/decisions.md; <50 choose safe default + document."

# Copy hats from presets/pdd-to-code-assist.yml
# (inquisitor, architect, design_critic, explorer, planner, task_writer, builder, validator, committer)
```

Then run:

```bash
ralph run -c ralph.roo.pdd.yml -p "Build a CLI tool for managing tasks"
```

Or use the built-in preset with roo args:

```bash
ralph run -c presets/pdd-to-code-assist.yml \
  -c cli.backend=roo \
  -- --provider bedrock --aws-profile roo-bedrock --aws-region us-east-1 \
     --model anthropic.claude-opus-4-6 --max-tokens 100000 \
     --reasoning-effort medium \
  -p "Build a CLI tool for managing tasks"
```

## Roo-Specific Configuration Options

### Model Selection

Pass model flags via `cli.args` or `--`:

| Flag | Description | Example |
|------|-------------|---------|
| `--provider` | LLM provider | `bedrock`, `anthropic`, `openai`, `openrouter` |
| `--model` | Model identifier | `anthropic.claude-opus-4-6`, `anthropic.claude-sonnet-4-6` |
| `--max-tokens` | Max output tokens | `100000` |
| `--reasoning-effort` | Thinking effort | `medium`, `high`, `xhigh` |
| `--aws-profile` | AWS credentials profile | `roo-bedrock` |
| `--aws-region` | AWS Bedrock region | `us-east-1` |

### Roo Modes

Roo has built-in modes (`code`, `architect`, `ask`, `debug`). By default, it uses `code` mode which has all tools (read, edit, command, mcp). Override with:

```yaml
cli:
  args:
    - "--mode"
    - "architect"  # For planning-focused hats
```

### Interactive Planning

Use `ralph plan` for interactive sessions with Roo's TUI:

```bash
ralph plan -b roo -- --provider bedrock --aws-profile roo-bedrock \
  --aws-region us-east-1 --model anthropic.claude-opus-4-6 \
  --max-tokens 100000 \
  -p "Design the auth system architecture"
```

## How It Works

### Architecture

```
Ralph Loop (each iteration):
1. Ralph builds prompt (context + events + memories + instructions)
2. Writes prompt to temp file
3. Spawns: roo --print --ephemeral --prompt-file /tmp/xxx [user args]
4. Roo reads prompt, executes tools, produces text output
5. Ralph parses output for events (<event topic="...">) and LOOP_COMPLETE
6. Next iteration with updated context
```

### Key Behaviors

| Aspect | Behavior |
|--------|----------|
| **Context** | Fresh each iteration — roo has no memory between iterations |
| **Tool approval** | Auto-approved by default (no flag needed) |
| **Disk state** | `--ephemeral` keeps disk clean between iterations |
| **Prompt passing** | Always via `--prompt-file` (handles any prompt size) |
| **Error detection** | LOOP_COMPLETE presence + consecutive failure counter |
| **Exit codes** | Config errors → exit 1; API errors → infinite retry (idle timeout handles) |

## Troubleshooting

### Bedrock Cross-Region Errors

If you see "Try enabling cross-region inference":
1. Ensure `--aws-region` matches your Bedrock setup
2. Check `--aws-profile` has correct credentials
3. Verify the model is available in your region

### Roo Retries Indefinitely

Roo retries API errors with exponential backoff. Ralph's `idle_timeout_secs` (default 30s) will kill the process. Increase if your model is slow:

```yaml
cli:
  idle_timeout_secs: 60  # or higher for large prompts
```

### CustomModesManager Warning

```
[CustomModesManager] Failed to load modes from .../custom_modes.yaml: ENOENT
```

This is benign — `--ephemeral` mode uses a temp directory where custom modes don't exist. No action needed.
