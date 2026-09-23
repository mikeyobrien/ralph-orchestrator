# Context: Complete Ralph 3.0 (autoloop is the engine, Ralph is the TUI shell)

## Source

Rough description. The objective is the Phase 0-3 brief in the run prompt,
authored 2026-09-22. There is no PDD directory and no `.code-task.md` file.
This directory (`.ralph/specs/v3-complete/`) is the working directory.

## Original request summary

Finish Ralph 3.0. autoloop owns loop execution, topology and role dispatch,
completion judgment, budgets, and agent subprocesses. Ralph owns the TUI, the
observation and coordination plane, the loop registry, the merge queue,
parallel worktree loops, doctor, and the operator surfaces. The in-house engine
is not coming back.

Work branch `v3/complete`, based on `integration/v3-prerelease` tip `22fc1fd`.

## Verified repo state at plan time

| Fact | Evidence |
|---|---|
| `v3/complete` equals `origin/integration/v3-prerelease` tip | `git branch --show-current` = `v3/complete`; `git log --oneline -1` = `22fc1fd` |
| The rollup delta is 20 commits, not 14 | `git log --oneline origin/integration/v3-prerelease..origin/wip/v3-prerelease-rollup` |
| The base delta is 8 commits | `git log --oneline origin/wip/v3-prerelease-rollup..origin/integration/v3-prerelease` |
| Every key file named in the brief exists | `.ralph/specs/v3-autoloops-cutover.spec.md`, `.beads/issues.jsonl`, `crates/ralph-core/src/hat_registry.rs`, `crates/ralph-core/src/landing.rs`, `crates/ralph-tui/src/widgets/help.rs`, `crates/ralph-tui/src/widgets/header.rs`, `crates/ralph-adapters/src/autoloop_event_tailer.rs`, `crates/ralph-cli/src/autoloop_preset_gen.rs`, `presets/wave-review.yml` |
| `.ralph/specs/v3-completion-audit.md` does not exist | `ls` returns "No such file or directory" |
| `.ralph/specs/v3-ga-readiness.spec.md` does not exist | `ls` returns "No such file or directory" |
| No autoloop engine is installed | `command -v autoloop` empty |
| npm has autoloop 0.11.0 | `npm view @mobrienv/autoloop version` = `0.11.0` |
| No beads CLI is available | `command -v bd` and `command -v beads` both empty; hand-edit `.beads/issues.jsonl` |
| CI entry point is a `Justfile`, not `justfile` | `ls Justfile` |

Six non-closed beads in `.beads/issues.jsonl` (one is a tombstone):

- `ralph-orchestrator-v3-autoloops-backend-a7e` (feature, epic)
- `ralph-orchestrator-v3-autoloops-backend-a7e.8` (feature, waves)
- `ralph-orchestrator-v3-autoloops-backend-a7e.10` (task, delete in-house engine)
- `ralph-orchestrator-ga3-c4-dashboard-dead-svf` (bug, dashboard data source)
- `ralph-orchestrator-landing-untracked-sweep-yxv` (bug, landing auto-commit)
- `ralph-orchestrator-tui-help-wave-stale-5hu` (bug, help overlay)

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
  preserve the schema.
- Documents and commit messages follow `unslop` and `technical-writing`: short
  declarative sentences, no em dashes.
- Verification gate: `just ci` (fmt, clippy, tests). Focused work uses
  `cargo test -p <crate>`. Full `cargo test --all` only at the final step.

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
10. A live autoloop-backed run verified end to end, journal shown.
11. `cargo test -p ralph-core -p ralph-cli` passes, clippy clean on touched
    crates.
12. `.ralph/specs/v3-ga-readiness.spec.md` exists and is current.
13. `v3/complete` pushed. `main` untouched.

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
- Work in rigor mode. Read `rigor-mode/SKILL.md` and the bug-fix and
  refactoring playbooks from
  `~/.pi/agent/git/github.com/icedrop-lab/pi-pstack/skills/` before the
  corresponding work. Cite `principle-*/SKILL.md` in full before citing it.
