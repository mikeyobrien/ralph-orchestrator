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

## Codebase Orientation

Start by reading these existing files before changing anything:

- `AGENTS.md`
  Explains the repo's current philosophy: keep Ralph thin, reuse existing machinery, and preserve current behaviors unless the spec explicitly changes them.
- `crates/ralph-cli/src/main.rs`
  Top-level CLI registration and dispatch. The new `Prps(prps::PrpsArgs)` command must be added here, plus `mod prps;` near the other subcommand modules.
- `crates/ralph-cli/src/loops.rs`
  Best current example of a local-workspace management namespace with subcommands, human/JSON output, and queue-style operator flows.
- `crates/ralph-cli/src/loop_runner.rs`
  Existing subprocess orchestration, event marker handling, merge-queue processor wiring, and loop lifecycle rules. Reuse the same conventions for `.ralph/current-events`, event files, and subprocess logging.
- `crates/ralph-core/src/config.rs`
  Canonical config model. Add the new `prps` section here and wire defaulting/serde the same way the other config groups work.
- `crates/ralph-core/src/worktree.rs`
  Existing git worktree lifecycle and sync behavior. Extend this rather than shelling out directly from `prps.rs`.
- `crates/ralph-core/src/merge_queue.rs`
  Best model for an append-only JSONL queue with replayed state and transition validation. PRP semantics differ, but the persistence pattern is the right starting point.
- `crates/ralph-core/src/loop_context.rs`
  Source of truth for `.ralph/current-events`, `.ralph/agent/handoff.md`, and loop workspace paths. Use these same paths when checking whether an implementation or integration pass completed.
- `crates/ralph-core/src/lib.rs`
  Re-export point. New core modules must be added here so `ralph-cli` can use them cleanly.
- `crates/ralph-cli/tests/integration_loops_merge.rs`
  Current style for CLI integration tests that create a temp git repo, write queue files, and call `env!("CARGO_BIN_EXE_ralph")`.
- `docs/guide/cli-reference.md` and `docs/advanced/parallel-loops.md`
  Natural documentation homes for the new command surface and the PRP-specific worktree model.

Implementation should stay in the existing repo shape:

- `ralph-core` owns state, filesystem helpers, and worktree primitives.
- `ralph-cli` owns parsing, operator UX, subprocess launching, and queue processing.
- Tests should be split the same way: unit tests in `ralph-core`, end-to-end CLI behavior in `ralph-cli/tests`.

## Target CLI UX

The new namespace should feel like `ralph loops`: direct, workspace-local, and scriptable.

Planned commands:

```bash
ralph prps import
ralph prps list
ralph prps list --json
ralph prps show PRP-001
ralph prps process
ralph prps resume PRP-001
ralph prps retry PRP-001
ralph prps discard PRP-001
```

Expected operator behavior:

- `import`
  Scans `config.prps.import_dir`, inserts any new PRPs into the canonical queue, and prints how many were imported vs already known.
- `list`
  Shows queue order, PRP id, title, state, last phase, current worktree/branch, and failure reason if present.
- `show`
  Shows one materialized queue entry in detail, including source path, archive path if integrated, phase metadata, and known subprocess ids.
- `process`
  Processes at most one PRP end-to-end. If the head PRP is merely implemented, `process` must continue straight into integration rather than returning success and letting the next item start.
- `resume`
  Reconciles live filesystem/process state for the specified PRP and continues the appropriate phase.
- `retry`
  Clears a `needs_review` terminal block back to a resumable state based on `last_phase`.
- `discard`
  Marks the PRP terminal and removes its dedicated implementation worktree when safe.

Suggested human-readable `list` columns:

```text
ORDER  PRP ID   STATE                  PHASE           BRANCH        WORKTREE                  REASON
1      PRP-001  implementing           implementation  prp/PRP-001  .worktrees/prp-PRP-001   -
2      PRP-002  queued                 -               -             -                         -
```

Suggested `show` JSON keys:

