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

- [x] `f70b95f8` — make `ralph-core::engine_state` the only production layout
      seam; validate run IDs and remove consumer-side `runs/<id>` composition.
- [x] `0314695b` — redact `AutoloopRunner` command rendering and public spawn
      errors; add direct and public red-team tests for prompts, paths, tokens,
      and overrides.
- [x] `6de20edf` — add distinct runtime integration cases for non-zero process
      exit, malformed events, and missing/empty/absent run IDs, each with
      filesystem and output-privacy assertions.
- [x] `01204572` — correct fixture capture guidance and distinguish Ralph-owned
      runtime state from standalone Autoloop defaults.
- [x] Record every remediation commit before marking its slice complete.

## Required gates

- [x] Focused tests for every changed surface
- [x] `cargo fmt --all --check`
- [x] Strict clippy with warnings denied for all touched crates/targets
- [x] Full `cargo test`
- [x] Relevant native contract tests, non-skipped, against
      `/Users/rook/.herdr/worktrees/autoloop/ralph-owned-state-dir`
- [x] `git diff --check`
- [x] Inspect `git diff 54430971...HEAD`
- [x] Confirm no tracked/untracked transient artifacts except ignored active-loop
      `.autoloop` runtime and launcher preset
- [ ] Independent critic verification of every acceptance criterion, including a
      manual fake/local smoke for non-doc changes
- [ ] New external two-axis review readiness

Builder gates passed on 2026-07-21 using fake/local providers only. The native
contract suite passed 3 tests and the parity suite passed 1 test with no `skip:`
output. Full `cargo test` completed successfully; loop-owned logs remain under
`.autoloop/runs/meta-agent/logs/` and are not tracked.

## Constraints and coordination

Use fake/local providers only. Do not authenticate, call paid providers, modify
upstream Autoloop, push, merge, tag, publish, bump versions, or mutate release
metadata. Do not commit loop runtime files. Reconcile likely observation/test
conflicts with pending TUI-history and live-smoke work around the approved core
layout seam; do not merge those branches wholesale.
