# Ralph Slack Thread Surface Implementation Plan

> **For Hermes:** Use subagent-driven-development skill to implement this plan task-by-task.

**Goal:** Add Slack as a first-class Ralph RObot surface where one Slack thread maps to one Ralph loop, the loop posts progress/questions/results into that thread, and human replies in the thread become `human.response` or `human.guidance` events.

**Architecture:** Reuse Ralph's existing `RobotService` abstraction for loop-local outbound messages and blocking `human.interact`, but add a central Slack daemon for inbound events and thread-started loop spawning. Do not make each loop open its own Slack Socket Mode connection; one daemon owns inbound Slack events, maps thread roots to loop IDs, spawns/continues loop processes, and writes incoming thread replies to the target loop's current events file. Per-loop `SlackService` only posts to Slack via Web API and polls the events file exactly like Telegram does today.

**Tech Stack:** Rust workspace, `reqwest` Web API client, `tokio-tungstenite` Socket Mode client, existing `ralph_proto::RobotService`, existing `human.interact` / `human.response` / `human.guidance` JSONL event model, `.ralph/current-events`, `.ralph/current-loop-id`, `.worktrees/<loop-id>/` loop routing.

---

## Current Context / Facts From Repo Inspection

- `crates/ralph-proto/src/robot.rs` already says communication backends can be Telegram, Slack, etc., and exposes the right trait:
  - `send_question(&self, payload) -> message id`
  - `wait_for_response(&self, events_path)`
  - `send_checkin(...)`
  - `shutdown_flag()` / `stop()`
- `crates/ralph-cli/src/loop_runner.rs:235-240` injects a robot service only when `config.robot.enabled && ctx.is_primary()`.
- `crates/ralph-cli/src/loop_runner.rs:4949-4985` currently hardcodes `ralph_telegram::TelegramService::new(...)`.
- `crates/ralph-core/src/config.rs:1938-2059` has `RobotConfig` with only `telegram: Option<TelegramBotConfig>` and validation currently requires `RObot.telegram.bot_token` when RObot is enabled.
- `crates/ralph-telegram/src/handler.rs` already has the core routing semantics we need to preserve:
  - reply-to pending question -> `human.response`
  - `@loop-id` prefix -> target that loop
  - otherwise -> default loop
  - append JSONL into the target loop's current events file.
- There is no Slack crate or Slack config today.
- The desired UX requires **more than a Telegram-style loop-local service** because Slack thread creation should start a new Ralph loop. That needs a daemon/supervisor that listens to Slack before any loop exists.

---

## Target UX

### Start a loop from Slack

Mikey posts one of:

```text
@ralph build me a quick plan for X
```

or uses:

```text
/ralph build me a quick plan for X
```

Ralph responds in a thread under that root message:

```text
🤖 Ralph loop started
Loop: slack-C123-1780792150-138669
Preset: code-assist
Status: running
```

The root Slack `thread_ts` is persisted as the canonical loop surface.

### Loop runs through hats

- Hat progress/check-ins are posted as replies in the same thread.
- If a hat emits `human.interact`, Ralph posts the question in-thread and blocks until Mikey replies or timeout expires.
- Final results are posted in-thread.

### Human steering

- A reply while a pending question exists becomes `human.response`.
- A reply when no question is pending becomes `human.guidance`.
- Guidance is injected into the loop on the next iteration via the existing RObot prompt/event path.
- Commands in the thread, e.g. `/status`, `/tail`, `/stop`, are authorized and routed to that loop only.

---

## Proposed Config Shape

Modify `RobotConfig` to support an explicit surface/provider instead of assuming Telegram:

```yaml
RObot:
  enabled: true
  surface: slack        # slack | telegram
  timeout_seconds: 86400
  checkin_interval_seconds: 300
  slack:
    bot_token: null     # or RALPH_SLACK_BOT_TOKEN
    app_token: null     # or RALPH_SLACK_APP_TOKEN; Socket Mode daemon only
    signing_secret: null # optional if using Events API instead of Socket Mode
    channel_ids:
      - C0B79UQP117
    allowed_users:
      - U123456789
    start_mode: app_mention # app_mention | slash_command | both
```

Keep Telegram compatibility for now, but validation should become provider-aware:

- `surface: telegram` -> require `RObot.telegram` token.
- `surface: slack` -> require Slack bot token for `SlackService`; require app token only for daemon/socket mode.
- If `surface` omitted, infer from configured subsection; if both are present, require explicit `surface`.

---

## Files Likely To Change

### Create

