# t_8e8070d1 — Slack UX final polish closeout

## Objective
Prove the Slack thread surface is shippable: docs reflect the current polished UX, tests pass, live-smoke availability is checked without exposing secrets, and the branch is left reviewable.

## Steps
1. Inspect current Slack implementation and docs against the source UX plan.
2. Update `docs/guide/slack.md` and `docs/runbooks/slack-live-smoke-test.md` for Block Kit cards/buttons, streaming APIs, thread commands, scopes/events, smoke checklist, and reinstall requirements.
3. Run required gates: `cargo +stable fmt --all --check`, `cargo +stable check -p ralph-slack -p ralph-cli`, `cargo +stable test -p ralph-slack`, `cargo +stable test -p ralph-cli bot::tests`, and `git diff --check`.
4. Check whether Slack tokens/app config are available without printing secrets; run live smoke only if fully configured.
5. Clean/inspect git status, commit coherent closeout docs, and post Kanban review-required handoff with exact evidence.
