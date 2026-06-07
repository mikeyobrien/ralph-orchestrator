# Slack live smoke test runbook

Use this after the local review passes. This proves the real Slack app can drive one Ralph loop through one Slack thread.

Do not run this with the Hermes Slack app. Use a dedicated Ralph Slack app so its Socket Mode event stream, scopes, and tokens are isolated.

## 0. Safety rules

- Do not paste token values into Slack, commits, screenshots, Kanban comments, or review notes.
- Do not run `set -x` while tokens are exported.
- Do not commit the smoke config if it contains real IDs you do not want public, and never commit token values.
- Use a low-risk test channel and repo/worktree.
- Invite only the reviewers who should be allowed to start/steer Ralph loops.

## 1. Slack app prerequisites

Create or open the dedicated Ralph Slack app in Slack app management.

Required app setup:

- Socket Mode: enabled.
- App-level token: created with `connections:write` scope.
- Bot token scopes:
  - `app_mentions:read`
  - `chat:write`
  - `channels:history` for public-channel smoke
  - `groups:history` only if smoking a private channel
  - `im:history` only if intentionally smoking DMs
- Event subscriptions:
  - `app_mention`
  - `message.channels` for public-channel smoke
  - `message.groups` / `message.im` only for those surfaces
- Bot installed to the workspace.
- Bot invited to the allowlisted test channel.

Collect non-secret IDs:

- team/workspace id, if your config/review wants it;
- test channel id, e.g. `C...`;
- reviewer Slack user id(s), e.g. `U...`;
- bot user id, if needed for self-message filtering review;
- absolute repo path that channel should map to.

## 2. Export tokens without printing them

```bash
cd /Users/rook/projects/ralph-orchestrator.slack-surface

# Paste values into your shell. Do not echo them.
export RALPH_SLACK_BOT_TOKEN='xoxb-...'
export RALPH_SLACK_APP_TOKEN='xapp-...'

python3 - <<'PY'
import os
for name in ["RALPH_SLACK_BOT_TOKEN", "RALPH_SLACK_APP_TOKEN"]:
    value = os.environ.get(name)
    print(f"{name}: {'set' if value else 'missing'} ({len(value) if value else 0} chars)")
PY
```

Expected: both variables report `set`. The command must not print token values.

## 3. Create a local smoke config

Use a local copy rather than editing `ralph.slack.yml` in place.

```bash
cp ralph.slack.yml /tmp/ralph-slack-smoke.yml
${EDITOR:-nano} /tmp/ralph-slack-smoke.yml
```

Replace placeholders:

```yaml
RObot:
  enabled: true
  surface: slack
  timeout_seconds: 86400
  checkin_interval_seconds: 300
  slack:
    bot_token: null       # use RALPH_SLACK_BOT_TOKEN
    app_token: null       # use RALPH_SLACK_APP_TOKEN
    signing_secret: null
    channel_ids:
      - C_REAL_TEST_CHANNEL
    allowed_users:
      - U_YOUR_USER_ID
    channel_repos:
      C_REAL_TEST_CHANNEL: /absolute/path/to/repo
    start_mode: app_mention
```

Rules:

- `channel_repos` value must be an absolute existing path.
- Every `channel_ids` entry must have a `channel_repos` entry.
- Do not put token values in the YAML.
- Do not add broad channel/user wildcards.

## 4. Preflight status and test post

```bash
cargo +stable run -p ralph-cli -- bot status --slack -c /tmp/ralph-slack-smoke.yml
cargo +stable run -p ralph-cli -- bot test --slack -c /tmp/ralph-slack-smoke.yml --channel C_REAL_TEST_CHANNEL "Ralph Slack smoke preflight: test post"
```

Expected:

- Status reports Slack config/token presence without revealing token values.
- Test post appears in the target Slack channel.
- If the test post fails with `not_in_channel`, invite the bot to the channel and retry.
- If it fails with `missing_scope`, add the missing Slack scope, reinstall the app, and retry.

## 5. Start daemon

In a dedicated terminal:

```bash
cd /Users/rook/projects/ralph-orchestrator.slack-surface
cargo +stable run -p ralph-cli -- bot daemon --slack -c /tmp/ralph-slack-smoke.yml
```

Expected:

- Daemon connects to Socket Mode.
- It does not print token values.
- Leave this terminal running for the smoke.

## 6. Start one Slack thread loop

In the allowlisted Slack channel, send a root message that mentions the Ralph app:

```text
@Ralph smoke: start a tiny loop and ask me one question before doing anything risky
```

Expected:

- Ralph replies in the root message's thread.
- The reply includes a loop id/status.
- A Slack state file appears under the mapped repo root, typically `.ralph/slack-state.json`.
- The loop/thread binding uses the test channel id and root `thread_ts`.

Inspect state locally without printing secrets:

```bash
cd /absolute/path/to/repo
python3 - <<'PY'
import json, pathlib
p = pathlib.Path('.ralph/slack-state.json')
print('state exists:', p.exists())
if p.exists():
    data = json.loads(p.read_text())
    print('top-level keys:', sorted(data.keys()))
    print('bindings:', len(data.get('threads', data.get('thread_bindings', {}))))
PY
```

## 7. Verify human-in-the-loop routing

In the Slack thread:

1. Reply with a normal answer/guidance sentence.
2. Send `status`.
3. Send `tail 10`.
4. If safe, send `stop` / `cancel` from the original creator account.

Expected:

- Plain reply is accepted only from an allowlisted user.
- If a pending `human.interact` exists, plain reply becomes `human.response` and clears pending state.
- If no pending question exists, plain reply becomes `human.guidance`.
- `status` responds as a command and does not clear a pending question.
- `tail 10` redacts token-shaped strings.
- `stop` / `cancel` works only from the thread creator.

Inspect events from the mapped repo:

```bash
cd /absolute/path/to/repo
rg -n 'human\.response|human\.guidance|human\.interact' .ralph .worktrees 2>/dev/null || true
```

## 8. Negative checks

Perform only safe negative checks:

- From a non-allowlisted Slack user, reply in the thread. Expected: no event append and no control action.
- In a non-allowlisted channel, mention the Ralph app. Expected: no loop spawn and no thread binding.
- Send `status` in unrelated channel chatter without a known thread binding. Expected: ignored unless it is an authorized start pattern.

Do not attempt destructive commands against a production repo.

## 9. Cleanup

Stop the daemon with `Ctrl-C`.

Remove local smoke config if it contains environment-specific IDs you do not want kept:

```bash
rm -f /tmp/ralph-slack-smoke.yml
```

Optional cleanup in the mapped repo after collecting evidence:

```bash
cd /absolute/path/to/repo
# Review before deleting; these are runtime artifacts.
find .ralph .worktrees -maxdepth 3 -type f 2>/dev/null | sed -n '1,120p'
```

## 10. Pass/fail criteria

Pass live smoke if:

- daemon connects through Socket Mode;
- root app mention in the allowlisted channel starts exactly one loop;
- Ralph posts replies in the root Slack thread;
- thread reply routes to `human.response` or `human.guidance` as expected;
- `status`/`tail` work in-thread;
- unauthorized channel/user attempts do not create side effects;
- tokens are never printed.

If any item fails, capture:

- command run;
- sanitized error message;
- Slack app scope/event/channel/user setup state;
- whether bot was installed/invited;
- relevant file/function if it looks like code behavior.

Then fill out [Slack review signoff template](slack-review-signoff-template.md) with `Decision: Blocked`.
