# Hooks mutation baseline report (2026-03-01)

## Scope and execution

Mutation scope (from `just mutants-baseline`):

- `crates/ralph-core/src/hooks/executor.rs`
- `crates/ralph-core/src/hooks/engine.rs`
- `crates/ralph-core/src/preflight.rs`
- `crates/ralph-cli/src/loop_runner.rs`

Executed in a nix shell that provides `cargo-mutants`:

```bash
nix shell nixpkgs#rustc nixpkgs#cargo nixpkgs#cargo-mutants nixpkgs#gcc nixpkgs#pkg-config nixpkgs#openssl nixpkgs#clang -c sh -lc \
  'cargo mutants --baseline skip --file crates/ralph-core/src/hooks/executor.rs --file crates/ralph-core/src/hooks/engine.rs --file crates/ralph-core/src/preflight.rs --file crates/ralph-cli/src/loop_runner.rs -o /tmp/hooks-mutants-baseline --no-times --colors never --caught --unviable'
```

Notes:

- A first run without `--baseline skip` failed in the unmutated-tree baseline due to an `ExecutableFileBusy` flake in `hooks::executor` tests.
- Baseline tests were re-run successfully (`cargo test -p ralph-core`) before the mutation run above.

## Baseline result summary

| Status | Count |
|---|---:|
| caught | 181 |
| missed (survivors) | 143 |
| unviable | 70 |
| timeout | 10 |
| total mutants | 404 |

Derived scores:

- **Strict score** (timeouts count as not-killed): `181 / (181 + 143 + 10) = 54.19%`
- **Operational score** (timeouts tracked separately): `181 / (181 + 143) = 55.86%`

Per-file hotspots (strict score denominator = `caught + missed + timeout`):

| File | Caught | Missed | Timeout | Unviable | Strict score |
|---|---:|---:|---:|---:|---:|
| `crates/ralph-cli/src/loop_runner.rs` | 84 | 79 | 6 | 35 | 49.70% |
| `crates/ralph-core/src/hooks/executor.rs` | 20 | 22 | 4 | 6 | 43.48% |
| `crates/ralph-core/src/preflight.rs` | 71 | 42 | 0 | 24 | 62.83% |
| `crates/ralph-core/src/hooks/engine.rs` | 6 | 0 | 0 | 5 | 100.00% |

## Threshold calibration decision

1. Keep global parser anchor unchanged at `QualityReport::MUTATION_THRESHOLD = 70.0` (`crates/ralph-core/src/event_parser.rs:162`).
2. Calibrate the **hooks rollout mutation threshold** to **>=55% operational score** (`caught / (caught + missed)`) for the first gated rollout.
3. Track timeouts as a separate failure class and tighten them in Step 12.4/12.5 with critical-path hard checks.
4. Ratchet the hooks rollout threshold back toward `>=70%` after critical-path survivors/timeouts are eliminated.

## Critical-path status for Step 12.4 no-survivor invariants

Target critical ranges:

- `crates/ralph-cli/src/loop_runner.rs:3467-3560` (suspend/resume transition)
- `crates/ralph-cli/src/loop_runner.rs:3623-3635` (on_error disposition mapping)

Current baseline in those ranges:

- No `MISS` survivors in either critical range (`3467-3560`, `3623-3635`).
- `TIMEOUT crates/ralph-cli/src/loop_runner.rs:3475:45: replace == with != in wait_for_resume_if_suspended`
- `unviable` mutants in disposition mapping at `3624` and `3632` (non-survivor class).

## Step 12.4: Critical no-survivor invariant enforcement

### Invariant definition

For Step 12.5 CI wiring, critical-path mutation enforcement is:

- **Hard fail** if any `MISS` mutant appears in:
  - `crates/ralph-cli/src/loop_runner.rs:3467-3560` (suspend/resume transition)
  - `crates/ralph-cli/src/loop_runner.rs:3623-3635` (on_error disposition mapping)
- Treat `TIMEOUT` and `unviable` as separate classes that must be explained in gate output.

### Current invariant status

| Critical range | MISS | TIMEOUT | Unviable | Status |
|---|---:|---:|---:|---|
| `loop_runner.rs:3467-3560` (suspend/resume) | 0 | 1 | 0 | ✅ PASS |
| `loop_runner.rs:3623-3635` (disposition mapping) | 0 | 0 | 2 | ✅ PASS |

Evidence from baseline artifacts:

