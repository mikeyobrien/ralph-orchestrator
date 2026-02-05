# Rough Idea: Add Comprehensive Pi Agent Support to Ralph Orchestrator

Add pi-coding-agent (pi) as a first-class backend in ralph-orchestrator, on par with Claude, Kiro, Gemini, Codex, Amp, Copilot, and OpenCode.

Pi is a TypeScript-based coding agent CLI (`@mariozechner/pi-coding-agent`) that supports:
- **Print mode** (`pi -p "prompt"`) — headless execution, outputs text or JSON
- **JSON streaming** (`pi -p --mode json "prompt"`) — NDJSON event stream with pi-specific event types
- **RPC mode** (`pi --mode rpc`) — bidirectional JSON protocol over stdin/stdout for rich integration
- **Interactive mode** (`pi "prompt"`) — TUI with initial prompt injection
- **SDK** (`@mariozechner/pi-coding-agent` npm package) — programmatic TypeScript API

Pi's NDJSON schema differs from Claude's `stream-json`: events are wrapped in pi-specific types (`agent_start`, `message_update` with `assistantMessageEvent`, `tool_execution_start/end`, etc.) rather than raw Anthropic Messages API events.

## Key Integration Points

1. **CliBackend**: New `pi()` and `pi_interactive()` constructors
2. **Auto-detection**: Add `pi` to the detection priority list
3. **Stream parsing**: Handle pi's NDJSON format for real-time output in Ralph's TUI/console
4. **RPC mode**: Potentially leverage pi's RPC for richer features (steering, follow-ups, compaction)
5. **Configuration**: Support pi-specific config options (provider, model, thinking level, extensions, skills)
6. **Cost tracking**: Parse pi's usage/cost data from NDJSON events
7. **Completion detection**: Parse pi's `agent_end` events for LOOP_COMPLETE detection
