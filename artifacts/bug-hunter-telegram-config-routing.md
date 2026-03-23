## Scope

- `crates/ralph-telegram/src/*`
- `crates/ralph-cli/src/bot.rs`
- `crates/ralph-core/src/config.rs`
- Relevant docs/tests for intended behavior and drift checks

## Report file

- `artifacts/bug-hunter-telegram-config-routing.md`

## Findings

### P1: Incoming Telegram messages are not authenticated to the configured operator chat, so any chat that can reach the bot can read status data or drive loop behavior

- Impact:
  - While a loop is running, any Telegram user who can message the bot can run `/status`, `/tasks`, `/memories`, `/tail`, `/model`, `/restart`, and `/stop`, and the response is sent back to that sender's chat.
  - The same unauthenticated sender can inject `human.guidance` or satisfy a pending `human.interact` question.
  - While the daemon is idle, any sender can start a new loop; the acknowledgement is sent to the configured operator chat, not the sender, so the operator sees unexpected loop launches.
- Exact evidence:
  - Active-loop polling accepts every `msg.chat.id` and never compares it to persisted `state.chat_id` before executing commands or routing messages: `crates/ralph-telegram/src/service.rs:275-337`.
  - `MessageHandler` only auto-detects the chat on first contact and otherwise never rejects a mismatched `chat_id`: `crates/ralph-telegram/src/handler.rs:36-66`.
  - Idle daemon polling discards sender chat identity entirely; `DaemonUpdate` only keeps `update_id` and `text`, and `run_daemon` uses any text update to execute commands or `start_loop(prompt)`: `crates/ralph-telegram/src/daemon.rs:117-217`, `crates/ralph-telegram/src/daemon.rs:231-276`.
  - The command surface exposes local state such as tasks, memories, and event tails: `crates/ralph-telegram/src/commands.rs:330-520`, `crates/ralph-telegram/src/commands.rs:522-760`.
  - Intended behavior is a single human operator bound to the detected chat: `crates/ralph-telegram/README.md:56-57`, `crates/ralph-core/src/config.rs:1921-1980`.
- Minimal repro / message flow:
  1. Legitimate operator onboards the bot and creates `.ralph/telegram-state.json` with their `chat_id`.
  2. A different Telegram account sends `/tail` to the bot during a run.
  3. `TelegramService::poll_updates` reads that update and immediately replies to the attacker's `chat_id` with current event contents.
  4. The same attacker can send plain text or reply to a pending question; the handler writes `human.guidance` / `human.response` into the orchestration events file.
  5. If the daemon is idle, any attacker message starts a new loop because the daemon never checks sender identity.
- Likely root cause:
  - The stored `chat_id` is treated as metadata, not as an authorization boundary.
  - The daemon polling path throws away sender chat identity before routing.
- Fix direction:
  - Reject or ignore inbound updates whose `chat_id` does not match the configured/persisted operator chat.
  - Preserve sender chat ID in the daemon update model and enforce the same check there.
  - Add explicit tests for foreign-chat command rejection, foreign-chat guidance rejection, and foreign-chat loop-start rejection.
- Current test coverage:
  - No tests exercise mismatched chat IDs or unauthorized chat rejection.

### P2: `@loop-id` routing accepts unsanitized path segments, allowing path traversal outside `.worktrees/`

- Impact:
  - A Telegram sender can cause Ralph to append orchestration events under arbitrary filesystem locations relative to the workspace, including paths outside the repo root's `.worktrees/` subtree.
  - This can corrupt sibling worktree state or create stray `.ralph/events.jsonl` files outside the intended routing area.
- Exact evidence:
  - The handler extracts the raw token after `@` with no validation: `crates/ralph-telegram/src/handler.rs:98-106`.
  - That raw string is joined directly into `.worktrees/<loop_id>/.ralph/...`: `crates/ralph-telegram/src/handler.rs:113-142`.
  - On this checkout, the effective path escapes as expected:
    - `realpath -m /home/coe/scroll/agent-orchestrator/.worktrees/../agent/.ralph/events.jsonl`
      -> `/home/coe/scroll/agent-orchestrator/agent/.ralph/events.jsonl`
    - `realpath -m /home/coe/scroll/agent-orchestrator/.worktrees/../../tmp/.ralph/events.jsonl`
      -> `/home/coe/scroll/tmp/.ralph/events.jsonl`
