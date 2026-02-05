# Diagnostics

The diagnostics system captures complete visibility into Hats's operation for debugging and analysis.

## Enabling Diagnostics

Opt-in via environment variable:

```bash
HATS_DIAGNOSTICS=1 hats run -p "your prompt"
```

**Zero overhead when disabled** — diagnostics code is bypassed entirely.

## Output Location

Diagnostics are written to timestamped session directories:

```
.hats/diagnostics/
└── 2024-01-21T08-45-30/           # ISO 8601 timestamp
    ├── agent-output.jsonl          # Agent text, tool calls, results
    ├── orchestration.jsonl         # Hat selection, events, backpressure
    ├── trace.jsonl                 # All tracing logs with metadata
    ├── performance.jsonl           # Timing, latency, token counts
    └── errors.jsonl                # Parse errors, validation failures
```

## File Contents

### agent-output.jsonl

Everything the AI agent outputs:

```json
{"timestamp":"2024-01-21T08:45:35Z","type":"text","content":"Let me analyze..."}
{"timestamp":"2024-01-21T08:45:40Z","type":"tool_call","tool":"read_file","args":{"path":"src/lib.rs"}}
{"timestamp":"2024-01-21T08:45:42Z","type":"tool_result","tool":"read_file","result":"..."}
```

### orchestration.jsonl

Hat selection and event flow:

```json
{"timestamp":"2024-01-21T08:45:30Z","event":{"type":"hat_selected","hat":"builder"}}
{"timestamp":"2024-01-21T08:46:00Z","event":{"type":"event_published","topic":"build.done"}}
{"timestamp":"2024-01-21T08:46:01Z","event":{"type":"event_routed","topic":"build.done","target":"reviewer"}}
```

### trace.jsonl

All tracing logs with metadata:

```json
{"timestamp":"2024-01-21T08:45:30Z","level":"INFO","target":"hats_core","message":"Starting iteration 1"}
{"timestamp":"2024-01-21T08:45:31Z","level":"DEBUG","target":"hats_adapters","message":"Spawning claude process"}
{"timestamp":"2024-01-21T08:46:00Z","level":"WARN","target":"hats_core","message":"Approaching context limit"}
```

### performance.jsonl

Timing and resource usage:

```json
{"timestamp":"2024-01-21T08:45:30Z","iteration":1,"duration_ms":30000,"tokens_in":1500,"tokens_out":2000}
{"timestamp":"2024-01-21T08:46:30Z","iteration":2,"duration_ms":25000,"tokens_in":1800,"tokens_out":1500}
```

### errors.jsonl

Errors and failures:

```json
{"timestamp":"2024-01-21T08:45:50Z","type":"parse_error","message":"Failed to parse event","raw":"invalid json"}
{"timestamp":"2024-01-21T08:46:10Z","type":"validation_error","message":"Hat 'unknown' not found"}
```

## Reviewing Diagnostics

### With jq

```bash
# All agent text output
jq 'select(.type == "text")' .hats/diagnostics/*/agent-output.jsonl

# All tool calls
jq 'select(.type == "tool_call")' .hats/diagnostics/*/agent-output.jsonl

# Hat selection decisions
jq 'select(.event.type == "hat_selected")' .hats/diagnostics/*/orchestration.jsonl

# All events
jq '.event' .hats/diagnostics/*/orchestration.jsonl

# All errors
jq '.' .hats/diagnostics/*/errors.jsonl

# ERROR level traces
jq 'select(.level == "ERROR")' .hats/diagnostics/*/trace.jsonl

# Performance by iteration
jq '{iteration, duration_ms, tokens_in, tokens_out}' .hats/diagnostics/*/performance.jsonl
```

### Common Queries

**Why was this hat selected?**

```bash
jq 'select(.event.type == "hat_selected" and .event.hat == "builder")' \
  .hats/diagnostics/*/orchestration.jsonl
```

**What events were published?**

```bash
jq 'select(.event.type == "event_published") | .event.topic' \
  .hats/diagnostics/*/orchestration.jsonl
```

**How long did each iteration take?**

```bash
jq '{iteration, duration_ms}' .hats/diagnostics/*/performance.jsonl
```

**Were there parse errors?**

```bash
jq 'select(.type == "parse_error")' .hats/diagnostics/*/errors.jsonl
```

## Cleanup

Remove diagnostics files:

```bash
hats clean --diagnostics
```

Or manually:

```bash
rm -rf .hats/diagnostics/
```

## When to Use

Enable diagnostics when:

- Debugging why a specific hat was selected
- Understanding agent output flow
- Investigating backpressure triggers
- Analyzing performance bottlenecks
- Post-mortem on failed runs
- Developing custom hats

## Best Practices

1. **Enable for debugging, disable for production** — Diagnostics add I/O overhead
2. **Clean up old sessions** — They can grow large
3. **Use jq for analysis** — JSONL is designed for streaming queries
4. **Save problematic sessions** — Copy before cleaning for later analysis

## Integration with TUI

The TUI shows summary information. For details, check diagnostics:

| TUI Shows | Diagnostics Provides |
|-----------|---------------------|
| Current hat | Full selection history |
| Recent output | Complete output log |
| Iteration count | Timing per iteration |
| Event topic | Full event payload |

## Example Debug Session

```bash
# 1. Run with diagnostics
HATS_DIAGNOSTICS=1 hats run -p "implement feature X"

# 2. Find the session
ls -la .hats/diagnostics/
# 2024-01-21T08-45-30/

# 3. Check for errors first
jq '.' .hats/diagnostics/2024-01-21T08-45-30/errors.jsonl

# 4. Review hat selections
jq '.event' .hats/diagnostics/2024-01-21T08-45-30/orchestration.jsonl

# 5. Check what the agent did
jq 'select(.type == "tool_call")' .hats/diagnostics/2024-01-21T08-45-30/agent-output.jsonl

# 6. Review performance
jq '{iteration, duration_ms}' .hats/diagnostics/2024-01-21T08-45-30/performance.jsonl
```

## Next Steps

- Learn about [Testing & Validation](testing.md)
- Explore [Creating Custom Hats](custom-hats.md)
- Understand the [Event System](event-system.md)