- `prp_id`
- `title`
- `state`
- `last_phase`
- `source_path`
- `archive_path`
- `implementation_branch`
- `implementation_worktree`
- `implementation_pid`
- `implementation_loop_id`
- `integration_branch`
- `integration_worktree`
- `integration_pid`
- `integration_loop_id`
- `integration_commit`
- `failure_reason`
- `created_at`
- `updated_at`

## Proposed Config Schema

Add a new nested config group under `RalphConfig`:

```yaml
prps:
  enabled: true
  import_dir: ./PRPs/remaining_work
  completed_dir: ./PRPs/completed
  queue_file: .ralph/prp-queue.jsonl
  implementation_config: ralph.yml
  integration_config: ralph-landing.yml
  integration_branch: integration
  implementation_worktree_dir: .worktrees
  integration_worktree: .worktrees/integration
  implementation_branch_prefix: prp/
```

Recommended Rust shape in `crates/ralph-core/src/config.rs`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrpsConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_prp_import_dir")]
    pub import_dir: PathBuf,
    #[serde(default = "default_prp_completed_dir")]
    pub completed_dir: PathBuf,
    #[serde(default = "default_prp_queue_file")]
    pub queue_file: PathBuf,
    #[serde(default = "default_prp_implementation_config")]
    pub implementation_config: PathBuf,
    #[serde(default = "default_prp_integration_config")]
    pub integration_config: PathBuf,
    #[serde(default = "default_prp_integration_branch")]
    pub integration_branch: String,
    #[serde(default = "default_prp_implementation_worktree_dir")]
    pub implementation_worktree_dir: PathBuf,
    #[serde(default = "default_prp_integration_worktree")]
    pub integration_worktree: PathBuf,
    #[serde(default = "default_prp_branch_prefix")]
    pub implementation_branch_prefix: String,
}
```

Add `#[serde(default)] pub prps: PrpsConfig` to `RalphConfig` and update `Default for RalphConfig`.

Notes:

- Keep paths workspace-relative unless the user configures absolute paths.
- `enabled` is mainly for config discoverability and future toggling; the CLI should still error clearly if `ralph prps ...` is run with `prps.enabled = false`.
- `implementation_worktree_dir` is worth adding instead of hardcoding `.worktrees`, because it keeps branch naming and worktree location configurable without changing the queue file format.

## Queue Storage Model

Use the same broad pattern as `MergeQueue`: append-only JSONL plus replayed materialized state. The main difference is richer event types and stricter linear semantics.

Suggested new module split:

- `crates/ralph-core/src/prp_queue.rs`
  Queue events, state machine, replay, validation, and entry lookups.
- `crates/ralph-core/src/prp_files.rs`
  PRP markdown discovery, id/title extraction, Definition-of-Done parsing, and archive moves.

Suggested event model:

```rust
pub struct PrpEvent {
    pub ts: DateTime<Utc>,
    pub prp_id: String,
    pub event: PrpEventType,
}

pub enum PrpEventType {
    Imported {
        title: String,
        source_path: String,
        queue_position: u64,
    },
    ImplementationStarted {
        branch: String,
        worktree: String,
        pid: u32,
        loop_id: Option<String>,
    },
    ImplementationReady {
        handoff_path: String,
        events_path: String,
    },
    IntegrationStarted {
        branch: String,
        worktree: String,
        pid: u32,
        loop_id: Option<String>,
    },
    Integrated {
        commit: String,
        archive_path: String,
    },
    NeedsReview {
        phase: PrpPhase,
        reason: String,
    },
    Retried {
        phase: PrpPhase,
    },
    Discarded {
        reason: Option<String>,
    },
}
```

Suggested materialized entry fields:

- `prp_id`
- `title`
- `source_path`
- `archive_path`
- `state`
- `last_phase`
- `queue_position`
- `implementation_branch`
- `implementation_worktree`
- `implementation_pid`
- `implementation_loop_id`
- `implementation_events_path`
- `implementation_handoff_path`
- `integration_branch`
- `integration_worktree`
- `integration_pid`
- `integration_loop_id`
- `integration_events_path`
- `integration_commit`
- `failure_reason`
- `created_at`
- `updated_at`

