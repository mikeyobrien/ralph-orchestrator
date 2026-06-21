---
status: complete
created: 2026-06-21
started: 2026-06-21
completed: 2026-06-21
---
# Task: Implement File-Backed Web RObot Service

## Description
Implement issue #242 by adding loop-side `RObot.mode: web` selection and a
file-backed `WebRobotService` for API-driven human interaction.

## Scope
- Keep `telegram` as the default `RObot.mode` and preserve existing Telegram behavior.
- Add `web` mode without requiring Telegram token configuration.
- Write questions, responses, and check-ins via `.ralph/api/robot-question.json`,
  `.ralph/api/robot-response.json`, and `.ralph/api/robot-checkin.json`.
- Treat `timeout_seconds: 0` as an indefinite wait that still honors shutdown.
- Do not implement the public `robot.*` API from issue #243.

## Verification
- Focused config and web robot service tests.
- `cargo test -p ralph-core smoke_runner`.
- `cargo test` if feasible.
