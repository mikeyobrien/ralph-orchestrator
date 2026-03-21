# Native PRP Queue Implementation Plan

> REQUIRED SUB-SKILL: Use `superpowers:using-git-worktrees` before implementing this plan.
>
> REQUIRED SUB-SKILL: Use `superpowers:executing-plans` to implement this plan task-by-task.

**Goal:** Add a first-class PRP queue to Ralph that imports PRP markdown files, runs PRPs serially, and blocks queue advancement until the current PRP is merged into a shared integration branch.

**Scope:** Add a native PRP queue/state model, CLI command surface, import/archive bookkeeping, implementation worktree execution, shared integration worktree execution, recovery flows, and tests/docs. Exclude web UI support, replacement of the generic loop merge queue, and automatic promotion from `integration` to `main`.

**Architecture / Approach:** Build a dedicated event-sourced PRP queue in `ralph-core`, then add a `ralph prps` CLI namespace in `ralph-cli` that owns import, listing, processing, recovery, and discard flows. Reuse existing loop registry, diagnostics, handoff, and `ralph run` subprocess orchestration, but do not model PRPs as web queued tasks or as generic merge-queue entries. Extend the git worktree helper so PRP processing can use deterministic `prp/<id>` implementation branches and one shared `integration` branch/worktree.

**Tech Stack:** Rust (`ralph-core`, `ralph-cli`), clap CLI parsing, existing git worktree helpers, loop registry, diagnostics, event history, and `cargo test`.

**Dependencies / Constraints:**
- Use repo-local `.worktrees/` for implementation work.
- Keep queue state canonical in `.ralph/prp-queue.jsonl`; do not rely on `PRPs/.prp_status.json`.
- Keep PRP markdown files as import/archive artifacts linked from queue entries.
- Implementation and integration behavior must continue to be driven by project config files (`ralph.yml`, `ralph-landing.yml` or configured equivalents).
- The queue must not dequeue the next PRP until the current one is integrated into the shared integration branch.
- Non-PRP loop, merge-queue, and web-dispatcher behavior must remain unchanged.

**Definition of Done:**
- `ralph prps import`, `list`, `show`, `process`, `resume`, `retry`, and `discard` exist and work against a durable PRP queue.
- PRP processing uses dedicated implementation worktrees and a shared integration worktree on the configured integration branch.
- A PRP is not considered complete until it is integrated into the shared integration branch and archived to the completed PRP directory.
- Queue advancement is blocked on failed or incomplete integration.
- Automated verification passes with new unit and integration coverage for import, state transitions, recovery, and queue ordering.

## Mandatory Execution Setup

Implementation must happen in an isolated worktree created from `/home/coe/scroll/agent-orchestrator`, not in the main checkout.

```bash
cd /home/coe/scroll/agent-orchestrator
git rev-parse --show-toplevel
git branch --show-current
git status --short
git check-ignore -q .worktrees/
mkdir -p .worktrees
git worktree add -b feat/prp-queue-native .worktrees/feat-prp-queue-native
cd /home/coe/scroll/agent-orchestrator/.worktrees/feat-prp-queue-native
cargo build
cargo test
```

If the baseline `cargo test` fails, record the exact failing command/output before making code changes. All later commands and edits must run from the worktree path only.

## Checklist

- [ ] Step 0: Add worktree-scoped red tests for PRP queue behavior
- [ ] Step 1: Add PRP config, queue state model, and markdown import/archive helpers in `ralph-core`
- [ ] Step 2: Extend worktree helpers for deterministic PRP and integration worktrees
- [ ] Step 3: Add `ralph prps import/list/show`
- [ ] Step 4: Add implementation-phase processing and readiness reconciliation
- [ ] Step 5: Add shared integration-phase processing and queue blocking semantics
- [ ] Step 6: Add `resume/retry/discard` operator flows and documentation
- [ ] Step 7: Drive verification to green and validate the end-to-end queue flow

---

## Step 0: Add worktree-scoped red tests for PRP queue behavior

**Objective:** Create failing CLI-level tests that lock in the required queue semantics before implementation.

**Files:**
- Create: `crates/ralph-cli/tests/integration_prps.rs`
- Review: `crates/ralph-cli/tests/integration_loops_merge.rs`
- Review: `crates/ralph-core/src/merge_queue.rs`

**Implementation guidance:**
- Follow the existing CLI integration-test style used for `loops` and merge-queue coverage.
- Build temp git repositories inside the tests with:
  - `PRPs/remaining_work/PRP-001.md`
  - `PRPs/remaining_work/PRP-002.md`
  - minimal `ralph.yml` and `ralph-landing.yml`
