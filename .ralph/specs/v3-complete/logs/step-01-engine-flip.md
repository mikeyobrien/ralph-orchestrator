# Step 1 evidence: the engine flip has landed

Task: `code-assist:v3-complete:step-01:verify-engine-flip` (`task-1790091216-0665`).
Runtime artifact. Not committed.

Repo state under test:

```
$ git log -1 --format='%H parents=%p'
df2449aaab15b3f2812369c784bfcc1c0642ed86 parents=22fc1fd 351b9f6
$ git rev-parse --is-shallow-repository
false
$ git branch --show-current
v3/complete
```

Verdict: **landed**. All four acceptance claims hold, each with a command and an
output excerpt below.

Line-number note. The brief cites `config.rs:2188` for the rejection and
`config.rs:2434` for the test. On this branch, after the main merge, the live
lines are `config.rs:512` and `config.rs:2689`. The brief's numbers are stale,
not wrong about the fact. Cite the live lines.

## Claim 1. Any `core.engine` other than `autoloop` is rejected

Command:

```
grep -n 'core.engine\|InvalidEngine' crates/ralph-core/src/config.rs
```

Output excerpt:

```
512:        if self.core.engine != "autoloop" {
513:            return Err(ConfigError::InvalidEngine {
514:                engine: self.core.engine.clone(),
515:            });
```

The guard sits at the top of `RalphConfig::validate`, before warning
suppression, so `_suppress_warnings` cannot reach it:

```rust
// Hard validation must run before warning suppression.
if self.core.engine != "autoloop" {
    return Err(ConfigError::InvalidEngine {
        engine: self.core.engine.clone(),
    });
}
```

The message is defined on the error variant at `crates/ralph-core/src/config.rs:2361` (the
`#[error(` attribute) with the literal text at `:2362`. An earlier revision of this
log said `:2360`; that line is blank. Corrected during the critic pass below.

```
#[error(
    "Invalid core.engine '{engine}': the in-house engine was removed in v3; remove the field or set autoloop."
)]
InvalidEngine { engine: String },
```

That is the v3 message the acceptance criterion names.

## Claim 2. `core_engine_rejects_removed_ralph_engine` passes

Command:

```
cargo test -p ralph-core core_engine_rejects_removed_ralph_engine
```

Output excerpt:

```
test config::tests::core_engine_rejects_removed_ralph_engine ... ok
test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 750 filtered out; finished in 0.00s
```

Exit status 0. The test asserts both the error variant and the exact string:

```
assert!(matches!(
    error,
    ConfigError::InvalidEngine { ref engine } if engine == "ralph"
));
assert_eq!(
    error.to_string(),
    "Invalid core.engine 'ralph': the in-house engine was removed in v3; remove the field or set autoloop."
);
```

Body at `crates/ralph-core/src/config.rs:2689`. Two sibling tests cover the
other half of the claim, that `autoloop` and the unset default still pass:
`core_engine_accepts_autoloop` (`:2705`) and `core_engine_accepts_unset_default`
(`:2713`). `core_engine_validation_is_not_suppressed` (`:2721`) proves the
`_suppress_warnings` path cannot bypass the rejection.

## Claim 3. Zero callers of `run_loop_impl` and `EventLoop::new`

Command:

```
grep -rn 'run_loop_impl\|EventLoop::new' crates/ --include=*.rs
```

Output: **empty**. Exit status 1 (no match).

Corroborating checks, bounded to the same tree:

```
$ grep -rn '\bEventLoop\b' crates/ --include=*.rs | head -10
crates/ralph-bench/src/main.rs:256:        // Run verification command (this works even without full EventLoop integration)
```

One hit, and it is a comment, not a live type. There is no `EventLoop` type
definition and no `EventLoop::new` call.

```
$ grep -rn '\brun_loop' crates/ --include=*.rs | head -10
```

Output: empty. No `run_loop` symbol of any name survives.

```
$ ls crates/ralph-core/src/ | grep -Ei 'event_loop|hatless|event_bus|wave|engine'
engine_state.rs
```

