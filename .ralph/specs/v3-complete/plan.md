# Plan

Strategy only. Runtime tasks own the current step's wave. `progress.md` records
which step is active.

Steps 1 and 2 gate everything after them. Steps 3 through 11 are each
independently committable. Steps 12 through 13 are verification. Step 14 is
terminal.

1. Step 1 - Phase 0 audit, one row per bead with evidence
   - Demo: `.ralph/specs/v3-completion-audit.md` exists, holds one row per bead
     with a verdict and a command plus output excerpt, and is committed.
   - Wave: verify the engine-flip claim, verify the in-house remnant and the
     upstream/drift facts, verify the release gate reference and both branch
     deltas, verify the three open defect beads, then author and commit the
     audit.
   - Gate: Phase 2 does not start until this file is committed.

2. Step 2 - Phase 1 reconcile the divergent v3 branches
   - Demo: a committed decision record classifies every commit in
     `origin/integration/v3-prerelease..origin/wip/v3-prerelease-rollup` as
     `port`, `already-covered`, or `superseded` with a reason, and each wanted
     port is its own commit. Backend unification is confirmed present and any
     backend-adjacent surface builds on `crates/ralph-core/src/backend.rs`.
   - Wave: enumerate and classify the 22-commit rollup delta, port the TUI
     stream history and backpressure fixes, port Ralph-owned autoloop run state
     under `.ralph/autoloop`, then port or justify the live smoke harness
     hardening.
   - Gate: a `superseded` verdict for any TUI commit needs a rendered artifact,
     not an argument.

3. Step 3 - Delete the last in-house remnant (a7e.10)
   - Demo: `crates/ralph-core/src/hat_registry.rs` is gone, or one justified
     caller is named and the type moved to its real owner. Workspace builds.
     Tests pass. Non-autoloop `core.engine` still fails with the v3 message.
   - Wave: map callers, relocate or delete, update `lib.rs`, verify the engine
     rejection path.

4. Step 4 - Certify or delete the wave surface (a7e.8)
   - Demo: `presets/wave-review.yml` runs concurrent branches under autoloop
     with journal evidence, or the Ralph-side wave surface is deleted with a
     migration message naming the autoloop-native replacement.
   - Wave: audit the concurrency/aggregate mapping in
     `autoloop_preset_gen.rs`, decide port or delete, remove or justify the wave
     naming in `crates/ralph-proto/src/json_rpc.rs` and
     `crates/ralph-tui/src/rpc_source.rs`, resolve `ralph wave emit`.

5. Step 5 - Dashboard data source: fix or retire (ga3-c4-dashboard-dead-svf)
   - Demo: either the dashboard renders live autoloop data with a captured
     payload, or v3 marks it non-functional in the README and the dead readers
     are deleted. A reader that silently returns nothing is not acceptable.
   - Wave: prove both readers are severed, choose the honest outcome, implement
     it, verify.

6. Step 6 - Scope the landing auto-commit (landing-untracked-sweep-yxv)
   - Demo: a regression test plants an unrelated untracked file, drives the
     landing auto-commit, and asserts the file is not committed. The test fails
     against the current code before the fix.
   - Wave: write the failing test first, then narrow the commit scope, then
     confirm the RED then GREEN transition.

7. Step 7 - Remove the stale help wave section (tui-help-wave-stale-5hu)
   - Demo: the help overlay has no wave section and no live `w` wave
     keybinding, proven by a widget test or an exercised overlay.
   - Wave: delete the help section, remove the keybinding, add the assertion.

8. Step 8 - Progress display, pi-tidy ethos (Phase 2b)
   - Demo: dropped or partial lines are surfaced and explained, the live view
     stays bounded under load, the active harness is visible per iteration, and
     a 52-56 column render proves priority selection with an unclipped warning.
   - Wave: surface drops with context, bound the live view, show the harness
     per iteration, make header and footer width-priority based, keep the
     headless line Ralph-native.

9. Step 9 - Jev routing parity, no silent drop (Phase 2c.1)
   - Demo: all three translation paths agree. `core.routing.jev` generates a
     working `[routing.jev]` block. No path drops routing silently. Doctor
     validates the credential and the catalog when routing is on.
   - Wave: confirm each path by execution, add the `core.routing.jev` surface,
     emit the block from preset generation, fail closed on untranslatable
     paths, add doctor checks, docs and replay fixtures. Also owns the carried
     config-layer defect `mem-1790103346-2b0f`: `cli.backend` and `cli.args`
     merge as independent keys across config layers, so a user-scope
     `~/.ralph/config.yml` that pairs a backend with its args leaks those args
     onto a project's `cli.backend` override (live CLI exit 1 at `635cb8c`).
     It lands as its own commit, and it needs the same fail-closed message that
     names the layer conflict this step already requires for routing config.

