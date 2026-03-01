# Research Plan (Draft)

## Goal
Gather enough technical evidence to design per-project orchestrator lifecycle hooks that are extensible, easy to configure, and highly testable.

## Proposed Topics

1. **Current lifecycle map in Ralph core**
   - Identify concrete lifecycle boundaries in the orchestrator/event loop.
   - Locate event publication points and pre/post opportunities.
   - Capture candidate hook event taxonomy for v1.

2. **Current extension/config mechanisms in Ralph**
   - How YAML config is parsed/validated today.
   - Patterns for per-project config, reserved events, and diagnostics logging.
   - Existing command execution patterns and safety controls (timeouts/output limits).

3. **Operator control surfaces**
   - Where to add `ralph loops resume <id>` without architecture mismatch.
   - Existing stop/pause-like mechanisms (`stop-requested`, loop lock, loop registry).
   - Feasible state model for suspended loops.

4. **External hook-system patterns (targeted)**
   - Compare with Cloud Code / agent-harness hook semantics.
   - Extract practical decisions for payload contracts, hook ordering, failure policy, and mutation scope.

## Planned Outputs
- `research/01-lifecycle-map.md` ✅
- `research/02-config-and-execution-model.md` ✅
- `research/03-operator-resume-surface.md` ✅
- `research/04-external-patterns.md` ✅

## Open Questions for User
- Any additional topics or specific files/docs you want prioritized?
- Any external references you especially want mirrored?
