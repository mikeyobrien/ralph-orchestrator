---
name: ralph-diagnostics
description: Use when analyzing Ralph orchestration runs, debugging hat selection, investigating backpressure, profiling performance, or post-mortem analysis of failed loops
tags: [debugging, diagnostics, analysis]
---

# Ralph Diagnostics

Analyze orchestration runs using structured JSONL diagnostic data. Captures agent output, hat selection, events, performance, errors, and traces.

## Enabling Diagnostics

```bash
RALPH_DIAGNOSTICS=1 ralph run -p "your prompt"
```

Zero overhead when disabled. Output: `.ralph/diagnostics/<YYYY-MM-DDTHH-MM-SS>/`

## Session Discovery

```bash
# List all sessions (newest last)
ls -lt .ralph/diagnostics/

# Latest session shorthand
LATEST=$(ls -t .ralph/diagnostics/ | head -1)
SESSION=".ralph/diagnostics/$LATEST"

# Check if any sessions exist
ls .ralph/diagnostics/ 2>/dev/null || echo "No diagnostic sessions found"
```

## File Reference

| File | Contains | Key Fields |
|------|----------|------------|
| `agent-output.jsonl` | Agent text, tool calls, results | `type`, `iteration`, `hat` |
| `orchestration.jsonl` | Hat selection, events, backpressure | `event.type`, `iteration`, `hat` |
| `performance.jsonl` | Timing, latency, token counts | `metric.type`, `iteration`, `hat` |
| `errors.jsonl` | Parse errors, validation failures | `error_type`, `message`, `context` |
| `trace.jsonl` | All tracing logs with metadata | `level`, `target`, `message` |

## Diagnostic Workflow

Follow this sequence for any investigation:

### 1. Errors First

```bash
# Any errors at all?
wc -l "$SESSION/errors.jsonl"

# Show all errors
jq '.' "$SESSION/errors.jsonl"

# Group by type
jq -s 'group_by(.error_type) | map({type: .[0].error_type, count: length})' "$SESSION/errors.jsonl"
```

**Error types:** `parse_error`, `validation_failure`, `backend_error`, `timeout`, `malformed_event`, `telegram_send_error`

### 2. Orchestration Flow

```bash
# Full iteration timeline
jq '{iter: .iteration, hat: .hat, event: .event.type}' "$SESSION/orchestration.jsonl"

# Hat selection decisions
jq 'select(.event.type == "hat_selected") | {iter: .iteration, hat: .event.hat, reason: .event.reason}' "$SESSION/orchestration.jsonl"

# Events published (the coordination bus)
jq 'select(.event.type == "event_published") | {iter: .iteration, topic: .event.topic}' "$SESSION/orchestration.jsonl"

# Backpressure triggers (failures that rejected work)
jq 'select(.event.type == "backpressure_triggered") | {iter: .iteration, reason: .event.reason}' "$SESSION/orchestration.jsonl"

# How did the loop end?
jq 'select(.event.type == "loop_terminated")' "$SESSION/orchestration.jsonl"
```

### 3. Agent Activity

```bash
# What tools did the agent call?
jq 'select(.type == "tool_call") | {iter: .iteration, tool: .name}' "$SESSION/agent-output.jsonl"

# Agent text output (what it said)
jq 'select(.type == "text") | {iter: .iteration, hat: .hat, text: .text[:100]}' "$SESSION/agent-output.jsonl"

# Tool calls per iteration
jq -s '[.[] | select(.type == "tool_call")] | group_by(.iteration) | map({iter: .[0].iteration, tools: [.[].name]})' "$SESSION/agent-output.jsonl"
```

### 4. Performance

```bash
# Iteration durations
jq 'select(.metric.type == "iteration_duration") | {iter: .iteration, ms: .metric.duration_ms}' "$SESSION/performance.jsonl"

# Token usage per iteration
jq 'select(.metric.type == "token_count") | {iter: .iteration, hat: .hat, in: .metric.input, out: .metric.output}' "$SESSION/performance.jsonl"

# Agent latency
jq 'select(.metric.type == "agent_latency") | {iter: .iteration, hat: .hat, ms: .metric.duration_ms}' "$SESSION/performance.jsonl"

# Total tokens
jq -s '[.[] | select(.metric.type == "token_count")] | {total_in: (map(.metric.input) | add), total_out: (map(.metric.output) | add)}' "$SESSION/performance.jsonl"
```

### 5. Trace Logs

