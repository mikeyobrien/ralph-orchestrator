# v3 branch reconciliation

Phase 1 of the v3 completion brief. This record enumerates the two v3 deltas,
classifies every rollup-only commit, and names the ports that follow.

- Branch: `v3/complete`
- Date: 2026-09-22
- Rollup under review: `origin/wip/v3-prerelease-rollup` at `6e2545d`
- Merge base of `v3/complete` and the rollup: `2e1fc52`
- Classification measured at `80309dc`, re-verified at `44afa19`, and verified
  against the landed ports at `56690c0` (code-identical to `635cb8c`; the only
  commit between them edits this file)
- Source edits made by this record: none
- Revision 2 corrects the port mechanism. The critic rejected revision 1 at
  `44afa19` for a false overlap claim, an inapplicable replay mechanism, and a
  wrong leaf count. The 22-row classification was confirmed sound and is
  unchanged.
- Revision 3 is the Step 2 gate, `task-1790097924-daa7` / `code-assist:v3-complete:step-02:verify-reconciliation`.
  It re-measured the landed state at `635cb8c` and appended the evidence in
  Verification. It changes no verdict and corrects one row reason, `3322fd7`.
  It did not re-measure the classification table's per-row evidence, which is
  stated at `80309dc`; revision 4 closes that gap.
- Revision 4 re-measures every state claim in the classification table and in
  Why the ports take final content at `56690c0`, and labels each one with the
  commit it was measured at. It changes no verdict.
- Revision 5 corrects two false state claims revision 4 introduced: the size of
  the `comm -12` overlap set at `56690c0` (25 paths, not 24) and a sentence that
  attributed our `80309dc` tree's missing `sha2` edge to the rollup tip, which
  carries it. It changes no verdict.
- Revision 6 corrects two false state claims the revision-5 critic found in
  revision-3 text: the smoke cluster carries the state root in five paths, not
  four, and five tracked `.ralph/tasks/*.code-task.md` docs at `HEAD` carry no
  frontmatter, not one. It changes no verdict.

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

Each row's Evidence states what the commit contains and what `HEAD` held when
the row was classified. Every state claim about `HEAD` names the commit it was
measured at: `80309dc` for the classification, `56690c0` for the landed tree.
Nine rows carried their `80309dc` measurement as unlabelled present tense; each
`port` row now states the landed state as well. The five non-`port` rows that
make a state claim were re-measured at `56690c0` and are unchanged.