- Minimal repro / message flow:
  1. Send `@../agent investigate this` to the bot.
  2. `determine_target_loop()` returns `../agent`.
  3. `get_events_path()` resolves the target to `.worktrees/../agent/.ralph/events.jsonl`.
  4. `append_event()` creates parent directories and appends a `human.guidance` / `human.response` line there.
- Likely root cause:
  - Loop IDs are treated as trusted filesystem path components.
- Fix direction:
  - Validate loop IDs against the actual registered loop/worktree IDs before routing.
  - Reject any prefix containing path separators, `.` / `..`, or other non-identifier characters.
  - Add regression tests for `@../x`, `@../../x`, and `@foo/bar`.
- Current test coverage:
  - Existing tests only cover a benign prefix like `@feature-auth`: `crates/ralph-telegram/src/handler.rs:245-257`.

### P2: `telegram-state.json` updates are unsynchronized read-modify-write operations, so concurrent poll/send activity can lose pending questions or update offsets

- Impact:
  - A background poll can overwrite a just-added pending question, causing a valid reply to be treated as guidance or ignored until timeout.
  - `last_update_id` can also be reverted or lost, increasing duplicate update processing risk after restart.
  - Concurrent saves also share a single temp path (`telegram-state.json.tmp`), so overlapping writes can race on rename.
- Exact evidence:
  - `TelegramService::start()` spawns a background polling task: `crates/ralph-telegram/src/service.rs:192-206`.
  - The event-loop thread calls `send_question()` / `wait_for_response()` using the same state file: `crates/ralph-telegram/src/service.rs:421-445`, `crates/ralph-telegram/src/service.rs:628-696`.
  - The polling task independently loads state, mutates it, and saves it after each inbound update: `crates/ralph-telegram/src/service.rs:304-352`.
  - `StateManager::save()` has no locking or compare-and-swap; it always writes through the fixed temp file `telegram-state.json.tmp` and renames it into place: `crates/ralph-telegram/src/state.rs:52-63`.
- Minimal repro / message flow:
  1. Poll task loads state `S0` with no pending questions.
  2. Event loop calls `send_question()`, which loads `S0`, sends the question, adds `pending_questions[loop_id]`, and saves `S1`.
  3. Poll task resumes from its stale copy of `S0`, updates `last_seen` / `last_update_id`, and saves `S2`.
  4. `S2` no longer contains the pending question from `S1`; a later human reply cannot be matched to the question.
- Likely root cause:
  - Multiple independent `StateManager` users mutate a shared JSON file without any serialization or merge logic.
- Fix direction:
  - Serialize state access with a process-local mutex or move to a single owner task.
  - Use unique temp files if atomic file replacement remains the persistence strategy.
  - Add a concurrency regression test that races `send_question()` against poll-state persistence and asserts the pending question survives.
- Current test coverage:
  - State tests only cover single-threaded round-trips and lookups: `crates/ralph-telegram/src/state.rs:126-191`.
  - Service tests cover timeout/shutdown cleanup but not concurrent writers: `crates/ralph-telegram/src/service.rs:1037-1314`.

### P2: CLI bootstrap/status/test helpers bypass the runtime config resolvers, so they can ignore custom API URLs and validate/send against a different bot than `ralph run`

- Impact:
  - `ralph bot onboard`, `ralph bot status`, and `ralph bot test` do not honor `RALPH_TELEGRAM_API_URL` / `RObot.telegram.api_url`, so mock Telegram servers or custom API endpoints cannot be used for bootstrap/diagnostics even though runtime supports them.
  - Token precedence differs between runtime and CLI helpers:
    - Runtime: env -> project config -> keychain (`RobotConfig::resolve_bot_token`)
    - CLI helpers: env -> keychain -> `ralph.yml` (`resolve_token`, `bot_status`)
  - A project-level token override can therefore be ignored by `ralph bot test/status` while `ralph run` uses it.
