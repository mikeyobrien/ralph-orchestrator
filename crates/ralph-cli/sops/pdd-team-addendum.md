# Agent Teams — PDD Addendum

The user enabled `--teams`. You SHOULD use Claude Code's Agent Teams feature to produce higher-quality planning artifacts through diverse perspectives and parallel investigation.

This addendum uses RFC 2119 language: MUST, SHOULD, MAY.

## General Constraints

These apply across all PDD phases when teams are active:

- You SHOULD use Agent Teams for research (Step 4), design review (Step 6), and plan evaluation (Step 7).
- You MAY skip teams for phases that are too small or tightly sequential (e.g., Step 1 project structure creation).
- You MUST give each teammate specific context — teammates do NOT inherit your conversation history. Include: the task, relevant file paths, output expectations, and any constraints.
- You MUST NOT have two teammates edit the same file — define clear file ownership before spawning.
- You MUST consolidate and verify teammate findings before presenting to the user.
- You SHOULD use delegate mode — coordinate and synthesize, don't duplicate the work yourself.

## Phase-Specific Team Patterns

### Research (Step 4) — Fan-Out

You SHOULD spawn 2–3 teammates, each investigating a different research sub-topic.

- Each teammate gets a focused research question and writes to a separate file in `research/` (e.g., `research/auth-libraries.md`, `research/database-options.md`).
- Teammates work in parallel — they do NOT need to see each other's output.
- You synthesize findings across all teammates before checking in with the user.

Example teammate prompt structure:

> You are researching {sub-topic} for a PDD planning session. Write your findings to `{project_dir}/research/{sub-topic}.md`. Include: overview, key options with trade-offs, recommendations, and references. Focus on {specific-angle}. Do NOT modify any other files.

### Design Review (Step 6) — Adversarial Review

After drafting the initial design, you SHOULD spawn a Devil's Advocate teammate to challenge it.

- The critic's job: challenge assumptions, identify gaps in edge case handling, propose alternatives, find failure modes.
- You incorporate valid criticisms into the design before presenting to the user.
- For complex designs, you MAY spawn additional perspective teammates (e.g., security reviewer, performance analyst).

Example critic prompt structure:

> You are a critical reviewer for a software design document at `{project_dir}/design.md`. Read the design and the requirements at `{project_dir}/requirements.md`. Write a critique to `{project_dir}/research/design-critique.md` covering: challenged assumptions, missing edge cases, alternative approaches, failure modes, and any requirements that appear under-specified. Be rigorous — your job is to find problems, not to validate.

### Implementation Plan (Step 7) — Competing Approaches

You SHOULD spawn 2 teammates to independently propose implementation plans from the same approved design.

- Each teammate reads the design and proposes a different step ordering or decomposition.
- You compare the plans: trade-offs, risk profiles, parallelization potential.
- Synthesize the best elements into the final plan presented to the user.

## Team Coordination

- **Task sizing:** Each teammate SHOULD have 1–3 focused deliverables. Vague scope leads to wasted tokens.
- **Communication:** Use direct messages to specific teammates. Do NOT broadcast — broadcasts send a separate message to every teammate and scale linearly with team size.
- **Shutdown:** Shut down teammates when their phase is complete. Do NOT let idle agents accumulate across phases.
- **Quality:** Read and verify teammate outputs before incorporating them. Teammates can make mistakes.

## When to Scale Down

- If the idea is small (single module, straightforward design), fewer teammates are appropriate.
- You MAY use a single teammate as a reviewer instead of full fan-out.
- The goal is better outcomes through diverse perspectives, not parallelism for its own sake.
