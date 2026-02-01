# The Ralph Wiggum Technique

The Ralph Wiggum technique is a simple but powerful approach to autonomous AI task completion through continuous iteration.

## Origin

The technique was created by [Geoffrey Huntley](https://ghuntley.com/ralph/) and named after Ralph Wiggum from The Simpsons, embodying the philosophy of persistent iteration: "Me fail English? That's unpossible!" — just keep trying until you succeed.

## The Basic Idea

At its core, as Huntley originally defined it: **"Ralph is a Bash loop."**

```bash
while :; do cat PROMPT.md | claude ; done
```

This simple approach is "deterministically bad in an undeterministic world" — it fails predictably but in ways you can address. The technique requires "faith and belief in eventual consistency," improving through iterative tuning.

## Why It Works

Traditional AI workflows direct step-by-step:

```
Human: Do step 1
AI: Done
Human: Now do step 2
AI: Done
Human: Now do step 3...
```

The Ralph Wiggum technique inverts this by defining **success criteria upfront**:

```
Human: Here's what success looks like. Keep going until you get there.
AI: [iterates until success]
```

The AI self-corrects through multiple iterations, leveraging its ability to:

- Recognize when things aren't working
- Try different approaches
- Build on previous attempts
- Eventually converge on a solution

## Key Properties

### 1. Fresh Context Each Iteration

Each cycle starts with a clean slate. The AI re-reads the prompt, re-analyzes the codebase, and makes fresh decisions. This prevents getting stuck in local minima.

### 2. Disk Is State

Files on disk are the only persistent state:

- The prompt file (`PROMPT.md`)
- The codebase itself
- Git history
- Memory files (`.ralph/agent/memories.md`)

### 3. Eventual Consistency

The technique doesn't guarantee immediate success. It guarantees that given enough iterations, a solution will emerge — as long as the task is achievable.

### 4. Predictable Failure Modes

When Ralph fails, it fails predictably:

- Iteration limit reached
- Cost limit exceeded
- Time limit exceeded
- Loop detection (repetitive outputs)

These are all observable and addressable.

## Real-World Results

The technique has proven effective at scale:

- **Y Combinator Hackathon**: Team shipped 6 repositories overnight using Ralph loops
- **Contract MVP**: One engineer completed a $50,000 contract for just $297 in API costs
- **Language Development**: Geoffrey Huntley's 3-month loop created a complete esoteric programming language (CURSED)

## When to Use

Ralph excels at:

- Large refactors and migrations
- Batch operations (docs, tests)
- Greenfield project scaffolding
- Well-defined tasks with clear completion criteria

Ralph struggles with:

- Ambiguous requirements
- Tasks requiring human judgment
- Security-sensitive code
- Exploratory work

## Enhanced Implementation

Ralph Orchestrator extends the basic technique with:

| Feature | Purpose |
|---------|---------|
| **Multi-backend support** | Works with Claude, Kiro, Gemini, and more |
| **Hat system** | Specialized personas for complex workflows |
| **Backpressure** | Quality gates that reject incomplete work |
| **TUI** | Real-time monitoring of progress |
| **Memories** | Persistent learning across sessions |
| **Safety limits** | Iteration, cost, and time limits |

## The Philosophy

> "Let Ralph Ralph" — Sit *on* the loop, not *in* it.

The goal is to tune like a guitar, not conduct like an orchestra. Set up the constraints and signals, then let Ralph do its thing.

## Next Steps

- Understand the [Six Tenets](tenets.md) that guide Ralph's design
- Learn how [Hats & Events](hats-and-events.md) add structure
- Master [Backpressure](backpressure.md) for quality control
