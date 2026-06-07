# Ralph runbooks

Operational checklists for reviewing and smoking Ralph features. These are meant to be followed by a human operator, not by the loop itself.

## Slack thread RObot surface

Use these in order for `feat/slack-thread-surface`:

1. [Slack final verification review](slack-final-verification-review.md) — local branch, code, docs, tests, and security review.
2. [Slack live smoke test](slack-live-smoke-test.md) — dedicated Slack app setup and one real Slack thread-as-loop smoke.
3. [Slack review signoff template](slack-review-signoff-template.md) — fill this out when closing the final review/Kanban/PR.

Safety defaults:

- Never paste Slack token values into logs, commits, Slack, or review comments.
- Keep real tokens in environment variables or a local secret store only.
- Do not check in a config file containing real `xoxb-`, `xapp-`, or signing-secret values.
- Treat Slack as an operator control plane: only allow explicit channels and explicit users.
