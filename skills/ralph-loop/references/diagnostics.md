# Ralph Loop Diagnostics

## Current Diagnostics Files

Enable diagnostics with:

```bash
RALPH_DIAGNOSTICS=1 ralph run -c ralph.yml -H .ralph/hats/my-workflow.yml -p "..."
```

The session directory lives under `.ralph/diagnostics/<timestamp>/`.

Key files:

- `agent-output.jsonl` for agent text and tool calls
- `orchestration.jsonl` for hat selection, events, and backpressure
- `performance.jsonl` for timing and token metrics
- `errors.jsonl` for parse and validation failures
- `trace.jsonl` for lower-level tracing
- `prompt-log.md` for the full prompt sent to the agent each iteration

Useful commands:

```bash
SESSION=".ralph/diagnostics/$(ls -t .ralph/diagnostics | head -1)"
jq 'select(.event.type == "hat_selected")' "$SESSION/orchestration.jsonl"
jq 'select(.type == "tool_call")' "$SESSION/agent-output.jsonl"
jq '.' "$SESSION/errors.jsonl"
jq '{iteration, duration_ms}' "$SESSION/performance.jsonl"

# View the full prompt for a specific iteration
grep -A 1000 "^# Iteration 3" "$SESSION/prompt-log.md" | sed '/^---$/q'
```

## Suspend and Resume Artifacts

Hook-driven suspension uses these operator-facing files:

- `.ralph/suspend-state.json`
- `.ralph/resume-requested`

Related control-signal files that can appear during loop operation:

- `.ralph/stop-requested`
- `.ralph/restart-requested`

Normal operator flow:

1. inspect `.ralph/suspend-state.json`
2. run `ralph loops resume <id>`
3. let Ralph consume `.ralph/resume-requested`

Avoid writing these files by hand unless the CLI path is unavailable and you
have already confirmed the recovery mechanics.

## State Files Worth Inspecting

- `.ralph/loop.lock` for the primary loop pid and prompt
- `.ralph/loops.json` for tracked loop metadata
- `.ralph/merge-queue.jsonl` for queued/merging/review events

When the user wants a concise operator summary, prefer `ralph loops list --json`
over hand-parsing the files.
