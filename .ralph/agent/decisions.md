# Decisions Log

## Format

Use this template for new entries:

```
## DEC-XXX (YYYY-MM-DDTHH:MMZ)
- Decision: <short statement>
- Chosen Option: <A/B/description>
- Confidence: <0-100>
- Alternatives Considered: <summary of options>
- Reasoning: <why this option was chosen>
- Reversibility: <High|Medium|Low>
- Timestamp: <UTC ISO 8601; use date-only if time unknown>
```

## DEC-001 (2026-01-31)
- Decision: Add an integration test for `ralph web` using fake node/npm/npx scripts and a PATH override to exercise the execute path.
- Confidence: 78
- Alternatives: (A) Only add unit tests for preflight/termination helpers with smaller coverage impact. (B) Refactor `execute` to accept injected command paths.
- Reasoning: Integration test covers large untested logic without altering production behavior; PATH override is scoped to the child process.
- Reversibility: High; test can be removed if flaky.

## DEC-002 (2026-01-31)
- Decision: Normalize Telegram bot tokens by trimming whitespace and treating empty env/keychain/config values as missing via `resolve_token_from`.
- Confidence: 82
- Alternatives: (A) Keep current precedence even if env var is empty. (B) Only normalize env tokens and leave keychain/config as-is.
- Reasoning: Prevents empty or whitespace env vars from blocking fallback sources and keeps token resolution predictable.
- Reversibility: High; adjust normalization or revert helper usage if needed.

## DEC-003 (2026-01-31)
- Decision: Extend `resolve_loop` to allow partial (suffix/contains) matches against merge queue entries, mirroring registry/worktree behavior.
- Confidence: 78
- Alternatives: (A) Keep merge queue exact-only lookup for safety. (B) Require explicit `loop-id` for queued/merged loops via CLI help update.
- Reasoning: CLI supports short IDs for loops in other sources; missing merge-queue partial match is inconsistent and causes "not found" for queued-only loops.
- Reversibility: High; revert branch or gate behind a flag if ambiguity arises.

## DEC-004 (2026-02-01)
- Decision: Strip common executable extensions (e.g., `.exe`, `.cmd`, `.bat`, `.com`) when canonicalizing custom backend command names for doctor checks.
- Confidence: 85
- Alternatives: (A) Preserve extensions and accept backend names like `claude.exe`. (B) Only strip on Windows with cfg-gating.
- Reasoning: Custom commands may include Windows-style extensions; normalizing avoids mislabeling known backends and keeps checks consistent across platforms.
- Reversibility: High; revert normalization or restrict it to Windows if needed.

## DEC-005 (2026-02-01)
- Decision: Target `crates/ralph-cli/src/loops.rs` with new tests for `merge_loop` error branches and default CLI execution to lift coverage in a high-uncovered module.
- Confidence: 78
- Alternatives: (A) Focus on event loop coverage with additional orchestration tests. (B) Add tests in `loop_runner` to cover async run paths.
- Reasoning: `merge_loop` branches are deterministic and testable without spawning external processes, providing safe coverage gains in a top-uncovered file.
- Reversibility: High; tests are additive and can be removed if they become brittle.

## DEC-006 (2026-02-01)
- Decision: Add a `--no-input` flag and auto-skip prompts when stdin is not a TTY for `ralph tutorial`.
- Confidence: 78
- Alternatives: (A) Always prompt and require interactive stdin. (B) Only auto-skip prompts without exposing a flag.
- Reasoning: Avoids hangs in non-interactive runs/tests while keeping the default tutorial interactive.
- Reversibility: High; flag and detection can be removed or adjusted without affecting core behavior.

## DEC-007 (2026-02-01)
- Decision: Resolve `ralph tools skill` workspace root by searching upward for ralph.yml when no --root is provided, and fall back to `.claude/skills` when skills.dirs is empty but the directory exists.
- Confidence: 78
- Alternatives: (A) Require explicit --root and leave skills.dirs empty unless configured. (B) Change SkillsConfig defaults globally to include .claude/skills.
- Reasoning: Makes `ralph tools skill` usable from subdirectories and in default workspaces that already have `.claude/skills`, without changing core config defaults.
- Reversibility: High; adjust resolution or remove fallback if it causes unexpected discovery.

