# v3 branch reconciliation

Phase 1 of the v3 completion brief. This record enumerates the two v3 deltas,
classifies every rollup-only commit, and names the ports that follow.

- Branch: `v3/complete`
- Date: 2026-09-22
- Rollup under review: `origin/wip/v3-prerelease-rollup` at `6e2545d`
- Merge base of `v3/complete` and the rollup: `2e1fc52`
- Classification measured at `80309dc`, re-verified at `44afa19`, and verified
  against the landed ports at `635cb8c`
- Source edits made by this record: none
- Revision 2 corrects the port mechanism. The critic rejected revision 1 at
  `44afa19` for a false overlap claim, an inapplicable replay mechanism, and a
  wrong leaf count. The 22-row classification was confirmed sound and is
  unchanged.
- Revision 3 is the Step 2 gate, `task-1790097924-daa7` / `code-assist:v3-complete:step-02:verify-reconciliation`.
  It re-measured every surviving claim at `635cb8c` and appended the evidence in
  Verification. It changes no verdict and corrects one row reason, `3322fd7`.

## Method

The clone is not shallow, so the counts are real. Both commands were run this
turn.

```bash
git rev-parse --is-shallow-repository
git rev-list --count origin/integration/v3-prerelease..origin/wip/v3-prerelease-rollup
git rev-list --count origin/wip/v3-prerelease-rollup..origin/integration/v3-prerelease
```

```text
false
22
8
```

Verdict vocabulary:

- `port`: the change is wanted and is absent from `v3/complete`.
- `already-covered`: the equivalent change is already in `v3/complete`.
- `superseded`: the rollup artifact is a merge or landing artifact, or the
  rollup copy is behind `v3/complete`.

## The 8 base-only commits are already in this branch

Every commit in `origin/wip/v3-prerelease-rollup..origin/integration/v3-prerelease`
is an ancestor of `HEAD`. Verified one by one.

```bash
for sha in $(git rev-list origin/wip/v3-prerelease-rollup..origin/integration/v3-prerelease); do
  git merge-base --is-ancestor "$sha" HEAD && echo "IN HEAD" || echo "MISSING"
done
```

```text
22fc1fd feat(cli): restore ralph run --rpc and native ralph resume (#368) => IN HEAD
70b3360 feat(cli): restore ralph run --rpc and native ralph resume => IN HEAD
8276db0 fix(telegram): wire /stop and /restart and skip stale guidance (#367) => IN HEAD
893f129 Merge pull request #366 from mikeyobrien/cursor/v3-telegram-autoloop-74c5 => IN HEAD
47a568e Merge pull request #365 from mikeyobrien/cursor/v3-ralph-bench-autoloop-74c5 => IN HEAD
775e98a fix(cli): invert RObot enablement check for clippy if_not_else => IN HEAD
42359f3 feat(robot): relay Telegram HITL through Autoloop control (#345) => IN HEAD
92e4992 feat(bench): drive ralph-bench task execution on the autoloop engine (#346) => IN HEAD
```

No base-only work is lost. The reconciliation only moves rollup work forward.

## Classification of the 22 rollup-only commits

Newest first, which is `git log` order.