Keep `queue_position` immutable after first import. Queue order should be "oldest non-terminal imported item first", not "whatever file happens to sort first today after retries".

## State Machine

Use an explicit state enum and reject illegal transitions in `prp_queue.rs`.

```text
queued
  -> implementing
  -> discarded

implementing
  -> ready_for_integration
  -> needs_review

ready_for_integration
  -> integrating
  -> discarded

integrating
  -> integrated
  -> needs_review

needs_review
  -> queued                  if last_phase = implementation
  -> ready_for_integration   if last_phase = integration
  -> discarded

integrated
  terminal

discarded
  terminal
```

Additional invariants:

- Only one PRP may be non-terminal and "active" (`implementing`, `ready_for_integration`, or `integrating`) at a time.
- `next_runnable()` must pick the oldest item whose state is `queued`; however, `process` must first finish any item already in `ready_for_integration` or `integrating`.
- `process` must refuse to start a later `queued` PRP if any earlier PRP is in `implementing`, `ready_for_integration`, `integrating`, or `needs_review`.
- `needs_review` is blocking. It is not a soft warning state.

## Processing Algorithm

`ralph prps process` should be implemented as a thin CLI wrapper around a dedicated processor type or functions in `prps.rs`, not as one giant command handler. A good shape is:

- `process_queue_once(workspace_root, config, output_mode)`
- `reconcile_entry(workspace_root, config, entry)`
- `run_implementation_phase(...)`
- `run_integration_phase(...)`
- `determine_implementation_readiness(...)`
- `determine_integration_success(...)`

Implementation-phase algorithm:

1. Load queue and materialized entries.
2. If any earlier PRP is `needs_review`, print a blocking message and return non-zero.
3. If the queue head is `integrating`, resume/reconcile integration for that same item.
4. Else if the queue head is `ready_for_integration`, start integration for that same item.
5. Else if the queue head is `implementing`, reconcile implementation for that same item.
6. Else if the queue head is `queued`, create or reuse its implementation worktree and start implementation.
7. Never inspect or start PRP-002 when PRP-001 is not yet terminal.

Concrete implementation-phase behavior:

1. Derive deterministic names:
   - branch: `prp/<PRP-ID>` or `{implementation_branch_prefix}{PRP-ID}`
   - worktree path: `{implementation_worktree_dir}/prp-<PRP-ID>`
2. Create or reuse that worktree from the current repo HEAD unless it already exists.
3. Spawn Ralph from that worktree using the implementation config:
   - prefer `std::env::current_exe()` or a helper seam that accepts an executable path
   - avoid raw `Command::new("ralph")` in the new processor so tests are not PATH-sensitive
4. Prompt should identify both the PRP id and the source markdown path. Use enough detail that the worker can open the PRP and act on it.
5. Record `ImplementationStarted` before waiting/reconciling.
6. After the subprocess exits, determine readiness from artifacts, not exit code alone.

Readiness check should require all of:

- current events marker exists in the implementation worktree
- the referenced events file contains `task.complete`
- `.ralph/agent/handoff.md` exists
- the worktree copy of the PRP file has all Definition-of-Done checkboxes checked

Corrective DoD pass:

1. If `task.complete` and handoff exist but DoD checkboxes remain unchecked, run one follow-up `ralph run --continue --no-tui` or equivalent second prompt in the same worktree.
2. The follow-up prompt should be narrow: update the PRP markdown to accurately reflect completed DoD items and stop.
3. Re-run readiness checks.
4. If still incomplete, emit `NeedsReview { phase: implementation, reason: ... }`.

Integration-phase algorithm:

1. Create or reuse the shared integration worktree at `config.prps.integration_worktree`.
2. Ensure it is on `config.prps.integration_branch`.
3. Spawn Ralph in that integration worktree with the integration config.
4. Prompt must include:
   - PRP id/title
   - implementation branch
   - implementation worktree path
   - instruction to land the current PRP into the integration branch
   - instruction to archive the PRP markdown into `completed_dir`
   - instruction to stop once this PRP is integrated
