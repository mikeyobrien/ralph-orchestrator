# Step 2, port mechanism evidence

Task: `task-1790097921-986e` (`code-assist:v3-complete:step-02:classify-rollup-delta`),
reopened after the critic rejected `.ralph/specs/v3-reconciliation.md` at
`44afa19`. This log records the measurements behind the corrected port
mechanism. The 22-row classification is unchanged.

Worktree: `/home/mobrienv/Developer/ralph-orchestrator.worktrees/v3-complete`,
`HEAD` at `44afa19`. All commands run 2026-09-22.

## 1. The overlap is Cargo.lock only

The rejected revision claimed "the files changed on both sides since then are
`autoloop_source.rs` and `backend_stream_tailer.rs`". That is false.

```bash
comm -12 <(git diff --name-only 2e1fc52 HEAD | sort) \
         <(git diff --name-only 2e1fc52 origin/wip/v3-prerelease-rollup | sort)
```

```text
Cargo.lock
```

Our branch never touched either target file after the merge base:

```bash
git diff --stat 2e1fc52 HEAD -- crates/ralph-tui/src/autoloop_source.rs \
  crates/ralph-adapters/src/backend_stream_tailer.rs
```

```text
(no output)
```

## 2. Merge-base ancestry

```bash
git merge-base --is-ancestor 2e1fc52 HEAD && echo "ancestor of HEAD"
git merge-base --is-ancestor 2e1fc52 origin/wip/v3-prerelease-rollup && echo "ancestor of rollup tip"
git merge-base HEAD origin/wip/v3-prerelease-rollup
```

```text
ancestor of HEAD
ancestor of rollup tip
2e1fc52e3286520540eb2538fa5d5ae53baf72c9
```

`2e1fc52` is the merge base and an ancestor of both tips, so the rollup tip
already contains every base-branch change up to the merge base.

## 3. Replay fails

Throwaway worktree at `HEAD`, removed afterwards.

```bash
git worktree add --detach /tmp/v3-cherry-test HEAD
git cherry-pick --no-commit de2eaa4
git cherry-pick --no-commit e275303
```

```text
Auto-merging crates/ralph-tui/src/autoloop_source.rs
exit=0
Auto-merging crates/ralph-tui/src/autoloop_source.rs
CONFLICT (content): Merge conflict in crates/ralph-tui/src/autoloop_source.rs
error: could not apply e275303... fix: preserve bounded TUI stream history
```

The conflict is the `.ralph/autoloop` state seam:

```text
1216:<<<<<<< HEAD
1217-        let reader_engine_root = ralph_core::engine_state::engine_state_root(&workspace);
1218-        let handle = tokio::spawn(async move {
1219-            run_autoloop_event_reader(
1220-                reader_events,
1221-                workspace,
1222-                reader_engine_root,
```

`5b7876c` alone is worse. It conflicts in three paths, not one:

```bash
git cherry-pick --no-commit 5b7876c
```

```text
CONFLICT (modify/delete): .ralph/tasks/tui-stream-history-backpressure.code-task.md deleted in HEAD and modified in 5b7876c (fix(tui): bound stream identities and lifecycle lines).  Version 5b7876c (fix(tui): bound stream identities and lifecycle lines) of .ralph/tasks/tui-stream-history-backpressure.code-task.md left in tree.
Auto-merging Cargo.lock
Auto-merging crates/ralph-adapters/src/backend_stream_tailer.rs
CONFLICT (content): Merge conflict in crates/ralph-adapters/src/backend_stream_tailer.rs
Auto-merging crates/ralph-tui/src/autoloop_source.rs
CONFLICT (content): Merge conflict in crates/ralph-tui/src/autoloop_source.rs
error: could not apply 5b7876c... fix(tui): bound stream identities and lifecycle lines
exit=1
```

```text
DU .ralph/tasks/tui-stream-history-backpressure.code-task.md
M  Cargo.lock
M  crates/ralph-adapters/Cargo.toml
UU crates/ralph-adapters/src/backend_stream_tailer.rs
UU crates/ralph-tui/src/autoloop_source.rs
```

Root cause: the stack is linear on `aff233d`, where `autoloop_source.rs` is 1258
lines, and the base branch grew that file to 1331 at `2e1fc52`.

## 4. The rollup tip carries the seam

```bash
git show HEAD:crates/ralph-tui/src/autoloop_source.rs | wc -l
git show origin/wip/v3-prerelease-rollup:crates/ralph-tui/src/autoloop_source.rs | wc -l
git show HEAD:crates/ralph-adapters/src/backend_stream_tailer.rs | wc -l
git show origin/wip/v3-prerelease-rollup:crates/ralph-adapters/src/backend_stream_tailer.rs | wc -l
git show HEAD:crates/ralph-tui/src/autoloop_source.rs | grep -c reader_engine_root
git show origin/wip/v3-prerelease-rollup:crates/ralph-tui/src/autoloop_source.rs | grep -c reader_engine_root
```

```text
1331
2053
514
881
8
12
```

Both trees declare the same six-parameter reader:

```text
376:pub async fn run_autoloop_event_reader<S>(
637:pub async fn run_autoloop_event_reader<S>(
```

## 5. The swap compiles and passes tests

Throwaway worktree at `HEAD`, target directory shared with the main worktree.

```bash
git worktree add --detach /tmp/v3-tui-port-check HEAD
git checkout origin/wip/v3-prerelease-rollup -- \
  crates/ralph-tui/src/autoloop_source.rs \
  crates/ralph-adapters/src/backend_stream_tailer.rs \
  crates/ralph-adapters/Cargo.toml
CARGO_TARGET_DIR=<main worktree>/target cargo check -p ralph-tui -p ralph-adapters
CARGO_TARGET_DIR=<main worktree>/target cargo test -p ralph-adapters -p ralph-tui
```

```text
    Checking ralph-adapters v3.0.0 (/tmp/v3-tui-port-check/crates/ralph-adapters)
    Checking ralph-tui v3.0.0 (/tmp/v3-tui-port-check/crates/ralph-tui)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 12.66s

running 382 tests
test result: ok. 382 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 6.29s
running 273 tests
test result: ok. 273 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.71s
(remaining suites: 4, 3, 4, 3, 1, 2, 2, 20, 25, 4, 0, 1 passed; 0 failed)
```

`git status --porcelain` after the swap lists only the three checked-out paths.
`Cargo.lock` is a fourth. `sha2` is already resolved in our lock
(`Cargo.lock:3657`), but the `ralph-adapters` dependency edge is new, so `cargo`
adds `"sha2"` to the `ralph-adapters` `dependencies` block. Corrected after the
Fresh-Eyes Critic reproduced the failure in a fresh worktree:

```text
error: cannot update the lock file .../Cargo.lock because --locked was passed to prevent this
@@ -2763,6 +2763,7 @@ dependencies = [
  "ratatui",
  "serde",
  "serde_json",
+ "sha2",
  "tempfile",
```

The lock update is committed with the `sha2.workspace = true` line.

```bash
git diff HEAD origin/wip/v3-prerelease-rollup -- crates/ralph-adapters/Cargo.toml
```

```diff
@@ -16,6 +16,7 @@ tokio.workspace = true
 async-trait.workspace = true
 serde.workspace = true
 serde_json.workspace = true
+sha2.workspace = true
 thiserror.workspace = true
```

Both worktrees were removed with `git worktree remove --force` after the run.
