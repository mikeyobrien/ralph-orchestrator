# Slack RObot guide

Ralph's Slack surface is a Socket Mode control plane for human-in-the-loop orchestration. One allowed Slack channel maps to one default repo alias, and one Slack root thread maps to one Ralph loop. Root app mentions start loops; replies in the bound thread answer pending questions, add guidance, or run thread-local commands.

## UX model

- Root app mention starts exactly one loop in the mentioned message's thread.
- Ralph posts a Block Kit start card with loop id, repo, branch, prompt summary, and Status / Tail 10 / Stop buttons.
- Progress is low-noise: the daemon stores message timestamps and updates one progress surface with Loop, Iteration, Hat, Topic, elapsed time, and the latest redacted message instead of spamming every event.
- Completion posts a final Block Kit card with status, duration, note, and Tail 10 / Status / Approve / Request changes buttons.
- Button payloads route through the same authorized command path as text commands.
- Plain thread replies become `human.response` when a `human.interact` question is pending; otherwise they become `human.guidance` for the next loop iteration.
- Thread commands are accepted with or without `!` or `/` prefixes: `help`, `status`, `tail [n]`, `stop`, `cancel`.

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
    repo_aliases:
      ralph: /absolute/path/to/repo
    channel_repos:
      C0123456789: ralph
    start_mode: app_mention
```

Daemon mode requires at least one `repo_aliases` entry and every allowed channel to have a `channel_repos` entry that references a configured alias. Repo alias paths must be absolute and exist. Authorization and repo routing happen before loop spawn, event-file writes, process stops, or file uploads.

Slack starts can override the channel default with safe aliases and relative subdirectories:

```text
@Ralph in ralph: fix the Slack repo UX
@Ralph repo=ralph dir=crates/ralph-slack test status command
```

Subdirectories must stay inside the repo root; absolute paths, `..`, and symlink escapes are rejected.

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
5. Completion posts the final card and keeps Tail / Status / feedback controls available.

## Commands and buttons

Text commands are thread-local and accepted as `status`, `!status`, or `/status`:

- `help` — show command help.
- `repo` — show the bound repo alias, root, subdirectory, loop id, worktree, and branch.
- `status` — show current thread binding, loop status, pending-question state, and process id.
- `tail [n]` — show the last `n` events/log lines, clamped to 1..25 and redacted for token-shaped strings.
- `stop` / `cancel` — terminate the loop process; only the Slack user who started the thread can stop it.

Buttons:

- Status — same as `status`.
- Tail 10 — same as `tail 10`.
- Stop/Cancel — same as `stop`, with creator authorization.
- Approve / Request changes — final-card feedback controls; they route as thread-local text (`approved` / `request changes`) through the same pending-question or guidance path rather than mutating repo state directly.

Commands win over pending questions. For example, `status` does not accidentally answer a `human.interact` prompt.

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
5. Require a configured repo alias, either from the channel default or a safe start-message override.
6. Validate loop IDs before deriving `.worktrees/<loop_id>` paths.
7. Route thread replies through the persisted binding, not daemon current working directory.
8. Require loop creator authorization before Stop/Cancel.
9. Require file attachments to stay inside the bound repo/workspace root.

Secrets are not printed by docs, status output, command tails, or Kanban reports. Event tails are redacted for token-shaped strings before Slack replies.

## Limitations

- Socket Mode is the supported MVP path; HTTP Events API/signing-secret deployment is reserved for hosted deployments.
- Slack is not end-to-end encrypted.
- Slack text can select only configured repo aliases and safe relative subdirectories.
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

Coverage includes Socket Mode envelope parsing and ack-before-slow-work, Block Kit renderers, streaming API payloads/fallback errors, root app mention starts, thread binding, fake loop spawning, progress/final message timestamps, file uploads, question post/response flow, guidance routing, commands, interactive buttons, multi-repo channel routing, unauthorized user/channel rejection, duplicate events, bot-message ignore, stop auth, and traversal-shaped loop IDs.
