# Ralph GitHub Issues/PR Sweep Report — 2026-05-20

## Scope

Repository: `mikeyobrien/ralph-orchestrator`

Live sweep of open GitHub issues and PRs using `gh`.

## Actions taken

### Issues

- Open issues remain: 22
- Unlabeled open issues after sweep: 0
- Added missing labels across open issues:
  - `bug`: #187, #217, #244, #295
  - `enhancement`: #182, #193, #194, #209, #242, #243, #244, #256, #287, #293, #295
  - `question`: #256, #267
  - `help wanted`: #120, #235
- Added triage comments:
  - #267 — asked for concrete integration surface / minimal first slice.
  - #256 — asked to split large GNAP proposal into first milestone.
  - #194 — asked to split bundled feature into separate issues.
  - #193 — asked to split bundled feature into separate issues.
  - #287 — relabeled as enhancement/performance for MCP-client support rather than core runtime bug.

No issues were closed; none were safely proven resolved from live state alone.

### PRs

- Open PRs before sweep: 9
- Open PRs after sweep: 7
- Unlabeled open PRs after sweep: 0
- Closed stale/conflicting PRs:
  - #246 — stale/conflicting, failing tests; should be revived as fresh scoped branch if still needed.
  - #286 — stale/conflicting docs PR; recreate as small docs PR if still needed.
- Added labels to all open PRs.
- Added triage comments:
  - #307 — draft, failing format/test checks; keep draft until fixed.
  - #301 — draft, failing tests; keep if MCP schema-size work is revived.
  - #264 — stale/conflicting; rebase if still on roadmap, otherwise close as superseded.
  - #212 — stale/conflicting large Agent Waves PR; rebase if still intended direction, otherwise close/supersede.
  - #319 — mergeable bugfix but only fork format check visible; needs maintainer review/full trusted CI.
  - #315 — green and mergeable but title/body describe timeout test fix while diff also includes unrelated goal preset files; split or retitle/scope before merge.
  - #211 — green docs/spec PR; merge candidate after freshness pass against current workflow/preset direction.

## Current PR buckets

### Needs maintainer/full CI review

- #319 — `fix(adapters): include user settings source for Claude auth`

### Green but needs scope/freshness decision

- #315 — green, but mixed scope: timeout flake fix + goal preset files.
- #211 — green docs/spec PR, but 80 days old; needs freshness pass.

### Draft/failing checks

- #307 — context-window utilization telemetry; draft, failing format/test.
- #301 — MCP schema dedup; draft, failing tests.

### Conflicting roadmap decisions

- #264 — idle_timeout with hatless fallback; conflicting.
- #212 — Agent Waves parallel hat execution; conflicting.

## Recommended next focus

1. Decide #315 scope: split the goal preset work away from the timeout-flake fix, or retitle/body to match actual diff.
2. Freshness-review and likely merge #211 if hat-import spec still matches current workflow direction.
3. Pick one critical issue to implement next:
   - #295/#244 for reliable `--continue` state replay/persistence.
   - #287/#301 for MCP schema payload reduction if MCP client support matters.
   - #187/#217 for runtime correctness bugs.
