# t_c7ca7f7a — Slack Block Kit renderer

## Contract
Add a Slack renderer seam for polished start/progress/final/status/help messages, preserve plain text fallback and existing post-thread behavior, and extend state with optional message timestamps while keeping old JSON readable.

## Steps
1. Inspect current ralph-slack API/state/service/daemon tests and choose the narrowest test seam.
2. Add failing tests first for renderer Block Kit shape, API block payload posting, and old/new state serialization behavior.
3. Implement minimal renderer structs/helpers, API optional blocks payload support, and optional timestamp fields.
4. Run `cargo +stable fmt --all --check`, `cargo +stable test -p ralph-slack`, `git diff --check`, then commit coherent changes.

## Guardrails
- No secrets in logs or fixtures.
- Preserve Telegram behavior by only touching `crates/ralph-slack` unless compilation requires exports.
- Do not modify pre-existing untracked plan/setup artifacts unrelated to this card.
