---
status: "gate met on branch v3/complete; release is the maintainer's decision"
created: 2026-09-27
updated: 2026-09-27
bead: ralph-orchestrator-v3-autoloops-backend-a7e
branch: v3/complete
engine_verified: autoloop 0.11.0 (live); 0.12.0 (scratch install, drift and Jev routing)
related:
  - v3-autoloops-cutover.spec.md
  - v3-completion-audit.md
  - v3-complete/progress.md
  - v3-complete/upstream-issues.md
---

# v3 GA readiness gate

This spec is the release gate that `v3-autoloops-cutover.spec.md` names. It
lists what v3 must do, the evidence that it does, and what remains open. The
cutover spec is a historical migration narrative. Where the two disagree, this
spec controls.

Ralph v3 contract: autoloop is the engine. Ralph is the TUI and the
coordination shell (merge queue, worktrees, loop registry, landing, doctor,
HITL relay). Ralph configures the engine only at seams the engine owns and
never re-implements engine judgment.

## How to run the gate

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo run -p ralph-e2e -- --mock          # Tier 0 engine-completion replay
```

The legacy cassette scenarios in `ralph-e2e` are not a gate; the R-matrix
below maps each to its v3 replacement coverage.

## Acceptance

Status values: **pass** (evidence below), **pass, not live** (automated
evidence only), **operator** (needs a decision or credential the maintainer
holds).

| # | Requirement | Status | Evidence |
|---|---|---|---|
| A1 | `ralph run` drives a multi-role loop through the engine, mock and live | pass | Live: `ralph run -H builtin:code-assist` ran Planner, Builder, Critic, Finalizer on 0.11.0 (progress.md, Step 12, 3.2). Mock: `ralph-e2e --mock` `engine-completion` (repaired in Step 14; its fixture still reported pre-Step-3 `.autoloop` paths); fake-autoloop integration suites in `crates/ralph-cli/tests/`. |
| A2 | The workspace suites pass on the engine | pass | Step 14 closing run: `cargo test --workspace` 83 suites, 2922 passed, 0 failed; clippy `-D warnings` and fmt clean; `ralph-e2e --mock` 1 passed, 25 legacy skipped; no leaked processes. |
| A3 | Migration guide and CHANGELOG entry | pass | `docs/migration/v3-autoloop-engine.md`; `CHANGELOG.md` `[3.0.0] - Unreleased`. |
| A4 | Engine state is Ralph-owned under `.ralph/autoloop`; no top-level `.autoloop` | pass | `integration_autoloop_failure_reporting` (symlink and outside-path refusals, summary path validation); live runs in Steps 12 and 13 created none, resume included. |
| A5 | Engine resolution, version gates, provisioning | pass | `integration_autoloop_dependency`, `integration_engine_install`; `ralph doctor` live: "Autoloop 0.11.0 available (PATH lookup)". |
| A6 | Parallel worktree loops, merge queue, landing | pass | Live primary plus worktree loop, then `ralph loops merge` (Step 12, 3.3). `integration_merge_drain_autoloop`, `integration_loops_merge`, `integration_landing_scope`. Merge children run the current executable (`merge_children_run_this_executable`). |
| A7 | `ralph resume` continues an interrupted engine run | pass | Live: stopped at count 2, resumed to count 4, `completion_promise` (Step 12). `integration_continue_resume`; `resume_lookup_dir_points_the_engine_registry_at_ralph_state`. Works around upstream issue 5. |
| A8 | Stopping Ralph stops the engine; nothing is orphaned | pass | `integration_engine_stop`; live `SIGINT` to Ralph's pid left the run `stopped` and resumable (Step 12). |
| A9 | `ralph run --rpc` | pass | `integration_autoloop_rpc`; live run emitted one `iteration_end` per iteration and one `loop_terminated`. |
| A10 | TUI renders the engine's run | pass | `crates/ralph-tui/tests/autoloop_frames.rs`: a live run replayed through the production reader; frames drawn by the app's `render_frame` (Step 13). |
| A11 | Lifecycle hooks | pass | `integration_engine_hooks`; hooks BDD suite in `ralph-e2e`; live delivery through `notify.command` (Step 8b). Only `post.loop.complete` and `post.loop.error` are supported; others refuse to start. |
| A12 | Hat concurrency and aggregation map to engine parallelism | pass | Preset generator tests; `presets/wave-review.yml` verified live (Step 4). No TUI wave view, by design. |
| A13 | RObot HITL relay (Telegram / web) | pass, not live | `autoloop_robot` tests (ask relay, guidance, restart), `ralph-api` `rpc_v1_robot`. No live Telegram relay was run in this pass. |
| A14 | Jev workflow routing, completion judge, topology routing | operator | Config validation, refuse-to-start, and fail-closed without a key are tested and were observed live (Steps 9 to 11). A live Jev decision needs a `TYPESAFE_API_KEY` the maintainer has not supplied. All three are opt-in. |

## Known gaps and mitigations

| Gap | Effect | Mitigation or owner |
|---|---|---|
| Upstream 1: a metareview `EXIT` completes past the acceptance gate | The Jev judge could be bypassed | The judge sets `review.enabled = false` |
| Upstream 2: `review.verdict` treated as a routing topic | After every metareview the next iteration gets all-roles freedom; it overrode a wave's post-join `resume_roles` | Engine-owned; the Jev judge turns the metareview off |
| Upstream 3: `--set notify.*` ignored | Hooks would not fire | `notify.*` written into the generated preset; explicit presets get the exact lines to add |
| Upstream 4: completion promise substring-matches a negated mention | A run can complete on "I didn't print LOOP_COMPLETE" | The Jev judge guards; otherwise engine-owned |
| Upstream 5: `autoloop resume` ignores the state dir and `--set` | Resume could not find runs | Resume lookup shim; state keys in the generated preset; explicit presets must carry them (refused otherwise) |
| `event_loop.max_consecutive_failures` | Not enforced; the engine has no equivalent | Preflight warning in `ralph run` and `ralph doctor` |
| TUI guidance keys | No back-channel from the TUI under the engine | Keys are inert and not listed in help; RObot carries guidance |
| `ralph-core` `testing::smoke_runner` (feature `recording`) | Orphaned v2 replay runner that parses v2 event text; no consumer, not in any gate | Delete |
| `TuiState::update(&Event)` / `Tui::observer()` | v2 path with no production caller; `integration_snapshots` still drives it | Cleanup; A10 evidence does not rely on it |
| Global engine is 0.11.0 | Jev workflow routing needs 0.12.0 | Ralph refuses to start routing on an older engine; upgrading is the maintainer's call |

The upstream issues are drafted in `v3-complete/upstream-issues.md` and have
not been filed.

## R-matrix: legacy E2E scenarios and their v3 coverage

| Legacy scenario | v3 coverage |
|---|---|
| `connect` | `ralph doctor` backend checks (`backend_checks_*` in `doctor.rs`); A1 live run |
| `single-iter` | `ralph-e2e --mock` `engine-completion`; `integration_autoloop_headless_stream` |
| `multi-iter` | A1 live run; `autoloop_frames` (five iterations) |
| `completion` | Engine-owned judgment; Ralph maps stop reasons (`autoloop_engine.rs` stop-reason tests) and coordinates completion (`completion_coord.rs`) |
| `events` | `ralph-adapters` `autoloop_events` parsing and `autoloop_event_tailer` tests; `integration_autoloop_failure_reporting` (malformed events fail closed) |
| `backpressure` | Engine-owned (`acceptance.verify_cmds`, typed gates); Ralph's use is covered by `integration_jev_judge` |
| `tool-use` | `backend_stream_tailer` tool-summary tests; `autoloop_frames` asserts tool calls per iteration |
| `streaming` | `backend_stream_tailer` (claude and pi streams); `integration_autoloop_headless_stream` |
| `hat-single` | Preset generator role tests; `integration_run_presets` |
| `hat-multi-workflow` | A1 live four-role run |
| `hat-instructions` | Preset generator writes `roles/<hat>.md` from instructions (generator tests) |
| `hat-event-routing` | Generator topology and handoff tests; `integration_jev_topology` (triggers route with topology off) |
| `hat-backend-override` | Generator backend tests (`writes_*_backend`, `rejects_unknown_backend_*`); doctor `backend_checks_include_cli_and_hat_backends` |
| `memory-add`, `memory-search`, `memory-persistence` | `integration_memory`; `memory_store` search and round-trip tests |
| `memory-injection` | Engine-owned memory (`.ralph/autoloop/memory.jsonl`); Ralph memories stay separate by design |
| `memory-missing-file` | `test_append_creates_file_if_missing`, `test_load_empty_file` |
| `memory-large-content` | Partly: `test_truncate_to_budget_*` bound injected size; no large-file test |
| `memory-corrupted-file`, `memory-rapid-write` | **No v3 replacement.** The memory store is unchanged Ralph code, not engine code; these remain uncovered. |
| `timeout-handling` | Budget forwarding (`writes_normalized_v2_budgets_with_autoloop_keys_and_units`); stop-reason mapping |
| `max-iterations` | `generated_preset_writes_effective_default_max_iterations` and explicit-preset forwarding tests |
| `auth-failure` | `ralph doctor` credential checks; engine failure reporting stays private (`integration_autoloop_failure_reporting`) |
| `backend-unavailable` | `backend_checks_fail_required_missing`; `integration_autoloop_dependency` (engine missing fails before the lock) |

## Gate verdict

Every required row (A1 to A12) passes. A13 has automated evidence only. A14
is opt-in and waits on a TypeSafe key. Releasing 3.0.0 is the maintainer's
decision: whether A13 needs a live Telegram pass and A14 a live Jev pass
first, whether to file the upstream issues, and whether to move the global
engine to 0.12.0.
