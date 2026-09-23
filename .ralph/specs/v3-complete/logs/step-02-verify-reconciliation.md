# daa7 verification evidence (measured 2026-09-22T19:01:14Z)

## A. Rollup-only commit inventory
```
$ git rev-list --count HEAD..origin/wip/v3-prerelease-rollup
22
$ git rev-parse origin/wip/v3-prerelease-rollup
6e2545d63e21c0fe34dc6bd48ad0c7b2929145c0
$ git merge-base HEAD origin/wip/v3-prerelease-rollup
2e1fc52e3286520540eb2538fa5d5ae53baf72c9
```

## B. Port-cluster paths, HEAD vs rollup tip (numstat HEAD->rollup)
A nonzero insertion count means the rollup has lines this branch lacks.
```
1	3	.github/workflows/ci.yml
1	1	.ralph/tasks/manual-live-harness-smoke.code-task.md
0	1	Cargo.lock
0	189	crates/ralph-tui/src/autoloop_source.rs
3	7	presets/live-harness-smoke/README.md
3	8	presets/live-harness-smoke/scripts/require_smoke_handoff.py
3	7	tools/smoke-live-harnesses.sh
8	12	tools/smoke_live_harness_results.py
5	10	tools/smoke_process_group.py
11	18	tools/tests/test_smoke_live_harnesses.py
```

## C. already-covered file identity (empty diff = identical)
```
crates/ralph-adapters/src/autoloop_events.rs identity-diff-lines=0
crates/ralph-cli/tests/autoloop_native_contract_integration.rs ABSENT-IN-ROLLUP
crates/ralph-core/src/event_parser.rs identity-diff-lines=0
crates/ralph-tui/src/widgets/help.rs identity-diff-lines=0
```

## D. superseded rows (rollup copy behind HEAD)
```
$ git diff HEAD origin/wip/v3-prerelease-rollup -- .ralph/tasks/ralph-owned-autoloop-state.code-task.md
diff --git a/.ralph/tasks/ralph-owned-autoloop-state.code-task.md b/.ralph/tasks/ralph-owned-autoloop-state.code-task.md
index 7a569ae..aec344d 100644
--- a/.ralph/tasks/ralph-owned-autoloop-state.code-task.md
+++ b/.ralph/tasks/ralph-owned-autoloop-state.code-task.md
@@ -76,10 +76,10 @@ review findings or final gates are complete.
       unit tests 27 passed; failure-reporting fake-process attacks 12 passed;
       affected headless/TUI/merge fake-process suites 5 passed.
 - [x] `cargo fmt --all --check`
-- [x] `cargo clippy --workspace --all-targets --all-features -- -D warnings`
+- [x] `cargo clippy --all-targets --all-features -- -D warnings`
 - [x] Full `cargo test`: 2730 passed, 37 ignored.
 - [x] Native contracts against
-      `/Users/rook/.herdr/worktrees/autoloop/ralph-owned-state-dir`: 3 passed,
+      `/Users/rook/.herdr/worktrees/autoloop/ralph-owned-state-dir`: 4 passed,
       non-skipped.
 - [x] `git diff --check`
 - [x] Inspect `git diff 54430971...HEAD`: state-root behavior and both
$ git cat-file -e HEAD:.ralph/tasks/tui-stream-history-backpressure.code-task.md
present in HEAD
$ git show HEAD:.ralph/tasks/tui-stream-history-backpressure.code-task.md | wc -l
83
$ git show origin/wip/v3-prerelease-rollup:.ralph/tasks/tui-stream-history-backpressure.code-task.md | wc -l
83
```

## E. base-only commits are ancestors of HEAD
```
22fc1fd1fbe6157207df70148401943f6b00c823 IN HEAD
70b3360aa1bac5a19a037a5d6effb268d30c798e IN HEAD
8276db03162b47961b7c9a00a197c56e2cbc5267 IN HEAD
893f129e9c5660cfecd07ae6b5d7a6162345bc97 IN HEAD
47a568e147ab9fe5cf79ff9b5a80b1ff2fcc7996 IN HEAD
775e98a939de9c0a43756595e0178a1dbcbd898f IN HEAD
42359f31a8a48cff8114b1f95a4bfe1d2ee326ff IN HEAD
92e4992b353da2323b3a34411ad9eb24e4576204 IN HEAD
```

## F. classification table completeness
```
rows in table: 22
rollup-only commits: 22
port rows: 16
already-covered rows: 4
superseded rows: 3
table SHAs not in the rollup-only set:
rollup-only SHAs missing from the table:
```

## G. remaining identity claims (empty output = identical to rollup tip)
```
(no lines above means every listed path is byte-identical)
$ git diff --stat HEAD origin/wip/v3-prerelease-rollup -- crates/ralph-adapters/Cargo.toml Cargo.lock
0	1	Cargo.lock
```

## H. 3322fd7 correction: the two doc copies
```
$ git diff HEAD origin/wip/v3-prerelease-rollup -- .ralph/tasks/ralph-owned-autoloop-state.code-task.md
-      `/Users/rook/.herdr/worktrees/autoloop/ralph-owned-state-dir`: 3 passed,
+      `/Users/rook/.herdr/worktrees/autoloop/ralph-owned-state-dir`: 4 passed,
```
