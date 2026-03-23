# Scope

Scoped lane `telegram-routing-state`, limited to:

- `crates/ralph-telegram/src/handler.rs`
- `crates/ralph-telegram/src/daemon.rs`
- `crates/ralph-telegram/src/bot.rs`
- `crates/ralph-cli/src/bot.rs`

# Findings

## P1: The daemon accepts updates from any chat and executes them against the configured chat/loop

- Impacted files:
  - `crates/ralph-telegram/src/daemon.rs`
  - `crates/ralph-cli/src/bot.rs`
- Why it is a bug:
  - The daemon is configured with one trusted `chat_id`, but incoming update handling drops source-chat metadata and processes text without verifying that the sender matches the trusted chat.
- Exact evidence:
  - The configured chat ID is passed into the daemon from onboarding: `crates/ralph-cli/src/bot.rs:554`.
  - `poll_updates` keeps only `update_id` and `text`: `crates/ralph-telegram/src/daemon.rs:231-260`.
  - `run_daemon` processes message text and replies to the configured chat without checking the source chat: `crates/ralph-telegram/src/daemon.rs:117-221`.
- Triggering scenario:
  - Chat A onboards the bot.
  - Chat B later messages the same bot.
  - The daemon launches work from B's message, but status and replies go back to A.
- Likely impact:
  - Cross-chat command execution and disclosure of bot output to the wrong user.
- Recommended fix direction:
  - Preserve sender chat metadata in the poll path and reject or ignore updates whose source chat does not match the configured chat.
- Confidence:
  - High.
- Whether current tests cover it:
  - No test in the inspected files asserts source-chat validation before executing commands.

## P2: The daemon persists `last_update_id` but never restores it, so restarts can replay old updates

- Impacted files:
  - `crates/ralph-telegram/src/daemon.rs`
- Why it is a bug:
  - Restarting the daemon resets the update offset to `0` even though previous progress is persisted.
- Exact evidence:
  - Startup initializes `let mut offset: i32 = 0;`: `crates/ralph-telegram/src/daemon.rs:98`.
  - Processed updates save `last_update_id`: `crates/ralph-telegram/src/daemon.rs:125`.
  - `poll_updates` uses the provided offset directly: `crates/ralph-telegram/src/daemon.rs:241`.
  - No startup path reloads the saved cursor in the inspected file.
- Triggering scenario:
  - Update `100` is processed and persisted.
  - The daemon restarts.
  - It begins polling from `0`, so retained updates can be replayed and commands rerun.
- Likely impact:
  - Duplicate loop starts, duplicate bot actions, and confusing operator experience after restart.
- Recommended fix direction:
  - Restore the saved `last_update_id` at startup and continue polling from the persisted checkpoint.
- Confidence:
  - High.
- Whether current tests cover it:
  - No restart-resume regression was visible in the inspected files.

# No-Finding Coverage Notes

- `crates/ralph-telegram/src/handler.rs`
  - Checked reply routing and event-file writes.
  - No stronger defect was confirmed in the requested scope.
- `crates/ralph-telegram/src/bot.rs`
  - Checked send formatting and HTML escaping.
  - No P0-P2 issue confirmed in this lane.
- `crates/ralph-cli/src/bot.rs`
  - Checked token precedence and fallback storage behavior.
  - No stronger defect confirmed beyond daemon onboarding/context handoff.

# Remaining Blind Spots

- This lane did not inspect the state-manager implementation in detail.
- This lane did not inspect in-loop Telegram service paths outside the requested files.

# Recommended Next Search

- Inspect state-manager and in-loop Telegram service paths for pending-question misclassification and cross-loop reply routing.
- Inspect daemon/loop handoff for overlap races between idle polling and in-loop polling.