## DEC-008 (2026-02-01)
- Decision: When `skills.dirs` is empty, search upward from the current directory (bounded to the workspace root) for `.claude/skills` and register that path, using a relative path when possible.
- Confidence: 78
- Alternatives: (A) Only use `root/.claude/skills` and require `--root` or config for other locations. (B) Search past the workspace root for any `.claude/skills` on the filesystem.
- Reasoning: Supports nested repo layouts where the workspace config is above the repo that owns the skills directory, preventing false "skill not found" errors without broadening discovery to unrelated parent directories.
- Reversibility: High; remove the fallback or restrict to explicit configuration if unexpected discovery occurs.

## DEC-009 (2026-02-01)
- Decision: If no skills directory is discovered under the workspace root, scan parent directories of the root for `.claude/skills` as a fallback.
- Confidence: 74
- Alternatives: (A) Require explicit skills.dirs configuration when the workspace root is nested. (B) Change workspace root resolution to prefer higher-level ralph.yml.
- Reasoning: Fixes nested-config layouts (like this workspace) without changing config semantics, while keeping the fallback narrow and only used when no local skills dir exists.
- Reversibility: High; remove the parent scan or gate it behind a flag if it causes unexpected discovery.

## DEC-010 (2026-02-01)
- Decision: Resolve configured relative skill directories by searching parent directories of the workspace root when the root-local path is missing, and store the resolved absolute path for skill discovery.
- Confidence: 82
- Alternatives: (A) Only fall back when skills.dirs is empty and ignore configured dirs. (B) Force users to pass --root or fix ralph.yml to point at the repo root.
- Reasoning: Honors configured relative paths while fixing nested-workspace layouts (e.g., `ralph.yml` inside a subdir) without surprising users with unrelated default discovery.
- Reversibility: High; revert to root-only resolution or gate the parent search behind a flag if it causes unexpected discovery.

## DEC-011 (2026-02-21T00:23Z)
- Decision: Update failing `integration_run_presets` expectations to use `--hats builtin:*` instead of legacy `--config builtin:*` syntax.
- Confidence: 76
- Alternatives Considered: (A) Ignore the failures as unrelated pre-existing drift. (B) Reintroduce compatibility path for legacy preset syntax.
- Reasoning: Full `cargo test` is required verification gate; adapting tests to current CLI behavior is lower risk than reviving deprecated flags.
- Reversibility: High
- Timestamp: 2026-02-21T00:23:00Z

## DEC-012 (2026-11-12T21:10Z)
- Decision: Implement the three loop-hygiene defects in order 2 → 3 → 1 rather than the order the spec numbers them.
- Chosen Option: Test-event isolation first, auto-commit artifact exclusion second, then the two Claude cost/context slices.
- Confidence: 80
- Alternatives Considered: (A) Spec order 1 → 2 → 3, landing the headline cost-accounting fix first. (B) Do defect 2 first and leave the rest in spec order.
- Reasoning: Defect 2 is the only one that corrupts the loop doing the work — every step runs `cargo test -p ralph-cli`, which until fixed injects stray topics into this loop's live events file and can trigger spurious hat handoffs mid-step. Defect 3 then keeps the repo clean for the commits the later steps make. Fixing hygiene first makes the remaining steps verifiable; spec order makes each successive run noisier.
- Reversibility: High; the four tasks are independent in substance and can be re-chained by the Planner on `plan.revise`.
- Timestamp: 2026-11-12T21:10:00Z

## DEC-013 (2026-11-12T21:10Z)
- Decision: Treat `.ralph/agent/memories.md` and `.ralph/agent/decisions.md` as intentionally tracked, and require the defect-3 fix to keep committing them.
- Chosen Option: Tracked (the spec left this open for the implementer to decide from the repository).
- Confidence: 92
- Alternatives Considered: (A) Treat them as run artifacts and exclude them from the auto-commit alongside diagnostics and logs.
- Reasoning: Root `.gitignore` carries explicit negations `!.ralph/agent/decisions.md` and `!.ralph/agent/memories.md` against the `.ralph/*` ignore, and `git ls-files` confirms `decisions.md` is tracked. `CLAUDE.md` also calls memories "persistent learning across sessions". The repository already answers the question.
- Reversibility: Medium; reversing it would mean untracking files that carry cross-session state, so it should be a deliberate separate change.
- Timestamp: 2026-11-12T21:10:00Z

