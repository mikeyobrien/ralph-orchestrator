# Task: Complete Ralph 3.0. autoloop is the engine, Ralph is the TUI shell.

Authored 2026-09-22. Base branch `integration/v3-prerelease` (tip `22fc1fd`).
Work branch `v3/complete`.

## Charter

autoloop owns loop execution, topology and role dispatch, completion judgment,
budgets, and agent subprocesses. Ralph owns the TUI, the observation and
coordination plane, the loop registry, the merge queue, parallel worktree loops,
doctor, and the operator surfaces. The in-house engine is not coming back.

## Ground truth at authoring time

Do not trust the bead titles or the specs. Both are stale. These claims were
verified on 2026-09-22 against the base branch and the upstream repos, and each
one changes what work remains.

| Claim | Evidence | Consequence |
|---|---|---|
| The in-house engine is already unreachable. | `crates/ralph-core/src/config.rs:2188` rejects any `core.engine` other than `autoloop`. Test `core_engine_rejects_removed_ralph_engine` at `config.rs:2434`. Zero callers of `run_loop_impl` or `EventLoop::new` remain in `crates/`. | The engine flip has landed. Do not re-do it. |
| One in-house remnant survives. | `crates/ralph-core/src/hat_registry.rs` exists and is exported from `crates/ralph-core/src/lib.rs`. `event_loop/`, `hatless_ralph.rs`, `event_bus`, `wave_*` are gone from the tree. | Bead a7e.10 reduces to one module, not a 21K line deletion. |
| Slice 8's blocking condition has cleared. | autoloop issues #34, #35, #37, #38, #39 are CLOSED. PRs #40, #41, #42 are MERGED. autoloop v0.11.0 released 2026-09-10. | The cutover spec's "blocked on upstream" narrative is obsolete. Waves and resume parity are now actionable. |
| The cutover spec's release gate does not exist. | `.ralph/specs/v3-autoloops-cutover.spec.md` sets `release_gate: .ralph/specs/v3-ga-readiness.spec.md`. That path is absent from `integration/v3-prerelease`, `wip/v3-prerelease-rollup`, and `main`. | The named gate must be authored or the reference corrected. |
| The two v3 branches are divergent, not stacked. | `integration/v3-prerelease` has 8 commits the rollup lacks: `#368` rpc + native resume, `#367` telegram stop/restart, `#366`, `#365`, `#345` Telegram HITL relay, `#346` ralph-bench on the engine, plus two clippy commits. `wip/v3-prerelease-rollup` has 22 the base lacks, verified with `git rev-list --count`, dominated by TUI stream history and backpressure bounding, Ralph-owned autoloop state, and live smoke harness hardening. | A completion that ignores the rollup ships a worse TUI. Reconcile explicitly. |
| No autoloop engine is installed on this machine. | `autoloop` is absent from PATH. `/opt/autoloop` does not exist. `/opt/autoloop-exporter` is a metrics exporter and unrelated. npm serves `@mobrienv/autoloop` 0.11.0. | A live verification run needs engine provisioning first. |
| Ralph v3 was authored against autoloop 0.10.x. | Comments at `crates/ralph-cli/src/autoloop_preset_gen.rs:158` and `:212` say 0.10.x. Latest published is 0.11.0. | Version drift is unverified. Treat a mapping break as yours to fix. |
| The beads tracker is partly stale. | `.beads/issues.jsonl` on the base branch holds 43 issues: 36 closed, 6 open, 1 tombstone. The 6 open are `a7e`, `a7e.8`, `a7e.10`, `ga3-c4-dashboard-dead-svf`, `landing-untracked-sweep-yxv`, `tui-help-wave-stale-5hu`. | Bead status is a hypothesis. The audit decides. |

**Measurement trap.** Check `git rev-parse --is-shallow-repository` before quoting
any divergence count. A shallow clone hides commits below the graft boundary and
reports divergence that does not exist, and it makes `git merge` refuse with
"unrelated histories". An earlier measurement on a shallow clone claimed 94
v3-only commits against main and 4 main-only. The true figures after
`git fetch --unshallow` are **179 v3-only and 9 main-only**. Quote the command
next to any count you rely on.

## Handoff state (2026-09-22, after main was merged)

This branch already contains main. Do not re-merge it.