| Commit | Subject | Verdict | Evidence and reason |
| --- | --- | --- | --- |
| `6e2545d` | Merge WIP live harness smoke preset | port | Merge that lands the smoke cluster. Porting the ten leaf commits below covers it. `git cat-file -e HEAD:presets/live-harness-smoke` fails, so the cluster is absent from `HEAD`. |
| `7c9b0ff` | Merge WIP TUI stream history and backpressure fixes | port | Merge that lands the TUI cluster. Covered by taking the rollup tip content of the two TUI files. The commits cannot be replayed; see Why the ports take final content. |
| `c90001e` | Merge WIP Ralph-owned Autoloop state | already-covered | The merge delta is 2 lines in `.ralph/tasks/ralph-owned-autoloop-state.code-task.md`. The code lives at `crates/ralph-core/src/engine_state.rs`, present in `HEAD` with 419 lines against the rollup's 377. `HEAD` tests assert `.ralph/autoloop` is the state root and that no top-level `.autoloop` appears. |
| `3322fd7` | docs(engine): correct final gate evidence | superseded | Run-log bookkeeping in a code-task doc, not a shipped surface. The code it describes is present through `c90001e`. The two doc copies diverge in both directions, so neither is simply behind the other: the rollup carries "4 passed" where `HEAD` carries "3 passed", and `HEAD` carries `cargo clippy --workspace` where the rollup does not. Measured in Verification, Correction to the `3322fd7` row. |
| `2242eec` | fix(smoke): harden live provider safety gates | port | Adds `tools/smoke_process_group.py` and rewrites the runner. `git cat-file -e HEAD:tools/smoke_process_group.py` fails. |
| `5b7876c` | fix(tui): bound stream identities and lifecycle lines | port | `crates/ralph-adapters/src/backend_stream_tailer.rs` is 514 lines in `HEAD` and 881 in the rollup. `Cargo.lock` adds `sha2` to `ralph-adapters`. |
| `1e67e52` | chore: auto-commit before merge (loop primary) | superseded | It is not a change. It swept a then-untracked `.ralph/tasks/tui-stream-history-backpressure.code-task.md`. It is live evidence for `landing-untracked-sweep-yxv`, not a port. The swept doc itself is ported with the TUI cluster; see The swept code-task doc. |
| `faa2b71` | fix(smoke): validate canonical completion without retries | port | Part of the smoke cluster. Absent from `HEAD`. |
| `86066b6` | fix(tui): protect reconciled history under line pressure | port | `crates/ralph-tui/src/autoloop_source.rs` is 1331 lines in `HEAD` and 2053 in the rollup. |
| `ebeb81f` | fix(smoke): abort immediately on missing provider handoff | port | Adds `presets/live-harness-smoke/scripts/require_smoke_handoff.py`. Absent from `HEAD`. |
| `1a2a43d` | fix(smoke): render executable contracts for every role | port | Rewrites the six role contracts. Absent from `HEAD`. |
| `0a5f660` | fix(smoke): resolve provider-visible run evidence path | port | Adds the manual spec and code-task docs. Absent from `HEAD`. |
| `49434db` | chore: satisfy strict touched-crate clippy | already-covered | `autoloop_events.rs` and `autoloop_native_contract_integration.rs` are identical between `HEAD` and the rollup by `git diff --numstat`. Its `autoloop_runner.rs` hunk sits on a pre-resume base and is superseded by `HEAD`, which carries `AutoloopRunner::resume` at 1032 lines against the rollup's 896. Its two TUI hunks are already inside the rollup tip content the TUI port takes. |
| `3d4b8ca` | fix: satisfy clippy lifetime lint | already-covered | `crates/ralph-core/src/event_parser.rs` is identical between `HEAD` and the rollup. |
| `2ac3c1f` | style: apply workspace rustfmt | already-covered | `crates/ralph-tui/src/widgets/help.rs` is identical between `HEAD` and the rollup. Its `backend_stream_tailer.rs` hunk is already inside the rollup tip content the TUI port takes. |
| `e275303` | fix: preserve bounded TUI stream history | port | Same file pair as `86066b6`. 348 insertions against the rollup's merge base. Ported by final content, not replayed. |
| `de2eaa4` | fix: bound backend stream identity and backpressure | port | 342 insertions across `backend_stream_tailer.rs` and `autoloop_source.rs`. |
| `3c8eaed` | docs(presets): explain manual live smoke | port | Adds the `presets/README.md` section and the preset README. |
| `91b094d` | test(smoke): cover fake live harness matrix | port | Adds `tools/tests/test_smoke_live_harnesses.py` and a CI step. |
| `47f8dfd` | feat(tools): validate live smoke evidence | port | Adds `tools/smoke_live_harness_results.py`. |
| `830ecfe` | feat(tools): add bounded live harness smoke runner | port | Adds `tools/smoke-live-harnesses.sh`. |
| `e0178bf` | feat(presets): add manual live harness smoke | port | Adds `presets/live-harness-smoke/`, which is absent from `HEAD`. |