## DEC-014 (2026-11-12T21:20Z)
- Decision: How ralph-cli integration tests stop inheriting the caller's loop env, and how that stays enforced.
- Chosen Option: One helper, `crates/ralph-cli/tests/common/mod.rs::ralph_command()`, that removes every inherited `RALPH_*` variable; all ~65 spawn sites migrated to it; `integration_env_isolation_guard.rs` fails on any direct `Command::new(env!("CARGO_BIN_EXE_ralph"))` / `Command::new(ralph_bin())` in `tests/*.rs`; `scripts/check-test-event-isolation.sh` is the end-to-end sentinel check.
- Confidence: 80
- Alternatives Considered: (A) Patch only the known offender at `integration_events_isolation.rs:149`. (B) Strip only `RALPH_EVENTS_FILE` and `RALPH_WORKSPACE_ROOT`. (C) Make the `ralph` binary refuse inherited env under `cfg(test)`.
- Reasoning: (A) leaves the next new test free to reintroduce the leak, which is how this one happened. (B) misses wave/diagnostics/config vars (`RALPH_WAVE_ID`, `RALPH_DIAGNOSTICS`, `RALPH_CONFIG`) that also change a child's behavior; tests needing one set it explicitly after the helper. (C) is impossible: integration tests run the release-shaped binary, not a cfg(test) build. The source-scan guard is a lint, not a behavior test; the sentinel script and `ralph_command_does_not_pass_loop_env_to_the_child` carry the behavior.
- Reversibility: High; test-only code.
- Timestamp: 2026-11-12T21:20:00Z

## DEC-015 (2026-11-12T22:00Z)
- Decision: Mechanism for keeping run artifacts out of the end-of-run auto-commit (spec defect 3).
- Chosen Option: Two layers. (1) `auto_commit_changes` stops running `git add -A`: it stages `git add --update` (every tracked change) plus untracked, non-ignored files that `is_run_artifact` does not reject. Rejected: anything under a nested `.ralph/`; anything under the workspace `.ralph/` except specs (minus `logs/`), `*.code-task.md`, `agent/memories.md`, `agent/decisions.md`; anything under the workspace-root `logs/`. (2) This repo's `.gitignore` adds `crates/**/.ralph/`, `/logs/`, `.ralph/specs/**/logs/`, which satisfies "nested .ralph/ is ignored" for the test-run half.
- Confidence: 78
- Alternatives Considered: (A) Ignore rules only. (B) Staging only tracked changes, never untracked files. (C) Make in-process tests stop creating `crates/*/.ralph/diagnostics` (inherited `RALPH_DIAGNOSTICS`).
- Reasoning: (A) fixes this repo but not the auto-commit, which runs in user repos whose `.gitignore` knows nothing about Ralph's layout; the landing tests even have to add `.ralph/` to `.gitignore` by hand to keep it out. (B) drops new source files, which the spec forbids. A code-level exclusion keyed to Ralph's own layout is the narrowest rule that works everywhere; it only ever narrows what gitignore already allows, and tracked files (even under `logs/` or a nested `.ralph/`) still commit, so deliberate tracking wins. Root `logs/` is Ralph's because its prompts (`hatless_ralph.rs`, `instructions.rs`) point agents there for temporary artifacts; nested `logs/` dirs (e.g. `src/logs/`) are source and stay in. (C) is the spec's other allowed route; skipped because the ignore satisfies the criterion and the diagnostics writers are scattered.
- Reversibility: High; one function and three ignore lines.
- Timestamp: 2026-11-12T22:00:00Z

