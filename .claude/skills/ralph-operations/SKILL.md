---
name: ralph-operations
description: Use when managing Ralph orchestration loops, analyzing diagnostic data, debugging hat selection, investigating backpressure, or performing post-mortem analysis
tags: [loops, diagnostics, debugging, analysis]
---

# Ralph Operations

Manage, monitor, and diagnose Ralph orchestration loops.

## Loop Management

### Quick Reference

| Task | Command |
|------|---------|
| List active loops | `ralph loops` |
| List all (including merged) | `ralph loops --all` |
| View loop changes | `ralph loops diff <id>` |
| View loop logs | `ralph loops logs <id>` |
| Follow logs live | `ralph loops logs <id> -f` |
| Stop running loop | `ralph loops stop <id>` |
| Merge completed loop | `ralph loops merge <id>` |
| Retry failed merge | `ralph loops retry <id>` |
| Abandon loop | `ralph loops discard <id>` |
| Clean stale processes | `ralph loops prune` |

**Loop ID format:** Partial matching works - `a3f2` matches `loop-20250124-143052-a3f2`

### Loop Status

| Status | Color | Meaning |
|--------|-------|---------|
| running | green | Loop is actively executing |
| queued | blue | Completed, waiting for merge |
| merging | yellow | Merge in progress |
| needs-review | red | Merge failed, requires intervention |
| merged | dim | Successfully merged (with `--all`) |
| discarded | dim | Abandoned (with `--all`) |

### Starting & Stopping

Loops start automatically via `ralph run`:
- **Primary loop**: Runs in main workspace, holds `.ralph/loop.lock`
- **Worktree loop**: Created when primary is running, isolated in `.worktrees/<loop-id>/`

```bash
ralph loops                       # Any loops running?
cat .ralph/loop.lock 2>/dev/null  # Primary loop details
ralph loops stop <id>             # Graceful stop
ralph loops stop <id> --force     # Immediate stop
ralph loops discard <id>          # Abandon + clean worktree
```

### Inspecting Loops

```bash
ralph loops diff <id>             # What changed
ralph loops logs <id> -f          # Live event log
ralph loops history <id>          # State changes
ralph loops attach <id>           # Shell into worktree
```

**Worktree context files** (`.worktrees/<loop-id>/`):

| File | Contents |
|------|----------|
| `.ralph/events.jsonl` | Event stream: hats, iterations, tool calls |
| `.ralph/agent/summary.md` | Current session summary |
| `.ralph/agent/handoff.md` | Handoff context for next iteration |
| `.ralph/agent/scratchpad.md` | Working notes |
| `.ralph/agent/tasks.jsonl` | Runtime task state |

**Primary loop** uses the same files at `.ralph/agent/` in repo root.

### Merge Queue

Flow: `Queued → Merging → Merged` or `→ NeedsReview → Merging (retry)` or `→ Discarded`

```bash
ralph loops merge <id>            # Queue for merge
ralph loops process               # Process pending merges now
ralph loops retry <id>            # Retry failed merge
```

**Reading state:**
```bash
jq -r '.prompt' .ralph/loop.lock 2>/dev/null
tail -20 .ralph/merge-queue.jsonl | jq .
```

---

## Diagnostics

### Enabling

```bash
RALPH_DIAGNOSTICS=1 ralph run -p "your prompt"
```

Zero overhead when disabled. Output: `.ralph/diagnostics/<YYYY-MM-DDTHH-MM-SS>/`

### Session Discovery

```bash
LATEST=$(ls -t .ralph/diagnostics/ | head -1)
SESSION=".ralph/diagnostics/$LATEST"
```

### File Reference

| File | Contains | Key Fields |
|------|----------|------------|
| `agent-output.jsonl` | Agent text, tool calls, results | `type`, `iteration`, `hat` |
| `orchestration.jsonl` | Hat selection, events, backpressure | `event.type`, `iteration`, `hat` |
| `performance.jsonl` | Timing, latency, token counts | `metric.type`, `iteration`, `hat` |
| `errors.jsonl` | Parse errors, validation failures | `error_type`, `message`, `context` |
| `trace.jsonl` | All tracing logs with metadata | `level`, `target`, `message` |