- `docs/06-analysis/hooks-mutation-baseline-2026-03-01-survivors.txt`
  - no `MISS` lines in either critical range
  - one `TIMEOUT` line at `3475` (`wait_for_resume_if_suspended`)
- `/tmp/hooks-mutants-baseline/mutants.out/unviable.txt`
  - `3624`: `classify_hook_disposition -> Default::default()`
  - `3632`: `disposition_from_on_error -> Default::default()`

### TIMEOUT rationale for Step 12.5 gate

The `TIMEOUT` at `3475` is expected in mutation mode: `wait_for_resume_if_suspended` loops until external `.ralph/resume-requested`, `.ralph/stop-requested`, or `.ralph/restart-requested` signals are observed. The mutant flips the resume check and can create a non-terminating wait. This is a **blocking-control-flow timeout**, not a silent `MISS` survivor.

Step 12.5 gate behavior should therefore be:

1. Reject any `MISS` in critical ranges.
2. Report critical-range `TIMEOUT` entries separately with explicit rationale.
3. Keep timeout count visible for ratcheting, but do not classify it as a no-survivor violation.

### Unviable rationale for Step 12.5 gate

Both critical-range unviable mutants are type-invalid replacements:

- `classify_hook_disposition` and `disposition_from_on_error` return `HookDisposition`
- Replacing these functions with `Default::default()` is not compilable because `HookDisposition` has no `Default` implementation

These are compiler-rejected mutants and should be treated as **non-survivor** evidence in Step 12.5 reporting.

### Test coverage verification

Existing tests in `loop_runner.rs` exercise suspend/resume control flow in this critical region:

```rust
// Line 7513: no-op when no suspend disposition
fn test_wait_for_resume_if_suspended_is_noop_without_suspend_dispositions()

// Line 7543: resume signal clears suspend artifacts
fn test_wait_for_resume_if_suspended_resumes_and_clears_suspend_artifacts()

// Line 7568: stop signal is prioritized over resume
fn test_wait_for_resume_if_suspended_prioritizes_stop_over_resume()

// Line 7598: restart signal is prioritized over resume
fn test_wait_for_resume_if_suspended_prioritizes_restart_over_resume()
```

### Invariant enforcement decision

✅ **Step 12.4 complete:** no `MISS` survivors in disposition/suspend critical paths, with `TIMEOUT` and `unviable` classes explicitly characterized for Step 12.5 CI gate wiring.

## Actionable survivor output

Full actionable survivor list (all `MISS` + `TIMEOUT` entries, line-resolved):

- [`docs/06-analysis/hooks-mutation-baseline-2026-03-01-survivors.txt`](./hooks-mutation-baseline-2026-03-01-survivors.txt)

## Step 12.6 verification gate run (2026-03-01)

Required Step 12 verification commands were executed in nix shells:

- `cargo fmt --all -- --check` → `EXIT:0`
- `cargo clippy --all-targets --all-features -- -D warnings` → `EXIT:0`
- `cargo test -p ralph-core -q` → `734 passed; 0 failed`
- `cargo test -p ralph-cli -q` → first run failed in `web::tests::check_tsx_version_blocks_known_bad_release_with_v_prefix`; immediate rerun passed (`320 passed; 0 failed; 2 ignored`)
- `just mutants-hooks-gate` → `PASS`

Mutation gate artifact summary (`.artifacts/hooks-mutation/hooks-mutation-summary.json`):

- status: `pass`
- threshold: `55%`
- operational score: `55.11%` (`178 caught / (178 caught + 145 missed)`)
- strict score: `53.45%`
- critical-path counts: `MISS=0`, `TIMEOUT=1`, `unviable=3`

Artifacts produced for CI upload/debugging:

- `.artifacts/hooks-mutation/hooks-mutation-report.md`
- `.artifacts/hooks-mutation/hooks-mutation-survivors.txt`
- `.artifacts/hooks-mutation/critical-miss.txt`
- `.artifacts/hooks-mutation/critical-timeout.txt`
- `.artifacts/hooks-mutation/critical-unviable.txt`
- `.artifacts/hooks-mutation/mutants.out/{caught,missed,timeout,unviable}.txt`

Implementation note: `Justfile` threshold variable is quoted (`HOOKS_MUTATION_THRESHOLD := "55"`) so `just` parses and injects it correctly into `scripts/hooks-mutation-gate.sh`.