The engine modules the brief lists as gone are gone. `engine_state.rs` remains
and is Ralph-side autoloop run state, not an in-house loop. No `event_loop/`
directory, no `hatless_ralph.rs`, no `event_bus`, no `wave_*`.

## Claim 4. `hat_registry.rs` still exists and is exported

Commands and output:

```
$ ls -la crates/ralph-core/src/hat_registry.rs
-rw-rw-r-- 1 mobrienv mobrienv 15082 Sep 22 15:08 crates/ralph-core/src/hat_registry.rs

$ grep -n 'hat_registry' crates/ralph-core/src/lib.rs
26:mod hat_registry;
84:pub use hat_registry::HatRegistry;
```

Both hold. The module is declared private at `lib.rs:26` and its `HatRegistry`
type is publicly re-exported at `lib.rs:84`.

**This is the one in-house remnant.** It is the only survivor the brief predicts,
and it carries no engine call path: claim 3 shows no in-house loop remains to
call it. Removal is Step 3 (bead `a7e.10`), owned by
`code-assist:v3-complete:step-01:verify-remnant-and-drift`. This task records the
fact only, per "No source edits in this task."

## Verdict

The engine flip has landed on `v3/complete`.

| Claim | Verdict | Evidence |
|---|---|---|
| Non-`autoloop` engine rejected, v3 message | landed | `config.rs:512` guard, `config.rs:2360` message |
| `core_engine_rejects_removed_ralph_engine` passes | landed | `1 passed; 0 failed`, exit 0 |
| Zero `run_loop_impl` or `EventLoop::new` callers | landed | `grep` exit 1, empty output |
| `hat_registry.rs` present and exported | landed | `ls` 15082 bytes, `lib.rs:26` and `lib.rs:84` |

Remaining work from this task: none. The engine flip does not need to be redone,
and the only in-house remnant is a single module already assigned to the
remnant/drift task.

## Critic pass, 2026-09-22

Every acceptance command was re-run from a fresh shell rather than trusted from
this log. Repo state under test is unchanged: `df2449aa` on `v3/complete`, not
shallow.

| Claim | Command | Fresh result | Verdict |
|---|---|---|---|
| 1 | `grep -n 'if self.core.engine != "autoloop"' -A4 crates/ralph-core/src/config.rs` | `512: if self.core.engine != "autoloop" {` through `515`, guard preceded by the `Hard validation must run before warning suppression` comment at `511` | confirmed |
| 1 | `grep -n 'Invalid core.engine' crates/ralph-core/src/config.rs` | `2362` message literal, `2700` the assertion copy in the test | confirmed, citation corrected from `2360` to `2361`/`2362` |
| 2 | `cargo test -p ralph-core core_engine_rejects_removed_ralph_engine` | `test config::tests::core_engine_rejects_removed_ralph_engine ... ok`, `1 passed; 0 failed; 750 filtered out`, exit 0 | confirmed |
| 2 | `cargo test -p ralph-core core_engine_` | `4 passed; 0 failed` (`accepts_unset_default`, `rejects_removed_ralph_engine`, `accepts_autoloop`, `validation_is_not_suppressed`) | confirmed, and the suppression bypass stays covered |
| 3 | `grep -rn 'run_loop_impl\|EventLoop::new' crates/ --include=*.rs` | empty output, exit 1 | confirmed |
| 3 | `grep -rn '\bEventLoop\b' crates/ --include=*.rs` | one hit, `crates/ralph-bench/src/main.rs:256`, a comment | confirmed |
| 3 | `ls crates/ralph-core/src/` filtered on engine module names | `engine_state.rs` only | confirmed |
| 4 | `ls -la crates/ralph-core/src/hat_registry.rs` | present, 15082 bytes | confirmed |
| 4 | `grep -n 'hat_registry' crates/ralph-core/src/lib.rs` | `26:mod hat_registry;`, `84:pub use hat_registry::HatRegistry;` | confirmed |

Verdict: **landed**, upheld on re-run. One citation was two lines stale and is
corrected above. No claim in this log was overstated.
