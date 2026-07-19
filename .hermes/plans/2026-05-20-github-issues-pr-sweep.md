# Ralph GitHub Issues/PR Sweep — 2026-05-20

## Goal
Clean up live GitHub issue and PR state for `mikeyobrien/ralph-orchestrator`: identify stale/merged/resolved items, take safe close/label/comment actions, and leave a concise status report.

## Guardrails
- Do not close ambiguous active user-facing bugs without clear evidence.
- Prefer comments/labels over destructive closure when uncertain.
- Use GitHub live state, not stale memory, as source of truth.
- If closing, include the reason and reference the resolving PR/issue when available.

## Steps
1. Snapshot open issues, open PRs, labels, and auth/repo state.
2. Classify issues: critical active, stale/resolved, needs info, duplicate/covered-by-PR, docs/cleanup.
3. Classify PRs: mergeable/ready, stale/close, needs rebase/CI, needs review.
4. Apply safe labels/comments/closures.
5. Produce a summary with actions taken and next recommended focus.
