# Requirements: Pi Agent Support

## Q&A Record

### Q1: What scope defines "comprehensive" for v1?

Based on research, there are 3 tiers of integration depth:

**Tier 1 (CLI Backend):** Add `pi` as a named backend with headless/interactive constructors, auto-detection, NDJSON stream parsing (`PiStreamParser`), cost tracking, and tool call display. Pi becomes a first-class backend on par with Claude for structured output.

**Tier 2 (Configuration):** Support pi-specific config options in `ralph.yml` — provider, model, thinking level, extensions, skills. Enable per-hat pi configuration.

**Tier 3 (RPC Integration):** Use pi's RPC mode for persistent sessions across iterations, real-time steering from RObot, and mid-run abort.

Which tiers should v1 cover? My recommendation: Tier 1 + Tier 2, defer Tier 3.

**A1:** Tier 1 + Tier 2. Defer Tier 3 (RPC) to a future version.

### Q2: Should pi be added to the default auto-detection priority list, and if so, where?

Current priority: `claude, kiro, gemini, codex, amp, copilot, opencode`

Options:
- **After claude** (2nd): Pi is the only other backend with NDJSON streaming, making it the best fallback when Claude CLI isn't available. Pi can also use Anthropic models, so it's closest in capability.
- **Last**: Conservative — don't change existing behavior for users who already have other backends installed.
- **Not in default list**: Only usable when explicitly configured (`backend: pi`).

**A2:** Last in the priority list. Conservative — existing users unaffected, but `agent: auto` will find pi if nothing else is available.

### Q3: How should pi's multi-provider capability be exposed in ralph.yml?

Pi uniquely supports multiple LLM providers (Anthropic, OpenAI, Google, etc.) via `--provider` and `--model` flags. This means a single `pi` backend can use different models depending on the hat.

Option A — Pass-through args only:
```yaml
hats:
  planner:
    backend:
      type: pi
      args: ["--provider", "anthropic", "--model", "claude-sonnet-4"]
```

Option B — Structured config with pi-specific fields:
```yaml
hats:
  planner:
    backend:
      type: pi
      provider: anthropic
      model: claude-sonnet-4
      thinking: medium
```

Option C — Both (structured fields that compile to args):
Structured fields in config, converted to CLI args at build time. Unknown fields passed through as raw args.

**A3:** Option A — pass-through args only. `NamedWithArgs` already supports this with zero config changes. Structured fields can be added later as backwards-compatible sugar if there's demand.

### Q4: Should the `PiStreamParser` extract tool call info from `tool_execution_start` or from `toolcall_end` inside `message_update`?

Both contain the same data (tool name, ID, arguments). Research found:
- `tool_execution_start` — flat, simple structure, appears once per tool call
- `toolcall_end` (in `message_update`) — nested inside `assistantMessageEvent`, redundant with `tool_execution_start`

Recommendation: Use `tool_execution_start` for `on_tool_call()` — it's cleaner, matches the event-level abstraction, and avoids parsing nested `message_update` sub-types just for tool info.

**A4:** Use `tool_execution_start`. Ignore `toolcall_start/delta/end` in `message_update`.

### Q5: How should the `OutputFormat` enum be extended?

Currently: `Text` and `StreamJson` (Claude only).

Options:
- **Add `PiStreamJson`**: Explicit variant, branched separately in `run_observe_streaming()`
- **Reuse `StreamJson`**: Single variant, but dispatch logic detects pi vs Claude from the first JSON line

Recommendation: Add `PiStreamJson`. The schemas are different enough that conflating them behind one variant would be confusing. The branch in `run_observe_streaming()` is the only place it matters, and the cost is one extra match arm.

**A5:** Add `PiStreamJson` variant. Explicit and clean.

### Q6: How should pi auto-detection handle the `pi` binary name collision risk?

The binary name `pi` could conflict with other tools (e.g., Raspberry Pi utilities). Options:

- **`pi --version` only**: Simple, matches other backends. Accept the collision risk.
- **`pi --version` + validate output**: Check that version output contains `pi-coding-agent` or similar marker.
- **`pi --help` parse**: More robust but slower.

**A6:** `pi --version` only. Accept the collision risk — pi is last in priority anyway, so it only triggers if nothing else is found.

### Q7: Should pi's thinking output (thinking_start/delta/end) be surfaced in Ralph's TUI/console, or silently ignored?

Claude's stream-json doesn't expose thinking. Pi does. Options:
- **Ignore**: Don't show thinking output. Simplest, matches Claude behavior.
- **Verbose only**: Show thinking in verbose mode, skip in normal mode.

**A7:** Verbose only. Show thinking deltas in verbose mode, ignore otherwise.

### Q8: For cost tracking, should Ralph sum per-turn costs from `turn_end` events, or use the final `message_end` usage?

Both contain cost data. `turn_end` is more reliable since it's always the last event. The final `message_end` only covers the last assistant response, not tool result messages.

Recommendation: Accumulate from `turn_end.message.usage.cost.total` across all turns. This gives total session cost for `on_complete()`.

**A8:** Sum `turn_end.message.usage.cost.total` across all turns for session total.

