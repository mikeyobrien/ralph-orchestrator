# v3 branch reconciliation

Phase 1 of the v3 completion brief. This record enumerates the two v3 deltas,
classifies every rollup-only commit, and names the ports that follow.

- Branch: `v3/complete` at `80309dc`
- Date: 2026-09-22
- Rollup under review: `origin/wip/v3-prerelease-rollup` at `6e2545d`
- Merge base of `v3/complete` and the rollup: `2e1fc52`
- Source edits made by this record: none

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
| `6e2545d` | Merge WIP live harness smoke preset | port | Merge that lands the smoke cluster. Porting the eleven leaf commits below covers it. `git cat-file -e HEAD:presets/live-harness-smoke` fails, so the cluster is absent from `HEAD`. |
| `7c9b0ff` | Merge WIP TUI stream history and backpressure fixes | port | Merge that lands the TUI cluster. Porting `de2eaa4`, `e275303`, `86066b6`, and `5b7876c` covers it. |
| `c90001e` | Merge WIP Ralph-owned Autoloop state | already-covered | The merge delta is 2 lines in `.ralph/tasks/ralph-owned-autoloop-state.code-task.md`. The code lives at `crates/ralph-core/src/engine_state.rs`, present in `HEAD` with 419 lines against the rollup's 377. `HEAD` tests assert `.ralph/autoloop` is the state root and that no top-level `.autoloop` appears. |
| `3322fd7` | docs(engine): correct final gate evidence | superseded | Bookkeeping in the same code-task doc. It rewrites a clippy invocation and a pass count. No effect on the shipped tree. |
| `2242eec` | fix(smoke): harden live provider safety gates | port | Adds `tools/smoke_process_group.py` and rewrites the runner. `git cat-file -e HEAD:tools/smoke_process_group.py` fails. |
| `5b7876c` | fix(tui): bound stream identities and lifecycle lines | port | `crates/ralph-adapters/src/backend_stream_tailer.rs` is 514 lines in `HEAD` and 881 in the rollup. `Cargo.lock` adds `sha2` to `ralph-adapters`. |
| `1e67e52` | chore: auto-commit before merge (loop primary) | superseded | It is not a change. It swept a then-untracked `.ralph/tasks/tui-stream-history-backpressure.code-task.md`. It is live evidence for `landing-untracked-sweep-yxv`, not a port. |
| `faa2b71` | fix(smoke): validate canonical completion without retries | port | Part of the smoke cluster. Absent from `HEAD`. |
| `86066b6` | fix(tui): protect reconciled history under line pressure | port | `crates/ralph-tui/src/autoloop_source.rs` is 1331 lines in `HEAD` and 2053 in the rollup. |
| `ebeb81f` | fix(smoke): abort immediately on missing provider handoff | port | Adds `presets/live-harness-smoke/scripts/require_smoke_handoff.py`. Absent from `HEAD`. |
| `1a2a43d` | fix(smoke): render executable contracts for every role | port | Rewrites the six role contracts. Absent from `HEAD`. |
| `0a5f660` | fix(smoke): resolve provider-visible run evidence path | port | Adds the manual spec and code-task docs. Absent from `HEAD`. |
| `49434db` | chore: satisfy strict touched-crate clippy | already-covered | `autoloop_events.rs` and `autoloop_native_contract_integration.rs` are identical between `HEAD` and the rollup by `git diff --numstat`. Its `autoloop_runner.rs` hunk sits on a pre-resume base and is superseded by `HEAD`, which carries `AutoloopRunner::resume` at 1032 lines against the rollup's 896. Its two TUI hunks ride with the TUI port. |
| `3d4b8ca` | fix: satisfy clippy lifetime lint | already-covered | `crates/ralph-core/src/event_parser.rs` is identical between `HEAD` and the rollup. |
| `2ac3c1f` | style: apply workspace rustfmt | already-covered | `crates/ralph-tui/src/widgets/help.rs` is identical between `HEAD` and the rollup. The `backend_stream_tailer.rs` hunk rides with the TUI port. |
| `e275303` | fix: preserve bounded TUI stream history | port | Same file pair as `86066b6`. 348 insertions against the rollup's merge base. |
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

## Port plan

Two commits, one per cluster. The verification task follows both.

1. `port-tui-stream-history` (`task-1790097924-b4fc`). Port `de2eaa4`,
   `e275303`, `86066b6`, `5b7876c`, and the two TUI hunks from `2ac3c1f` and
   `49434db`. Add `sha2.workspace = true` to `crates/ralph-adapters/Cargo.toml`.
   `sha2 = "0.10"` is already a workspace dependency at `Cargo.toml:100`.
2. `port-live-harness-smoke` (`task-1790097924-c8bd`). Port `e0178bf`,
   `830ecfe`, `47f8dfd`, `91b094d`, `3c8eaed`, `0a5f660`, `1a2a43d`, `ebeb81f`,
   `faa2b71`, and `2242eec`, including the `presets/README.md` entry and the
   `ci.yml` step. Leave `tools/smoke-core-presets.sh` alone, since it already
   exists in both trees.
3. `verify-reconciliation` (`task-1790097924-daa7`). Confirm every row above is
   settled and that `v3/complete` is not behind the rollup on any wanted
   surface.

## Unmeasured and inferred

- Whether the TUI port applies cleanly is inferred, not measured. The merge base
  is `2e1fc52`, and the files changed on both sides since then are
  `autoloop_source.rs` and `backend_stream_tailer.rs`. The port task proves it.
- Whether autoloop 0.11.0 breaks a preset mapping remains unverified. It belongs
  to Step 12, as the remnant log records.