- `crates/ralph-slack/Cargo.toml`
- `crates/ralph-slack/src/lib.rs`
- `crates/ralph-slack/src/error.rs`
- `crates/ralph-slack/src/api.rs`
- `crates/ralph-slack/src/state.rs`
- `crates/ralph-slack/src/handler.rs`
- `crates/ralph-slack/src/service.rs`
- `crates/ralph-slack/src/socket_mode.rs`
- `crates/ralph-slack/src/daemon.rs`
- `crates/ralph-slack/tests/slack_routing.rs`
- `crates/ralph-slack/tests/slack_service.rs`
- `crates/ralph-slack/tests/slack_daemon.rs`
- `docs/guide/slack.md`
- `ralph.slack.yml`

### Modify

- `Cargo.toml` — add `crates/ralph-slack` workspace member and dependency.
- `crates/ralph-cli/Cargo.toml` — depend on `ralph-slack`.
- `crates/ralph-core/src/config.rs` — add `surface`, `SlackBotConfig`, provider-aware token resolution/validation.
- `crates/ralph-cli/src/loop_runner.rs` — create provider-specific `RobotService`.
- `crates/ralph-cli/src/bot.rs` — add `ralph bot onboard --slack`, `status`, `test`, and `daemon --slack` support.
- `crates/ralph-telegram/src/*` — only if shared routing helpers are extracted.
- `README.md` / docs nav — add Slack guide.
- `AGENTS.md` — update RObot section from Telegram-only to provider-backed.

---

## Data Model

Create `.ralph/slack-state.json`:

```json
{
  "team_id": "T...",
  "last_socket_envelope_id": null,
  "threads": {
    "slack-C123-1780792150-138669": {
      "loop_id": "slack-C123-1780792150-138669",
      "channel_id": "C123",
      "thread_ts": "1780792150.138669",
      "root_ts": "1780792150.138669",
      "created_by": "U123",
      "created_at": "2026-06-06T19:30:27Z",
      "workspace_root": "/Users/rook/projects/ralph-orchestrator",
      "status": "running"
    }
  },
  "thread_to_loop": {
    "C123:1780792150.138669": "slack-C123-1780792150-138669"
  },
  "pending_questions": {
    "slack-C123-1780792150-138669": {
      "channel_id": "C123",
      "thread_ts": "1780792150.138669",
      "message_ts": "1780792160.000100",
      "asked_at": "2026-06-06T19:31:00Z"
    }
  },
  "seen_event_ids": []
}
```

Notes:

- Use `channel_id + thread_ts` as the durable external address.
- Generate safe loop IDs from Slack coordinates, e.g. `slack-C123-1780792150-138669`.
- Validate loop IDs before constructing `.worktrees/<loop_id>` paths. Do not sanitize traversal into a different target.
- Store Slack timestamps as strings; they are not floats.

---

## Step-by-Step Plan

### Task 1: Add provider-aware RObot config

**Objective:** Make `RObot` support `surface: telegram | slack` without breaking existing Telegram config.

**Files:**
- Modify: `crates/ralph-core/src/config.rs`

**Steps:**
1. Add `RobotSurface` enum with serde lowercase values `telegram`, `slack`.
2. Add `pub surface: Option<RobotSurface>` to `RobotConfig`.
3. Add `pub slack: Option<SlackBotConfig>`.
4. Add `SlackBotConfig` with `bot_token`, `app_token`, `signing_secret`, `channel_ids`, `allowed_users`, `start_mode`.
5. Add `resolve_slack_bot_token()` from `RALPH_SLACK_BOT_TOKEN` then config.
6. Add `resolve_slack_app_token()` from `RALPH_SLACK_APP_TOKEN` then config.
7. Update `validate()` to infer or require a surface and validate only that provider.
8. Add config unit tests for:
   - legacy Telegram config still valid.
   - Slack config with env/config token valid.
   - both Telegram and Slack without surface errors.
   - Slack RObot enabled without timeout errors as before.

**Validation:**

```bash
cargo test -p ralph-core config::tests::test_robot
```

---

### Task 2: Create `ralph-slack` crate skeleton

**Objective:** Add a workspace crate that compiles and exports the Slack integration modules.

**Files:**
- Modify: `Cargo.toml`
- Create: `crates/ralph-slack/Cargo.toml`
- Create: `crates/ralph-slack/src/lib.rs`
- Create: `crates/ralph-slack/src/error.rs`