- Add red tests for:
  - import is idempotent
  - queue ordering is FIFO by PRP id/import order
  - `process` does not start PRP-002 before PRP-001 reaches `integrated`
  - implementation-ready but non-integrated PRPs still block the queue
  - failed integration moves the item to `needs_review`
- Use fake `ralph` executables or fixture subprocesses where needed so tests stay deterministic.

**Test requirements:**
- The new test file compiles and runs red for unimplemented PRP behavior.
- Existing loop/merge tests still pass unchanged.

**Validation:**
- Run: `cargo test -p ralph-cli integration_prps -- --nocapture`
- Expect: new PRP tests fail for the intended missing behavior, existing CLI tests still compile.

**Checkpoint:** `test: add failing integration_prps coverage`

---

## Step 1: Add PRP config, queue state model, and markdown import/archive helpers in `ralph-core`

**Objective:** Create the canonical PRP queue state model and the file-system helpers that link queue entries to PRP markdown artifacts.

**Files:**
- Modify: `crates/ralph-core/src/config.rs`
- Modify: `crates/ralph-core/src/lib.rs`
- Create: `crates/ralph-core/src/prp_queue.rs`
- Create: `crates/ralph-core/src/prp_files.rs`

**Implementation guidance:**
- Add a new `prps` config section to `RalphConfig` with these defaults:
  - `import_dir: "./PRPs/remaining_work"`
  - `completed_dir: "./PRPs/completed"`
  - `queue_file: ".ralph/prp-queue.jsonl"`
  - `implementation_config: "ralph.yml"`
  - `integration_config: "ralph-landing.yml"`
  - `integration_branch: "integration"`
  - `integration_worktree: ".worktrees/integration"`
- In `prp_queue.rs`, implement an append-only JSONL event store modeled after `MergeQueue`.
- Use a concrete PRP state machine:
  - `queued`
  - `implementing`
  - `ready_for_integration`
  - `integrating`
  - `integrated`
  - `needs_review`
  - `discarded`
- Store enough metadata in the materialized entry to resume deterministically:
  - `prp_id`
  - `title`
  - `source_path`
  - `state`
  - `last_phase`
  - `implementation_branch`
  - `implementation_worktree`
  - `implementation_pid`
  - `integration_branch`
  - `integration_worktree`
  - `integration_pid`
  - `integration_commit`
  - `failure_reason`
- In `prp_files.rs`, implement:
  - scan/import of `PRP-*.md` from `import_dir`
  - PRP id/title extraction from markdown
  - DoD checkbox completion detection
  - archive move from `import_dir` to `completed_dir`
- Queue state is canonical; markdown files are artifacts only.
- Do not read or write `PRPs/.prp_status.json`.

**Test requirements:**
- Unit tests for queue event replay, legal/illegal state transitions, and idempotent import behavior.
- Unit tests for DoD checkbox parsing and archive moves.
- Config parse/serde tests for the new `prps` section.

**Validation:**
- Run: `cargo test -p ralph-core prp_queue`
- Run: `cargo test -p ralph-core prp_files`
- Expect: green unit coverage for queue persistence, import, and archive logic.

**Checkpoint:** `feat(core): add canonical PRP queue and file helpers`

---

## Step 2: Extend worktree helpers for deterministic PRP and integration worktrees

**Objective:** Reuse the existing git-worktree utility layer for PRP-specific branch/worktree naming instead of duplicating shell-style git orchestration.

**Files:**
- Modify: `crates/ralph-core/src/worktree.rs`
- Modify: `crates/ralph-core/src/lib.rs`

**Implementation guidance:**
- Add a new helper that can create a named worktree with explicit:
  - branch name
  - worktree path
  - base ref
- Keep the current `create_worktree()` behavior unchanged for existing parallel-loop flows.
- Use the new helper for:
  - implementation worktrees: branch `prp/<PRP-ID>`
  - shared integration worktree: branch from `config.prps.integration_branch`
- Preserve current sync behavior for untracked and unstaged files.
- Add helper(s) to read the current branch for an existing worktree and to remove named PRP worktrees without assuming the `ralph/` branch prefix.

**Test requirements:**
- Unit tests for creating named worktrees and removing them cleanly.
- Regression tests proving existing `ralph/*` worktree behavior is unchanged.