Tally: 16 `port`, 4 `already-covered`, 2 `superseded`. The 16 port rows sit in
two clusters, so two port commits cover them.

## Why the smoke cluster is in scope

The brief's ground-truth table names "live smoke harness hardening" as one of
the three things the rollup dominates. Phase 3 requires an end-to-end live
verification with the journal shown. The smoke cluster is the harness that
produces that evidence: it renders an executable contract per role, aborts on a
missing provider handoff, validates canonical completion without retries, and
bounds the process group. It is additive, so the port carries no regression risk
to existing behavior.

## Backend unification is already present

The brief warns that backend-adjacent work must build on main's unified catalog.
That catalog is already in this branch from the main merge.

```bash
ls -la crates/ralph-core/src/backend.rs
git log --oneline -1 -- crates/ralph-core/src/backend.rs
```

```text
-rw-rw-r-- 1 mobrienv mobrienv 11133 Sep 22 16:45 crates/ralph-core/src/backend.rs
edc2b32 feat(adapters): add OMP (oh-my-pi) backend (#358)
```

No port is needed for the catalog itself. Phase 2c Jev work must build on
`crates/ralph-core/src/backend.rs`.

## Why the ports take final content, not commits

The rollup's clusters are commit stacks, not replayable patches. The TUI stack
(`de2eaa4`, `e275303`, `86066b6`, `5b7876c`) is a linear chain based on
`aff233d`, where `autoloop_source.rs` is 1258 lines. The base branch grew that
file to 1331 lines at `2e1fc52` before the merge base, so replaying the stack
applies against a stale file.

Measured in a throwaway worktree at `44afa19`, removed afterwards:

```bash
git worktree add --detach /tmp/v3-cherry-test HEAD
git cherry-pick --no-commit de2eaa4   # APPLIED
git cherry-pick --no-commit e275303   # CONFLICT (content) in autoloop_source.rs
```

The conflict is the state seam this reconciliation exists to protect. The rollup
side of the hunk replaces

```rust
let reader_engine_root = ralph_core::engine_state::engine_state_root(&workspace);
```

with `let reader_workspace = workspace.clone();`, which drops the
`.ralph/autoloop` state-root threading. `5b7876c` alone conflicts in three paths:
`(modify/delete): .ralph/tasks/tui-stream-history-backpressure.code-task.md
 deleted in HEAD and modified in 5b7876c`, plus content conflicts in both
source files.

The replacement mechanism is to take the rollup tip's content of each wanted
path. That is lossless here, and the reason is structural rather than
file-by-file.

```bash
git merge-base --is-ancestor 2e1fc52 HEAD && echo "ancestor of HEAD"
git merge-base --is-ancestor 2e1fc52 origin/wip/v3-prerelease-rollup && echo "ancestor of rollup tip"
comm -12 <(git diff --name-only 2e1fc52 HEAD | sort) \
         <(git diff --name-only 2e1fc52 origin/wip/v3-prerelease-rollup | sort)
```

```text
ancestor of HEAD
ancestor of rollup tip
Cargo.lock
```

`2e1fc52` is the merge base and an ancestor of both tips, so the rollup tip
already contains every base-branch change up to the merge base. `Cargo.lock` is
the only file both sides changed after it, and our two TUI files are
byte-identical to the merge base.

```bash
git diff --stat 2e1fc52 HEAD -- crates/ralph-tui/src/autoloop_source.rs \
  crates/ralph-adapters/src/backend_stream_tailer.rs
```

```text
(no output)
```

