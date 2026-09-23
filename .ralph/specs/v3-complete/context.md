# Context: Complete Ralph 3.0 (autoloop is the engine, Ralph is the TUI shell)

## Source

Rough description. The objective is the Phase 0-3 brief in the run prompt
(also saved to `PROMPT-V3-COMPLETE.md`). There is no PDD directory and no
`.code-task.md` file. This directory (`.ralph/specs/v3-complete/`) is the
working directory.

## Original request summary

Finish Ralph 3.0. autoloop owns loop execution, topology and role dispatch,
completion judgment, budgets, and agent subprocesses. Ralph owns the TUI, the
observation and coordination plane, the loop registry, the merge queue,
parallel worktree loops, doctor, and the operator surfaces. The in-house engine
is not coming back.

Work branch `v3/complete`, based on `integration/v3-prerelease` tip `22fc1fd`
and already merged with `main` at `351b9f6`.

## Verified repo state at plan time (re-measured 2026-09-22, this turn)

| Fact | Evidence |
|---|---|
| HEAD is the merge of v3 and main | `git log -1 --format='%H %p'` = `df2449aaab15b3f2812369c784bfcc1c0642ed86 22fc1fd 351b9f6` |
| Clone is not shallow | `git rev-parse --is-shallow-repository` = `false` |
| Rollup delta is 22 commits | `git rev-list --count origin/integration/v3-prerelease..origin/wip/v3-prerelease-rollup` = `22` |
| Base delta is 8 commits | `git rev-list --count origin/wip/v3-prerelease-rollup..origin/integration/v3-prerelease` = `8` |
| `.ralph/specs/v3-completion-audit.md` does not exist | `test -e` = ABSENT |
| `.ralph/specs/v3-ga-readiness.spec.md` does not exist | `test -e` = ABSENT |
| `crates/ralph-core/src/backend.rs` is present | `test -e` = PRESENT (the unified catalog survived the merge) |
| `.ralph/specs/v3-autoloops-cutover.spec.md` is present | `test -e` = PRESENT |
| `crates/ralph-core/src/hat_registry.rs` is present | `test -e` = PRESENT (the last in-house remnant) |
| `crates/ralph-core/src/landing.rs` is present | `test -e` = PRESENT |
| `crates/ralph-tui/src/widgets/help.rs` is present | `test -e` = PRESENT |
| `crates/ralph-tui/src/widgets/header.rs` is present | `test -e` = PRESENT |
| `crates/ralph-adapters/src/autoloop_event_tailer.rs` is present | `test -e` = PRESENT |
| `crates/ralph-cli/src/autoloop_preset_gen.rs` is present | `test -e` = PRESENT |
| `presets/wave-review.yml` is present | `test -e` = PRESENT |
| The autoloop engine is installed | `command -v autoloop` = `/home/mobrienv/.npm-global/bin/autoloop` |
| No beads CLI exists | `command -v bd` is empty; hand-edit `.beads/issues.jsonl` |
| CI entry point is a `Justfile` (capital J); `just` binary is absent | `ls Justfile`; use `cargo` or `./scripts/ci-rust-gate.sh` |

Correction to the stalled run's notes at
`.ralph/specs/v3-complete.stalled-2026-09-22/`: it claimed a 20-commit rollup
delta. The measured value is 22, which matches the brief. Use the enumerated
list, not a remembered count.

## Six non-closed beads in `.beads/issues.jsonl`

- `ralph-orchestrator-v3-autoloops-backend-a7e` (feature, epic)
- `ralph-orchestrator-v3-autoloops-backend-a7e.8` (feature, waves)
- `ralph-orchestrator-v3-autoloops-backend-a7e.10` (task, delete in-house engine)
- `ralph-orchestrator-ga3-c4-dashboard-dead-svf` (bug, dashboard data source)
- `ralph-orchestrator-landing-untracked-sweep-yxv` (bug, landing auto-commit)
- `ralph-orchestrator-tui-help-wave-stale-5hu` (bug, help overlay)

Bead status is a hypothesis. Phase 0 decides.

## Repo patterns and integration points