**Validation:**
- Run: `cargo test -p ralph-core worktree`
- Expect: green coverage for both generic loop worktrees and PRP-specific worktrees.

**Checkpoint:** `feat(core): extend worktree helpers for PRP branches`

---

## Step 3: Add `ralph prps import/list/show`

**Objective:** Add the operator-facing CLI surface for loading PRPs into the canonical queue and inspecting queue state.

**Files:**
- Create: `crates/ralph-cli/src/prps.rs`
- Modify: `crates/ralph-cli/src/main.rs`

**Implementation guidance:**
- Register a new top-level CLI namespace: `Prps(prps::PrpsArgs)`.
- Implement these subcommands first:
  - `ralph prps import`
  - `ralph prps list`
  - `ralph prps show <prp-id>`
- `import` should scan `config.prps.import_dir` and upsert queue entries by PRP id.
- `list` should show queue order, current state, phase, and failure reason when present.
- `show` should print the fully materialized entry, including linked worktree paths and archive location.
- Support human-readable and JSON output, mirroring existing CLI patterns where practical.

**Test requirements:**
- CLI tests for:
  - empty import dir
  - first import
  - repeated import without duplicate queue entries
  - list output for queued and in-progress items
  - show output for a specific PRP

**Validation:**
- Run: `cargo test -p ralph-cli integration_prps -- --nocapture`
- Expect: import/list/show tests move green while process-path tests remain red.

**Checkpoint:** `feat(cli): add initial ralph prps command surface`

---

## Step 4: Add implementation-phase processing and readiness reconciliation

**Objective:** Teach `ralph prps process` how to run exactly one queued PRP in its implementation worktree and promote it to `ready_for_integration`.

**Files:**
- Modify: `crates/ralph-cli/src/prps.rs`
- Review: `crates/ralph-cli/src/loop_runner.rs`
- Review: `crates/ralph-core/src/landing.rs`

**Implementation guidance:**
- Implement `ralph prps process` as a blocking serial processor, not a web dispatcher.
- Pick the next queue entry in `queued` or resumable `implementing` state.
- Create or reuse the deterministic implementation worktree using the Step 2 helper.
- Spawn `ralph run --autonomous -c <implementation_config> -p <prp title>` from that worktree.
- Record `implementing` with child pid and worktree metadata in the PRP queue.
- Determine readiness using the worktree copy of the PRP plus Ralph artifacts:
  - `task.complete` exists in the current events stream
  - `.ralph/agent/handoff.md` exists
  - all `## Definition of Done` checkboxes are checked
- If `task.complete` and handoff exist but DoD boxes are still unchecked, run exactly one corrective follow-up prompt in the same worktree to mark completed DoD items, then re-check readiness.
- If the corrective pass still leaves DoD incomplete, move the entry to `needs_review` with `last_phase = implementation`.
- Do not start the next PRP from `process` after an implementation pass unless the current PRP transitions into integration and then finishes integration successfully.

**Test requirements:**
- Integration tests for:
  - first queued PRP enters `implementing`
  - successful implementation promotes to `ready_for_integration`
  - incomplete DoD triggers one corrective retry
  - unresolved DoD after retry transitions to `needs_review`
  - processor never advances to PRP-002 while PRP-001 is only `ready_for_integration`

**Validation:**
- Run: `cargo test -p ralph-cli integration_prps -- --nocapture`
- Expect: implementation-phase and queue-blocking tests are green.

**Checkpoint:** `feat(cli): add PRP implementation processor`

---

## Step 5: Add shared integration-phase processing and queue blocking semantics

**Objective:** Keep queue ownership of the current PRP until its changes are merged into the shared integration branch/worktree.

**Files:**
- Modify: `crates/ralph-cli/src/prps.rs`
- Modify: `crates/ralph-core/src/prp_queue.rs`
- Review: `crates/ralph-cli/src/loops.rs`

**Implementation guidance:**
- When the active PRP reaches `ready_for_integration`, `ralph prps process` must immediately switch into integration handling for that same PRP.
- Create or reuse one shared integration worktree at `config.prps.integration_worktree` on `config.prps.integration_branch`.
- Run `ralph run --autonomous -c <integration_config>` inside that worktree with a structured prompt that includes:
  - PRP id and title
  - source implementation branch/worktree
  - required validation gates
  - instruction to archive the PRP markdown into `completed_dir`
  - instruction not to advance other queue items