So for every wanted path except `Cargo.lock`, taking the rollup tip's content
loses nothing of ours. `Cargo.lock` is not taken verbatim, and the `sha2` port
does change it. The `sha2` package is already resolved at `Cargo.lock:3657`, but
`Cargo.lock` records dependency edges per package, and `ralph-adapters` has no
`sha2` edge today. Measured in a detached worktree at `HEAD` with
`sha2.workspace = true` added to `crates/ralph-adapters/Cargo.toml`:

```bash
cargo check --locked -p ralph-adapters   # exit 101
cargo check -p ralph-adapters            # rewrites the lock, exit 0
git diff -- Cargo.lock
```

```text
error: cannot update the lock file .../Cargo.lock because --locked was passed to prevent this
@@ -2763,6 +2763,7 @@ dependencies = [
  "ratatui",
  "serde",
  "serde_json",
+ "sha2",
  "tempfile",
```

The lock update lands in the same commit as the `Cargo.toml` line. Committing the
line alone leaves a locked build broken.

The rollup tip also carries the seam in the other direction.

```bash
git show HEAD:crates/ralph-tui/src/autoloop_source.rs | grep -c reader_engine_root
git show origin/wip/v3-prerelease-rollup:crates/ralph-tui/src/autoloop_source.rs | grep -c reader_engine_root
```

```text
8
12
```

`run_autoloop_event_reader` keeps the same six parameters in both trees:
`events_path, workspace_root, engine_state_root, state, cancel_rx, role_display_names`.

Measured compile and test evidence for the swap, in a detached worktree at
`44afa19` with the rollup tip content of the two TUI files and the `sha2` line:

```bash
cargo check -p ralph-tui -p ralph-adapters   # Finished in 12.66s
cargo test -p ralph-adapters -p ralph-tui    # 382 passed, 273 passed, 0 failed
```

Raw transcript at `logs/step-02-port-mechanism.md`. The render-under-load proof
the brief requires is not measured here. It belongs to
`port-tui-stream-history`.

## The swept code-task doc

`.ralph/tasks/tui-stream-history-backpressure.code-task.md` is absent from `HEAD`
and present in the rollup at 83 lines. `1e67e52`, the landing auto-commit sweep,
created it. `5b7876c` appends 5 lines.

Port it with the TUI cluster. It is the acceptance spec for that work: it names
the two beads, the two required behaviors, the July reproduction evidence, and
ten verification steps, including the render under load the brief requires. Its
provenance is a loop sweep, so it lands through a normal commit inside
`port-tui-stream-history`. Porting it does not retroactively justify the sweep
that first committed it. The file carries no frontmatter, unlike the other
`.ralph/tasks/*.code-task.md` documents.

## Port plan

Two commits, one per cluster. Both take rollup tip content. Neither replays a
commit. The verification task follows both.

1. `port-tui-stream-history` (`task-1790097924-b4fc`). Take the rollup tip
   content of `crates/ralph-tui/src/autoloop_source.rs` (2053 lines) and
   `crates/ralph-adapters/src/backend_stream_tailer.rs` (881 lines). Add
   `sha2.workspace = true` to `crates/ralph-adapters/Cargo.toml`; it is the only
   change to that file. Port
   `.ralph/tasks/tui-stream-history-backpressure.code-task.md`. Let `cargo`
   rewrite `Cargo.lock` and commit the result with the `sha2` line, since the
   `ralph-adapters` edge is new. Prove the bounding with a render under load.
2. `port-live-harness-smoke` (`task-1790097924-c8bd`). Take the rollup tip
   content of `presets/live-harness-smoke/` (12 files),
   `tools/smoke-live-harnesses.sh`, `tools/smoke_live_harness_results.py`,
   `tools/smoke_process_group.py`, `tools/tests/test_smoke_live_harnesses.py`,
   `.ralph/specs/manual-live-harness-smoke.spec.md`,
   `.ralph/tasks/manual-live-harness-smoke.code-task.md`, the `presets/README.md`
   section (7 added lines), and the `.github/workflows/ci.yml` step (6 added
   lines). Every one of those paths is absent from `HEAD` or untouched by us
   since `2e1fc52`. Leave `tools/smoke-core-presets.sh` alone, since it already
   exists in both trees.