```bash
# Errors and warnings only
jq 'select(.level == "ERROR" or .level == "WARN")' "$SESSION/trace.jsonl"

# Filter by module
jq 'select(.target | startswith("ralph_core"))' "$SESSION/trace.jsonl"
jq 'select(.target | startswith("ralph_adapters"))' "$SESSION/trace.jsonl"
```

## Common Investigations

### Why was this hat selected?

```bash
jq 'select(.event.type == "hat_selected") | {iter: .iteration, hat: .event.hat, reason: .event.reason}' "$SESSION/orchestration.jsonl"
```

Reasons: `pending_events` (event routing), `process_output` (continuation), `tasks_ready` (task-driven), `default` (fallback)

### Why did the loop terminate?

```bash
# Termination event
jq 'select(.event.type == "loop_terminated")' "$SESSION/orchestration.jsonl"

# Last few orchestration events for context
jq '.' "$SESSION/orchestration.jsonl" | tail -10
```

Reasons: `completion_promise` (normal), `max_iterations` (limit hit), `error` (fatal)

### Why was work rejected (backpressure)?

```bash
# All backpressure events
jq 'select(.event.type == "backpressure_triggered") | {iter: .iteration, reason: .event.reason}' "$SESSION/orchestration.jsonl"

# Corresponding validation failures
jq 'select(.error_type == "validation_failure") | {iter: .iteration, rule: .context.rule, evidence: .context.evidence}' "$SESSION/errors.jsonl"
```

### Is the agent stuck in a loop?

```bash
# Check for repeated tool calls
jq -s '[.[] | select(.type == "tool_call")] | group_by(.name) | map({tool: .[0].name, count: length}) | sort_by(-.count)' "$SESSION/agent-output.jsonl"

# Check iteration count vs events published
echo "Iterations:"
jq 'select(.event.type == "iteration_started")' "$SESSION/orchestration.jsonl" | wc -l
echo "Events published:"
jq 'select(.event.type == "event_published")' "$SESSION/orchestration.jsonl" | wc -l
```

**Red flag:** Many iterations with few events = agent not making progress.

### Hat routing health

```bash
# Iterations per hat (should be roughly 1 hat per iteration)
jq -s '[.[] | select(.event.type == "hat_selected")] | group_by(.event.hat) | map({hat: .[0].event.hat, count: length})' "$SESSION/orchestration.jsonl"

# Check for same-iteration hat switching (bad: multiple hats in one iteration)
jq -s '[.[] | select(.event.type == "hat_selected")] | group_by(.iteration) | map(select(length > 1)) | map({iter: .[0].iteration, hats: [.[].event.hat]})' "$SESSION/orchestration.jsonl"
```

### Telegram/human interaction issues

```bash
# Telegram errors
jq 'select(.error_type == "telegram_send_error")' "$SESSION/errors.jsonl"

# Timeout errors (human didn't respond in time)
jq 'select(.error_type == "timeout" and .context.operation == "human_response")' "$SESSION/errors.jsonl"
```

## Quick Health Check

Run all checks at once:

```bash
SESSION=".ralph/diagnostics/$(ls -t .ralph/diagnostics/ | head -1)"
echo "=== Session: $SESSION ==="

echo -e "\n--- Errors ---"
wc -l < "$SESSION/errors.jsonl" 2>/dev/null || echo "0"

echo -e "\n--- Iterations ---"
jq -s 'map(select(.event.type == "iteration_started")) | length' "$SESSION/orchestration.jsonl"

echo -e "\n--- Hats Used ---"
jq -s '[.[] | select(.event.type == "hat_selected") | .event.hat] | unique' "$SESSION/orchestration.jsonl"

echo -e "\n--- Events Published ---"
jq -s '[.[] | select(.event.type == "event_published") | .event.topic] | unique' "$SESSION/orchestration.jsonl"

echo -e "\n--- Termination ---"
jq 'select(.event.type == "loop_terminated")' "$SESSION/orchestration.jsonl"

echo -e "\n--- Backpressure Count ---"
jq -s 'map(select(.event.type == "backpressure_triggered")) | length' "$SESSION/orchestration.jsonl"
```

## Comparing Sessions

```bash
# Compare iteration counts between two sessions
for s in .ralph/diagnostics/*/; do
  iters=$(jq -s 'map(select(.event.type == "iteration_started")) | length' "$s/orchestration.jsonl" 2>/dev/null)
  errors=$(wc -l < "$s/errors.jsonl" 2>/dev/null | tr -d ' ')
  echo "$(basename $s): ${iters:-0} iterations, ${errors:-0} errors"
done
```

## Cleanup

```bash
ralph clean --diagnostics              # Delete all sessions
ralph clean --diagnostics --dry-run    # Preview what would be deleted
```
