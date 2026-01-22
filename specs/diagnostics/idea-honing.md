# Requirements Clarification

This document captures the Q&A process for refining the diagnostic logging feature.

---

## Q1: What diagnostic information is most critical to capture?

Ralph currently has multiple data streams that *could* be logged:

1. **Agent output** — Raw text/JSON from Claude CLI or Kiro (what the agent says/does)
2. **Event bus events** — Topic-based events (task.start, build.done, etc.) with metadata
3. **Orchestration decisions** — Hat selection, backpressure validation, iteration state
4. **Tracing logs** — Existing `info!`/`debug!`/`warn!` macro calls in the codebase
5. **Timing/performance** — Durations, latencies, token counts
6. **Errors and failures** — Parse errors, validation failures, termination reasons

Which of these are most important for diagnosing issues? (Select all that apply, or prioritize)

**A1:** All of them. Capture everything:
- Agent output (raw text/JSON)
- Event bus events (with metadata)
- Orchestration decisions
- Tracing logs
- Timing/performance data
- Errors and failures

---

## Q2: Where should diagnostic logs be written?

Options include:

1. **Single unified log file** — e.g., `.agent/diagnostics.log` (all data in one place)
2. **Separate files by type** — e.g., `.agent/agent-output.log`, `.agent/events.log`, `.agent/trace.log`
3. **Structured format (JSONL)** — Machine-readable, queryable
4. **Human-readable format** — Plain text with timestamps
5. **Both** — JSONL for tooling + human-readable summary

Also: Should logs be written to the **same location** in both TUI and non-TUI modes?

**A2:**
- Separate files by type
- Structured format (JSONL)
- Same location for both TUI and non-TUI modes
- Store in `.ralph/` directory (not `.agent/`)

Example structure:
```
.ralph/
├── events.jsonl        # (already exists)
├── agent-output.jsonl  # Raw agent text/JSON
├── orchestration.jsonl # Decisions, hat selection, backpressure
├── trace.jsonl         # Tracing logs (info/debug/warn/error)
├── performance.jsonl   # Timing, durations, token counts
└── errors.jsonl        # Parse errors, validation failures
```

---

## Q3: Should diagnostic logging be always-on or opt-in?

Current behavior:
- TUI mode: Logging **suppressed by default** (requires `RALPH_DEBUG_LOG=1`)
- Non-TUI mode: Logs go to stdout (mixed with agent output)

Options for the new diagnostic system:

1. **Always-on** — Diagnostic files always written (like `.ralph/events.jsonl` today)
2. **Opt-in via flag** — e.g., `--diagnostics` or `--debug`
3. **Opt-in via env var** — e.g., `RALPH_DIAGNOSTICS=1`
4. **Configurable in ralph.yml** — `diagnostics: { enabled: true, level: debug }`
5. **Tiered** — Basic diagnostics always-on, verbose diagnostics opt-in

**A3:** Opt-in via environment variable.

Example: `RALPH_DIAGNOSTICS=1 ralph run ...`

This keeps default behavior clean while making diagnostics easy to enable when needed.

---

## Q4: Should diagnostic files persist across runs or be overwritten?

Options:

1. **Overwrite each run** — Fresh diagnostic files per session (simpler, no cleanup needed)
2. **Append with session markers** — Single file per type, sessions separated by markers
3. **Timestamped files** — e.g., `.ralph/agent-output-2024-01-15T10-23-45.jsonl`
4. **Rotation with limit** — Keep last N sessions or last N MB

If files persist, should there be a cleanup mechanism (e.g., `ralph clean --diagnostics`)?

**A4:** Timestamped files with cleanup mechanism.

Example:
```
.ralph/diagnostics/
├── 2024-01-15T10-23-45/
│   ├── agent-output.jsonl
│   ├── orchestration.jsonl
│   ├── trace.jsonl
│   ├── performance.jsonl
│   └── errors.jsonl
└── 2024-01-15T14-30-00/
    └── ...
```

Cleanup: `ralph clean --diagnostics` or automatic rotation (keep last N sessions).

---

## Q5: What level of detail for agent output logging?

Agent output can be captured at different granularities:

