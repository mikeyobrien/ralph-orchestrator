# t_4dc805ed Slack interactive controls plan

## Objective
Add Slack-native Block Kit control handling for status, tail, stop/cancel, approve, and request changes while preserving typed commands and existing Telegram/RObot behavior.

## Steps
1. RED: add focused ralph-slack tests for interactive Socket Mode `block_actions` parsing and button auth/command routing.
2. GREEN: map Slack action payloads into the existing thread routing path, preserving allow-list checks and creator-only stop.
3. Polish Block Kit renderers so cards expose Status, Tail 10, Stop/Cancel, Approve, and Request changes where appropriate.
4. Verify with `cargo +stable fmt --all --check` and `cargo +stable test -p ralph-slack`.
5. Commit only coherent source/test/plan changes.
