# Research: Text vs Stream-JSON for Roo — Impact on Loop Success

## What the Developer Experiences

### Text Mode (like kiro, gemini, codex, amp, copilot, opencode)

The developer running `ralph run -b roo` sees:
- **Raw terminal output** — whatever roo prints, streamed to console/TUI
- **No structured event parsing** — Ralph treats all output as plain text
- **Event extraction from text** — Ralph uses regex to find `<event topic="...">` tags in the text output
- **No cost tracking** — Total cost per iteration is unknown
- **No token counting** — No visibility into input/output tokens or cache hits
- **No tool-use visibility** — Ralph can't distinguish tool calls from text output in the TUI

### Stream-JSON Mode (like Claude, Pi)

The developer running `ralph run -b roo` would see:
- **Parsed, structured output** — PrettyStreamHandler renders markdown, tool calls, thinking
- **Real-time streaming** — Text arrives as deltas, displayed incrementally
- **Cost tracking** — Each iteration reports `totalCost`, `inputTokens`, `outputTokens`, `cacheReads`, `cacheWrites`
- **Tool-use visibility** — Tool calls and results are displayed with names and inputs
- **Thinking/reasoning** — Thinking tokens are shown (in TUI mode) or hidden (in console)
- **Extracted text** — Clean text is extracted from JSON for event tag parsing (more reliable)

## Impact on Loop Success Rate

### Does stream-json improve loop success?

**No direct impact on success rate.** The agent's behavior is identical regardless of output format — the prompt and agent logic don't change. The loop's success depends on:
1. The prompt quality
2. The agent's capability  
3. Backpressure signals (test failures, lint errors)

**However, stream-json provides significant operational benefits:**

| Capability | Text Mode | Stream-JSON | Impact |
|-----------|-----------|-------------|--------|
| **Event tag extraction** | Regex on raw output (fragile) | Clean extracted_text (reliable) | Moderate — more reliable event parsing means fewer "missed events" |
| **Cost monitoring** | None — no per-iteration cost | Per-iteration cost + totals | High for observability — developer sees burn rate |
| **Token usage** | None | Input/output/cache tokens | Medium — developer can spot context window issues |
| **TUI display quality** | Raw text with ANSI codes | Structured markdown rendering | Medium — cleaner display, easier to spot issues |
| **Error detection** | Parse exit code only | Structured `result.success` + `error` events | Low — exit code is usually sufficient |
| **Thinking visibility** | Not available | Visible in TUI | Low — nice to have for debugging |
| **Idle detection** | Same | Same | None — both use process-level idle detection |

### Key Insight: Event Tag Extraction

The most impactful technical difference is **event tag extraction**. Ralph's event system relies on finding `<event topic="build.done">` tags in the agent output. With text mode, these must be found via regex in the raw terminal output (which may contain ANSI escape codes, line wrapping artifacts, etc.). With stream-json, the `extracted_text` field contains clean text extracted from structured events, making event parsing more reliable.

From the code (`loop_runner.rs:3936-3939`):
```rust
let output_for_parsing = if pty_result.extracted_text.is_empty() {
    pty_result.stripped_output    // Text mode: ANSI-stripped raw output
} else {
    pty_result.extracted_text    // Stream-JSON: clean extracted text
};
```

## Implementation Cost

| Aspect | Text Mode | Stream-JSON |
|--------|-----------|-------------|
| New files | 0 | 1 (`roo_stream.rs`) |
| Lines of code | ~30 (backend defs) | ~200-400 (parser + dispatch) |
| Test complexity | Low | Medium (parser tests needed) |
| Maintenance | Low | Medium (must track roo-cli stream format changes) |
| Time to implement | ~1 hour | ~4-8 hours |

## Recommendation

**Start with text mode, add stream-json later.**

Rationale:
1. Text mode is proven — 6 of 9 providers use it successfully
2. The core loop works identically with text mode
3. Stream-JSON can be added as an enhancement without breaking changes
4. Roo's stream format is still evolving (v0.1.15) — investing in a parser now may require rework
5. Text mode gets the feature shipped fast; stream-json is a quality-of-life improvement

**However**, if cost monitoring and clean event extraction are priorities, stream-json is worth the upfront investment.