1. **Raw bytes** — Exact terminal output including ANSI codes (most complete, largest files)
2. **Stripped text** — ANSI removed, clean UTF-8 text
3. **Parsed JSON events** — For StreamJson format, individual events (text chunks, tool calls, tool results)
4. **Summary only** — Just tool calls and final results, not streaming text

The research showed Ralph already captures raw bytes via `CliCapture` and parses JSON via `ClaudeStreamParser`. Which level(s) should diagnostic logging capture?

**A5:** Both:
- **Stripped text** — Clean UTF-8 without ANSI codes
- **Parsed JSON events** — Individual events (text chunks, tool calls, tool results)

This gives human-readable output plus structured data for analysis, without the overhead of raw bytes.

---

## Q6: Should there be a CLI command to query/analyze diagnostic logs?

Similar to the existing `ralph events` command, options include:

1. **No CLI** — Users can use `jq`, `grep`, or other tools on JSONL files
2. **Basic query command** — e.g., `ralph diagnostics --last-session` or `ralph diagnostics --errors`
3. **Rich filtering** — Filter by type, time range, iteration, log level
4. **Export/format options** — Output as table, JSON, or markdown report

Given that files are JSONL, external tools work well. Is a dedicated CLI command valuable?

**A6:** No dedicated CLI command.

Users can use standard tools (`jq`, `grep`, `cat`) on JSONL files. This keeps the CLI surface small and leverages existing Unix tooling.

Example usage:
```bash
# Find all errors
jq 'select(.level == "error")' .ralph/diagnostics/*/errors.jsonl

# Get tool calls from last session
jq 'select(.type == "tool_call")' .ralph/diagnostics/$(ls -t .ralph/diagnostics | head -1)/agent-output.jsonl
```

---

## Q7: What metadata should be included in each log entry?

For diagnostic logs to be useful, each entry needs context. Options:

**Always include:**
- Timestamp (ISO 8601)
- Log type/category

**Potentially include:**
- Iteration number
- Active hat
- Session ID (to correlate across files)
- Source file/line (for trace logs)
- Duration/elapsed time

What metadata is essential for your debugging workflows?

**A7:** Recommended metadata based on research:

**All log entries:**
- `ts` — Timestamp (ISO 8601)
- `iteration` — Current iteration number
- `hat` — Active hat (or "loop" for orchestrator)

**Type-specific additions:**
- **Agent output:** `type` (text/tool_call/tool_result), `content`
- **Orchestration:** `event` (hat_selected/backpressure/termination), `details`
- **Trace:** `level` (debug/info/warn/error), `target` (module path), `message`
- **Performance:** `metric` (duration/tokens/latency), `value`, `unit`
- **Errors:** `error_type`, `message`, `context`

Session ID not needed since files are already in timestamped directories.

---

## Q8: How should the cleanup mechanism work?

You mentioned wanting cleanup for timestamped diagnostic files. Options:

1. **Manual only** — `ralph clean --diagnostics` removes all diagnostic directories
2. **Keep last N sessions** — e.g., `ralph clean --diagnostics --keep 5`
3. **Age-based** — e.g., `ralph clean --diagnostics --older-than 7d`
4. **Size-based** — e.g., `ralph clean --diagnostics --max-size 100MB`
5. **Automatic rotation** — Built into Ralph, runs on startup (e.g., keep last 10 sessions)

**A8:** Manual only — `ralph clean --diagnostics` removes all diagnostic directories.

Simple and explicit. Users control when cleanup happens.

---

## Q9: Any specific debugging scenarios this should address?

We've covered the general requirements. Are there specific pain points or debugging scenarios you've encountered that this should help with?

Examples:
- "Why did the loop terminate early?"
- "What tool calls did the agent make?"
- "Why was backpressure triggered?"
- "How long did iteration N take?"

Or are there any edge cases to consider (e.g., very long sessions, crashes mid-run)?

**A9:** All debugging scenarios should be addressable:
- Loop termination reasons
- Tool call history
- Backpressure triggers
- Iteration timing
- Crash recovery (logs should flush incrementally, not buffer)

The diagnostic system should provide complete visibility into Ralph's operation.

