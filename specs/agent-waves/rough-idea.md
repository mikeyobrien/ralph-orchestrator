# Agent Waves: Fan-out / Fan-in Parallel Hat Execution

Source: https://github.com/mikeyobrien/ralph-orchestrator/issues/210

## Problem

Ralph's orchestration loop is strictly sequential — one hat executes at a time. For tasks that are embarrassingly parallel (reviewing N files, building N modules, running N analyses), this means N serial round-trips when they could run concurrently. There's no way to express "split this work into pieces, process them in parallel, then combine the results."

Ralph already has worktree-based parallel loops for running independent loops concurrently, but that system operates at the loop level. There's no mechanism for intra-loop parallelism.

## Proposed Solution

Introduce three primitives inspired by Enterprise Integration Patterns (Splitter + Aggregator):

1. **Wave-aware event emission** — a hat can emit a batch of events tagged with a correlation ID
2. **Concurrent hat execution** — the loop runner spawns multiple backend instances in parallel, up to a configurable concurrency limit
3. **Aggregator hat** — a hat that buffers incoming events and only activates once all correlated results arrive

## Key Design Points from Issue Discussion

- Support both explicit tool-call-based wave dispatch AND natural language dispatch (model decides which hats to activate based on context)
- Context injection: orchestrator resolves `publishes` topics to downstream hats and injects their descriptions into the prompt
- `publishes` field acts as a guardrail — model can choose a subset but can't invent new topics
- Multi-round debate pattern supported via moderator hat that can re-scatter
- Default shared workspace (no isolation) for read-only tasks; opt-in worktree isolation for write-heavy waves
- Two primary patterns:
  1. **Splitter → Workers → Aggregator**: Split N items across instances of the same hat
  2. **Scatter-Gather (Moderator/Debater)**: Send same input to N different specialized hats

## Relationship to Existing Parallel Loops

| | Worktree Parallel Loops (exists) | Agent Waves (proposed) |
|---|---|---|
| **Granularity** | Entire loops | Individual hat activations |
| **Isolation** | Full git worktree per loop | Shared workspace by default |
| **Coordination** | Merge queue + git merge | Event correlation + aggregator hats |
| **Use case** | Independent features/tasks | Parallel subtasks within one task |
| **Overhead** | Git worktree creation, branch, merge | Backend process spawn only |

## Affected Areas

- Event loop / loop runner — spawning multiple backends concurrently
- Event bus — wave tracking and aggregation buffering
- Event model — wave metadata fields
- Config — concurrency, isolation, aggregate config fields
- Worktree system — reuse for opt-in isolation mode
- CLI — `ralph wave` subcommand
- Hat system — context injection of downstream hat descriptions
