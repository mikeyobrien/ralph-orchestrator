# Plan

Strategy only. Runtime tasks own the current step's wave. `progress.md` records
which step is active.

1. Step 1 - Phase 0 audit, evidence per bead
   - Demo: `.ralph/specs/v3-completion-audit.md` exists, has one row per bead
     with a verdict and a command plus output excerpt, and is committed.
   - Wave: verify the engine-flip claim, verify the in-house remnant and
     upstream/drift facts, verify the release gate and both branch deltas,
     verify the three open defect beads, then author and commit the audit.
   - Gate: Phase 2 does not start until this file is committed.

2. Step 2 - Phase 1 reconcile the divergent v3 branches
   - Demo: a committed decision record classifies every commit in
     `origin/integration/v3-prerelease..origin/wip/v3-prerelease-rollup` as
     `port`, `already-covered`, or `superseded` with a reason, and each wanted
     port is its own commit.
   - Wave: enumerate and classify the rollup delta, port TUI stream history and
     backpressure fixes, port Ralph-owned autoloop run state, then port or
     justify the live smoke harness hardening.
   - Gate: a `superseded` verdict for any TUI commit needs a rendered artifact.

3. Step 3 - Delete the last in-house remnant (a7e.10)
   - Demo: `crates/ralph-core/src/hat_registry.rs` is gone, or one justified
     caller is named and the type moved to its real owner. Workspace builds and
     tests pass.
   - Wave: map callers, relocate or delete, update `lib.rs`, verify
     `core.engine` still rejects non-autoloop.

4. Step 4 - Certify or delete the wave surface (a7e.8)
   - Demo: `presets/wave-review.yml` runs concurrent branches under autoloop
     with journal evidence, or the Ralph-side wave surface is deleted with a
     migration message naming the autoloop-native replacement.
   - Wave: audit the concurrency/aggregate mapping in `autoloop_preset_gen.rs`,
     decide port-or-delete, remove or justify the wave naming in
     `crates/ralph-proto/src/json_rpc.rs` and `crates/ralph-tui/src/rpc_source.rs`,
     resolve `ralph wave emit`.

5. Step 5 - Dashboard data source: fix or retire (ga3-c4-dashboard-dead-svf)
   - Demo: either the dashboard renders live autoloop data with a captured
     payload, or v3 marks it non-functional in the README and the dead readers
     are deleted.
   - Wave: prove both readers are severed, choose the honest outcome, implement
     it, verify.

6. Step 6 - Scope the landing auto-commit (landing-untracked-sweep-yxv)
   - Demo: a regression test plants an unrelated untracked file, drives the
     landing auto-commit, and asserts the file is not committed. The test fails
     against the current code.
   - Wave: write the failing test first, then narrow the commit scope, then
     confirm the RED then GREEN transition.

7. Step 7 - Remove the stale help wave section (tui-help-wave-stale-5hu)
   - Demo: the help overlay has no wave section and no live `w` wave keybinding,
     proven by a widget test or an exercised overlay.
   - Wave: delete the help section, remove the keybinding, add the assertion.

8. Step 8 - Progress display, pi-tidy ethos (Phase 2b)
   - Demo: dropped or partial lines are surfaced and explained, the live view
     stays bounded under load, the active harness is visible per iteration, and a
     52-56 column render proves priority selection with an unclipped warning.
   - Wave: surface drops with context, bound the live view, show the harness
     per iteration, make header and footer width-priority based, keep the
     headless line Ralph-native.

9. Step 9 - Engine provisioning and live end-to-end run
   - Demo: `ralph doctor` reports engine resolution and a scratch-repo
     `ralph run -H builtin:code-assist` shows engine resolution, the live event
     stream, the journal under `.ralph/autoloop`, no top-level `.autoloop`, and
     the completion judgment.
   - Wave: provision 0.11.0, verify doctor, run the live loop, record the
     journal, check version drift on budgets, backend keys, and stopReason.

10. Step 10 - Parallel loops, merge queue, resume and RPC
    - Demo: two concurrent loops show registry, journal, summary, and landing
      behavior; `ralph resume` and `ralph run --rpc` still work.
    - Wave: run two loops, capture registry and merge evidence, exercise resume
      and RPC.

11. Step 11 - TUI parity inspection with rendered cells
    - Demo: `cargo test -p ralph-tui` passes, the widget example writes real
      cells, and an autoloop event fixture proves prior-iteration history and
      tool calls render.
    - Wave: run the widget example and capture the buffer, add the
      autoloop-fixture snapshot case, verify the harness and role render per
      iteration, add the drop, narrow-width, and `[WAVE]` cases.

12. Step 12 - GA gate, close the epic, final suites, push
    - Demo: `.ralph/specs/v3-ga-readiness.spec.md` exists and is current, `a7e`
      is closed in the tracker, `cargo test -p ralph-core -p ralph-cli` passes,
      clippy is clean on touched crates, and `v3/complete` is pushed with `main`
      untouched.
    - Wave: author the gate spec, close the epic with tracker update, run the
      suites, push.

## Notes

- Steps 1 and 2 gate everything after them.
- Steps 3 through 8 are each independently committable.
- Steps 9 through 11 are verification steps. They may surface fixes, which land
  as their own commits before the step closes.
- Step 12 is terminal. The Finalizer terminates after it.