**Steps:**
1. Add `"crates/ralph-slack"` to workspace members.
2. Add `ralph-slack = { version = "2.9.3", path = "crates/ralph-slack" }` to workspace dependencies.
3. Add dependencies mirroring `ralph-telegram`: `ralph-proto`, `tokio`, `serde`, `serde_json`, `thiserror`, `anyhow`, `tracing`, `chrono`, `reqwest`, `tokio-tungstenite`, `futures`.
4. Export planned modules from `lib.rs`.
5. Add `SlackError` / `SlackResult` variants for missing tokens, API failures, websocket failures, event write failures, config issues.

**Validation:**

```bash
cargo check -p ralph-slack
```

---

### Task 3: Implement Slack Web API client

**Objective:** Provide a small testable wrapper around Slack `chat.postMessage` and Socket Mode URL acquisition.

**Files:**
- Create: `crates/ralph-slack/src/api.rs`
- Test: `crates/ralph-slack/tests/slack_service.rs`

**API methods:**

```rust
pub struct SlackApi { bot_token: String, client: reqwest::Client, base_url: String }

impl SlackApi {
    pub async fn post_message(&self, channel: &str, thread_ts: Option<&str>, text: &str) -> SlackResult<String>;
    pub async fn open_socket_mode_url(&self, app_token: &str) -> SlackResult<String>;
}
```

**Steps:**
1. Use `Authorization: Bearer <token>`.
2. POST JSON to `/api/chat.postMessage`.
3. Parse Slack `{ ok: true, ts: "..." }`; return `ts`.
4. Parse `{ ok: false, error: "..." }` into `SlackError::Api`.
5. Allow `base_url` override for tests.

**Validation:**
- Unit/integration test with a local mock HTTP server or test double verifies request body includes `channel`, `thread_ts`, `text`.

---

### Task 4: Implement Slack state manager

**Objective:** Persist thread-to-loop mappings and pending questions.

**Files:**
- Create: `crates/ralph-slack/src/state.rs`
- Test: `crates/ralph-slack/tests/slack_routing.rs`

**Steps:**
1. Define `SlackState`, `SlackThreadBinding`, `PendingSlackQuestion`.
2. Add atomic save via temp file + rename, matching Telegram `StateManager` pattern.
3. Implement:
   - `bind_thread(loop_id, channel_id, thread_ts, created_by, workspace_root)`
   - `loop_for_thread(channel_id, thread_ts)`
   - `add_pending_question(loop_id, channel_id, thread_ts, message_ts)`
   - `remove_pending_question(loop_id)`
   - `has_pending_question(loop_id)`
4. Add idempotency cache for Slack event IDs if available.

**Validation:**

```bash
cargo test -p ralph-slack state
```

---

### Task 5: Implement inbound Slack message handler

**Objective:** Convert Slack thread replies into Ralph events.

**Files:**
- Create: `crates/ralph-slack/src/handler.rs`
- Test: `crates/ralph-slack/tests/slack_routing.rs`

**Routing rules:**
1. Ignore messages from bots, including Ralph's own bot user.
2. Reject messages outside `allowed_users` / `channel_ids` before state changes.
3. Determine thread key:
   - `thread_ts` if present.
   - otherwise `ts` for root messages.
4. If root message starts a new loop, bind `channel_id:ts` to generated loop ID and ask daemon to spawn it.
5. If thread is known and pending question exists, append `human.response` to current events file.
6. If thread is known and no pending question exists, append `human.guidance`.
7. Fallback `@loop-id` only if no thread mapping exists.

**Important:** Do not let arbitrary channel messages become guidance. Only process bot mentions, slash commands, or messages inside Ralph-owned threads.

**Event append shape:**

```json
{"topic":"human.guidance","payload":"...","ts":"2026-06-06T19:30:27Z"}
```

**Validation:**
- Known thread + pending question writes `human.response`.
- Known thread + no pending question writes `human.guidance`.
- Unknown channel/user ignored.
- Root app mention returns a `StartLoop` action instead of appending to a non-existent loop.
- Loop ID validation rejects traversal/path separators/control chars.

---

### Task 6: Implement `SlackService` as `RobotService`

**Objective:** Let an already-running loop post questions/check-ins to its Slack thread and block on `human.response`.

**Files:**
- Create: `crates/ralph-slack/src/service.rs`
- Test: `crates/ralph-slack/tests/slack_service.rs`

**Constructor:**

```rust
pub fn new(
    workspace_root: PathBuf,
    bot_token: Option<String>,
    timeout_secs: u64,
    loop_id: String,
    channel_id: String,
    thread_ts: String,
    api_base_url: Option<String>,
) -> SlackResult<Self>
```