5. Record `IntegrationStarted`.
6. After the subprocess exits, validate success from repo state and artifacts.

Integration success should require all of:

- queue head still points to the same PRP
- shared integration worktree exists and is on the configured integration branch
- PRP archive exists in `completed_dir`
- integration branch contains a new commit attributable to this pass
- integration worktree is clean after landing

Then emit `Integrated { commit, archive_path }`.

Failure handling:

- If subprocess exits unsuccessfully, emit `NeedsReview`.
- If subprocess exits successfully but archive/commit/cleanliness checks fail, also emit `NeedsReview`.
- Do not auto-reset or auto-clean the integration worktree beyond safe, deterministic operations.

## Recovery And Reconciliation Rules

The current shell script is buggy partly because it treats pid files and transient shell state as truth. The Rust version should always reconcile from durable queue state plus the live repository.

Recommended reconciliation rules:

- If queue says `implementing` but stored PID is dead:
  inspect the implementation worktree artifacts and decide whether the PRP is actually `ready_for_integration` or `needs_review`.
- If queue says `integrating` but stored PID is dead:
  inspect the integration worktree, archive path, and branch state to decide whether the PRP is actually `integrated` or `needs_review`.
- If queue says `ready_for_integration` and integration worktree does not exist:
  recreate the shared integration worktree and continue; this should not require re-running implementation.
- If queue says `implementing` and the implementation worktree is missing:
  move to `needs_review` unless the PRP can be proven complete from archived artifacts.
- If the source markdown has already been moved to `completed_dir` but queue is not `integrated`:
  treat this as inconsistent state and require operator review rather than silently rewriting history.
- If a worktree branch exists but the worktree directory is gone:
  clean up or recreate deliberately; do not assume `git worktree add` will succeed without handling the stale registration.

`resume` should perform the same reconciliation logic as `process`, but targeted to the named PRP and with clearer operator-facing output.

## Subprocess And Testability Guidance

Do not hardcode the new PRP processor around `Command::new("ralph")` with no seam. That makes integration tests brittle and would recreate shell-script-style assumptions.

Preferred implementation pattern:

- Factor the core processor as `process_queue_once_with_command(repo_root, config, ralph_cmd: &OsStr)` similar to `process_pending_merges_with_command(...)` in `loop_runner.rs`.
- Keep the public CLI path as a thin wrapper that passes `std::env::current_exe()` or an explicit command override.
- In tests, pass either:
  - `env!("CARGO_BIN_EXE_ralph")` for real end-to-end behavior, or
  - a fixture shell script / tiny fake executable that writes expected `.ralph/current-events`, `.ralph/agent/handoff.md`, and PRP checkbox updates.

The fake-runner path is especially useful for deterministic queue tests because it lets tests simulate:

- implementation success
- implementation success with unchecked DoD
- integration success
- integration failure
- interrupted child processes

## File-By-File Implementation Map

Use this as the concrete code edit checklist.

`crates/ralph-core/src/config.rs`

- Add `PrpsConfig`.
- Add defaults and serde coverage.
- Add the field to `RalphConfig` and `Default`.

`crates/ralph-core/src/prp_queue.rs` (new)

- Define `PrpEvent`, `PrpEventType`, `PrpState`, `PrpPhase`, `PrpEntry`, and `PrpQueueError`.
- Implement:
  - `new(workspace_root)`
  - `import(...)`
  - `list()`
  - `get_entry(prp_id)`
  - `head_blocking_entry()`
  - `next_runnable()`
  - transition helpers (`mark_implementing`, `mark_ready_for_integration`, `mark_integrating`, `mark_integrated`, `mark_needs_review`, `retry`, `discard`)
- Mirror `MergeQueue`'s locking and append/replay model.

`crates/ralph-core/src/prp_files.rs` (new)

- Implement:
  - `discover_prps(import_dir) -> Vec<DiscoveredPrp>`
  - `parse_prp_id(path)`
  - `extract_title(markdown)`
  - `definition_of_done_complete(markdown) -> bool`
  - `archive_prp(source_path, completed_dir) -> archive_path`