- Exact evidence:
  - Runtime resolver order and custom API support live in `RobotConfig`: `crates/ralph-core/src/config.rs:1983-2027`.
  - Daemon path uses those runtime resolvers: `crates/ralph-cli/src/bot.rs:554-573`.
  - CLI bootstrap helpers hardcode `https://api.telegram.org/...` for `getMe`, `getUpdates`, and `sendMessage`: `crates/ralph-cli/src/bot.rs:607-608`, `crates/ralph-cli/src/bot.rs:667-669`, `crates/ralph-cli/src/bot.rs:726-727`.
  - `resolve_token()` prefers keychain before config: `crates/ralph-cli/src/bot.rs:1006-1023`.
  - `bot_status()` also builds its effective token as env -> keychain -> config: `crates/ralph-cli/src/bot.rs:411-414`.
  - The README promises custom API URL support for setup/testing: `crates/ralph-telegram/README.md:40-48`.
- Minimal repro / message flow:
  1. Set `RALPH_TELEGRAM_API_URL=http://localhost:8081` or configure `RObot.telegram.api_url`.
  2. Run `ralph bot onboard` or `ralph bot test`.
  3. The helper still calls `https://api.telegram.org/...` because it never consults the runtime resolver.
  4. Separately, store token `A` in keychain and token `B` in project `ralph.yml`; `ralph run` uses `B`, but `ralph bot test/status` resolve `A`.
- Likely root cause:
  - The CLI helper layer reimplemented token/API resolution instead of reusing `RobotConfig`.
- Fix direction:
  - Route all bot subcommands through the same resolver functions used by runtime.
  - Add tests asserting parity between `RobotConfig::resolve_*` and CLI helper resolution, and a test that custom API URLs flow into onboarding/test/status requests.
- Current test coverage:
  - `bot.rs` tests cover YAML helpers and token normalization, but there are no tests for CLI/runtime resolver parity or custom API URL use in the helper HTTP calls.

## Evidence

- Source inspection covered the daemon intake path, active-loop polling path, handler routing, state persistence, CLI onboarding/testing/status helpers, and runtime config resolution.
- `cargo test -p ralph-telegram` passed locally (89 tests, 0 failures), which supports the drift claim: the current unit suite does not cover the above auth, traversal, concurrency, or CLI/runtime parity failures.
- No code in scope compares an incoming Telegram `chat_id` against persisted `state.chat_id`; searching for a mismatch check in the scoped files returned nothing.

## Areas inspected

- `crates/ralph-telegram/src/daemon.rs`
- `crates/ralph-telegram/src/service.rs`
- `crates/ralph-telegram/src/handler.rs`
- `crates/ralph-telegram/src/state.rs`
- `crates/ralph-telegram/src/commands.rs`
- `crates/ralph-telegram/src/bot.rs`
- `crates/ralph-telegram/src/error.rs`
- `crates/ralph-cli/src/bot.rs`
- `crates/ralph-core/src/config.rs`
- `crates/ralph-core/src/preflight.rs`
- `crates/ralph-cli/src/loop_runner.rs` (only the robot-service creation site)
- `crates/ralph-telegram/README.md`

Covered no-finding notes:

- Retry backoff timing itself looks correct and is directly tested (`1s`, `2s`, no final sleep): `crates/ralph-telegram/src/service.rs:21-63`, tests at `crates/ralph-telegram/src/service.rs:1115-1261`.
- `wait_for_response()` cleanup on timeout and shutdown is implemented and tested: `crates/ralph-telegram/src/service.rs:648-696`, tests at `crates/ralph-telegram/src/service.rs:1085-1314`.
- Core config validation for `RObot.enabled`, required `timeout_seconds`, and token presence is present and covered by unit tests, but those tests do not cover empty-string env values or resolver parity with CLI helpers: `crates/ralph-core/src/config.rs:1957-2027`, tests around `crates/ralph-core/src/config.rs:3384-3579`.

## Recommended next search

- Verify whether the same unauthenticated-chat assumption exists in the Telegram bot service outside this lane's files, especially any web/telegram bridge or event consumers that assume `human.response` is trustworthy once it reaches the bus.
- After fixes, add integration tests that exercise:
  - foreign-chat rejection,
  - `@loop-id` sanitization,
  - concurrent state updates under load,
  - CLI helper parity with runtime token/API resolution.
