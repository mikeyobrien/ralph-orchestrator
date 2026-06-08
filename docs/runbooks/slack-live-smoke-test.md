# Slack live smoke test runbook

Use this after local review passes. It proves the real Slack app can drive one Ralph loop through one Slack thread with polished cards, low-noise progress, in-thread commands/buttons, optional streaming, file uploads, and completion closeout.

Do not run this with the Hermes Slack app. Use a dedicated Ralph Slack app so Socket Mode events, scopes, tokens, and test channels are isolated.

## 0. Safety rules

- Do not paste token values into Slack, commits, screenshots, Kanban comments, or review notes.
- Do not run `set -x` while tokens are exported.
- Do not commit smoke configs that contain real workspace/channel/user IDs you do not want public, and never commit token values.
- Use a low-risk test channel and repo/worktree.
- Invite only reviewers/operators who should be allowed to start or steer Ralph loops.
- Run destructive controls only against a disposable smoke loop.

## 1. Slack app prerequisites

Create or open the dedicated Ralph Slack app in Slack app management.

Required app setup:

- Socket Mode enabled.
- App-level token created with `connections:write` scope.
- Interactivity enabled so Block Kit buttons reach Socket Mode as `block_actions`.
- Bot installed to the workspace after the final scope/event changes.
- Bot invited to the allowlisted test channel.

Required bot token scopes:

- `chat:write` for cards, progress updates, command replies, final cards, and stream fallback messages.
- `files:write` for artifact/file upload smoke.
- `app_mentions:read` for root app mentions.
- `channels:history` for public-channel smoke.
- `groups:history` only if smoking private channels.
- `im:history` only if intentionally smoking DMs.
- `commands` only if smoking slash-command starts.
- `assistant:write` only if smoking Slack AI streaming APIs (`chat.startStream`, `chat.appendStream`, `chat.stopStream`). Without it, verify the Block Kit fallback path instead.

Required event subscriptions:

- `app_mention`
- `message.channels` for public-channel replies
- `message.groups` / `message.im` only for those surfaces
- `block_actions` through Slack Interactivity
- `slash_commands` only if slash-command starts are configured
- Optional AI events for future assistant entrypoint smoke: `assistant_thread_started`, `assistant_thread_context_changed`

After adding scopes/events or enabling interactivity, reinstall the app to the workspace. If smoke returns `missing_scope`, add the exact missing scope, reinstall, and retry.

Collect non-secret IDs:

- test channel id, e.g. `C...`;
- reviewer/operator Slack user id(s), e.g. `U...`;
- bot user id, if needed for self-message filtering review;
- repo alias and absolute repo path that the channel maps to;
- optional team/workspace id if your streaming smoke requires it.

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
    repo_aliases:
      ralph: /absolute/path/to/repo
    channel_repos:
      C_REAL_TEST_CHANNEL: ralph
    start_mode: app_mention
```

Rules:

- `repo_aliases` values must be absolute existing paths.
- Every `channel_ids` entry must have a `channel_repos` entry.
- Every `channel_repos` value must reference a configured repo alias.
- Do not put token values in YAML.
- Do not add broad channel/user wildcards.

## 4. Local preflight before live Slack

```bash
cargo +stable fmt --all --check
cargo +stable check -p ralph-slack -p ralph-cli
cargo +stable test -p ralph-slack
cargo +stable test -p ralph-cli bot::tests
git diff --check
```

Expected: all pass before the real Slack app is exercised.

## 5. Slack status and test post

```bash
cargo +stable run -p ralph-cli -- bot status --slack -c /tmp/ralph-slack-smoke.yml
cargo +stable run -p ralph-cli -- bot test --slack -c /tmp/ralph-slack-smoke.yml --channel C_REAL_TEST_CHANNEL "Ralph Slack smoke preflight: test post"
```

Expected:

- Status reports Slack config/token presence without revealing token values.
- Test post appears in the target Slack channel.
- If the test post fails with `not_in_channel`, invite the bot to the channel and retry.
- If it fails with `missing_scope`, add the missing scope, reinstall the app, and retry.

## 6. Start daemon

In a dedicated terminal:

```bash
cd /Users/rook/projects/ralph-orchestrator.slack-surface
cargo +stable run -p ralph-cli -- bot daemon --slack -c /tmp/ralph-slack-smoke.yml
```

Expected:

- Daemon connects through Socket Mode.
- It does not print token values.
- It acks Socket Mode envelopes before slow loop work.
- Leave this terminal running for the smoke.

## 7. Start one Slack thread loop

In the allowlisted Slack channel, send a root message that mentions the Ralph app:

```text
@Ralph smoke: start a tiny loop, report status, and ask me one question before doing anything risky
```

Expected:

- Ralph replies in the root message's thread.
- The first reply is a Block Kit start card with loop id, status, repo/branch, prompt summary, and Status / Tail 10 / Stop buttons.
- A Slack state file appears under the mapped repo root, typically `.ralph/slack-state.json`.
- The state binding contains `channel_id`, root `thread_ts`, `loop_id`, `workspace_root`, and `start_card_ts`.
- A loop log appears under the mapped repo root, typically `.ralph/slack-loop-logs/<loop-id>.log`.
- The loop runs in an isolated `.worktrees/<loop-id>` worktree when appropriate.

Inspect state locally without printing secrets:

```bash
cd /absolute/path/to/repo
python3 - <<'PY'
import json, pathlib
p = pathlib.Path('.ralph/slack-state.json')
print('state exists:', p.exists())
if p.exists():
    data = json.loads(p.read_text())
    threads = data.get('threads', data.get('thread_bindings', {}))
    print('top-level keys:', sorted(data.keys()))
    print('bindings:', len(threads))
    for loop_id, binding in list(threads.items())[-2:]:
        print('loop:', loop_id)
        print('binding keys:', sorted(binding.keys()))
