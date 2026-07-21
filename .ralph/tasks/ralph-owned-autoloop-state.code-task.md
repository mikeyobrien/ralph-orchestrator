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
- [x] `c1fb58bd` — remove raw engine stdout/stderr from normal non-zero error,
      tracing, headless, and TUI paths; add behavioral fake-engine secret-output
      attacks; reject malformed successful streams and symlink escapes; retain
      safe diagnostic categories.
- [x] Run and repair the strict workspace gate:
      `cargo clippy --workspace --all-targets --all-features -- -D warnings`.
- [x] Re-run the implementation validation matrix and record exact current
      evidence below. External review remains a separate pending gate.
- [x] Record every new remediation commit before marking its slice complete.

## Independent review blocker remediation

- [x] `6aa02b6` — reject every pre-existing descendant symlink without following
      it; validate exact owned summary stores and symlink leaves; suppress
      untrusted terminal stop details; preserve daemon dependency ordering; and
      redact parallel-worktree paths and cleanup diagnostics.
- [x] `626d1926` + `554cda5` — replace unknown stop reasons with a fixed category,
      remove active prompts and residual worktree/scratchpad paths from runtime
      diagnostics, and cover both parallel lock-contention branches plus a real
      fake-process stop-reason attack.
- [x] Reject any malformed nonblank structured-event record before trusting a
      successful process and summary; cover the exact fake-process case.
- [x] Redact raw child stderr and physical preset/workspace/state paths from
      public failures and tracing while retaining safe categories and actions.
- [x] Canonicalize the workspace and reject symlinks in both owned-root path
      components before state creation; cover external-target non-mutation.
- [x] Correct fake journal precedence documentation.
- [x] Prove a generated Ralph preset (without an explicit-preset override) keeps
      events, journal, summary paths, and run streams beneath the owned root.

## Required gates

- [x] Second-remediation focused matrix: core engine-state 10 passed; CLI engine
      unit tests 27 passed; failure-reporting fake-process attacks 12 passed;
      affected headless/TUI/merge fake-process suites 5 passed.
- [x] `cargo fmt --all --check`
- [x] `cargo clippy --workspace --all-targets --all-features -- -D warnings`
- [x] Full `cargo test`: 2730 passed, 37 ignored.
- [x] Native contracts against
      `/Users/rook/.herdr/worktrees/autoloop/ralph-owned-state-dir`: 3 passed,
      non-skipped.
- [x] `git diff --check`
- [x] Inspect `git diff 54430971...HEAD`: state-root behavior and both
      remediation rounds retained; no release metadata changed.
- [x] Approval artifacts precede production implementation in
      `git log 54430971..HEAD --reverse --oneline`.
- [x] Worktree contains no transient tracked/untracked runtime artifacts.
- [ ] Independent critic verification of every acceptance criterion, including a
      manual fake/local smoke for non-doc changes
- [x] New external two-axis review readiness

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
