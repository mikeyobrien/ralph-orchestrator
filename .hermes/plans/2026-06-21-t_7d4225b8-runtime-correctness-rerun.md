# t_7d4225b8 runtime correctness rerun (#187, #217)

## Goal
Reproduce and fix two runtime correctness bugs in Ralph:

- #187: `default_publishes` can cascade to `LOOP_COMPLETE` after agent silence, falsely completing work.
- #217: `human.guidance` is advisory and can be ignored before completion/commit-style events.

## Constraints
- Follow TDD: add failing regression tests first, verify RED, then implement the smallest clean fix.
- Use `/Users/rook/projects/ralph-orchestrator` as the target repo because the Kanban workspace path was stale/wrong for this card.
- Run `cargo test`, `cargo test -p ralph-core smoke_runner`, and `cargo fmt --all --check` before handoff.
- Leave a review-required Kanban block with diff/test evidence instead of auto-completing merge-sensitive code.

## Plan
1. Inspect current event-loop code and tests around `default_publishes`, completion promise, and human guidance.
2. Write regression tests for #187 and #217 and verify they fail on current main/branch.
3. Implement minimal event-loop/prompt changes that enforce explicit evidence/acknowledgement.
4. Run targeted tests, full test suite, smoke tests, and fmt.
5. Comment on GitHub issues with evidence and block for review.