3. `verify-reconciliation` (`task-1790097924-daa7`). Confirm every row above is
   settled and that `v3/complete` is not behind the rollup on any wanted
   surface.

## Unmeasured and inferred

- The rollup tip content of the two TUI files and the `sha2` line compiles and
  passes `cargo test -p ralph-adapters -p ralph-tui` at `44afa19`. Measured.
  Transcript at `logs/step-02-port-mechanism.md`.
- The render-under-load proof for bounded stream history is not measured here.
  It belongs to `port-tui-stream-history`.
- Whether autoloop 0.11.0 breaks a preset mapping remains unverified. It belongs
  to Step 12, as the remnant log records.

## Verification (revision 3, the Step 2 gate at `635cb8c`)

`task-1790097924-daa7` /
`code-assist:v3-complete:step-02:verify-reconciliation`. Every claim below was
measured this turn. Raw transcripts live in the runtime planning directory at
`.ralph/specs/v3-complete/logs/step-02-verify-reconciliation.md`,
`step-02-verify-rust.txt`, and `step-02-verify-python.txt`. That directory is not
committed.

### The inventory is closed

```bash
git rev-parse --is-shallow-repository
git rev-list --count HEAD..origin/wip/v3-prerelease-rollup
git rev-parse origin/wip/v3-prerelease-rollup
git merge-base HEAD origin/wip/v3-prerelease-rollup
```

```text
false
22
6e2545d63e21c0fe34dc6bd48ad0c7b2929145c0
2e1fc52e3286520540eb2538fa5d5ae53baf72c9
```

The table above holds 22 rows, and their short SHAs are exactly the 22
rollup-only SHAs. `comm` is empty in both directions, and the tally is 16
`port`, 4 `already-covered`, 2 `superseded`. Each of the 8 base-only commits
passes `git merge-base --is-ancestor <sha> HEAD`, so no base-side work is
missing.

### Every touched path is classified

The union of the file lists of the 22 commits, merges excluded, is 31 paths.
Every path appears in the port plan or in a row reason. No path is unaccounted
for.

### The TUI cluster is contained, measured

```bash
git diff --numstat HEAD origin/wip/v3-prerelease-rollup -- \
  crates/ralph-adapters/src/backend_stream_tailer.rs \
  crates/ralph-tui/src/autoloop_source.rs
```

```text
0	189	crates/ralph-tui/src/autoloop_source.rs
```

`backend_stream_tailer.rs` is byte-identical to the rollup tip, so it prints no
row. `autoloop_source.rs` prints `0` insertions, which means the rollup tip adds
no line this branch lacks. The 189 deletions are this branch's render-under-load
test and its helper, the one hunk this record left open for
`port-tui-stream-history`.

No TUI commit carries a `superseded` verdict, so the plan's rendered-artifact
requirement has no subject. The two TUI-adjacent `already-covered` rows are
settled by artifact rather than argument:
`crates/ralph-tui/src/widgets/help.rs` is byte-identical to the rollup tip, and
`49434db`'s dead-code removal is present, `push_lines` appearing 0 times at
`HEAD`.

The render proof runs green.

```text
cargo test -p ralph-tui --lib render_under_load_keeps_one_truthful_status_and_newest_lines
test autoloop_source::tests::render_under_load_keeps_one_truthful_status_and_newest_lines ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 273 filtered out

cargo test -p ralph-cli --test integration_autoloop_tui_live_stream
test result: ok. 2 passed; 0 failed
```

### The smoke cluster is present and green