## DEC-016 (2026-11-12T21:45Z)
- Decision: How headless (`CliExecutor`) Claude iterations get real cost/token metrics, and how far step 03 reaches past the summary.
- Chosen Option: Move the PTY executor's private `ClaudeSessionState` aggregation into `claude_stream.rs` as `ClaudeSessionMetrics::observe(&event, context_window) -> Option<SessionResult>`, used by both `pty_executor::dispatch_stream_event` and `CliExecutor` (which now parses StreamJson lines for metrics only: display, text normalization and exit-code `success` are unchanged). The headless branch in `loop_runner.rs` takes metrics from `session_result` independently of `extracted_text`, and sets `context_tokens` to the peak input like the PTY path. No shared PTY/headless outcome helper: the PTY reads flattened `PtyExecutionResult` fields, the headless path reads `SessionResult`, so a helper would just be an adapter between two shapes. Also in scope: wiring `EventLoop::log_iteration_performance` (performance.jsonl had no production writer at all) and moving `add_cost` before the `--no-tui` footer so `cum` includes the current iteration.
- Confidence: 80
- Alternatives Considered: (A) Duplicate the peak/sum logic in `cli_executor.rs`. (B) Make `pty_executor`'s private state and `dispatch_stream_event` `pub(crate)` and call it from `CliExecutor` with a `QuietStreamHandler`. (C) Leave performance.jsonl and the footer for a later task.
- Reasoning: (A) lets the two executors drift on the same stream. (B) couples the CLI executor to PTY internals and re-accumulates text it throws away. (C): the plan said `context_tokens = 0` caused the empty performance.jsonl; it didn't. `DiagnosticsCollector::log_performance` had no caller, so the spec's observed symptom would survive this fix. Both additions are a few lines and come with red/green tests.
- Reversibility: High; the tracker is a move, the wiring is one method and one call.
- Timestamp: 2026-11-12T21:45:00Z

## DEC-017 (2026-11-12T22:30Z)
- Decision: Where the per-iteration context window comes from, and its precedence over configuration (spec defect 1, third criterion).
- Chosen Option: `ClaudeStreamEvent::Result` parses `modelUsage.<model>.contextWindow` (leniently: a malformed `modelUsage` yields no window instead of failing the line, which also carries cost). `ClaudeSessionMetrics` remembers the `system` init line's model and picks that model's window, else the largest reported one, else the caller's fallback. The PTY path gets it via a new `PtyExecutionResult.context_window`; the headless path via `SessionResult.context_window` (CliExecutor still passes fallback 0). `loop_runner::reported_or_configured_context_window` picks a reported window (> 0) over `resolve_context_window_for_backend`, in both paths. Precedence: stream > `event_loop.context_window_tokens` override > backend default.
- Confidence: 75
- Alternatives Considered: (A) Explicit override beats the stream. (B) Always take the max window in `modelUsage`. (C) Only the headless path (the spec's observed symptom).
- Reasoning: (A): the override's documented purpose is correcting a wrong backend default (e.g. 1M-window users); the stream reports the running model's actual window, so it can't be the wrong one, while a stale override can be. The task text also names `resolve_context_window_for_backend` (which includes the override) as the fallback for when the stream reports nothing. The window only drives display and performance.jsonl percentages, so the stakes of the choice are low. (B) is wrong when a helper/subagent model has the larger window than the session's model. (C) would leave PTY Claude runs reporting 200000 for the same stream.
- Reversibility: High; swapping (A) in is a one-line change in `reported_or_configured_context_window`'s callers.
- Timestamp: 2026-11-12T22:30:00Z

## DEC-012 (2026-11-12T22:34Z)
- Decision: Wave workers resolve their backend through the same `resolve_hat_backend` as the hat's own iteration (hat backend, then `backend_args`, then model, then effort; hat-level else `cli`).
- Chosen Option: One shared resolver; `execute_wave` takes `&CliConfig`.
- Confidence: 80
- Alternatives Considered: (A) Add only `hat.model` to the wave path, leaving it applying effort before `backend_args` and ignoring `cli.effort`. (B) Pre-apply `cli.model` to the global backend (rejected: it would then count as an explicit flag and beat `hat.model`).
- Reasoning: A wave worker should launch exactly what its hat launches. The old wave path applied effort before `backend_args` (so an explicit `--effort` there was duplicated) and never used `cli.effort`; sharing the resolver fixes both and keeps "explicit args win" true for model too.
- Reversibility: High
- Timestamp: 2026-11-12T22:34Z

## DEC-013 (2026-11-12T22:34Z)
- Decision: `model` on a script hat warns as ignored, exactly like `effort`.
- Chosen Option: Add `("model", ...)` to the script-hat ignored-field warnings.
- Confidence: 90
- Alternatives Considered: Reject it as a config error.
- Reasoning: Matches `effort`/`backend_args` handling (warn, not error); a script hat launches no backend.
- Reversibility: High
- Timestamp: 2026-11-12T22:34Z