| Fact | Evidence |
|---|---|
| `v3/complete` is a merge commit with parents `22fc1fd` (v3) and `351b9f6` (main). | `git log -1 --format="parents: %p"` |
| 13 conflicts were resolved by hand. Kept from main: the unified backend catalog, the three CLI UX fixes, OMP, and the hermetic `events_override` threading. Kept from v3: deletion of the in-house engine and its tests, engine validation, and autoloop health checks. Dropped: main's wave re-exports and wave auto-tagging. | the merge commit message |
| `cargo check -p ralph-core -p ralph-cli` is clean. | run it |
| Tests are 411 passed, 2 failed. One failure is a timing-sensitive robot relay test that passes on re-run. The other is `test_auto_preflight_skip_list_can_omit_hooks_check_failures`, which **also fails on the pre-merge commit `22fc1fd`**, because v3's preflight includes an autoloop engine health check. | verified by re-running both, and by running the second against `22fc1fd` in a throwaway worktree |
| Treat those two as the baseline. Do not chase them as regressions, and do not "fix" the flaky one by weakening its assertion. | |
| The clone is no longer shallow. True divergence from main: 179 v3-only, 9 main-only. | `git rev-parse --is-shallow-repository` returns false |
| `cargo`, `cargo-clippy`, and `rustc` are on `PATH` for non-interactive shells via symlinks in `~/.local/bin`. | `bash -c 'cargo --version'` |
| The autoloop engine is installed: 0.11.0, on `PATH`. | `autoloop --version` |
| `npm install -g` did not run one native build script for `@homebridge/node-pty-prebuilt-multiarch`. If a PTY-backed harness fails, allow it with `npm install -g --allow-scripts=@homebridge/node-pty-prebuilt-multiarch`. | the install output |
| Git push to `origin` authenticates, and a repo-local identity (`Mikey O'Brien <hmobrienv@gmail.com>`) is set, so commits and pushes work. | `git push --dry-run origin v3/complete` |
| No beads CLI exists. Hand-edit `.beads/issues.jsonl` with its schema preserved. | `command -v bd` returns nothing |
| `just` is not installed. Use `cargo` directly or `./scripts/ci-rust-gate.sh` instead of `just ci`. | `command -v just` returns nothing |
| The GA gate the cutover spec names is still absent. `.ralph/specs/` holds `v3-autoloops-cutover.spec.md`, `feature-parity.spec.md`, `v1-v2-feature-parity.spec.md`, and `v2-engine-autoloop-gap-analysis.md`, but no `v3-ga-readiness.spec.md`. | `ls .ralph/specs/` |
| A previous run of this prompt stalled after planning Step 1 and produced no commits. Its notes are preserved at `.ralph/specs/v3-complete.stalled-2026-09-22/`. Read them for context, but re-verify anything they claim. | that directory |
| A working Pushover hook exists for the in-house engine and is verified end to end: `~/.ralph/hooks/pushover-notify.mjs`, wired in `~/.ralph/config.yml` on `post.loop.complete` and `post.loop.error`. It accepts both the Ralph hook payload and the autoloop notify payload. Under the engine it does not fire today, which is the hook-parity item in Phase 2. | `~/.ralph/logs/pushover-hook.log` |
| A live-updates and steering contract was added to this prompt after the live run captured its plan, so its MUSTs bind the next run. The channels work on the live run today: the task store is file-based and externally writable, and the scratchpad is re-injected each iteration. Measured: the live run emitted zero interact events in its first hour. | the run's events file, grepped |

## Phase 0. Audit before any code

No bead is assumed open or closed. Every verdict carries a command and an output
excerpt. A claim without evidence is a failed audit.

Write `.ralph/specs/v3-completion-audit.md` with one row per bead and this shape.

| Bead | Tracker claim | Verdict | Evidence | Remaining work |
|---|---|---|---|---|

Verdicts are `landed`, `stale-open`, `genuinely-open`, or `blocked`. For
`landed`, name the commit or the passing test. For `genuinely-open`, name the
file and the failing behavior. For `blocked`, name the exact external blocker and
the URL.

Phase 2 does not start until that file exists and is committed.

Phase 0 also includes one comms smoke: send a single
`ralph tools interact progress` line before any code work. A channel that is
discovered broken at hour six was broken at minute one.

## Phase 1. Reconcile the divergent v3 branches

Enumerate both deltas. For every commit in
`origin/integration/v3-prerelease..origin/wip/v3-prerelease-rollup`, decide
`port`, `already-covered`, or `superseded`, with a reason. The rollup's TUI
stream history and backpressure fixes are the ones most likely to be wanted. The
`Merge WIP Ralph-owned Autoloop state` work matters twice over, because the
engine must keep run state under `.ralph/autoloop` and must not create a
top-level `.autoloop` in the workspace.

