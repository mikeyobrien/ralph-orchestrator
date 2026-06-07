# Ralph Slack Thread Surface Production Readiness Plan

## Goal
Make Ralph's Slack surface production-ready for: one Slack root thread starts one loop, Ralph posts progress/results in-thread, Mikey can steer via replies, and stop/status/help controls are safe and reliable.

## Current live state
- Dedicated Slack app tokens and channel/user mapping exist locally under `/tmp/ralph-slack-setup/`.
- Live Slack daemon can connect and receive events.
- Live root thread successfully bound to loop `slack-C0B8SHYCBBP-1780860470-298719`.
- First live loop exposed runner/contract issues: spawned process exited/stalled, then manual codex smoke produced `debug.step` repeatedly and terminated stale instead of `LOOP_COMPLETE`.

## Production readiness gates

### Gate 1: Deterministic local E2E
- Socket Mode hello frames ignored; ackable envelopes acked before spawn.
- Root app mention creates exactly one thread binding and spawns one loop.
- Bound reply routes to `human.response` if pending question exists.
- Bound reply routes to `human.guidance` otherwise.
- Duplicate event ids are ignored.
- Unauthorized users/channels/bot messages are ignored before side effects.
- Stop/status/help/tail commands work in known threads and do not become guidance.
- Stopped loops block future guidance.
- Loop spawner uses autonomous/headless mode and writes inspectable per-loop logs.
- Slack live-smoke runbook covers state + log verification.

### Gate 2: Runner contract
- Slack-spawned loop gets the correct Slack binding (`RALPH_LOOP_ID`, channel, thread_ts, workspace root).
- Loop uses a known-good backend available on this machine.
- Config/hat contract yields valid Ralph events and either `LOOP_COMPLETE` or actionable failure, not repeated dummy/debug topics.
- Per-loop stdout/stderr are captured.
- State process_id points to the actual child Ralph process.
- Slack thread receives terminal/failure update if the loop terminates non-zero/stale.

### Gate 3: Live Slack smoke
Run live tests in the dedicated channel:
1. Preflight post succeeds.
2. Top-level app mention starts a new loop and posts loop id.
3. Reply while no pending question writes guidance to the loop event stream.
4. `/status` or `status` in thread returns current status.
5. `tail` in thread returns useful recent events/log lines without secrets.
6. `stop`/`cancel` by creator stops loop and marks state stopped.
7. Unauthorized simulated user/channel tests are covered by local tests; live test should not require unsafe real users.

### Gate 4: Hygiene/security
- No real Slack tokens in tracked files, docs, logs, or final output.
- Runtime artifacts are ignored or local-only.
- Git diff only includes coherent source/docs/tests.
- Relevant test slices and at least one broad crate test pass.

## Immediate execution steps
1. Triage live daemon and stale loop state; stop stale loops and keep one daemon running.
2. Fix the runner contract root causes with regression tests.
3. Add a narrow production-smoke config/fixture that yields `LOOP_COMPLETE` deterministically, or make the live smoke use a proven builtin hat collection instead of an ad-hoc dummy config.
4. Re-run local tests.
5. Run live Slack smoke cycle and capture state/log evidence.
6. Commit coherent changes and report remaining gaps.
