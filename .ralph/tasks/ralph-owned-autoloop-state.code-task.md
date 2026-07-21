---
status: in-progress
created: 2026-07-21
updated: 2026-07-21
bead: ralph-orchestrator-v3-autoloops-backend-a7e.12
spec: .ralph/specs/ralph-owned-autoloop-state.spec.md
---

# Code task: Ralph-owned Autoloop state

## Outcome

Close the independent review findings after `54430971` by making Ralph-launched
Autoloop state single-seam, fail-closed, and private, with complete runtime and
documentation evidence from fake/local providers.

## Recorded implementation baseline

- [x] `a0b51efe` — approved behavior and task recorded before implementation in
      `.ralph/specs/ralph-owned-autoloop-state.spec.md` and this ledger
- [x] `6f61d43b` — engine-state ownership beneath `.ralph/autoloop`
- [x] `e98241bb` — initial contract certification
- [x] `8f0ad021` — documentation/path sweep

These checks record landed commits only; they do not imply that the independent
review findings or final gates are complete.

## Remediation slices

- [x] `10702742` — make `ralph-core::engine_state` the only production layout
      seam; validate run IDs and remove consumer-side `runs/<id>` composition.
- [x] `ba45d9ab` — redact `AutoloopRunner` command rendering and public spawn
      errors; add direct and public red-team tests for prompts, paths, tokens,
      and overrides.
- [x] `9c17aa08` — add distinct runtime integration cases for non-zero process
      exit, malformed events, and missing/empty/absent run IDs, each with
      filesystem and output-privacy assertions.
- [x] `084177b1` — correct fixture capture guidance and distinguish Ralph-owned
      runtime state from standalone Autoloop defaults.
- [x] Rewrite only the local feature history after `54430971` so `a0b51efe`
      precedes every production implementation commit; the post-rewrite tree
      remains `ca21fd61bc0f4e20ce7253159495d137b3e2cc24`.
- [ ] Remove raw engine stdout/stderr from all normal non-zero error, tracing,
      headless, and TUI paths; retain command-display red-team coverage and add
      a behavioral fake-engine secret-output attack test.
- [ ] Run and repair the exact strict gate:
      `cargo clippy --all-targets --all-features -- -D warnings`.
- [ ] Re-run the complete final acceptance matrix, update this ledger with exact
      results/current references, and mark readiness for external review only.
- [ ] Record every new remediation commit before marking its slice complete.

## Required gates

- [ ] Focused privacy and failure-matrix tests
- [ ] `cargo fmt --all --check`
- [ ] `cargo clippy --all-targets --all-features -- -D warnings`
- [ ] Full `cargo test`
- [ ] Relevant native contract tests, non-skipped, against
      `/Users/rook/.herdr/worktrees/autoloop/ralph-owned-state-dir`
- [ ] `git diff --check`
- [ ] Inspect `git diff 54430971...HEAD` for retained state-root behavior,
      both remediation rounds, and no release mutation
- [ ] Confirm approval artifacts precede production implementation with
      `git log 54430971..HEAD --reverse --oneline`
- [ ] Confirm no tracked/untracked transient artifacts except ignored active-loop
      `.autoloop` runtime and launcher preset
- [ ] Independent critic verification of every acceptance criterion, including a
      manual fake/local smoke for non-doc changes
- [ ] New external two-axis review readiness

Earlier builder evidence predates the final acceptance findings and history
rewrite, so it is not accepted as current gate evidence. Record fresh exact
commands and results here only after the remaining remediation is committed and
independently reproducible. External review readiness is not external review
success.

## Constraints and coordination

Use fake/local providers only. Do not authenticate, call paid providers, modify
upstream Autoloop, push, merge, tag, publish, bump versions, or mutate release
metadata. Do not commit loop runtime files. Reconcile likely observation/test
conflicts with pending TUI-history and live-smoke work around the approved core
layout seam; do not merge those branches wholesale.
