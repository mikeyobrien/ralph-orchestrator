---
status: in-progress
created: 2026-07-21
updated: 2026-07-21
bead: ralph-orchestrator-v3-autoloops-backend-a7e.12
spec: .ralph/specs/ralph-owned-autoloop-state.spec.md
---

# Code task: Ralph-owned Autoloop state

## Outcome

Close the independent review of `54430971...a9a3c19d` by making Ralph-launched
Autoloop state single-seam, fail-closed, and private, with complete runtime and
documentation evidence from fake/local providers.

## Recorded implementation baseline

- [x] `8c08ed16` — engine-state ownership beneath `.ralph/autoloop`
- [x] `9b747027` — initial contract certification
- [x] `a9a3c19d` — documentation/path sweep
- [x] Approved behavior recorded in
      `.ralph/specs/ralph-owned-autoloop-state.spec.md`

These checks record landed commits only; they do not imply that the independent
review findings or final gates are complete.

## Remediation slices

- [ ] Make `ralph-core::engine_state` the only production layout seam; validate
      run IDs and remove consumer-side `runs/<id>` composition.
- [ ] Redact `AutoloopRunner` command rendering and public spawn errors; add
      direct and public red-team tests for prompts, paths, tokens, and overrides.
- [ ] Add distinct runtime integration cases for non-zero process exit,
      malformed events, and missing/empty/absent run IDs, each with filesystem
      and output-privacy assertions.
- [ ] Correct fixture capture guidance and distinguish Ralph-owned runtime state
      from standalone Autoloop defaults across adjacent user-facing docs.
- [ ] Record each remediation commit and only then mark its slice complete.

## Required gates

- [ ] Focused tests for every changed surface
- [ ] `cargo fmt --check`
- [ ] Strict clippy with warnings denied for all touched crates/targets
- [ ] Full `cargo test`
- [ ] Relevant native contract tests, non-skipped, against
      `/Users/rook/.herdr/worktrees/autoloop/ralph-owned-state-dir`
- [ ] `git diff --check`
- [ ] Inspect `git diff 54430971...HEAD`
- [ ] Confirm no tracked/untracked transient artifacts except ignored active-loop
      `.autoloop` runtime and launcher preset
- [ ] Independent critic verification of every acceptance criterion, including a
      manual fake/local smoke for non-doc changes
- [ ] New external two-axis review readiness

## Constraints and coordination

Use fake/local providers only. Do not authenticate, call paid providers, modify
upstream Autoloop, push, merge, tag, publish, bump versions, or mutate release
metadata. Do not commit loop runtime files. Reconcile likely observation/test
conflicts with pending TUI-history and live-smoke work around the approved core
layout seam; do not merge those branches wholesale.