| Commit | Subject | Verdict | Evidence and reason |
| --- | --- | --- | --- |
| `6e2545d` | Merge WIP live harness smoke preset | port | Merge that lands the smoke cluster. Porting the ten leaf commits below covers it. Measured at `80309dc`: `git cat-file -e HEAD:presets/live-harness-smoke` failed, so the cluster was absent. At `56690c0` the directory is present, ported by `b062e52` (`c8bd`). |
| `7c9b0ff` | Merge WIP TUI stream history and backpressure fixes | port | Merge that lands the TUI cluster. Covered by taking the rollup tip content of the two TUI files. The commits cannot be replayed; see Why the ports take final content. |
| `c90001e` | Merge WIP Ralph-owned Autoloop state | already-covered | The merge delta is 2 lines in `.ralph/tasks/ralph-owned-autoloop-state.code-task.md`. The code lives at `crates/ralph-core/src/engine_state.rs`, present at `56690c0` with 419 lines against the rollup's 377. Tests at `56690c0` assert `.ralph/autoloop` is the state root and that no top-level `.autoloop` appears. |
| `3322fd7` | docs(engine): correct final gate evidence | superseded | Run-log bookkeeping in a code-task doc, not a shipped surface. The code it describes is present through `c90001e`. The two doc copies diverge in both directions, so neither is simply behind the other: the rollup carries "4 passed" where `56690c0` carries "3 passed", and `56690c0` carries `cargo clippy --workspace` where the rollup does not. Measured in Verification, Correction to the `3322fd7` row. |
| `2242eec` | fix(smoke): harden live provider safety gates | port | Adds `tools/smoke_process_group.py` and rewrites the runner. At `80309dc` `git cat-file -e HEAD:tools/smoke_process_group.py` failed; at `56690c0` the path is present, ported by `b062e52`. |
| `5b7876c` | fix(tui): bound stream identities and lifecycle lines | port | `crates/ralph-adapters/src/backend_stream_tailer.rs` was 514 lines at `80309dc` and 881 in the rollup. `Cargo.lock` adds `sha2` to `ralph-adapters`. At `56690c0` the file is 881 lines and byte-identical to the rollup tip. |
| `1e67e52` | chore: auto-commit before merge (loop primary) | superseded | It is not a change. It swept a then-untracked `.ralph/tasks/tui-stream-history-backpressure.code-task.md`. It is live evidence for `landing-untracked-sweep-yxv`, not a port. The swept doc itself is ported with the TUI cluster; see The swept code-task doc. |
| `faa2b71` | fix(smoke): validate canonical completion without retries | port | Part of the smoke cluster. Absent at `80309dc`; present at `56690c0` through the `b062e52` port. |
| `86066b6` | fix(tui): protect reconciled history under line pressure | port | `crates/ralph-tui/src/autoloop_source.rs` was 1331 lines at `80309dc` and 2053 in the rollup. At `56690c0` it is 2242 lines: the rollup tip content plus this branch's 189-line render-under-load test. |
| `ebeb81f` | fix(smoke): abort immediately on missing provider handoff | port | Adds `presets/live-harness-smoke/scripts/require_smoke_handoff.py`. Absent at `80309dc`; present at `56690c0` through the `b062e52` port. |
| `1a2a43d` | fix(smoke): render executable contracts for every role | port | Rewrites the six role contracts. Absent at `80309dc`; all six `roles/*.md` are present at `56690c0` through the `b062e52` port. |
| `0a5f660` | fix(smoke): resolve provider-visible run evidence path | port | Adds the manual spec and code-task docs. Absent at `80309dc`; `.ralph/specs/manual-live-harness-smoke.spec.md` and `.ralph/tasks/manual-live-harness-smoke.code-task.md` are present at `56690c0` through the `b062e52` port. |
| `49434db` | chore: satisfy strict touched-crate clippy | already-covered | `autoloop_events.rs` and `autoloop_native_contract_integration.rs` are identical between `56690c0` and the rollup by `git diff --numstat`. Its `autoloop_runner.rs` hunk sits on a pre-resume base and is superseded by `56690c0`, which carries `AutoloopRunner::resume` at 1032 lines against the rollup's 896. Its two TUI hunks are already inside the rollup tip content the TUI port takes. |
| `3d4b8ca` | fix: satisfy clippy lifetime lint | already-covered | `crates/ralph-core/src/event_parser.rs` is identical between `56690c0` and the rollup. |
| `2ac3c1f` | style: apply workspace rustfmt | already-covered | `crates/ralph-tui/src/widgets/help.rs` is identical between `56690c0` and the rollup. Its `backend_stream_tailer.rs` hunk is already inside the rollup tip content the TUI port takes. |
| `e275303` | fix: preserve bounded TUI stream history | port | Same file pair as `86066b6`. 348 insertions against the rollup's merge base. Ported by final content, not replayed. |
| `de2eaa4` | fix: bound backend stream identity and backpressure | port | 342 insertions across `backend_stream_tailer.rs` and `autoloop_source.rs`. |
| `3c8eaed` | docs(presets): explain manual live smoke | port | Adds the `presets/README.md` section and the preset README. |
| `91b094d` | test(smoke): cover fake live harness matrix | port | Adds `tools/tests/test_smoke_live_harnesses.py` and a CI step. |
| `47f8dfd` | feat(tools): validate live smoke evidence | port | Adds `tools/smoke_live_harness_results.py`. |
| `830ecfe` | feat(tools): add bounded live harness smoke runner | port | Adds `tools/smoke-live-harnesses.sh`. |
| `e0178bf` | feat(presets): add manual live harness smoke | port | Adds `presets/live-harness-smoke/`, absent at `80309dc`; the 12-file preset is present at `56690c0` through the `b062e52` port. |

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
git merge-base --is-ancestor 2e1fc52 80309dc && echo "ancestor of the classification commit"
git merge-base --is-ancestor 2e1fc52 origin/wip/v3-prerelease-rollup && echo "ancestor of rollup tip"
comm -12 <(git diff --name-only 2e1fc52 80309dc | sort) \
         <(git diff --name-only 2e1fc52 origin/wip/v3-prerelease-rollup | sort)
