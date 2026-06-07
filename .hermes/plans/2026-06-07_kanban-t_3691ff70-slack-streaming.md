# Kanban t_3691ff70 — Slack AI streaming capability gate

## Goal
Add Slack `chat.startStream` / `chat.appendStream` / `chat.stopStream` support to `ralph-slack` without requiring streaming for every Slack deployment.

## Plan
1. Add failing fake-HTTP tests for stream start/append/stop request shape, Slack error surfacing, and unsupported/missing-scope fallback classification.
2. Implement small Slack API wrappers and a stream support classifier/capability type; keep tokens redacted and use JSON payloads matching current Slack docs (`markdown_text`, optional chunks, `task_update`).
3. Expose a config flag in `SlackBotConfig` for opt-in streaming while preserving default `chat.update`/message fallback behavior.
4. Run narrow tests, then `cargo +stable fmt --all --check` and `cargo +stable test -p ralph-slack`.
5. Commit coherent changes if verification passes.
