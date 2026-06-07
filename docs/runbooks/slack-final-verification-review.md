# Slack final verification review runbook

Use this to review `feat/slack-thread-surface` before accepting the final Kanban/PR handoff. This is the local, token-free review. The live Slack smoke is covered separately in [Slack live smoke test](slack-live-smoke-test.md).

## Scope

The feature is intended to provide:

- one Slack thread per Ralph loop;
- one Slack channel mapped to one repo/workspace root via `RObot.slack.channel_repos`;
- Slack text cannot choose arbitrary repos;
- root app mentions start/bind loops;
- replies in known threads become `human.response` when a question is pending, otherwise `human.guidance`;
- thread-local commands: `help`, `status`, `tail [n]`, `stop` / `cancel`;
- authorization and dedupe before side effects.

## Prerequisites

```bash
cd /Users/rook/projects/ralph-orchestrator.slack-surface
git switch feat/slack-thread-surface
rustup toolchain list | grep -q '^stable' || rustup toolchain install stable
```

Do not export or print Slack tokens for this local review.

## 1. Confirm branch state

```bash
git status --short --branch
git log -1 --oneline
```

Expected:

- Branch is `feat/slack-thread-surface`.
- The feature commit is present; current handoff commit was `0936bbaded70660214ed56376354ccdf321b7b02` (`feat: add Slack thread RObot surface`) before these runbooks were added.
- Product changes are committed. Local `.hermes/` Kanban planning artifacts may exist in this worktree; they are not part of the product branch.

## 2. Inspect changed files

```bash
git diff --stat main...HEAD
git diff --name-only main...HEAD
```

Expected high-level shape:

- `crates/ralph-slack/` exists with API, Socket Mode, daemon, handler, service, state, and tests.
- `crates/ralph-core/src/config.rs` contains provider-aware `RObot` config, `RobotSurface`, and Slack config validation.
- `crates/ralph-cli/src/bot.rs`, `loop_runner.rs`, and `main.rs` dispatch Telegram/Slack surfaces intentionally.
- `docs/guide/slack.md` and `ralph.slack.yml` document/configure the Slack surface.
- No runtime state such as `.ralph/slack-state.json`, logs, `target/`, or token-bearing local config is committed.

## 3. Run formatting and tests

Prefer the stable toolchain explicitly because this worktree may not have a default Rust toolchain set.

```bash
cargo +stable fmt --all --check
cargo +stable test -p ralph-slack
cargo +stable test -p ralph-core
cargo +stable test -p ralph-cli
cargo +stable test
git diff --check
```

Expected:

- All commands pass.
- The prior closeout worker reported:
  - `ralph-slack`: 22 tests passed;
  - `ralph-core`: 855 unit + 24 integration + 13 doctests passed, 1 doctest ignored;
  - `ralph-cli`: 436 unit/integration tests passed, 3 ignored;
  - full workspace tests/doctests passed.

If disk space is tight, check first:

```bash
df -h .
```

If less than ~20 GB is available, run package tests first and postpone full workspace test until space is freed.

## 4. Security review checklist

Review the code paths before accepting the branch. Commands below are search aids, not substitutes for reading the matching functions.

```bash
rg -n "allowed_users|channel_ids|channel_repos|authorize|bot_id|event_id|thread_ts|loop_id|tail|stop|cancel|redact|token" crates/ralph-slack crates/ralph-core/src/config.rs crates/ralph-cli/src
```

Pass criteria:

- [ ] Bot/self messages are ignored before state changes.
- [ ] Channel allowlist is checked before binding a thread, spawning a loop, handling commands, or appending events.
- [ ] User allowlist is checked before binding a thread, spawning a loop, handling commands, or appending events.
- [ ] Empty/missing allowlists fail closed for Slack daemon startup; there is no "trust first inbound Slack event" path.
- [ ] `channel_repos` is required for allowed channels and maps to canonical absolute repo roots.
- [ ] Slack text cannot select arbitrary repos or paths.
- [ ] Thread replies route using the persisted `channel_id + root thread_ts -> loop_id + repo_root` binding, not daemon cwd.
- [ ] Event/envelope dedupe occurs before loop spawn or event append.
- [ ] Loop IDs used for worktree/event paths reject traversal, slash, empty, overly long, or control-character inputs.
- [ ] Thread commands are handled before guidance/response routing; command text does not accidentally answer a pending question.
- [ ] `stop` / `cancel` is restricted to the thread creator.
- [ ] `tail` output redacts token-shaped strings before posting to Slack.
- [ ] Examples use `null`/env placeholders and never commit real `xoxb-`, `xapp-`, signing-secret, or bearer token values.

## 5. Product behavior review checklist

```bash
cargo +stable test -p ralph-slack -- --nocapture
```

Confirm test names cover these behaviors:

- [ ] Fake Socket Mode root `app_mention` starts a fake loop.
- [ ] Socket Mode envelope ack happens before slow loop spawn work.
- [ ] Thread binding stores channel, root `thread_ts`, loop id, and repo root.
- [ ] `human.interact` question posts into the bound root thread.
- [ ] Reply with pending question writes `human.response` and clears pending question.
- [ ] Reply without pending question writes `human.guidance`.
- [ ] Duplicate event id does not double-spawn or double-write.
- [ ] Unauthorized user/channel and unknown channel mapping are rejected before side effects.
- [ ] Bot/self messages are ignored.
- [ ] Path traversal loop IDs are rejected.
- [ ] Multi-channel routing uses the persisted repo root for replies.
- [ ] `help`, `status`, `tail`, `stop` command semantics are covered.

## 6. Docs/config review checklist

Read these end to end:

```bash
sed -n '1,220p' docs/guide/slack.md
sed -n '1,180p' ralph.slack.yml
```

Pass criteria:

- [ ] Guide explains Socket Mode and required Slack scopes/events.
- [ ] Guide states one channel maps to one repo/workspace root.
- [ ] Guide states Slack text cannot choose arbitrary repos.
- [ ] Guide explains root app mention start and thread reply behavior.
- [ ] Guide explains slash-command/thread nuance.
- [ ] Example config uses env vars for secrets and placeholder IDs/paths only.
- [ ] Example config includes `RObot.surface: slack`, `channel_ids`, `allowed_users`, and `channel_repos`.

## 7. Decision

Accept local verification if:

- formatting/tests pass;
- security checklist passes;
- docs/config checklist passes;
- the only remaining blocker is the real Slack smoke prerequisites listed in [Slack live smoke test](slack-live-smoke-test.md).

If a local issue is found, do not proceed to live smoke. Capture the exact failing command, file, and reason in [Slack review signoff template](slack-review-signoff-template.md), then send it back for repair.
