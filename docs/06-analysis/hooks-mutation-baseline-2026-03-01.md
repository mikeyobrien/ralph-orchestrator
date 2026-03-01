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

- `TIMEOUT crates/ralph-cli/src/loop_runner.rs:3475:45: replace == with != in wait_for_resume_if_suspended`
- No `MISS` survivors in `3623-3635`.
- `disposition_from_on_error` currently has an `unviable` mutant at `3632` (non-survivor class, but still important context).

## Actionable survivor output

Full actionable survivor list (all `MISS` + `TIMEOUT` entries, line-resolved):

- [`docs/06-analysis/hooks-mutation-baseline-2026-03-01-survivors.txt`](./hooks-mutation-baseline-2026-03-01-survivors.txt)