10. Step 10 - Jev-backed completion judge (Phase 2c.2)
    - Demo: the judge lives at an engine-owned gate seam. Approval needs the
      verdict and the noul threshold. Fallbacks record provenance and never
      read as Jev approvals. The journal holds no key, objective, or
      instructions.
    - Wave: prove the seam, implement the producer, record telemetry, add the
      replay fixture matrix, render the gate decision densely.

11. Step 11 - Topology routing surface and feasibility (Phase 2c.3)
    - Demo: a ralph surface distinguishes workflow from topology selection.
      Enabling topology mode with self-routing hats refuses to start and names
      the hat and field. Triggers still route when the mode is off. Either an
      implementation at a proven seam, or a filed upstream issue with the seam
      table.
    - Wave: record the seam verdicts with citations, branch on the verdict,
      implement the surface and validation, write the docs and tests.

12. Step 12 - Live verification under the engine (Phase 3.1-3.5)
    - Demo: `ralph doctor` reports engine resolution. A scratch-repo
      `ralph run -H builtin:code-assist` shows engine resolution, the live event
      stream, the journal under `.ralph/autoloop`, no top-level `.autoloop`, and
      the completion judgment. Two concurrent loops show registry, journal,
      summary, and landing. `ralph resume` and `ralph run --rpc` still work.
      0.11.0 drift on budgets, backend keys, or stopReason is fixed or recorded.
    - Wave: provision and confirm 0.11.0, run doctor, run the live loop, run
      parallel loops, exercise resume and RPC, check drift. Also lands the
      carried test-hygiene rename from the Step 2 gate repair: the test at
      `crates/ralph-cli/src/completion_coord.rs:191` is named
      `coordinate_without_context_writes_nothing_and_does_not_panic` but asserts
      only the panic half, so drop the unasserted absence claim from the name.

13. Step 13 - TUI parity inspection with rendered cells (Phase 3.7)
    - Demo: `cargo test -p ralph-tui` passes, `validate_widgets` writes real
      cells, an autoloop fixture proves prior-iteration history and tool calls
      render, the harness and role render per iteration, and the drop,
      narrow-width, and `[WAVE]` cases are covered.
    - Wave: run the widget example and capture the buffer, add the
      autoloop-fixture snapshot case, verify harness and role, add the remaining
      cases.

14. Step 14 - GA gate, close the epic, final suites, push
    - Demo: `.ralph/specs/v3-ga-readiness.spec.md` exists and is current, `a7e`
      is closed in the tracker, the final suites pass, clippy is clean on
      touched crates, and `v3/complete` is pushed with `main` untouched.
    - Wave: author the gate spec, close the epic with the tracker update, run
      the suites, push.

## Notes

- Do not create a future step's wave early. Only the current step's runtime
  tasks may be open.
- Carried findings carry a step owner instead of a runtime row while their step
  is not current. `mem-1790103346-2b0f` (config-layer `cli.args` onto a
  `cli.backend` override, live exit 1) belongs to Step 9. The
  `completion_coord.rs:191` test-name overclaim belongs to Step 12. Step 4 owns
  DEC-039's three dead `SessionRecorder` bus methods (`Record::from_bus_event`,
  `SessionRecorder::record_bus_event`, `SessionRecorder::make_observer`, zero
  references outside `crates/ralph-core/src/session_recorder.rs`), because Step 4
  is the step that decides port-or-delete over the `ralph-proto`/TUI surface it
  already rewrites. Step 14 owns rewriting
  `.ralph/specs/v3-autoloops-cutover.spec.md:81`, `:227`, and `:238`, which still
  claim `hat_registry.rs` and `event_bus.rs` were KEPT against the shipped tree.
  Step 12 also owns the unguarded production spawn of the engine at
  `crates/ralph-adapters/src/autoloop_runner.rs:292` (`run_control`) and `:358`
  (`spawn`), which has no `ExecutableFileBusy` retry while the `ralph-cli`
  integration harness (`crates/ralph-cli/tests/integration_autoloop_failure_reporting.rs:86-92`)
  puts a fixture-written `bin/autoloop` first on `PATH`, so the same ETXTBSY
  class is reachable there too; it is carried rather than queued because its
  rate is unmeasured and its repair is a production-seam change, and Step 12 is
  the live engine verification step that exercises that spawn.
  Each is materialized as a runtime task when its step becomes the current step.
- A step that surfaces a fix lands the fix as its own commit before the step
  closes.
- `.ralph/specs/v3-completion-audit.md` and `.ralph/specs/v3-ga-readiness.spec.md`
  are deliverables and are committed. This planning directory and
  `.ralph/agent/*` are runtime files and are not committed.
