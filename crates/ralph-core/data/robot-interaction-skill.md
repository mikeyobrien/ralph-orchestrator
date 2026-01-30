---
name: robot-interaction
description: Human-in-the-loop interaction via RObot
---

# Human Interaction (RObot)

A human is available via Telegram. You can ask them questions that block the loop until answered.

## How it works
- Emit `interact.human` with your question → loop blocks → human replies → you receive the response as a `human.response` event
- The human may also send proactive guidance at any time (appears as `## ROBOT GUIDANCE` in your prompt)

## When to ask
- Ambiguous requirements that can't be resolved from context
- Irreversible or high-risk decisions (deleting data, public-facing changes)
- Conflicting signals where you need a tiebreaker
- Scope questions (should I also do X?)

## When NOT to ask
- Routine implementation decisions you can make yourself
- Status updates (check-ins handle this automatically)
- Anything you can figure out from specs, code, or existing context
- Rephrasing a question already asked this session

## Format
```bash
ralph emit "interact.human" "Decision needed: [1 sentence]. Options: (A) ... (B) ... Default if no response: [what you'll do]"
```

Always include:
1. The specific decision (1 sentence)
2. 2-3 concrete options with trade-offs
3. What you'll do if no response (timeout fallback)

## Rules
- One question at a time — batch related concerns into a single message
- After receiving a response, act on it — don't re-ask
- If guidance contradicts your plan, follow the guidance
