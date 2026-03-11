# Project Summary: Tracking UX

## Artifacts Created

| File | Purpose |
|------|---------|
| `rough-idea.md` | Original concept |
| `idea-honing.md` | 15 Q&A requirements clarification |
| `research/existing-api-surface.md` | Audit of current API methods, models, and stream infrastructure |
| `research/core-task-system.md` | Deep dive into core TaskStore, LoopRegistry, RpcState, and TUI data flow |
| `research/ambiguities-and-gaps.md` | Architectural gaps analysis with proposed resolutions |
| `design/detailed-design.md` | Full design: types, API methods, stream events, architecture diagrams |
| `implementation/plan.md` | 13-step incremental implementation plan with test requirements |

## Design Summary

Extends Ralph's API surface so any frontend can build a Kanban board for tracking workstreams. Tasks are cards, columns are 6 statuses (Open, Blocked, InProgress, InReview, Closed, Failed), hat info is card metadata. Loops are the grouping/filtering dimension.

Key changes span three layers:
- **ralph-core**: New task statuses, status transition history, hat tracking on tasks, enriched loop registry and history
- **ralph-api**: TaskDomain rewired to core JSONL store, loop context on task responses, enriched loop responses, new EventDomain for orchestration event queries, enriched stream events
- **ralph-cli**: Loop runner bridges state to API stream, new CLI commands for Blocked/InReview transitions

## Next Steps

1. Review the detailed design at `design/detailed-design.md`
2. Review the implementation plan at `implementation/plan.md`
3. Begin implementation following the 13-step checklist
4. Steps 1-3 (core task model) can be done independently of Steps 4-5 (loop enrichment) — parallelizable
