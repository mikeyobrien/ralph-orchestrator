# The Six Tenets

Ralph's design is guided by six core tenets. Understanding these helps you work with Ralph effectively.

## 1. Fresh Context Is Reliability

> Each iteration clears context. Re-read specs, plan, code every cycle.

**What it means:**

Each iteration starts fresh. The AI doesn't carry over state from previous iterations in its context window — it re-reads everything.

**Why it matters:**

- Prevents accumulating confusion
- Each iteration is a fresh chance to succeed
- No "poisoned" context from failed attempts
- Optimized for the "smart zone" (40-60% of usable tokens)

**How to apply:**

- Don't rely on the AI remembering previous iterations
- Use files for persistent state, not conversation history
- Make prompts self-contained

## 2. Backpressure Over Prescription

> Don't prescribe how; create gates that reject bad work.

**What it means:**

Instead of telling Ralph exactly how to do something, define quality gates that block incomplete work.

**Why it matters:**

- AI agents are smart; they can figure out the "how"
- Prescription is brittle; backpressure is robust
- Tests, lints, and typechecks are universal quality measures

**How to apply:**

```yaml
# Don't prescribe steps
instructions: |
  1. First, create the file
  2. Then, add the function
  3. Then, write tests

# Do use backpressure
instructions: |
  Implement the feature.
  Evidence required: tests: pass, lint: pass, typecheck: pass, audit: pass, coverage: pass
```

## 3. The Plan Is Disposable

> Regeneration costs one planning loop. Cheap.

**What it means:**

Plans are cheap to regenerate. Don't fight to save a failing plan — just make a new one.

**Why it matters:**

- Planning is fast; implementing is slow
- A fresh plan avoids sunk cost fallacy
- Better to start over than patch a broken approach

**How to apply:**

- Let Ralph regenerate plans freely
- Don't add complex "plan repair" logic
- If something isn't working, a new plan is one iteration away

## 4. Disk Is State, Git Is Memory

> Files are the handoff mechanism.

**What it means:**

All persistent state lives on disk:

- `.ralph/agent/memories.md` — Accumulated wisdom
- `.ralph/agent/tasks.jsonl` — Runtime work tracking
- The codebase itself
- Git history

**Why it matters:**

- Simple, reliable, debuggable
- No complex coordination mechanisms needed
- Git provides free versioning and history

**How to apply:**

- Use memory files for cross-session learning
- Use tasks for runtime tracking
- Commit important checkpoints to git

## 5. Steer With Signals, Not Scripts

> Add signs for next time. The prompts you start with won't be the prompts you end with.

**What it means:**

When Ralph fails in a specific way, add a signal (test, lint rule, guardrail) to prevent it next time. Don't try to script exact behavior.

**Why it matters:**

- Scripts are rigid; signals are adaptive
- The codebase becomes the instruction manual
- Ralph learns from its environment

**How to apply:**

```yaml
# Signal: Add a guardrail
guardrails:
  - "Always run tests before declaring done"
  - "Never modify production database directly"

# Not: Script exact steps
# (Don't do this)
```

## 6. Let Ralph Ralph

> Sit *on* the loop, not *in* it. Tune like a guitar, don't conduct like an orchestra.

**What it means:**

Your job is to set up the environment and constraints, then let Ralph work. Don't micromanage each iteration.

**Why it matters:**

- Ralph is autonomous by design
- Intervention disrupts the iteration cycle
- The whole point is hands-off operation

**How to apply:**

- Set limits (iterations, cost, time)
- Configure backpressure gates
- Monitor via TUI
- Intervene only when necessary

## Anti-Patterns

These patterns violate the tenets:

| Anti-Pattern | Why It's Bad | Tenet Violated |
|--------------|--------------|----------------|
| Building features into orchestrator | Agents can handle it | #6 Let Ralph Ralph |
| Complex retry logic | Fresh context handles recovery | #1 Fresh Context |
| Detailed step-by-step instructions | Use backpressure instead | #2 Backpressure |
| Scoping work at task selection | Scope at plan creation | #3 Plan Is Disposable |
| Assuming features don't exist | Search first | (General) |

## Summary

| Tenet | Core Idea |
|-------|-----------|
| 1. Fresh Context | Each iteration starts clean |
| 2. Backpressure | Gates, not scripts |
| 3. Disposable Plans | Regenerate freely |
| 4. Disk Is State | Files are truth |
| 5. Signals Not Scripts | Add signs, not steps |
| 6. Let Ralph Ralph | Hands off |

## Next Steps

- Learn how [Hats & Events](hats-and-events.md) implement these tenets
- Understand [Backpressure](backpressure.md) in depth
- See [Configuration](../guide/configuration.md) for applying these principles