**Behavior:**
1. Resolve bot token from constructor or `RALPH_SLACK_BOT_TOKEN`.
2. `send_question(payload)` posts `payload` to `channel_id` + `thread_ts`, stores pending question in `.ralph/slack-state.json`, and returns message timestamp converted to a stable internal ID if needed.
3. `send_checkin(...)` posts compact progress in the same thread.
4. `wait_for_response(events_path)` mirrors Telegram: poll only new bytes in the events file until `human.response`, timeout, or shutdown.
5. `stop()` sets shutdown flag.

**Note:** `RobotService::send_question` currently returns `i32`, which fits Telegram message IDs but not Slack timestamps. For MVP return `1` on success and store Slack `message_ts` in Slack state. Longer term, change `RobotService` to return a platform-neutral `String` or no ID.

**Validation:**

```bash
cargo test -p ralph-slack service
```

---

### Task 7: Wire Slack service into `loop_runner`

**Objective:** Select Telegram or Slack based on `RObot.surface` and pass thread context into `SlackService`.

**Files:**
- Modify: `crates/ralph-cli/Cargo.toml`
- Modify: `crates/ralph-cli/src/loop_runner.rs`

**Steps:**
1. Add `ralph-slack.workspace = true` dependency.
2. Replace `create_robot_service` Telegram hardcode with provider dispatch:
   - `create_telegram_robot_service(...)`
   - `create_slack_robot_service(...)`
3. For Slack loops, resolve channel/thread from one of:
   - explicit env vars injected by daemon: `RALPH_SLACK_CHANNEL_ID`, `RALPH_SLACK_THREAD_TS`, `RALPH_LOOP_ID`.
   - `.ralph/slack-state.json` binding for current loop ID.
4. If Slack surface is configured but no thread binding exists, warn and return `None` rather than sending nowhere.

**Validation:**

```bash
cargo test -p ralph-cli bot::tests
cargo check -p ralph-cli
```

---

### Task 8: Implement Slack daemon / supervisor

**Objective:** Listen to Slack and start one Ralph loop per root Slack thread.

**Files:**
- Create: `crates/ralph-slack/src/socket_mode.rs`
- Create: `crates/ralph-slack/src/daemon.rs`
- Modify: `crates/ralph-cli/src/bot.rs`
- Test: `crates/ralph-slack/tests/slack_daemon.rs`

**Daemon command:**

```bash
ralph bot daemon --slack --workspace /Users/rook/projects/ralph-orchestrator --config ralph.slack.yml
```

**Behavior:**
1. Connect to Slack Socket Mode using `RALPH_SLACK_APP_TOKEN`.
2. Subscribe to app mentions and slash-command payloads.
3. Acknowledge Socket Mode envelopes immediately.
4. On start event:
   - create loop ID from `channel_id + root_ts`.
   - bind thread in `.ralph/slack-state.json`.
   - post “loop started” reply.
   - spawn `ralph run --no-tui --loop-id <loop_id> -p <root text>` with env:
     - `RALPH_LOOP_ID=<loop_id>`
     - `RALPH_SLACK_CHANNEL_ID=<channel_id>`
     - `RALPH_SLACK_THREAD_TS=<thread_ts>`
     - `RALPH_SLACK_BOT_TOKEN` inherited/resolved.
4. On reply event in a known thread:
   - route to `human.response` or `human.guidance` via handler.
   - optionally post a lightweight acknowledgement reaction/message.
5. Track child processes and mark thread status complete/failed.

**Open design decision:** Decide whether each Slack-started loop uses a git worktree. For “one Slack thread = one loop” and parallelism, use worktrees unless explicitly starting in the main workspace.

**Validation:**
- Fake Socket Mode event stream starts a loop using a fake process spawner.
- Reply event writes to the correct events file.
- Duplicate Slack event is ignored.

---

### Task 9: Add Slack commands in thread

**Objective:** Make Slack thread control useful without opening a terminal.

**Files:**
- Modify: `crates/ralph-slack/src/handler.rs`
- Modify: `crates/ralph-slack/src/daemon.rs`

**Commands:**
- `status` / `/status` — thread-local loop status from `.ralph/loops.json`, tasks, current hat if available.
- `tail` / `/tail` — recent events/log excerpt.
- `stop` / `/stop` — authorized stop for that loop.
- `continue <instruction>` / `revise <instruction>` — append `human.guidance`.

**Security:** Gate all commands by allowed user/channel before reading or mutating loop state.

---

### Task 10: Add onboarding, status, and test UX

**Objective:** Make setup operator-friendly.

**Files:**
- Modify: `crates/ralph-cli/src/bot.rs`
- Create/modify tests under `crates/ralph-cli/src/bot.rs` or `crates/ralph-cli/tests/`

**Commands:**