```

```text
ancestor of the classification commit
ancestor of rollup tip
Cargo.lock
```

`2e1fc52` is the merge base and an ancestor of both tips, so the rollup tip
already contains every base-branch change up to the merge base. At `80309dc`,
the classification commit, `Cargo.lock` was the only file both sides had changed
after it, and our two TUI files were byte-identical to the merge base. At
`56690c0` both sides also carry the two landed ports and the smoke cluster, so
that `comm` set has grown to 25 paths (`comm -12` and `grep -Fxf` over the same
two file lists both print 25). That does not weaken the structural argument; it
means the argument's measurement commit has to be named.

```bash
git diff --stat 2e1fc52 80309dc -- crates/ralph-tui/src/autoloop_source.rs \
  crates/ralph-adapters/src/backend_stream_tailer.rs
git diff --stat 2e1fc52 56690c0 -- crates/ralph-tui/src/autoloop_source.rs \
  crates/ralph-adapters/src/backend_stream_tailer.rs
```

```text
(no output at 80309dc: both files were byte-identical to the merge base)
 crates/ralph-adapters/src/backend_stream_tailer.rs |  451 +++++++-
 crates/ralph-tui/src/autoloop_source.rs            | 1113 ++++++++++++++++++--
 2 files changed, 1421 insertions(+), 143 deletions(-)
