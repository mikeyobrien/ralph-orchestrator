# Upstream autoloop issues found during v3-complete (drafts, not filed)

Each issue was observed on the installed engine and, where noted, re-checked
against 0.12.0. File them at mikeyobrien/autoloop after review.

## 1. A metareview EXIT verdict completes the loop past a held acceptance gate

- Versions: 0.11.0 and 0.12.0 (`autoloop-harness/dist/index.js`,
  `runReviewThenIterate`: `if (verdict.verdict === "EXIT") return completeLoop(reviewed, iteration, "verdict_exit")`).
- Repro: a preset with `acceptance.verify_cmds = ["exit 1"]` and the metareview
  enabled. The agent emits the completion event, the gate holds it
  (`acceptance.result passed=false`, `completion.held`), and the next metareview
  returns EXIT. The run ends `loop.complete reason=verdict_exit`.
- Observed 2026-09-27 (run `sparse-memory`, Ralph scratch repo `judge-live`):
  `completion.held` at iteration 1, then `review.verdict EXIT`, `review.exit`,
  and `loop.complete reason=verdict_exit` at iteration 2.
- Expected: an EXIT verdict should go through `resolveCompletionClaim` (or at
  least be refused while a claim is held), so a deterministic acceptance gate
  cannot be bypassed.
- Ralph mitigation: `core.completion.jev` sets `review.enabled = false`.

## 2. `review.verdict` is treated as a routing topic

- Versions: 0.11.0 and 0.12.0 (`emit.js` `routingTopic` non-routing set and
  `CORE_SYSTEM_TOPICS` list `review.start` and `review.finish` but not
  `review.verdict`).
- Effect: after every metareview, the routing position becomes
  `review.verdict`, which has no handoff, so the next iteration gets all-roles
  freedom. It overwrote a declarative wave's post-join
  `resume_roles=reviewer`, and let a coordinator emit a later role's event.
- Expected: `review.verdict` (and `review.exit`, `review.quarantine`,
  `review.unknown`) are non-routing system topics.

## 3. `--set notify.*` is silently ignored

- Versions: 0.11.0 and 0.12.0 (`notify.js` `runFinishNotification` re-reads
  `config.loadProject(opts.projectDir)`, so CLI `--set` layers are not
  included).
- Effect: `autoloop run <preset> --set notify.command=...` never notifies and
  journals nothing.
- Expected: notify reads the same layered config as the run, or rejects
  `--set notify.*`.
- Ralph mitigation: writes `notify.*` into the generated preset.

## 4. The stdout completion promise matches a quoted or negated mention

- Version: 0.11.0 (observed 2026-09-27, Ralph scratch repo `topology-live`).
- Repro: a `pre_emit` hook blocks the agent's only emit (exit 1,
  `on_error = "block"`). The agent explains the block and writes "I also didn't
  print `LOOP_COMPLETE`". The engine journals
  `completion.provisional reason=completion_promise`, `completion.accepted`,
  and `loop.complete reason=completion_promise`.
- Two causes: (a) `completedViaPromise` is a substring match, so a quoted or
  negated mention counts; (b) a hook-blocked emit does not mark the turn as
  having an invalid event, so `resolveOutcome` does not veto the promise.
- Expected: the promise should be an exact line or marker, and a blocked emit
  should count as an invalid event for that turn.


## 5. `autoloop resume` ignores the state directory and CLI config layers

- Versions: 0.11.0 and 0.12.0 (`autoloop-cli/dist/commands/resume.js`,
  `autoloop-harness/dist/resume.js`).
- Effect, registry: `resume` looks runs up in
  `$AUTOLOOP_PROJECT_DIR/.autoloop/registry.jsonl` (default `./.autoloop`).
  It ignores `AUTOLOOP_STATE_DIR` and `core.state_dir`, so a run started with
  either set cannot be resumed: "error: no run matching `<id>`".
- Effect, memory: `resume` takes no `--set` and rebuilds config from the
  run's preset file. When the preset does not name `core.memory_file`, the
  default is `join(basename(record.state_dir), "memory.jsonl")` joined onto the
  work dir. `state_dir` is the per-run directory, so the resumed run's memory
  path becomes `<work_dir>/<run_id>/memory.jsonl`, not the memory the run
  started with.
- Expected: resume finds runs where `run` put them (the registry beside the
  recorded `state_dir`, or honor `AUTOLOOP_STATE_DIR`), and reuses the paths
  recorded for the run instead of recomputing them.
- Ralph mitigation: `ralph resume` points `AUTOLOOP_PROJECT_DIR` at
  `.ralph/autoloop-resume`, whose `.autoloop` is a symlink to `.ralph/autoloop`.
  It also writes the four `core.*` state keys into the generated preset, and
  refuses to resume through an explicit preset that lacks them.