```bash
ralph bot onboard --slack
ralph bot status --slack
ralph bot test --slack --channel C0B79UQP117
ralph bot daemon --slack
```

**Onboarding should:**
1. Verify `RALPH_SLACK_BOT_TOKEN` with `auth.test`.
2. Verify `RALPH_SLACK_APP_TOKEN` with `apps.connections.open` if daemon mode.
3. Prompt for allowed channel IDs and allowed user IDs.
4. Write provider-aware `RObot` config.
5. Never print secrets.

---

### Task 11: Documentation and example preset

**Objective:** Make the feature usable without source spelunking.

**Files:**
- Create: `docs/guide/slack.md`
- Create: `ralph.slack.yml`
- Modify: `README.md`
- Modify: `AGENTS.md`

**Docs should include:**
1. Slack app creation.
2. Required scopes:
   - `chat:write`
   - `app_mentions:read`
   - `commands` if using slash command
   - `channels:history` / `groups:history` only if subscribing to ordinary thread replies in public/private channels
3. Socket Mode enablement and `xapp-...` token.
4. Config example.
5. Starting the daemon.
6. Thread UX examples.
7. Security model and allowed users/channels.
8. Known limitation: only Ralph-owned threads are processed.

---

### Task 12: End-to-end dogfood

**Objective:** Prove the real flow works.

**Validation path:**
1. Run all narrow tests:

```bash
cargo test -p ralph-core config::tests::test_robot
cargo test -p ralph-slack
cargo test -p ralph-cli bot::tests
```

2. Run broad tests:

```bash
cargo fmt
cargo test
```

3. Local fake Slack E2E:

```bash
cargo run -p ralph-e2e -- --mock
```

4. Real Slack smoke:

```bash
RALPH_SLACK_BOT_TOKEN=... RALPH_SLACK_APP_TOKEN=... \
  cargo run -p ralph-cli -- bot daemon --slack --config ralph.slack.yml
```

Then post in Slack:

```text
@ralph make a one-file hello world and ask me before finishing
```

Expected:
- Ralph starts a thread-bound loop.
- Posts progress in same thread.
- Emits a `human.interact` question.
- Reply in the thread unblocks the loop.
- Final result posts to same thread.
- `.ralph/slack-state.json` contains the thread binding and no stale pending question.

---

## MVP Cut

If we want this fast, build in two increments:

### MVP 1 — Slack HITL for a manually-started loop

- Add `RObot.surface: slack`.
- Add `SlackService` that posts to a configured `channel_id/thread_ts`.
- No daemon yet.
- Start loop manually with env:

```bash
RALPH_SLACK_CHANNEL_ID=C... \
RALPH_SLACK_THREAD_TS=1780792150.138669 \
ralph run --no-tui --loop-id slack-test -c ralph.slack.yml -p "..."
```

This proves the service, posting, blocking, and event-file response logic.

### MVP 2 — Thread starts loop

- Add daemon/socket mode.
- Root app mention or slash command creates binding and spawns the loop.
- Thread replies route to events.

Do MVP 1 first only if we need rapid risk reduction; otherwise go straight to the daemon because the user-facing product requirement is thread-started loops.

---

## Risks / Tradeoffs

- **Socket Mode vs Events API:** Socket Mode avoids public webhook/tunnel setup and fits local dogfooding. Events API is better for hosted production. Start with Socket Mode.
- **Slack message IDs are strings:** `RobotService::send_question -> i32` is Telegram-shaped. Avoid a broad trait refactor in MVP by returning `1` and storing Slack timestamps in Slack state; refactor later if needed.
- **One Socket connection per process is wrong:** A loop-local Slack service should not listen for events. Use one daemon for inbound events.
- **Auth is mandatory:** Slack channel/user allowlists must gate before commands, loop spawning, or event writes.
- **Ordinary channel message ingestion is dangerous/noisy:** Only app mentions, slash commands, and Ralph-owned threads should be processed.
- **Parallel loops need worktrees:** If multiple Slack threads can run at once, daemon should spawn worktree loops rather than everything in the main workspace.
- **Current `ctx.is_primary()` robot injection may block worktree loops:** If Slack-started loops run as worktrees, revisit the `config.robot.enabled && ctx.is_primary()` condition so each loop can have outbound Slack service while only the daemon handles inbound.

---

## Recommendation

Implement this as a first-class `ralph-slack` crate plus `ralph bot daemon --slack`. Do not try to route Slack through Hermes as a shim: Hermes can deliver messages, but it does not give Ralph the durable thread-to-loop ownership, event-file writes, process supervision, or per-thread HITL semantics we need.