- Crates: `ralph-cli` (CLI/TUI frontend, autoloop launch, completion, merge),
  `ralph-core` (config and coordination state), `ralph-adapters` (autoloop
  process integration and journal/event/summary contracts), `ralph-tui`
  (ratatui), `ralph-telegram`, `ralph-proto`, `ralph-bench`, `ralph-e2e`.
- Engine driver: `crates/ralph-cli/src/autoloop_engine.rs`.
- Preset generation: `crates/ralph-cli/src/autoloop_preset_gen.rs`.
- Event ingestion: `crates/ralph-adapters/src/autoloop_events.rs` and
  `autoloop_event_tailer.rs`.
- Engine state lives under `.ralph/autoloop`. A top-level `.autoloop` in a
  workspace is forbidden.
- Beads tracker is a JSONL file, `.beads/issues.jsonl`. No CLI. Hand-edit and
  preserve the schema. Keep the tracker update in the same commit as the change
  it describes.
- Documents and commit messages follow `unslop` and `technical-writing`: short
  declarative sentences, no em dashes.
- Verification gate: `cargo test -p ralph-core -p ralph-cli` plus clippy on
  touched crates. Full `cargo test --all` only at the final step.

## Acceptance criteria

The brief's `## Done` list is the acceptance criteria. Summarized:

1. `.ralph/specs/v3-completion-audit.md` exists with per-bead verdicts and
   evidence, committed before any Phase 2 code.
2. Both v3 branches reconciled with a committed decision record and per-port
   commits.
3. Every one of the 6 beads terminal in the tracker, or `blocked` with a named
   external blocker and URL.
4. In-house remnant gone. Waves certified under autoloop, or the dead surface
   deleted with a migration message.
5. Dashboard live under v3 or honestly retired, dead readers deleted.
6. Landing auto-commit regression test exists and fails against current code.
7. TUI help overlay scrubbed of the wave section.
8. TUI parity inspected with rendered cells, plus an autoloop fixture proving
   prior-iteration history and tool calls reach the screen.
9. Progress display meets the pi-tidy ethos: drops surfaced and explained,
   bounded live view under load, harness visible per iteration, 52-56 column
   render proving priority selection with no clipped warning.
10. Jev routing parity on all three translation paths, `core.routing.jev`
    generates a working `[routing.jev]` block, doctor validates credential and
    catalog.
11. Jev-backed completion judge at an engine-owned gate seam, threshold plus
    verdict required, provenance recorded, fail closed.
12. Topology mode implemented at a proven seam or filed upstream, never
    approximated. Refuse-to-start validation when the mode is on.
13. A live autoloop-backed run verified end to end, journal shown.
14. `cargo test -p ralph-core -p ralph-cli` passes, clippy clean on touched
    crates.
15. `.ralph/specs/v3-ga-readiness.spec.md` exists and is current.
16. `v3/complete` pushed. `main` untouched.

## Constraints and boundaries

- Do not merge to `main`. Do not force push. Land on `v3/complete`.
- Do not reintroduce an in-house engine escape hatch. `core.engine = "ralph"`
  stays rejected.
- Do not create a top-level `.autoloop` in a workspace.
- One coherent change per commit. Each commit references its bead id.
- Every claim in the audit, decision record, and final report carries evidence
  in the same sentence. Label unmeasured claims as inferred.
- Acceptance tests must be real. No placeholder, pending, or stub scenarios.
- BDD acceptance checks must exercise runtime code paths.
- Never commit Ralph runtime files (`.ralph/agent/*`, tasks, scratchpad).
- `TYPESAFE_API_KEY` and any provider credential stay out of TOML, code,
  arguments, commits, logs, journal records, and fixtures. Name the environment
  variable only.
- Do not add a Jev backend to the harness catalog. Jev is a decision model, not
  a coding-agent harness.
- Work in rigor mode. Read `rigor-mode/SKILL.md` and the bug-fix and
  refactoring playbooks from
  `~/.pi/agent/git/github.com/icedrop-lab/pi-pstack/skills/` before the
  corresponding work. Cite `principle-*/SKILL.md` in full before citing it.
- Do not spawn subagents to implement. The active hat owns its work.
