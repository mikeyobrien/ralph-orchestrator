# Slack RObot guide

Ralph's Slack surface is a Socket Mode control plane for human-in-the-loop orchestration. One allowed Slack channel maps to one repo/workspace root, and one Slack root thread maps to one Ralph loop. Root app mentions start loops; replies in a running bound thread answer pending questions, add guidance, or run thread-local commands. Once a loop is completed, failed, or stopped, its bound thread becomes an archived read-only audit record, not a reusable steering target.

## UX model

- Root app mention starts exactly one loop in the mentioned message's thread.
- Ralph posts a Block Kit start card with loop id, repo, branch, prompt summary, and Status / Tail 10 / Stop buttons.
- Progress is low-noise: the daemon stores message timestamps and updates one progress surface with Loop, Iteration, Hat, Topic, elapsed time, and the latest redacted message instead of spamming every event.
- Completion posts a final Block Kit card with status, duration, and Tail 10 / Status buttons; the binding is retained with loop id, channel/thread timestamps, repo root, final card timestamp, and local log/handoff/artifact paths when available.
- Button payloads route through the same authorized command path as text commands.
- Plain thread replies in running threads become `human.response` when a `human.interact` question is pending; otherwise they become `human.guidance` for the next loop iteration.
- Plain replies in completed/failed/stopped threads are ignored so they cannot append `human.response` or `human.guidance` to the old loop event file.
- Thread commands are accepted with or without `!` or `/` prefixes: `help`, `status`, `tail [n]`, `log [n]`, `handoff`, `repo`, `artifacts`, `stop`, `cancel`.
- New work from an archived thread must be explicit: `followup <prompt>`, `follow-up <prompt>`, `fork <prompt>`, or `new work <prompt>`. Ralph creates a new bound loop/worktree keyed to that follow-up message and records `parent_loop_id` on the new binding.

Slack timestamps are strings, not floats. Treat the external address as `channel_id + root thread_ts`; never synthesize routing from display names or channel text.

## Slack app setup

Create a dedicated Ralph Slack app. Do not reuse the Hermes Slack app; Socket Mode event streams and bot tokens should be isolated.

Required tokens:

- Bot token: `xoxb-...`, exposed as `RALPH_SLACK_BOT_TOKEN` or `RObot.slack.bot_token`.
- App-level token: `xapp-...` with `connections:write`, exposed as `RALPH_SLACK_APP_TOKEN` or `RObot.slack.app_token`.

Required bot scopes for the shippable Slack thread surface:

- `chat:write` — start cards, questions, progress updates, command replies, final cards, and Block Kit messages.
- `files:write` — loop-local artifact uploads through Slack's external file-upload flow.
- `app_mentions:read` — root app mentions that start loops.
- `channels:history` — public channel root mentions and thread replies.
- `groups:history` — only for private channels you explicitly allow.
- `im:history` — only for DM smoke/support you intentionally enable.
- `commands` — only if you configure slash-command starts.
- `assistant:write` — optional Slack AI/streaming polish. Required before `chat.startStream` / `chat.appendStream` / `chat.stopStream` works in workspaces that gate AI app APIs.

Subscribe to events:

- `app_mention` for root starts.
- `message.channels` for public-channel thread replies.
- `message.groups` / `message.im` only for enabled private-channel/DM surfaces.
- `block_actions` interactivity for Status / Tail / Stop / Approve / Request changes buttons.
- `slash_commands` only if slash-command starts are enabled.
- Optional Slack AI events for the later Assistant entrypoint: `assistant_thread_started` and `assistant_thread_context_changed`.

After changing scopes, events, Socket Mode, or interactivity, reinstall the app to the workspace and reinvite the bot to the smoke channel if needed. `missing_scope` during smoke almost always means the app was not reinstalled after the scope change.

## Example config

See the root `ralph.slack.yml` example. Keep token values out of YAML whenever possible and prefer environment variables.

```yaml
RObot:
  enabled: true
  surface: slack
  timeout_seconds: 86400
  checkin_interval_seconds: 300
  slack:
    bot_token: null       # prefer RALPH_SLACK_BOT_TOKEN
    app_token: null       # prefer RALPH_SLACK_APP_TOKEN
    channel_ids:
      - C0123456789
    allowed_users:
      - U0123456789
    channel_repos:
      C0123456789: /absolute/path/to/repo
    start_mode: app_mention
```

Daemon mode requires every allowed channel to have a `channel_repos` entry, and every repo path must be absolute and exist. Authorization and repo routing happen before loop spawn, event-file writes, process stops, or file uploads.

## Launch

```bash
# Export real values without echoing them or committing them.
export RALPH_SLACK_BOT_TOKEN='xoxb-...'
export RALPH_SLACK_APP_TOKEN='xapp-...'

cargo +stable run -p ralph-cli -- bot status --slack -c ralph.slack.yml
cargo +stable run -p ralph-cli -- bot test --slack -c ralph.slack.yml --channel C0123456789 "Ralph Slack is wired"
cargo +stable run -p ralph-cli -- bot daemon --slack -c ralph.slack.yml
```

A user starts a loop in an allowed channel:

```text
@Ralph implement the next small slice
```

Expected UX:

