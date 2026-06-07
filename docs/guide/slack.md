# Slack RObot guide

Ralph's Slack surface is a Socket Mode control plane for human-in-the-loop orchestration. One Slack thread maps to one Ralph loop. A root app mention starts a loop; replies in that thread answer pending questions, add guidance, or run thread-local commands.

## Model

- One daemon owns Slack inbound traffic: `ralph bot daemon --slack -c ralph.slack.yml`.
- Each allowed Slack channel is locked to one repo/workspace root through `RObot.slack.channel_repos`.
- Slack text cannot choose arbitrary repos. The daemon resolves the repo from the Slack `channel_id`; after a loop starts, the persisted thread binding supplies the repo root for every reply.
- The external address is `channel_id + root thread_ts`; Slack timestamps are strings, not floats.
- Loop-local outbound messages use `SlackService` through the shared `RobotService` trait.
- Structured `human.interact` attachment payloads can upload local files; Slack inbound text never selects arbitrary local paths.

## Slack app setup

Create a Slack app in your workspace and enable Socket Mode.

Required tokens:

- Bot token: `xoxb-...`, exposed as `RALPH_SLACK_BOT_TOKEN` or `RObot.slack.bot_token`.
- App-level token: `xapp-...` with `connections:write`, exposed as `RALPH_SLACK_APP_TOKEN` or `RObot.slack.app_token`. This is required for `ralph bot daemon --slack`.

Required bot scopes for the MVP:

- `chat:write` — post loop starts, questions, progress, command replies, and results.
- `files:write` — upload loop-local artifacts through Slack's external file-upload flow into the bound thread.
- `app_mentions:read` — receive root app mentions that start loops.
- `channels:history` — receive replies in public channels.
- `groups:history` — only if you allow private channels.
- `im:history` — only if you intentionally support direct-message threads.
- `commands` — only if you enable channel-level slash command starts.

Subscribe to events:

- `app_mention` for root starts.
- `message.channels` for public-channel thread replies.
- `message.groups` / `message.im` only if those surfaces are enabled.

## Example config

See the root `ralph.slack.yml` example. The important part is the explicit provider and routing map:

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

Daemon mode requires every allowed channel to have a `channel_repos` entry, and every repo path must be absolute and exist. This is deliberate: authorization and repo routing happen before loop spawn or event-file writes.

## Launch

```bash
export RALPH_SLACK_BOT_TOKEN=xoxb-...
export RALPH_SLACK_APP_TOKEN=xapp-...
ralph bot status --slack
ralph bot test --slack --channel C0123456789 "Ralph Slack is wired"
ralph bot daemon --slack -c ralph.slack.yml
```

A user starts a loop in an allowed channel:

```text
@ralph implement the next small slice
```

Ralph replies in the root message's thread:

```text
🤖 Ralph loop started
Loop: slack-C0123456789-1780792150-138669
Status: running
```

## Thread UX

Inside a bound Ralph thread:

- If a `human.interact` question is pending, a plain reply becomes `human.response` and clears the pending question.
- If no question is pending, a plain reply becomes `human.guidance` and is injected on the next loop iteration.
- Commands are thread-local: `help`, `status`, `tail [n]`, `stop` / `cancel`.
- Commands win over pending questions; `status` does not accidentally answer a question.
- `stop` is limited to the Slack user who started the thread.

Slack nuance: slash commands generally do not behave like native thread replies. For in-thread steering, use plain thread text (`status`, `tail 10`, `stop`) or app mention behavior. Treat slash commands as channel-level starts only unless Slack changes its threading behavior.

## File attachments

Ralph can attach loop-local files to a pending `human.interact` question when the event payload is structured JSON, for example:

```json
{
  "question": "Review the generated report?",
  "attachments": [{"path": "/absolute/path/to/repo/report.md", "caption": "Generated report"}]
}
```

The file path must resolve under the configured repo/workspace root and be a regular file. Slack uploads use `files.getUploadURLExternal` followed by `files.completeUploadExternal`; deprecated `files.upload` is not used. The completion call targets the persisted `channel_id` and root thread `thread_ts` for the bound loop.

## Security model

Ralph gates before side effects:

1. Ignore bot/self messages.
2. Require an allowed channel.
3. Require an allowed user.
4. Dedupe Slack event IDs/envelope IDs.
5. Require a configured `channel_id -> repo_root` mapping for starts.
6. Validate loop IDs before deriving `.worktrees/<loop_id>` paths.
7. Route thread replies through the persisted binding, not daemon current working directory.

Secrets are not printed by docs or status output. Event tails are redacted for token-shaped strings before Slack replies.

## Limitations

- Socket Mode is the supported MVP path; HTTP Events API/signing-secret deployment is reserved for hosted deployments.
- Slack is not end-to-end encrypted.
- Slack text cannot select arbitrary repos or workspace roots.
- Each Slack-started loop should be reviewed like any other control-plane action; use explicit allowed users/channels.
- A live Slack smoke test requires a dedicated Slack app token set. Local/fake tests do not require Slack credentials.

## Local test path

The fake/local test suite covers the control-plane contract without real Slack tokens:

```bash
cargo test -p ralph-slack
cargo test -p ralph-cli bot::tests
```

Coverage includes Socket Mode envelope parsing and ack-before-slow-work, root app mention starts, thread binding, fake loop spawning, question post/response flow, guidance routing, commands, multi-repo channel routing, unauthorized user/channel rejection, duplicate events, bot-message ignore, stop auth, and traversal-shaped loop IDs.
