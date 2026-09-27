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