PY
```

## 8. Verify polished progress UX

Wait for the loop to emit progress, then verify:

- A progress Block Kit card or streaming surface appears in the same root thread.
- It shows Loop, Iteration, Hat, Topic, elapsed time, and Last message.
- Progress updates coalesce through stored `progress_message_ts` / `stream_ts` rather than creating noisy event spam.
- Token-shaped strings in tails/progress are redacted.
- If Slack AI streaming is enabled, `chat.startStream`/`appendStream`/`stopStream` works; otherwise the fallback Block Kit messages remain usable and no auth gate is bypassed.

## 9. Verify in-thread commands and buttons

In the Slack thread:

1. Reply with a normal answer/guidance sentence.
2. Send `repo`.
3. Send `status` and `!status`.
4. Send `tail 10` and `!tail 10`.
5. Click Status.
6. Click Tail 10.
7. If safe, click Stop/Cancel or send `stop` / `cancel` from the original creator account.

Expected:

- Plain reply is accepted only from an allowlisted user.
- If a pending `human.interact` exists, plain reply becomes `human.response` and clears pending state.
- If no pending question exists, plain reply becomes `human.guidance`.
- Commands win over pending questions and do not accidentally answer a question.
- `repo` responds with the bound alias, root, subdirectory, loop id, worktree, and branch.
- `status` responds with the bound loop/thread state.
- `tail 10` redacts token-shaped strings.
- Status/Tail buttons behave like their command equivalents.
- Stop/Cancel works only for the thread creator and does not allow a different authorized user to kill someone else's loop.

Inspect events from the mapped repo:

```bash
cd /absolute/path/to/repo
rg -n 'human\.response|human\.guidance|human\.interact|approved|request changes' .ralph .worktrees 2>/dev/null || true
```

## 10. Verify file upload routing

From the mapped repo/workspace, create a harmless smoke artifact and have a loop emit a structured `human.interact` attachment payload for that path:

```bash
cd /absolute/path/to/repo
mkdir -p .ralph/smoke-artifacts
printf 'Slack file upload smoke artifact\n' > .ralph/smoke-artifacts/slack-file-smoke.txt
```

Expected:

- The file appears in the same Slack thread as the loop question, not in a new channel/root message.
- The upload completes only after the app has `files:write` and has been reinstalled after adding the scope.
- The upload uses the bound `channel_id` and root `thread_ts`; a reply timestamp must not become the file thread target.
- Slack text/replies do not trigger arbitrary local file uploads.
- Token values and file contents are not printed in daemon logs or review notes.

## 11. Verify completion card

Let the loop complete naturally or stop the disposable smoke loop.

Expected:

- The final card appears in the same root thread.
- It includes status, loop id, duration, and a short note.
- Tail 10 / Status remain available.
- Approve / Request changes buttons route as `approved` / `request changes` thread text through the pending-question or guidance path rather than mutating repo state directly.
- The final state in `.ralph/slack-state.json` is `completed`, `failed`, or `stopped`, and `process_id` is cleared.

## 12. Negative checks

Perform only safe negative checks:

- From a non-allowlisted Slack user, reply in the thread. Expected: no event append and no control action.
- In a non-allowlisted channel, mention the Ralph app. Expected: no loop spawn and no thread binding.
- Send `status` in unrelated channel chatter without a known thread binding. Expected: ignored unless it is an authorized start pattern.
- Try Stop/Cancel from an allowlisted user who did not create the thread. Expected: denied/no process kill.

Do not attempt destructive commands against a production repo.

## 13. Cleanup

Stop the daemon with `Ctrl-C`.

Remove local smoke config if it contains environment-specific IDs you do not want kept:

```bash
rm -f /tmp/ralph-slack-smoke.yml
```

Review runtime artifacts before deleting them:

```bash
cd /absolute/path/to/repo
find .ralph .worktrees -maxdepth 3 -type f 2>/dev/null | sed -n '1,120p'
```

Do not commit `.ralph/slack-state.json`, `.ralph/slack-loop-logs/`, smoke artifacts, token-bearing configs, or `.worktrees/` runtime output.

## 14. Pass/fail criteria

Pass live smoke if:

- daemon connects through Socket Mode;
- root app mention in the allowlisted channel starts exactly one loop;
- Ralph posts the start/progress/final surfaces in the root Slack thread;
- Status / Tail buttons work;
- `status` / `!status` and `tail 10` / `!tail 10` work in-thread;
- completion card appears with Tail / Status / feedback controls;
- structured loop-local file attachment upload appears in that same root thread;
- thread reply routes to `human.response` or `human.guidance` as expected;
- unauthorized channel/user/stop attempts do not create side effects;
- streaming works when `assistant:write` and workspace AI app support are available, or Block Kit fallback is confirmed when they are not;
- tokens are never printed.

If any item fails, capture:

- command run;
- sanitized error message;
- Slack app scope/event/interactivity/channel/user setup state;
- whether bot was installed/invited/reinstalled after the latest scope changes;
- relevant file/function if it looks like code behavior.

Then fill out [Slack review signoff template](slack-review-signoff-template.md) with `Decision: Blocked`.
