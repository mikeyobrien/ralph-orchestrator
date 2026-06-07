# Slack file upload support plan

Goal: update Ralph Slack surface so loop-local Slack output can send file attachments/artifacts into the bound thread, and update the Slack app manifest/scopes/runbooks accordingly.

Plan:
1. Inspect `crates/ralph-slack` API/service traits and `RobotService` shape to find the clean file-send seam.
2. Add Slack file upload support with the current Slack external upload flow (`files.getUploadURLExternal` -> upload bytes -> `files.completeUploadExternal`) rather than deprecated `files.upload`.
3. Keep uploads scoped to the already-authorized/bound thread: channel + root `thread_ts` from persisted binding; validate local file path before reading; do not upload secrets accidentally by logging paths/tokens only sanitized.
4. Add tests with fake Slack API/server covering upload request shape, thread association, and token redaction.
5. Update manifest/runbooks/docs/setup helper to include `files:write` and file smoke steps. (Manifest/helper already include `files:write`; docs/runbooks/signoff now call out upload smoke.)
6. Run focused tests and commit.

Implementation notes:
- Use structured `human.interact` attachment payloads only (`attachments`/`files` array with `path` and optional `caption`). Do not parse arbitrary Slack inbound text as file upload commands.
- `SlackService` validates uploads against the configured workspace root before reading the file and always completes uploads into its bound `channel_id` + root `thread_ts`.

Quality bar:
- No token printing.
- No arbitrary Slack-chosen repo/path selection.
- File upload uses the same authorized bound thread model as messages.
- Concrete verification evidence before closeout.
