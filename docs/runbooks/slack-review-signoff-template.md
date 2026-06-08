# Slack review signoff template

Copy this into the final Kanban comment, PR review, or release handoff after running the review runbooks.

```markdown
# Ralph Slack thread surface final review

Branch: `feat/slack-thread-surface`
Commit(s) reviewed:
Reviewer:
Date:

## Decision

- [ ] Accepted: local verification passed and live Slack smoke passed.
- [ ] Accepted with caveat: local verification passed; live Slack smoke is blocked only on missing dedicated Slack app/tokens/real IDs.
- [ ] Blocked: local verification or live smoke found an issue that needs repair.

## Local verification

Commands run:

- `cargo +stable fmt --all --check` →
- `cargo +stable test -p ralph-slack` →
- `cargo +stable test -p ralph-core` →
- `cargo +stable test -p ralph-cli` →
- `cargo +stable test` →
- `git diff --check` →

Local result:

Security checklist:

- [ ] Auth before side effects
- [ ] Explicit allowed users/channels
- [ ] No first-inbound trust
- [ ] Safe repo aliases configured via `RObot.slack.repo_aliases`
- [ ] Channel maps to repo alias via `RObot.slack.channel_repos`
- [ ] Slack text can choose only configured aliases and safe relative subdirs
- [ ] Thread replies use persisted repo/dir binding
- [ ] Event/envelope dedupe before spawn/write
- [ ] Loop/path traversal rejected
- [ ] Commands do not accidentally answer pending questions
- [ ] `stop`/`cancel` creator-only
- [ ] Tail/status/token handling redacts secrets
- [ ] File uploads require `files:write`, use Slack external upload flow, and stay bound to the root thread
- [ ] File path validation rejects non-workspace paths before reading
- [ ] Docs/examples do not contain real secrets

Notes:

## Live Slack smoke

Dedicated Ralph Slack app used: yes/no
Bot installed and invited to channel: yes/no
Tokens present locally without printing values: yes/no
Allowlisted channel id configured: yes/no
Allowlisted user id configured: yes/no
`repo_aliases` absolute repo mapping configured: yes/no
`channel_repos` channel-to-alias default configured: yes/no

Smoke commands/results:

- `ralph bot status --slack -c /tmp/ralph-slack-smoke.yml` →
- `ralph bot test --slack -c /tmp/ralph-slack-smoke.yml --channel ...` →
- `ralph bot daemon --slack -c /tmp/ralph-slack-smoke.yml` →
- Root app mention starts one thread loop →
- Thread reply routes to `human.response`/`human.guidance` →
- `status` / `tail` work →
- Structured file upload appears in root thread after `files:write` reinstall →
- Unauthorized user/channel negative check →

Live smoke result:

## Remaining blocker, if any

Exact missing setup/action:

## Final handoff

Changed files reviewed:

Known caveats:

Recommended next action:
```