### Diagnostic Workflow

**1. Errors first:**
```bash
wc -l "$SESSION/errors.jsonl"
jq '.' "$SESSION/errors.jsonl"
jq -s 'group_by(.error_type) | map({type: .[0].error_type, count: length})' "$SESSION/errors.jsonl"
```

**2. Orchestration flow:**
```bash
jq '{iter: .iteration, hat: .hat, event: .event.type}' "$SESSION/orchestration.jsonl"
jq 'select(.event.type == "hat_selected") | {iter: .iteration, hat: .event.hat, reason: .event.reason}' "$SESSION/orchestration.jsonl"
jq 'select(.event.type == "backpressure_triggered") | {iter: .iteration, reason: .event.reason}' "$SESSION/orchestration.jsonl"
```

**3. Agent activity:**
```bash
jq 'select(.type == "tool_call") | {iter: .iteration, tool: .name}' "$SESSION/agent-output.jsonl"
jq -s '[.[] | select(.type == "tool_call")] | group_by(.iteration) | map({iter: .[0].iteration, tools: [.[].name]})' "$SESSION/agent-output.jsonl"
```

**4. Performance:**
```bash
jq 'select(.metric.type == "iteration_duration") | {iter: .iteration, ms: .metric.duration_ms}' "$SESSION/performance.jsonl"
jq -s '[.[] | select(.metric.type == "token_count")] | {total_in: (map(.metric.input) | add), total_out: (map(.metric.output) | add)}' "$SESSION/performance.jsonl"
```

**5. Trace logs:**
```bash
jq 'select(.level == "ERROR" or .level == "WARN")' "$SESSION/trace.jsonl"
```

### Quick Health Check

```bash
SESSION=".ralph/diagnostics/$(ls -t .ralph/diagnostics/ | head -1)"
echo "=== Session: $SESSION ==="
echo -e "\n--- Errors ---"
wc -l < "$SESSION/errors.jsonl" 2>/dev/null || echo "0"
echo -e "\n--- Iterations ---"
jq -s 'map(select(.event.type == "iteration_started")) | length' "$SESSION/orchestration.jsonl"
echo -e "\n--- Hats Used ---"
jq -s '[.[] | select(.event.type == "hat_selected") | .event.hat] | unique' "$SESSION/orchestration.jsonl"
echo -e "\n--- Backpressure Count ---"
jq -s 'map(select(.event.type == "backpressure_triggered")) | length' "$SESSION/orchestration.jsonl"
echo -e "\n--- Termination ---"
jq 'select(.event.type == "loop_terminated")' "$SESSION/orchestration.jsonl"
```

---

## Troubleshooting

### Stale Processes
`ralph loops` shows loops that aren't running → `ralph loops prune`

### Orphan Worktrees
`.worktrees/` has directories not in `ralph loops` → `ralph loops prune` or `git worktree remove .worktrees/<id> --force`

### Merge Conflicts
Loop stuck in `needs-review`:
1. `ralph loops diff <id>` — see conflicting changes
2. `ralph loops attach <id>` — resolve manually, commit, retry
3. `ralph loops discard <id>` — abandon if not worth fixing

### Lock Stuck
"Loop already running" but nothing is → `rm .ralph/loop.lock` (safe if process is dead)

### Agent Stuck in Loop
```bash
jq -s '[.[] | select(.type == "tool_call")] | group_by(.name) | map({tool: .[0].name, count: length}) | sort_by(-.count)' "$SESSION/agent-output.jsonl"
```
Red flag: Many iterations with few events = agent not making progress.

### Merge Stuck in "merging"
Process died mid-merge. Unblock:
```bash
echo '{"ts":"'$(date -u +%Y-%m-%dT%H:%M:%S.000000Z)'","loop_id":"<loop-id>","event":{"type":"needs_review","reason":"Merge process died"}}' >> .ralph/merge-queue.jsonl
ralph loops discard <loop-id>
```

### Worktree Corruption
```bash
git worktree repair
ralph loops prune
```

### Cleanup
```bash
ralph clean --diagnostics              # Delete all sessions
ralph clean --diagnostics --dry-run    # Preview deletions
```