- Keep the markdown parsing deliberately simple and deterministic.

`crates/ralph-core/src/worktree.rs`

- Add an explicit named-worktree creator that accepts branch name, path, and base ref.
- Add helpers to:
  - inspect branch for arbitrary worktree
  - create/recreate a shared worktree safely
  - remove non-`ralph/*` worktrees when asked
- Preserve existing `create_worktree()` behavior for loop flows.

`crates/ralph-core/src/lib.rs`

- Export `prp_queue` and `prp_files`.
- Re-export their public types.

`crates/ralph-cli/src/prps.rs` (new)

- Define clap structs for all PRP subcommands.
- Implement human-readable output and `--json` where useful.
- Implement the phase processor and recovery logic.
- Keep queue manipulation in core, but keep subprocess spawning and console output in CLI.

`crates/ralph-cli/src/main.rs`

- Add `mod prps;`
- Add `Commands::Prps(prps::PrpsArgs)`
- Wire dispatch in the main `match`.

`crates/ralph-cli/tests/integration_prps.rs` (new)

- Build temp git repos.
- Cover import/list/show/process/resume/retry/discard.
- Use deterministic fixtures for subprocess behavior.

`docs/guide/cli-reference.md`

- Document the new command namespace and arguments.

`docs/advanced/parallel-loops.md`

- Add a PRP-specific section explaining why PRP worktrees differ from ordinary parallel loop worktrees and why integration uses one shared branch/worktree.

`README.md`

- Add a short discovery-level mention only if the project currently surfaces new CLI namespaces there. Keep this lightweight.

## Manual Smoke Scenario

Before calling the feature done, run one manual operator flow from the worktree:

1. Create a temp repo or fixture repo with:
   - `PRPs/remaining_work/PRP-001.md`
   - `PRPs/remaining_work/PRP-002.md`
   - `ralph.yml`
   - `ralph-landing.yml`
2. Run:

```bash
cargo build
./target/debug/ralph prps import
./target/debug/ralph prps list
./target/debug/ralph prps process
./target/debug/ralph prps show PRP-001
./target/debug/ralph prps list
```

3. Verify:
   - PRP-001 moved through `queued -> implementing -> ready_for_integration -> integrating -> integrated`
   - `PRPs/completed/PRP-001.md` exists
   - shared integration worktree exists at the configured path
   - PRP-002 remained `queued` until PRP-001 was integrated
4. Re-run `ralph prps process` and verify only then does PRP-002 start.

## Implementation Notes For A Cold Engineer

These are the easy places to go wrong:

- Do not build this on the web queue. The user explicitly wants a task-queue-like flow without using the web dispatcher model.
- Do not treat `PRPs/.prp_status.json` as truth. The queue file replaces it.
- Do not let "implementation complete" count as queue completion. The queue advances only after shared-branch integration.
- Do not create one integration branch per PRP. The v1 design uses one long-lived shared `integration` branch and worktree.
- Do not advance around `needs_review`. That state blocks the line until the operator retries or discards.
- Do not bake repository-specific prompt text into core. Prompt assembly belongs in `ralph-cli`, while core owns state and file mechanics.
- Do not make success depend purely on child exit codes. Validate from Ralph artifacts and git state.
- Do not regress the existing `ralph loops` behavior while extending worktree helpers.

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
  - `enabled: true`
  - `import_dir: "./PRPs/remaining_work"`
  - `completed_dir: "./PRPs/completed"`
  - `queue_file: ".ralph/prp-queue.jsonl"`
  - `implementation_config: "ralph.yml"`
  - `integration_config: "ralph-landing.yml"`
  - `integration_branch: "integration"`
  - `implementation_worktree_dir: ".worktrees"`
  - `integration_worktree: ".worktrees/integration"`
  - `implementation_branch_prefix: "prp/"`
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
  - implementation worktrees: branch `{config.prps.implementation_branch_prefix}<PRP-ID>`
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