```

So for every wanted path except `Cargo.lock`, taking the rollup tip's content
loses nothing of ours. `Cargo.lock` is not taken verbatim, and the `sha2` port
does change it. The `sha2` package was already resolved at `Cargo.lock:3657` at
`80309dc`, but `Cargo.lock` records dependency edges per package, and
`ralph-adapters` had no `sha2` edge then. Measured in a detached worktree at the
then-`HEAD` with `sha2.workspace = true` added to
`crates/ralph-adapters/Cargo.toml`:

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

At `80309dc` the rollup tip carried the seam in the other direction: 12
`reader_engine_root` call sites against our 8.

```bash
git show 80309dc:crates/ralph-tui/src/autoloop_source.rs | grep -c reader_engine_root
git show 56690c0:crates/ralph-tui/src/autoloop_source.rs | grep -c reader_engine_root
git show origin/wip/v3-prerelease-rollup:crates/ralph-tui/src/autoloop_source.rs | grep -c reader_engine_root
```

```text
8
14
12
```

At `56690c0` the count is 14: the rollup tip's 12 came across with the port, and
this branch's render-under-load test adds the other two.

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

`.ralph/tasks/tui-stream-history-backpressure.code-task.md` was absent from
`HEAD` at `80309dc` and present in the rollup at 83 lines. `1e67e52`, the
landing auto-commit sweep, created it. `5b7876c` appends 5 lines. At `56690c0`
it is present, ported by `5fd8828`, and 83 lines in both trees.

Port it with the TUI cluster. It is the acceptance spec for that work: it names
the two beads, the two required behaviors, the July reproduction evidence, and
ten verification steps, including the render under load the brief requires. Its
provenance is a loop sweep, so it lands through a normal commit inside
`port-tui-stream-history`. Porting it does not retroactively justify the sweep
that first committed it. The file carries no frontmatter. Four other tracked
`.ralph/tasks/*.code-task.md` docs at `HEAD` carry none either:
`backend-agnostic-e2e`, `context-window-utilization`,
`manual-live-harness-smoke`, and `multi-loop-concurrency`.

```bash
git ls-tree --name-only HEAD .ralph/tasks/ | grep '\.code-task\.md$' | while read f; do
  [ "$(git show HEAD:"$f" | head -1)" = "---" ] || echo "$f"
done
```

```text
.ralph/tasks/backend-agnostic-e2e.code-task.md
.ralph/tasks/context-window-utilization.code-task.md
.ralph/tasks/manual-live-harness-smoke.code-task.md
.ralph/tasks/multi-loop-concurrency.code-task.md
.ralph/tasks/tui-stream-history-backpressure.code-task.md
```

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
   lines). Every one of those paths was absent from `HEAD` at `80309dc` or
   untouched by us since `2e1fc52`. Leave `tools/smoke-core-presets.sh` alone, since it already
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
measured at `635cb8c`, the landed tree this revision re-verified. Raw transcripts
live in the runtime planning directory at
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

Eight paths differ, and every difference is this branch's own adaptation. Five
carry the Ralph-owned state root, so `HEAD` reads and writes `.ralph/autoloop`
where the rollup reads `.autoloop`: `presets/live-harness-smoke/README.md`,
`presets/live-harness-smoke/scripts/require_smoke_handoff.py`,
`tools/smoke-live-harnesses.sh`,
`.ralph/tasks/manual-live-harness-smoke.code-task.md`, and
`tools/tests/test_smoke_live_harnesses.py`. Three carry analyzer fixes on the
ported bytes, all behavior preserving:
`tools/smoke_live_harness_results.py` fixes a dataclass-alias collision and a
`None` guard, `tools/smoke_process_group.py` drops an unused import and uses
`contextlib.suppress`, and `require_smoke_handoff.py` imports `NoReturn` instead
of annotating with a string. `require_smoke_handoff.py` is the one path in both
groups, so the two groups name eight distinct files with `ci.yml`. That file
differs cosmetically: `HEAD` folds the unittest invocation with `>-` and the
rollup writes it on one line. The command is the same.

```bash
for p in $(git diff --name-only HEAD origin/wip/v3-prerelease-rollup -- \
  presets/live-harness-smoke tools/smoke-live-harnesses.sh \
  tools/smoke_live_harness_results.py tools/smoke_process_group.py \
  tools/tests/test_smoke_live_harnesses.py presets/README.md \
  .ralph/specs/manual-live-harness-smoke.spec.md \
  .ralph/tasks/manual-live-harness-smoke.code-task.md \
  .github/workflows/ci.yml); do
  printf '%s %s %s\n' "$(git show HEAD:"$p" | grep -c '\.ralph/autoloop')" \
    "$(git show origin/wip/v3-prerelease-rollup:"$p" | grep -c '\.ralph/autoloop')" "$p"
done | sort -k1,1nr
```

```text
11 0 tools/tests/test_smoke_live_harnesses.py
4 0 presets/live-harness-smoke/README.md
2 0 tools/smoke-live-harnesses.sh
1 0 presets/live-harness-smoke/scripts/require_smoke_handoff.py
1 0 .ralph/tasks/manual-live-harness-smoke.code-task.md
0 0 .github/workflows/ci.yml
0 0 tools/smoke_live_harness_results.py
0 0 tools/smoke_process_group.py
```

`test_smoke_live_harnesses.py` also differs from the rollup tip in two
behavior-preserving edits the state root does not explain: its import order puts
`textwrap` before `time` where the rollup puts `time` first, and it narrows with
`if match is None: self.fail(combined)` where the rollup calls
`self.assertIsNotNone(match, combined)`.

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

## Verification (revision 4, the same gate re-checked at `56690c0`)

`task-1790097924-daa7` /
`code-assist:v3-complete:step-02:verify-reconciliation`. Revision 3 was rejected
at `56690c0`. The rejection named five sentences that carried their `80309dc`
measurement as unlabelled present tense. Re-measuring every state claim in the
classification table and in Why the ports take final content found **nine rows
and four further claims** of the same class: the `comm` set, the "two TUI files
are byte-identical to the merge base" reading, the "`ralph-adapters` has no
`sha2` edge today" reading, and the `reader_engine_root` 8/12 reading. Each now
names its measurement commit and, where a port landed afterwards, the landed
reading. No verdict changed.

Raw transcript at
`.ralph/specs/v3-complete/logs/step-02-verify-reconciliation-rev4.txt`.

### `56690c0` is `635cb8c` plus this file

```bash
git diff --name-only 635cb8c 56690c0
git diff --stat 635cb8c 56690c0 | tail -1
```

```text
.ralph/specs/v3-reconciliation.md
 .ralph/specs/v3-reconciliation.md | 168 +++++++++++++++++++++++++++++++++++++-
 1 file changed, 166 insertions(+), 2 deletions(-)
```

The only commit between the two is the docs commit that landed revision 3, so
every code and test measurement in the revision 3 section above holds unchanged
at `56690c0`.

### The corrected state claims, re-measured

Presence of the nine smoke-cluster paths, at the classification commit and at
`HEAD`:

```bash
for p in presets/live-harness-smoke presets/live-harness-smoke/scripts/require_smoke_handoff.py \
         tools/smoke_process_group.py tools/smoke_live_harness_results.py \
         tools/smoke-live-harnesses.sh tools/tests/test_smoke_live_harnesses.py \
         .ralph/specs/manual-live-harness-smoke.spec.md \
         .ralph/tasks/manual-live-harness-smoke.code-task.md \
         .ralph/tasks/tui-stream-history-backpressure.code-task.md; do
  for c in 80309dc 56690c0; do
    git cat-file -e "$c:$p" 2>/dev/null && echo "PRESENT $c $p" || echo "ABSENT  $c $p"
  done
done
```

```text
ABSENT  80309dc for all nine paths
PRESENT 56690c0 for all nine paths
```

Line counts, `HEAD` against the rollup tip:

```text
HEAD=881   ROLLUP=881   crates/ralph-adapters/src/backend_stream_tailer.rs
HEAD=2242  ROLLUP=2053  crates/ralph-tui/src/autoloop_source.rs
HEAD=419   ROLLUP=377   crates/ralph-core/src/engine_state.rs
HEAD=1032  ROLLUP=896   crates/ralph-adapters/src/autoloop_runner.rs
HEAD=83    ROLLUP=83    .ralph/tasks/tui-stream-history-backpressure.code-task.md
80309dc=514   crates/ralph-adapters/src/backend_stream_tailer.rs
80309dc=1331  crates/ralph-tui/src/autoloop_source.rs
```

`engine_state.rs` (419/377) and `autoloop_runner.rs` (1032/896) are the two row
counts that were already stated in present tense and are still correct at
`56690c0`. The other three rows were not.

The TUI numstat, unchanged by this revision:

```bash
git diff --numstat HEAD origin/wip/v3-prerelease-rollup -- \
  crates/ralph-adapters/src/backend_stream_tailer.rs \
  crates/ralph-tui/src/autoloop_source.rs
```

```text
0	189	crates/ralph-tui/src/autoloop_source.rs
```

`reader_engine_root` counts: 8 at `80309dc`, 14 at `56690c0`, 12 at the rollup
tip. The `sha2` package sits at `Cargo.lock:3657` at `80309dc` and `:3658` at
`56690c0`; the `ralph-adapters` edge is at `Cargo.lock:2766` and
`crates/ralph-adapters/Cargo.toml:19` at `56690c0`. Our `80309dc` tree carried
neither: no `sha2` line in `crates/ralph-adapters/Cargo.toml`, and no `"sha2"`
entry in the lock's `ralph-adapters` block. That is why the lock has to move
with the manifest. The rollup tip carries both, so the port takes them from
there.

### The ported surfaces still run green at `56690c0`

```text
cargo test -p ralph-tui --lib render_under_load_keeps_one_truthful_status_and_newest_lines
test result: ok. 1 passed; 0 failed; 0 ignored; 273 filtered out

cargo test -p ralph-cli --test integration_autoloop_tui_live_stream
test result: ok. 2 passed; 0 failed; 0 ignored; 0 filtered out

cargo test -p ralph-core --lib engine_state
test result: ok. 11 passed; 0 failed; 0 ignored; 740 filtered out

PYTHONDONTWRITEBYTECODE=1 .venv/bin/python -m unittest -v tools.tests.test_smoke_live_harnesses
Ran 10 tests in 5.258s
OK
claude claude-sdk PASS / codex command PASS / opencode command PASS
pi pi PASS / hermes acp PASS / kiro acp PASS
```

### What revision 4 changed

Verdicts: none. Reasons: none. The nine `port` rows, the three structural state
claims in Why the ports take final content, the `reader_engine_root` reading, and
the swept-doc section now name the commit they were measured at and state the
landed reading. The header's revision-3 bullet now claims only that the landed
state was re-measured, which is what revision 3 did.

### What revision 5 changed

Revision 4 was rejected at `a04126e` for two false state claims. Both were
introduced by revision 4 itself, and both are one-line fixes.

**The `comm` set size.** The sentence closing Why the ports take final content
said the set "has grown to 24 paths". It is 25 at `56690c0`. `comm -12` over the
two `git diff --name-only` lists prints 25, and `grep -Fxf` over the same two
lists prints 25. The 24 is the set with `Cargo.lock` removed, which is not what
the sentence counted, because its previous clause counts `Cargo.lock` as part of
the set.

**The `sha2` attribution.** The sentence said "the rollup tip carries no `sha2`
edge for `ralph-adapters` at `80309dc` either". That attributed our tree's state
to the rollup tip. The rollup tip carries the edge twice: `sha2.workspace = true`
at `crates/ralph-adapters/Cargo.toml:19` and `"sha2"` inside its `Cargo.lock`
`ralph-adapters` block. Our `80309dc` tree carried neither, which is the claim
the paragraph needs.

The rest of the revision-4 text was re-measured at `56690c0` and holds: 22 rows
tallied 16 `port` / 4 `already-covered` / 2 `superseded`, the nine smoke paths
absent at `80309dc` and present at `56690c0`, line counts 881/881, 2242/2053,
419/377, 1032/896, 83/83, `reader_engine_root` 8/14/12, the `sha2` package at
`Cargo.lock:3657` and `:3658`, the `ralph-adapters` edge at `Cargo.lock:2766`
inside the block at `:2750`, the TUI numstat `0 189`, and the four
`already-covered` paths byte-identical to the rollup tip. No verdict changes and
no source is edited.

### What revision 6 changed

Revision 5 was rejected at `27ec1df` for two false state claims in revision-3
text that revisions 4 and 5 reported as re-measured. Both are one-line fixes.
Both commands below were re-run at `27ec1df` for this section.

**The state-root itemization.** The smoke cluster section said "Four carry the
Ralph-owned state root" and "Three carry analyzer fixes". Those two groups
named seven distinct files, because `require_smoke_handoff.py` sits in both, so
the itemization reached eight only by naming it twice. Five paths carry the
state root. The fifth is `tools/tests/test_smoke_live_harnesses.py`, and it is
also the eighth differing path the old text never named. Its count is 11 at
`HEAD` against 0 in the rollup, the largest in the cluster.

```bash
for p in $(git diff --name-only HEAD origin/wip/v3-prerelease-rollup -- \
  presets/live-harness-smoke tools/smoke-live-harnesses.sh \
  tools/smoke_live_harness_results.py tools/smoke_process_group.py \
  tools/tests/test_smoke_live_harnesses.py presets/README.md \
  .ralph/specs/manual-live-harness-smoke.spec.md \
  .ralph/tasks/manual-live-harness-smoke.code-task.md \
  .github/workflows/ci.yml); do
  printf '%s %s %s\n' "$(git show HEAD:"$p" | grep -c '\.ralph/autoloop')" \
    "$(git show origin/wip/v3-prerelease-rollup:"$p" | grep -c '\.ralph/autoloop')" "$p"
done | sort -k1,1nr
```

```text
11 0 tools/tests/test_smoke_live_harnesses.py
4 0 presets/live-harness-smoke/README.md
2 0 tools/smoke-live-harnesses.sh
1 0 presets/live-harness-smoke/scripts/require_smoke_handoff.py
1 0 .ralph/tasks/manual-live-harness-smoke.code-task.md
0 0 .github/workflows/ci.yml
0 0 tools/smoke_live_harness_results.py
0 0 tools/smoke_process_group.py
```

**The frontmatter sentence.** The swept code-task doc section said "The file
carries no frontmatter, unlike the other `.ralph/tasks/*.code-task.md`
documents." That made the file an exception to a rule that does not hold. Five
tracked docs at `HEAD` carry no frontmatter, the file itself plus the four the
text now names.

```bash
git ls-tree --name-only HEAD .ralph/tasks/ | grep '\.code-task\.md$' | while read f; do
  [ "$(git show HEAD:"$f" | head -1)" = "---" ] || echo "$f"
done
```

```text
.ralph/tasks/backend-agnostic-e2e.code-task.md
.ralph/tasks/context-window-utilization.code-task.md
.ralph/tasks/manual-live-harness-smoke.code-task.md
.ralph/tasks/multi-loop-concurrency.code-task.md
.ralph/tasks/tui-stream-history-backpressure.code-task.md
```

Nothing else moved. The rest of the revision-3 and revision-4 text was
re-measured at `27ec1df`: 12 preset files, 13 paths byte-identical to the rollup
tip by `diff -q` over `git show` output, the 8 differing paths listed above, the
83-line code-task doc in `HEAD`, `56690c0`, and the rollup tip, and the `ci.yml`
cosmetic difference in the direction the record states. The tally stays 16
`port`, 4 `already-covered`, 2 `superseded`, 22 rows. No verdict changes and no
source is edited.
