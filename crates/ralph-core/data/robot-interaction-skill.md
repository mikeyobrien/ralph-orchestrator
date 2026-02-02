---
name: robot-interaction
description: Human-in-the-loop interaction via RObot
---

# Human Interaction (RObot)

A human is available via Telegram. You can ask blocking questions or send non-blocking progress updates.

**Hard rule:** Do NOT send `progress` and `human.interact` in the same turn.  
If you need to ask, include the status in the question and skip the progress update.

## Progress updates (non-blocking)

Send one-way notifications — the loop does NOT block:

```bash
ralph tools interact progress "All tests passing — starting integration phase"
```

Use for: phase transitions, milestone completions, status updates, FYI messages.

## Asking questions (blocking)

Emit `human.interact` with your question — the loop blocks until the human replies or timeout:

```bash
ralph emit "human.interact" "Decision needed: [1 sentence]. Options: (A) ... (B) ... Default if no response: [what you'll do]"
```

Always include:
1. The specific decision (1 sentence)
2. 2-3 concrete options with trade-offs
3. What you'll do if no response (timeout fallback)

The human may also send proactive guidance at any time (appears as `## ROBOT GUIDANCE` in your prompt).

## When to ask (blocking)
- Ambiguous requirements that can't be resolved from context
- Irreversible or high-risk decisions (deleting data, public-facing changes)
- Conflicting signals where you need a tiebreaker
- Scope questions (should I also do X?)

## When NOT to ask
- Routine implementation decisions you can make yourself
- Status updates — use `ralph tools interact progress` instead
- Anything you can figure out from specs, code, or existing context
- Rephrasing a question already asked this session

## Rules
- One question at a time — batch related concerns into a single message
- After receiving a response, act on it — don't re-ask
- If guidance contradicts your plan, follow the guidance
- Prefer `progress` for FYI messages; reserve `human.interact` for decisions that need input