Deliverable: a decision record committed alongside the ports, each port its own
commit. If a wanted fix cannot be ported, say so and name the obstacle.

The TUI commits in the rollup are the ones the Phase 3 TUI parity inspection
guards. A `superseded` verdict for any of them needs a rendered artifact as its
evidence, not an argument that the base already looks fine.

This branch also predates main's backend unification. `origin/main` carries
`crates/ralph-core/src/backend.rs`, one authoritative catalog that replaced what
the OMP commit described as "at least three divergent backend lists across
core/adapters/CLI", alongside the OMP backend itself. This branch has none of
it, and still matches on backend ids in `crates/ralph-adapters/src/cli_backend.rs`
and `crates/ralph-cli/src/backend_support.rs`. Any backend-adjacent surface work,
including the Jev configuration below, must build on the unified catalog. Port it,
or the new work repeats the divergence that unification just removed.

## Phase 2. Bead work

Only what Phase 0 proves is missing. Each item is independently committable.

### a7e.10. Delete the last in-house remnant.

Remove `crates/ralph-core/src/hat_registry.rs` when no production caller
remains. If a type in it is still used by engine-facing code, move that type to
its real owner and delete the module.

Acceptance. The module is gone, or a justified single caller is named. Workspace
builds. Tests pass. Any `core.engine` value other than `autoloop` still fails
with the v3 message.

### a7e.8. Certify waves under the autoloop parallel model.

