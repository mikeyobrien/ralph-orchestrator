# Agent Waves: Summary

## Artifacts

| File | Purpose |
|------|---------|
| `specs/agent-waves/rough-idea.md` | Captured from issue #210 + discussion |
| `specs/agent-waves/requirements.md` | 9 requirements questions with answers |
| `specs/agent-waves/research/event-system.md` | Event struct, bus, reader, logger analysis |
| `specs/agent-waves/research/loop-runner.md` | Event loop, backend execution, sequential bottleneck |
| `specs/agent-waves/research/hat-system.md` | HatConfig, Hatless Ralph, prompt building, activation lifecycle |
| `specs/agent-waves/research/worktree-system.md` | Worktree isolation, parallel loop coordination |
| `specs/agent-waves/research/cli-tools.md` | CLI structure, `ralph emit`, tool documentation |
| `specs/agent-waves/design.md` | Full design document with architecture, data models, acceptance criteria |
| `specs/agent-waves/plan.md` | 11-step implementation plan |
| `specs/agent-waves/summary.md` | This file |

## Overview

Agent Waves introduce intra-loop parallelism to Ralph's orchestration — fan-out work to concurrent backend instances, collect results, aggregate. Built on three primitives: wave-aware event emission, concurrent hat execution, and an aggregator gate.

## Key Decisions

- Ralph dispatches (decides what to parallelize), loop runner executes concurrently
- Full hat execution per wave instance — agents are smart, let them do the work
- Ralph as aggregator with `wait_for_all` gate — just another hat activation
- CLI tool (`ralph wave emit`) + context injection — same mechanism enables both explicit and NL dispatch
- Per-worker events files — avoids concurrent write issues, merged by loop runner after collection
- Shared workspace only — waves are for lightweight intra-loop parallelism; write-heavy work uses parallel loops instead (worktree isolation for waves removed from v2 scope)
- Best-effort failure handling — partial results are almost always useful
- Each instance = one activation — transparent cost accounting
- 300s default aggregation timeout — prevents hung waves

## Relationship to Parallel Loops

Waves and parallel loops are complementary, not overlapping. Parallel loops (user-initiated, full orchestration in git worktrees) handle independent write-heavy tasks. Waves (Ralph-initiated, targeted hat activations in shared workspace) handle intra-loop fan-out. Key differences: who initiates (user vs Ralph), what runs (full hat sequence vs specific hat), isolation (worktree vs shared), and completion (merge queue vs aggregator). The two compose — a parallel loop can use waves internally.

## Example Patterns

The design includes two detailed example configurations:
- **Scatter-gather code review** — dispatcher fans out to 4 specialized reviewers (security, performance, architecture, correctness), synthesizer aggregates findings into a unified review
- **Multi-round moderator-debater** — moderator runs up to 3 rounds of structured debate with dynamic participant selection per round, using `max_activations` as the rounds knob

## Next Steps

1. **Implement with Ralph:** `ralph run --config presets/pdd-to-code-assist.yml`
2. **Simpler flow:** `ralph run --config presets/spec-driven.yml`
3. **Manual implementation:** Follow the 11-step plan in `plan.md`