All 12 files under `presets/live-harness-smoke/` exist at `HEAD`, as do the four
`tools/` paths and the two `.ralph/` docs. Thirteen paths across the two ports
are byte-identical to the rollup tip: `autoloops.toml`, `DOGFOOD.md`,
`harness.md`, all six `roles/*.md`, `topology.toml`,
`.ralph/specs/manual-live-harness-smoke.spec.md`,
`.ralph/tasks/tui-stream-history-backpressure.code-task.md`, and
`presets/README.md`.

Eight paths differ, and every difference is this branch's own adaptation. Four
carry the Ralph-owned state root, so `HEAD` reads and writes `.ralph/autoloop`
where the rollup reads `.autoloop`: `presets/live-harness-smoke/README.md`,
`presets/live-harness-smoke/scripts/require_smoke_handoff.py`,
`tools/smoke-live-harnesses.sh`, and
`.ralph/tasks/manual-live-harness-smoke.code-task.md`. Three carry analyzer fixes
on the ported bytes, all behavior preserving:
`tools/smoke_live_harness_results.py` fixes a dataclass-alias collision and a
`None` guard, `tools/smoke_process_group.py` drops an unused import and uses
`contextlib.suppress`, and `require_smoke_handoff.py` imports `NoReturn` instead
of annotating with a string. `.github/workflows/ci.yml` differs cosmetically:
`HEAD` folds the unittest invocation with `>-` and the rollup writes it on one
line. The command is the same.

The ported tooling runs.

```text
PYTHONDONTWRITEBYTECODE=1 .venv/bin/python -m unittest -v tools.tests.test_smoke_live_harnesses
Ran 10 tests in 5.454s
OK
claude claude-sdk PASS / codex command PASS / opencode command PASS
pi pi PASS / hermes acp PASS / kiro acp PASS
```

### The state root holds

`c90001e` is `already-covered`, and that verdict is verified against runtime
behavior rather than the file-size claim in its row.

```text
cargo test -p ralph-core --lib engine_state
test result: ok. 11 passed; 0 failed; 0 ignored; 740 filtered out
```

The set includes `engine_state_root_is_under_ralph`,
`engine_env_pins_every_store_beneath_the_root`,
`engine_config_overrides_pin_every_store_beneath_the_root`, and four symlink
rejection cases.

### The `already-covered` paths are byte-identical

`crates/ralph-adapters/src/autoloop_events.rs`,
`crates/ralph-adapters/tests/autoloop_native_contract_integration.rs`,
`crates/ralph-core/src/event_parser.rs`, and
`crates/ralph-tui/src/widgets/help.rs` each produce no output from
`git diff HEAD origin/wip/v3-prerelease-rollup -- <path>`.

`49434db`'s `autoloop_runner.rs` hunk does not survive as a patch, and it does
not need to. `HEAD` already carries the same clippy outcome: `AutoloopBin`
derives `Default` with a `#[default]` variant at `:82` to `:86`, no manual
`impl Default for AutoloopBin` remains, and both `cost_usd` assertions compare
with `f64::EPSILON` at `:595` and `:611`.

### Correction to the `3322fd7` row

Revision 2 called that row `superseded` because "the rollup copy is behind
`v3/complete`". The verdict stands and the reason does not. The two copies of
`.ralph/tasks/ralph-owned-autoloop-state.code-task.md` diverge in both
directions.

```text
-- [x] `cargo clippy --workspace --all-targets --all-features -- -D warnings`
+- [x] `cargo clippy --all-targets --all-features -- -D warnings`
-      `.../ralph-owned-state-dir`: 3 passed,
+      `.../ralph-owned-state-dir`: 4 passed,
```

`-` is `HEAD` and `+` is the rollup. `HEAD` carries the workspace-wide clippy
invocation and the rollup carries the higher pass count. Both lines are run-log
bookkeeping in a code-task document, and neither is a shipped surface. The code
the row describes is present through `c90001e`, which the state-root section
above verifies. The row is corrected in place.

### What this revision changed

Verdicts: none. Reasons: one, `3322fd7`. Evidence added: the sections above.