1. Ralph replies in the root message's thread with the start card.
2. The loop runs in an isolated worktree for that Slack thread.
3. Progress updates coalesce into the progress card/stream surface.
4. Human questions appear in-thread; answer in the thread.
5. Completion posts the final card and keeps read-only Tail / Status surfaces available. Follow-up work requires `followup <prompt>` or `fork <prompt>` and starts a new linked binding/worktree.

## Commands and buttons

Text commands are thread-local and accepted as `status`, `!status`, or `/status`:

- `help` — show command help.
- `status` — show current thread binding, loop status, pending-question state, and process id.
- `tail [n]` — show the last `n` loop events, clamped to 1..25 and redacted for token-shaped strings.
- `log [n]` — show the last `n` process log lines, clamped to 1..50 and redacted.
- `handoff` — show `.ralph/agent/summary.md` from the loop worktree when present.
- `repo` — show the bound repo/workspace root, status, and parent loop id.
- `artifacts` — list local loop artifact paths under the loop `.ralph` directory when present.
- `followup <prompt>` / `fork <prompt>` — from an archived thread only, start new work in a new loop/worktree linked to the archived loop.
- `stop` / `cancel` — terminate the running loop process; only the Slack user who started the thread can stop it.

Buttons:

- Status — same as `status`.
- Tail 10 — same as `tail 10`.
- Stop/Cancel — same as `stop`, with creator authorization.
- Final cards intentionally stay read-only; new work requires an explicit follow-up/fork command rather than recycling the completed thread.

Commands win over pending questions. For example, `status` does not accidentally answer a `human.interact` prompt. Terminal status also clears `process_id` and `pending_questions[loop_id]`, so archived threads cannot accidentally answer stale questions.

## Archived thread cleanup

Archived Slack bindings are local audit records; pruning them never mutates Slack history. Operators can list terminal bindings and prune local archived state/worktree/log artifacts older than a retention window from the bound repo root:

```bash
cargo +stable run -p ralph-cli -- bot slack-archives list
cargo +stable run -p ralph-cli -- bot slack-archives prune --older-than-days 30
```

Prune removes matching local `.ralph/slack-state.json` binding entries, `.worktrees/<loop_id>` directories, and `.ralph/slack-loop-logs/<loop_id>.log` files after the retention window. Keep retention long enough for review and incident forensics.

## Streaming capability

`SlackApi` includes wrappers for Slack's AI streaming methods:

- `chat.startStream`
- `chat.appendStream`
- `chat.stopStream`

The implementation detects common streaming fallback errors (`unsupported_method`, `missing_scope`, `method_not_supported_for_channel_type`, `not_allowed_token_type`) so a workspace without the Slack AI app capability can fall back to normal Block Kit messages. Treat streaming as an enhancement, not an authorization bypass: starts, updates, stops, attachments, and replies still use the same channel/user/thread gates.

## File attachments

Ralph can attach loop-local files to a pending `human.interact` question when the event payload is structured JSON, for example:

```json
{
  "question": "Review the generated report?",
  "attachments": [{"path": "/absolute/path/to/repo/report.md", "caption": "Generated report"}]
}
```

The file path must resolve under the configured repo/workspace root and be a regular file. Slack uploads use `files.getUploadURLExternal` followed by `files.completeUploadExternal`; deprecated `files.upload` is not used. The completion call targets the persisted `channel_id` and root `thread_ts` for the bound loop. Slack inbound text never selects arbitrary local files.

## Security model

Ralph gates before side effects:

1. Ignore bot/self messages.
2. Require an allowed channel.
3. Require an allowed user.
4. Dedupe Slack event IDs/envelope IDs.
5. Require a configured `channel_id -> repo_root` mapping for starts.
6. Validate loop IDs before deriving `.worktrees/<loop_id>` paths.
7. Route thread replies through the persisted binding, not daemon current working directory.
8. Require loop creator authorization before Stop/Cancel.
9. Require file attachments to stay inside the bound repo/workspace root.
10. Treat completed/failed/stopped bindings as read-only: only status/log/handoff/repo/artifacts commands are allowed, and explicit follow-up/fork creates a separate linked loop.

Secrets are not printed by docs, status output, command tails, or Kanban reports. Event tails are redacted for token-shaped strings before Slack replies.

## Limitations

- Socket Mode is the supported MVP path; HTTP Events API/signing-secret deployment is reserved for hosted deployments.
- Slack is not end-to-end encrypted.
- Slack text cannot select arbitrary repos or workspace roots.
- Slack AI streaming requires workspace/app support and may need app reinstall after adding `assistant:write`.
- Slash commands generally do not behave like native thread replies. Use in-thread text or Block Kit buttons for steering.
- A live Slack smoke test requires a dedicated Slack app token set. Local/fake tests do not require Slack credentials.

## Local verification path

```bash
cargo +stable fmt --all --check
cargo +stable check -p ralph-slack -p ralph-cli
cargo +stable test -p ralph-slack
cargo +stable test -p ralph-cli bot::tests
git diff --check
```

Coverage includes Socket Mode envelope parsing and ack-before-slow-work, Block Kit renderers, streaming API payloads/fallback errors, root app mention starts, thread binding, fake loop spawning, progress/final message timestamps, file uploads, question post/response flow, guidance routing, read-only archived-thread commands, follow-up/fork routing, cleanup command parsing, interactive buttons, multi-repo channel routing, unauthorized user/channel rejection, duplicate events, bot-message ignore, stop auth, and traversal-shaped loop IDs.