The upstream blocker (#35) is closed. Prove `presets/wave-review.yml`
scatter-gathers under the autoloop engine with a real run, and show the
concurrent branches in the journal. If Ralph's per-hat `concurrency` and
`aggregate` do not map onto autoloop's `<event>.parallel` and `maxBranches`, then
either implement the mapping in `crates/ralph-cli/src/autoloop_preset_gen.rs` or
delete the Ralph-side wave surface and document the autoloop-native way to do it.

Dead surface to remove or justify: `crates/ralph-proto/src/json_rpc.rs` and
`crates/ralph-tui/src/rpc_source.rs` still carry wave naming.

Acceptance. A wave preset runs concurrent branches under autoloop, evidenced by
the journal. No wave naming survives in a live code path. `ralph wave emit`
either works or is rejected with a migration message that names the replacement.

### ga3-c4-dashboard-dead-svf. Fix or retire the dashboard data source.

`ralph-api`'s event_watcher reads `.ralph/events.jsonl` through a current-events
marker, and `EventLogger` has zero production callers. The Node backend parses a
stdout JSONL format that no longer exists. The autoloop path writes only
`.ralph/autoloop-events.ndjson`.

Acceptance. One of two honest outcomes. Either the dashboard renders live data
from the autoloop event stream, proven by a run and a screenshot or captured
payload, or the dashboard is marked non-functional for v3 in the README and the
dead readers are deleted. A reader that silently returns nothing is not an
acceptable end state.

### landing-untracked-sweep-yxv. Scope the landing auto-commit.

`crates/ralph-core/src/landing.rs` committed an untracked operator file
(`ralph.fable-sol.yml`) that merely sat in the worktree, which then collided on
merge. The commit must cover only paths the loop touched, or an allowlist, or
respect the ignore rules.

Acceptance. A regression test plants an untracked, unrelated file in the
worktree, drives the landing auto-commit path, and asserts the file is not
committed. The test fails against the current code before the fix.

### tui-help-wave-stale-5hu. Remove the stale help section.

`crates/ralph-tui/src/widgets/help.rs` still lists a Wave Workers section.

Acceptance. The help overlay has no wave section, and no `w` wave keybinding is
live in input handling, proven by exercising the overlay or a widget test.

### Hook parity under the engine.

Ralph's lifecycle hooks do not fire under the autoloop engine, and the migration
guide does not say so. Evidence, all read from source:

- `crates/ralph-cli/src/autoloop_engine.rs` contains no hook references.
- The hook engine is invoked only from the legacy path in `main.rs`
  (`hooks::execute` at line 1069).
- `main.rs:1701-1706` states that the legacy in-house engine paths are descoped
  in v3 and autoloop is the sole runtime.
- `docs/migration/v3-autoloop-engine.md` mentions hooks nowhere.

Consequence: an operator with configured hooks gets silence under v3. A working
example exists and must keep working: `~/.ralph/config.yml` sends a Pushover
notification on `post.loop.complete` and `post.loop.error` through
`~/.ralph/hooks/pushover-notify.mjs`. That script already accepts both payload
contracts, so it is the regression check for this item.

Fix: translate Ralph's configured hooks into the engine's finish-notification
config when generating a preset.

- Engine contract: `notify.command` (a shell command string), `notify.on` (CSV of
  `completed`, `failed`, `stopped`; default `completed,failed`), and
  `notify.timeout_ms` (default 10000). The command receives `AUTOLOOP_RUN_ID`,
  `AUTOLOOP_STOP_REASON`, `AUTOLOOP_ITERATIONS`, `AUTOLOOP_PRESET`, and
  `AUTOLOOP_PROJECT_DIR` in its environment, plus JSON on stdin as
  `{run_id, stop_reason, iterations, preset, project_dir}`. Delivery is
  best-effort and journaled as `notify.sent` or `notify.failed`.
- Mapping: `post.loop.complete` maps to the `completed` class, and
  `post.loop.error` maps to `failed`.
- Payload compatibility: Ralph's hook payload (`loop`, `iteration`, `context`)
  differs from the engine's (`run_id`, `stop_reason`, ...). Existing hook scripts
  were written against Ralph's shape. Do not silently change what they receive.
  Either wrap the command so the user's hook still gets Ralph's payload, or fail
  loudly with a migration message. Record the decision and its reasoning.
- Unsupported events: `pre.*` hooks, mutating hooks, and any event with no engine
  equivalent must refuse to start and name the event. Silently ignoring them is
  the defect this item exists to remove.
- Documentation: add hooks to the "What breaks" table in
  `docs/migration/v3-autoloop-engine.md`.
- Tests: a generated preset carries `notify.command` and the right `notify.on`
  classes when hooks are configured; it carries no `notify` block when they are
  not; and each unsupported event produces a refuse-to-start error naming it.

Acceptance: with a hook on `post.loop.complete`, a real run through the engine
delivers the notification and the journal shows `notify.sent`. With a hook on
`post.loop.error` and a failing run, `notify.on` includes `failed` and the
notification fires. A configured `pre.*` hook refuses to start. The migration
guide lists the change.

### a7e. Close the epic.

Close it only when every child is terminal and the GA gate below is present and
current.

Update `.beads/issues.jsonl` as work lands, preserving its schema, and keep the
tracker update in the same commit as the change it describes. If a beads CLI is
available, use it instead of hand editing.

## Phase 2b. Progress output and display

Ralph is the shell, so progress is the product. Everything the operator knows
arrives through the TUI or the headless line. This phase fixes how progress is
shown. It adds no new panels and no new screens.

### The ethos

Follow the pi-tidy ethos, which exists for exactly this problem. Its own
framing: long agent turns are hard to read, the goal behind each action is
invisible, and delegated work has no compact live view. The answers:

1. **Dense and reason-first.** One or two lines carry the goal, the concrete
target, and the useful result.
2. **Priority-based semantic items, selected by render width.** Build each item
   with full, compact, and minimal forms and choose by the actual `render(width)`.
   Never assemble one long string and truncate the tail.
3. **Warnings replace lower-priority content, never get clipped.** A clipped
   warning is a lie by omission.
4. **Progressive disclosure.** Answer the four live questions first: where, what
   is running, how close to the limits, what needs attention. Historical
   counters and decoration wait for wider widths.
5. **Preserve native behavior.** Restyling output must not change execution
   semantics.

### Fixes required

- **Surface dropped progress instead of swallowing it.**
  `crates/ralph-adapters/src/autoloop_event_tailer.rs` skips malformed and
  partial lines by design (see the notes at lines 52, 80, 130) and
  `autoloop_events.rs:140` does the same. The July run showed the operator a
  bare `500161 bytes skipped` with no context and lost tool calls. A drop count
  the operator cannot see, or cannot interpret, fails the ethos. Reported drops
  must name what was dropped and whether history is intact.
- **Bound the live view under pressure.** The rollup carries the history and
  backpressure fixes (`bound stream identities and lifecycle lines`, `protect
  reconciled history under line pressure`, `preserve bounded TUI stream
  history`). Port them, and prove the bounding with a render under load. The
  base branch lacks them, so a dropped port is invisible to every existing test.
- **Show the harness per iteration.** The header renders
  `[iter n/total]` and the role, and `crates/ralph-tui/src/widgets/header.rs`
  carries `[LIVE]`, `[REVIEW]`, and `[WAVE]` mode labels. Confirm the active
  harness (claude-sdk, pi, acp, command) is visible where the operator needs it,
  and that no `[WAVE]` label path survives once waves are gone.
- **Make width behavior priority-based.** The header and footer must select item
  forms by width, not truncate completed strings. Add a narrow-width case at 52
  to 56 columns, matching the pi-tidy-footer research, and assert that a warning
  displaces lower-priority content rather than being clipped.
- **Keep the headless voice dense and Ralph-native.** The headless progress line
  follows the same ethos and must not become a raw engine passthrough dump.

### Verification

Extend the Phase 3 TUI parity inspection with these cases, and show rendered
cells for each:

- a fixture containing a dropped or partial line, proving the drop is surfaced
  and explained rather than silently counted
- a 52 to 56 column render proving priority selection and unclipped warnings
- an assertion that no `[WAVE]` label path remains reachable
- rendered-output assertions, not merely that a render function ran. Per the
  pi-tidy quality gates, a test that executes rendering without noticing changed
  output does not count as evidence.

## Phase 2c. Jev support, first class

Jev is TypeSafe's System One decision model. It is not a coding-agent harness. It
does not stream text, call tools, or edit files, and TypeSafe's own docs state
there is no setting that turns a coding agent into a Jev-powered agent. It takes
state plus typed questions and returns a `choice` with per-option probabilities,
a `score` on a rubric, and a `noul` for a true or false statement.

Therefore: **do not add a Jev backend to the harness catalog.** A catalog entry
means argv construction and stream parsing for a process that ralph spawns, and
Jev is none of that. Adding one is the category error to avoid.

Reference material: `docs/reference/jev-routing.md` and
`docs/reference/topology.md#evidence-gates` in the autoloop repo, and
`https://docs.typesafe.ai/llms.txt` for the API, primitives, and confidence
semantics.

### 2c.1 Routing parity, with no silent drop

autoloop already ships `[routing.jev]` in a preset: `enabled`, `routes_file`,
`model` (default `jev-1.13.0`), `min_confidence` (default `0.8`), `timeout_ms`.
It selects a workflow from a preset-owned catalog, injects only the selected
local instructions, and is off by default and fail-closed when on. Missing
credentials, an invalid catalog, a provider failure, a timeout, a malformed
answer, `no_match`, and confidence below threshold all stop the run before the
backend starts. There is no fallback and no shadow mode.

Ralph's three translation paths do not currently agree on that block.

| Path | Expected today |
|---|---|
| `core.autoloop_preset` | the preset reaches the engine directly, so the block should survive |
| `-H <toml preset>` | the overlay filter keeps only hats, events, and event_loop, so the block is dropped |
| generated from ralph hats | there is no ralph config surface for it, so it cannot be expressed |

Confirm each path by execution before changing anything, and record the command
and result. The third row is read from the source comment at
`crates/ralph-core/src/preset_source.rs:384` and is not yet confirmed at runtime.

Then implement:

- A ralph-native surface, `core.routing.jev`, carrying `enabled`, `routes_file`,
  `model`, `min_confidence`, and `timeout_ms`.
- Preset generation emits `[routing.jev]` from that surface. An explicit
  `core.autoloop_preset` stays authoritative and is never rewritten.
- Ralph never silently drops routing configuration. If a translation path cannot
  carry it, fail before starting the backend with a message that names the path
  and the fix. A fail-closed engine behind a silently-dropping layer is the worst
  of both, because the operator believes routing is on.
- `ralph doctor` checks, only when routing is enabled: `TYPESAFE_API_KEY` is
  present in the environment, and `routes_file` resolves relative to the preset
  directory, parses as a JSON array, holds 1 to 64 routes, uses the documented id
  pattern, reserves nothing that collides with `no_match`, and has nonempty
  descriptions and instructions.
- Docs: the v3 migration guide, the configuration reference, and one worked
  example preset with a `routes.json`. State that `TYPESAFE_API_KEY` belongs in
  the harness process environment and never in TOML, a route catalog, or an
  argument string.
- Tests: replay fixtures only. No live provider call in CI, matching the
  fake-autoloop substrate and autoloop's own no-production-model posture.

### 2c.2 A Jev-backed completion judge

One architecture rule controls this. Completion judgment belongs to the engine in
v3. Ralph configures the gate and supplies the judgment. Ralph does not fork,
wrap, or shadow the engine's completion decision.

Implement at an engine-owned seam, in this order of preference:

1. A typed evidence gate on the completion event, per
   `topology.md#evidence-gates`, satisfied by a Jev judgment producer. The engine
   still requires the proof; Jev produces it.
2. The completion-gate store override from autoloop #36, if a store entry is the
   right shape.
3. A phase hook, given the lifecycle-hook engine from #38 and the
   `pre_iteration` mutation point that `routing.jev` already uses.

If none of the three can express it, stop and record an autoloop issue with the
evidence. Do not hack around the engine, and do not reintroduce a ralph-side loop
to hold the gate.

Judgment semantics, modeled on the working pi-goal-x auditor:

- a `noul` for `completion_verified`, with a threshold constant that has a name
  and a default
- a `choice` verdict over a closed set, at minimum approved, rejected, and
  needs_more
- approve only when the verdict is approved **and** the noul clears the threshold
- record the deciding values, the model id, and the provenance of every decision
- on provider failure, fall back to the deterministic marker decision and record
  that provenance explicitly, in the same spirit as pi-goal-x recording
  `decided via report marker fallback`. A fallback must never present itself as a
  Jev approval.
- fail closed when the gate is enabled and the judgment cannot be obtained. Do
  not convert an unjudged run into a passed run.

Credential and telemetry discipline, matching `routing.jev`:

- `TYPESAFE_API_KEY` comes from the process environment only. Never in TOML, code,
  an argument, a commit, or a log line.
- The journal record must not contain the key, the objective text, or gate
  instructions, mirroring what `routing.jev.selected` records.
- The endpoint is fixed and redirects are rejected.

Progress display: a gate decision is a progress event. Render it densely per
Phase 2b. A gate that stops the run must state which condition failed and the
observed value. Never print a raw provider dump.

Verification for both halves:

- replay fixtures covering approved, rejected, needs_more, low confidence,
  `no_match`, timeout, malformed answer, missing credential, and invalid catalog
- assertion that a disabled gate reads no catalog and requires no credential
- assertion that the recorded telemetry contains no key, objective, or
  instructions
- one opt-in live smoke, never part of CI
- the journal shown for a real gate stop, and for a real gate pass

### 2c.3 Topology routing moves to Jev

Requirement: when Jev routing is enabled in topology mode, intra-loop role routing
is Jev's decision. Roles stop being the routing authority, so hats stop declaring
triggers and publishes as a routing mechanism, and the engine's successor choice
comes from a Jev decision instead.

Read this constraint before writing code. `routing.jev` as shipped selects a
workflow only, and its own documentation states it "does not select agents or
models, change topology, or replace completion gates." Even the dynamic-chain
layer states that agent-emitted `chain.spawn` events pass through "without
affecting topology routing." So there is no shipped engine seam that lets Jev
choose the next role.

**Step 1, feasibility, mandatory and evidence-first.** For each candidate seam,
record a verdict with a citation, source path, or doc anchor:

- a phase hook with I/O mutation, from the lifecycle-hook engine in autoloop #38
- a typed evidence gate placed on the successor decision
- the completion-gate store override from autoloop #36
- the dynamic-chain `chain.spawn` path
- any other engine-owned surface that can influence which role runs next

A seam only counts if the engine's own journal and event record stay truthful
about what ran. Any approach that makes the recorded role disagree with the role
that actually executed is a hack, not a seam, and is forbidden here.

**Step 2, branch on the verdict.**

- If a seam exists, implement at that seam. The engine keeps ownership of the
decision point and Jev supplies the judgment.
- If no seam exists, do not implement it and do not approximate it. Write the
  upstream issue or RFC against autoloop with the seam table as its evidence, and
  link it. Ralph does not fork topology to get this feature early.

**Step 3, ralph-side surface, implementable regardless of the seam verdict.**

- A ralph config surface that distinguishes workflow selection from topology
  selection, so an operator states which authority they are enabling.
- Refuse-to-start validation. When topology mode is enabled and a hat declares
  triggers or publishes as a routing authority, fail before the backend starts
  and name the hat and the offending field. This matches the router's own
  fail-closed posture: a run configured for Jev routing must not quietly keep
  routing through hats.
- The validation must not fire when topology mode is off, and must not fire in
  workflow mode, where triggers and publishes are legitimate and load-bearing.
- Docs: describe the three layers (topology, chains, dynamic chains) and state
  which one Jev owns in each mode, plus the explicit statement that ralph does
  not fork topology.
- Tests: the refuse-to-start cases with the message asserted, and a case proving
  triggers still route correctly when topology mode is off.

Completion for this piece is the surface, the validation, the docs, and the
tests, plus **either** an implementation at a proven seam **or** the filed
upstream issue. It is never a guessed implementation. v3 completion does not
block on the upstream feature landing.

## Phase 3. Verification

Artifact level, not test-suite level. Show the command and the output.

1. Engine provisioning. `npm install -g @mobrienv/autoloop` (0.11.0), or
   `RALPH_AUTO_INSTALL_ENGINE=1 ralph run`. Then `ralph doctor` reports engine
   resolution.
2. Live end-to-end. In a scratch git repo, run `ralph run -H builtin:code-assist
   -p "<small real task>"`. Show engine resolution, the live event stream, the
   journal under `.ralph/autoloop`, no top-level `.autoloop`, and the completion
   judgment.
3. Parallel loops and merge queue. Two concurrent loops. Show registry, journal,
   summary, and landing behavior.
4. Resume and RPC. `ralph resume` and `ralph run --rpc` still work under the
   engine. These landed on the base branch and must not regress.
5. Version drift. If 0.11.0 breaks a mapping that 0.10.x accepted (budgets,
   backend keys, stopReason), fix it in Ralph and record the drift.
6. Suites. `cargo test -p ralph-core -p ralph-cli` passes. `cargo clippy` is
   clean on touched crates.
7. TUI parity inspection. Ralph is the shell now, so the UI is a parity
   surface, not a nicety. Inspect it, do not infer it from passing state
   snapshots.
   - Run `cargo test -p ralph-tui`, then
     `cargo run -p ralph-tui --example validate_widgets`. The example renders
     the header and footer through ratatui `TestBackend` and writes the real
     cell buffer, so include the rendered output.
   - Add a snapshot case driven by an autoloop event fixture that proves history
     and tool calls from prior iterations render. This is the July failure
     (`bytes skipped`, tool calls missing from earlier iterations) and nothing
     in the tree covers it today.
   - Verify the header and footer name the active harness and role for each
     iteration. That was an open UX gap and the shell is where it gets answered.
   - A state-snapshot pass alone is not UI verification, for the same reason a
     green unit test is not artifact verification. Show the rendered cells.
8. Hook delivery under the engine. Configure a hook, run a real loop through the
   engine, and show both the delivered notification and the journal's
   `notify.sent` record. Also show one unsupported hook event refusing to start.
9. GA gate. Author `.ralph/specs/v3-ga-readiness.spec.md` with the acceptance
   checklist and current statuses, or correct the cutover spec's reference. The
   cutover spec calls this the canonical release gate, so an absent file is a
   broken gate.

## Live updates and steering (operator contract)

A standing contract for every phase, not a sequential step.

Why this is a MUST: the live run on this branch emitted zero `interact`
events in its first hour despite the workflow's SHOULD. A SHOULD that is
ignored is a silent channel. Measured by grepping the run's events file.

### Updates the run owes the operator

Send `ralph tools interact progress` with one dense, reason-first line, the
Phase 2b ethos, at each of these triggers.

- every commit, naming the sha and what landed
- every phase transition
- any blocker, failed gate, or task moved to `fail`
- any decision recorded below 80 confidence, surfaced as an ask rather than
  a file entry
- a heartbeat at least every 15 minutes of wall time when nothing above fired

Phase 0 includes a comms smoke: one progress line before any code work, so a
missing RObot configuration surfaces at minute one rather than hour six.
Loop-end notification already exists outside this prompt through the
Pushover hook. This contract covers the middle of the run.

### Steering channels, in precedence order

1. **Task injection, the primary channel.** The operator runs
   `ralph tools task add "Operator steer: ..." -p 1` in this worktree from
   any shell. The store is file-based and externally writable, tasks carry
   the loop id, and the run re-reads ready tasks every iteration. Treat any
   priority-1 task whose title starts `Operator steer:` as binding for the
   next iteration. Acknowledge it in the scratchpad, and in `decisions.md`
   if it changes a consequential decision.
2. **Scratchpad append.** The operator appends a dated `OPERATOR STEER:`
   block to `.ralph/agent/scratchpad.md`, which is injected next iteration.
   Same binding treatment.
3. **Memory injection.** `ralph tools memory add -t context` for durable
   guidance and conventions, not one-off orders.
4. **Hard control.** Telegram `/stop` and `/restart` on this branch's wiring,
   or kill the process and `ralph resume`.

### Precedence and limits

- Operator steering overrides the plan, the phase order, and task
  priorities. It does not override Guardrails: no main merge, no force push,
  secrets discipline, deterministic completion.
- Editing this prompt file mid-run is not a steering channel. An in-flight
  run has captured its objective. Use the channels above.

### Honesty about the engine

The Telegram HITL relay under the autoloop engine rides the wiring merged in
this branch. Phase 3 must include one live ask round trip as proof, not just
config presence.

## Guardrails

### Work in rigor mode.

The entry point is `disable-model-invocation`, so read it from disk explicitly.

Base: `~/.pi/agent/git/github.com/icedrop-lab/pi-pstack/skills/`

1. `rigor-mode/SKILL.md`, the router. Read it in full first.
2. `rigor-mode/playbooks/bug-fix.md` for the three defects, and
   `rigor-mode/playbooks/refactoring.md` for the engine-remnant and wave work.
3. `principle-*/SKILL.md`, read in full before citing.

Principles that carry weight here. `principle-prove-it-works`, because every
bead in this list is a claim about runtime behavior. `principle-fix-root-causes`,
because the dashboard and the landing sweep are both wrong-source bugs.
`principle-laziness-protocol`, because most of this prompt is deletion.
`principle-subtract-before-you-add`, because the wave surface should shrink
before it grows. `principle-separate-before-serializing-shared-state`, because
run state ownership is the substance of the reconciliation.

Adaptations. The hat you are in owns the work. Do not spawn subagents to
implement. Skip the playbook's PR step. Delivery is this branch.

### Boundaries

- Do not merge to `main`. Do not force push. Land on `v3/complete` and push it.
- Do not reintroduce an in-house engine escape hatch. No `core.engine = "ralph"`.
- Do not create a top-level `.autoloop` in a workspace. Engine state belongs
  under `.ralph/`.
- One coherent change per commit. Each commit stands alone and references its
  bead id.
- Every claim in the audit, the decision record, and the final report carries
  its evidence in the same sentence. Label anything unmeasured as inferred.
- Secrets discipline: `TYPESAFE_API_KEY` and any other provider credential stay
  out of TOML, code, arguments, commits, logs, journal records, and test
  fixtures. Reference the environment variable by name only.
- Commit messages and documents follow `unslop` and `technical-writing`. Short
  declarative sentences. No em dashes.

## Done

All of the following, then print `LOOP_COMPLETE`.

- `.ralph/specs/v3-completion-audit.md` exists with per-bead verdicts and
  evidence.
- The two v3 branches are reconciled, with a committed decision record.
- Every one of the 6 beads is terminal in the tracker, or explicitly `blocked`
  with a named external blocker and URL.
- The in-house remnant is gone. Waves are certified under autoloop, or the dead
  surface is deleted with a migration message.
- The dashboard is live under v3 or honestly retired.
- The landing auto-commit test exists and fails without the fix.
- The TUI help overlay is scrubbed.
- Lifecycle hooks fire under the engine, or refuse to start with a named reason.
  The Pushover hook in `~/.ralph/config.yml` is the regression check, and the
  migration guide documents the change.
- TUI parity is inspected, not inferred. `cargo test -p ralph-tui` passes, the
  widget render is shown, and an autoloop fixture proves prior-iteration history
  and tool calls reach the screen.
- Progress display meets the pi-tidy ethos. Dropped events are surfaced and
  explained, the live view stays bounded under load, the active harness is
  visible per iteration, and a 52 to 56 column render proves priority selection
  with no clipped warning.
- Jev routing parity holds on all three translation paths, `core.routing.jev`
  generates a working `[routing.jev]` block, and no path silently drops routing
  configuration. `ralph doctor` validates the credential and the route catalog
  when routing is enabled.
- The Jev-backed judge lives at an engine-owned gate seam, not in a ralph-side
  loop. Approvals require the threshold and the verdict. Fallbacks record their
  provenance and never read as Jev approvals. The journal holds no key, no
  objective, and no instructions.
- Topology mode is either implemented at a proven engine seam or filed upstream,
  never approximated. Enabling it with self-routing hats refuses to start and
  names the conflict, and triggers still route normally when the mode is off.
- A live autoloop-backed run is verified end to end, with the journal shown.
- `cargo test -p ralph-core -p ralph-cli` passes and clippy is clean on touched
  crates.
- The GA gate spec exists and is current.
- `v3/complete` is pushed. `main` is untouched.
- The Phase 0 comms smoke ran, and every update trigger above fired when it
  applied, or the channel's absence is recorded with the reason.
