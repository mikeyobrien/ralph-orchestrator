# Running and measuring a loop headlessly

For runs with no human at the terminal: an agent launching a loop, an unattended job, or an A/B comparison of two hat collections.

## Launch

Isolate the run, so the loop's commits and `.ralph/` state stay out of the checkout you work in. Use a scratch git repo (`git init` plus one commit) for experiments, or `git worktree add ../<name> -b <branch>` for real work in an existing repo. Commit any spec the loop needs into that branch first.

Keep the core config and the prompt outside the workspace, so the loop doesn't edit or commit them:

```bash
cd <workspace> && RALPH_DIAGNOSTICS=1 timeout <cap-seconds> \
  ralph run -c /outside/run.yml -H <hats> -P /outside/PROMPT.md \
  --max-iterations <n> --autonomous > /outside/run.log 2>&1
echo "exit=$?" > /outside/run.done
```

- `--autonomous` is the headless flag, and it cannot be combined with `--no-tui`.
- Put the runtime limits (`event_loop.max_iterations`, `max_runtime_seconds`) in the core config. Per-hat models and effort belong in the hats file (see `ralph-hats` `references/schema.md`).
- Run it as a background job, so you are notified when it exits.
- Before launching, `ralph doctor -c run.yml -H <hats>` should report the hats parsed and every backend available. Its git check fails on a repo with no commits.

## Monitor

The handoffs are the `topic` of each line in the newest `.ralph/events-*.jsonl` in the workspace.
- `iteration.summary` is a per-iteration marker, not a handoff.
- The payloads of `review.rejected` and `*.blocked` events carry the findings, so read them.
- Watchers that expire or get restarted should resume from a saved line offset into the events file, so nothing published in the gap is lost.
- `ps -eo args` shows the model and effort flags each backend process actually received.

## Measure

| Fact | Source |
|---|---|
| Termination reason, iterations, duration | The `Wrapping up: <reason>. N iterations in T` line in the run log |
| Cost, duration, turns per iteration | Run-log JSON lines with `type == "result"` (`total_cost_usd`, `duration_ms`, `num_turns`), from stream-json backends such as Claude. Text backends such as Codex report no cost. |
| Model per iteration | Run-log JSON lines with `type == "system"` (`model`) |
| Hat and event sequence | `.ralph/diagnostics/<ts>/orchestration.jsonl` (`hat_selected`, `event_published`) |
| Task state | `ralph tools task list --all` in the workspace |

Cross-check cost: if `iteration.summary` payloads or `performance.jsonl` show zero cost while the run log's `result` lines show real cost, trust the `result` lines.

When the completion event lands on the last allowed iteration, the reported reason is still `max_iterations`. Check the events file for the completion topic before calling the run unfinished.

## Compare two hat collections

Give both sides identical scratch repos from the same commit, the same prompt, and the same caps, and run them at the same time. Record outcome, iterations, wall time, cost, tests, and tasks closed for each. Then check each deliverable by hand against every requirement in the prompt. Name visible confounds, such as one side planning fewer tasks. One run per side is a signal, not a benchmark.

## When to step in

Sit on the loop, not in it. Step in for:
- **Review over-reach:** the same task is rejected repeatedly for minor findings, such as a wrong error message where the outcome is still correct, or robustness on unrealistic input. Stop the loop and give the reviewer hat a severity rule (see `ralph-hats` `references/design.md`), instead of letting the iteration cap burn.
- **A silent hat:** its output describes an emit, but no event reached the events file. The log shows `Output indicated ralph emit, but no event became readable`, and `default_publishes` fires with an empty payload. A successful `ralph emit` prints `Event emitted: <topic>`.
- **An extra coordinator iteration:** a hat published a topic nobody subscribes to, and Ralph's coordinator spent an iteration translating it. Fix the trigger in the hats file.