- Transition queue state to `integrating` while the child process is active.
- Mark `integrated` only when all of these are true:
  - integration worktree is clean
  - the integration branch contains the merged PRP changes
  - the PRP markdown has been moved to `completed_dir`
  - the queue entry records the resulting integration commit SHA
- On any failed integration, move the entry to `needs_review` with `last_phase = integration` and do not dequeue the next PRP.
- Do not fast-forward `main` in v1. The integration branch is the queue's terminal landing target.

**Test requirements:**
- Integration tests for:
  - successful integration archives the PRP and records the integration commit
  - queue advancement to the next PRP happens only after `integrated`
  - failed integration leaves PRP-002 untouched and PRP-001 in `needs_review`
  - repeated `process` calls on an already integrated PRP skip it and move to the next queued item

**Validation:**
- Run: `cargo test -p ralph-cli integration_prps -- --nocapture`
- Expect: full process path is green, including shared integration branch behavior.

**Checkpoint:** `feat(cli): block PRP queue on shared integration branch landing`

---

## Step 6: Add `resume/retry/discard` operator flows and documentation

**Objective:** Make interrupted and failed PRP jobs operable without falling back to shell scripts or manual state edits.

**Files:**
- Modify: `crates/ralph-cli/src/prps.rs`
- Modify: `docs/guide/cli-reference.md`
- Modify: `docs/advanced/parallel-loops.md`
- Modify: `README.md`

**Implementation guidance:**
- Add:
  - `ralph prps resume [prp-id]`
  - `ralph prps retry <prp-id>`
  - `ralph prps discard <prp-id>`
- `resume`:
  - valid only for `implementing` and `integrating`
  - re-enters the correct phase using existing queue metadata and worktrees
- `retry`:
  - valid only for `needs_review`
  - if `last_phase = implementation`, transition back to `queued`
  - if `last_phase = integration`, transition back to `ready_for_integration`
  - preserve existing worktrees so the next `process` call can continue from repo reality
- `discard`:
  - valid only for `queued`, `ready_for_integration`, or `needs_review`
  - refuse to discard a live `implementing` or `integrating` item unless a later explicit `--force` feature is added
  - clean up the implementation worktree when present
  - leave the shared integration branch/worktree intact
- Document the mandatory worktree-first implementation flow for future developers.

**Test requirements:**
- CLI tests for resume, retry, and discard state transitions.
- Docs/examples updated with actual command usage.

**Validation:**
- Run: `cargo test -p ralph-cli integration_prps -- --nocapture`
- Expect: operator-flow tests are green and docs reference the new commands.

**Checkpoint:** `feat(cli): add PRP recovery and operator commands`

---

## Step 7: Drive verification to green and validate the end-to-end queue flow

**Objective:** Close the implementation with end-to-end verification in the worktree and a documented operator flow.

**Files:**
- Review: `crates/ralph-core/src/prp_queue.rs`
- Review: `crates/ralph-cli/src/prps.rs`
- Review: `crates/ralph-cli/tests/integration_prps.rs`

**Implementation guidance:**
- Run the narrow tests throughout development, then finish with broader repo checks.
- Do one manual smoke flow in the worktree using sample PRPs:
  - `ralph prps import`
  - `ralph prps list`
  - `ralph prps process`
  - `ralph prps show PRP-001`
- Confirm that:
  - PRP-001 reaches `integrated`
  - the markdown file moves to `completed_dir`
  - PRP-002 does not begin until after PRP-001 integration completes

**Validation:**
- Run: `cargo test -p ralph-core`
- Run: `cargo test -p ralph-cli`
- Run: `cargo test`
- Expect: all targeted PRP tests are green and no existing loop/merge regressions are introduced.

**Checkpoint:** `feat: land native PRP queue`

## Risks and Assumptions

- The highest-risk area is reconciliation after interrupted child `ralph run` processes. The implementation must derive truth from queue state plus live worktree artifacts instead of relying only on stale pids.
- Extending `worktree.rs` is preferable to duplicating git worktree logic in `prps.rs`; otherwise PRP flow will drift from the rest of Ralph's worktree behavior.
- The generic merge queue is intentionally not the source of truth here because it models post-loop merge jobs, not queue-owned PRP execution that remains active through integration.
- The integration branch is the queue's landing target in v1. Promotion from `integration` to `main` is a separate operator workflow.

## Deferred / Out of Scope

- Web dashboard support for PRP queues.
- Automatic import-on-process or continuous folder reconciliation.
- Replacing ordinary runtime task queues or the existing generic merge queue.
- Auto-promotion from the shared integration branch to `main`.
