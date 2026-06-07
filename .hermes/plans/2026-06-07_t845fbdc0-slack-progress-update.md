# t_845fbdc0 — Slack live progress update surface

## Goal
Replace per-event Slack progress spam with one thread message that is created once and updated as loop progress changes, while preserving raw `!tail` and final completion messages.

## Steps
1. Inspect current ralph-slack API, state, monitor, and fake tests to identify progress posting path.
2. Add test-first coverage for `chat.update` and for two progress events reusing the same Slack timestamp.
3. Implement minimal Slack API/state/monitor changes with update coalescing.
4. Run targeted tests, then required fmt/test gates.
5. Commit coherent changes and block for review with evidence.
