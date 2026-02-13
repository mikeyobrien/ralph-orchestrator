# Research: Pi NDJSON Schema (Definitive Reference)

## Summary

Captured and verified against real `pi -p --mode json --no-session` output. Pi emits events as one JSON object per line. **No `agent_end` event is emitted in print mode** — the last event is always `turn_end`. Cost/usage data is available per-turn in `turn_end.message.usage`.

## Event Sequence

Typical session with one tool call:

```
session                              ← once, first event
agent_start                          ← once
turn_start                           ← per turn
  message_start (role: user)         ← user prompt
  message_end (role: user)
  message_start (role: assistant)    ← empty content initially
    message_update (toolcall_start)  ← tool call streaming
    message_update (toolcall_delta)  ← partial JSON args
    message_update (toolcall_end)    ← complete tool call
  message_end (role: assistant)      ← full content with toolCall
  tool_execution_start               ← tool runs
  tool_execution_update              ← partial output
  tool_execution_end                 ← final result
  message_start (role: toolResult)   ← tool result message
  message_end (role: toolResult)
turn_end                             ← stopReason: "toolUse", has usage
turn_start                           ← next turn (response to tool result)
  message_start (role: assistant)
    message_update (text_start)
    message_update (text_delta)      ← repeated, text chunks
    message_update (text_end)
  message_end (role: assistant)      ← full text content
turn_end                             ← stopReason: "stop", has usage
                                     ← END (no agent_end in print mode)
```

## Event Schemas

### session

First event. Identifies the session.

```json
{
  "type": "session",
  "version": 3,
  "id": "uuid",
  "timestamp": "2026-02-05T02:39:26.125Z",
  "cwd": "/path/to/cwd"
}
```

### agent_start

Agent begins processing. No payload.

```json
{"type": "agent_start"}
```

### turn_start

New turn begins (one LLM call + tool executions). No payload.

```json
{"type": "turn_start"}
```

### message_start

Message begins. Contains the message object with role.

**User message:**
```json
{
  "type": "message_start",
  "message": {
    "role": "user",
    "content": [{"type": "text", "text": "the prompt"}],
    "timestamp": 1770259166905
  }
}
```

**Assistant message (initial, empty):**
```json
{
  "type": "message_start",
  "message": {
    "role": "assistant",
    "content": [],
    "api": "anthropic-messages",
    "provider": "anthropic",
    "model": "claude-opus-4-5",
    "usage": {"input": 0, "output": 0, "cacheRead": 0, "cacheWrite": 0, "totalTokens": 0,
              "cost": {"input": 0, "output": 0, "cacheRead": 0, "cacheWrite": 0, "total": 0}},
    "stopReason": "stop",
    "timestamp": 1770259166907
  }
}
```

**Tool result message:**
```json
{
  "type": "message_start",
  "message": {
    "role": "toolResult",
    "toolCallId": "toolu_01BKzy4E5YAeFLdgwFKtNRqv",
    "toolName": "bash",
    "content": [{"type": "text", "text": "hello\n"}],
    "isError": false,
    "timestamp": 1770259168473
  }
}
```

### message_update

Streaming deltas during assistant response. Always has `assistantMessageEvent` sub-object.

**Sub-event types (in `assistantMessageEvent.type`):**

| Sub-type | Key fields | Purpose |
|----------|-----------|---------|
| `text_start` | `contentIndex` | Text block begins |
| `text_delta` | `contentIndex`, `delta` | Text chunk (the actual text) |
| `text_end` | `contentIndex`, `content` | Text block ends, `content` has full text |
| `thinking_start` | `contentIndex` | Thinking block begins |
| `thinking_delta` | `contentIndex`, `delta` | Thinking chunk |
| `thinking_end` | `contentIndex`, `content` | Thinking block ends |
| `toolcall_start` | `contentIndex` | Tool call begins (has partial with name/id) |
| `toolcall_delta` | `contentIndex`, `delta` | Partial JSON args |
| `toolcall_end` | `contentIndex`, `toolCall` | Complete tool call object |
| `done` | `reason` | Message complete ("stop", "length", "toolUse") |
| `error` | `reason` | Error ("aborted", "error") |

**text_delta (the event Ralph cares about most):**
```json
{
  "type": "message_update",
  "assistantMessageEvent": {
    "type": "text_delta",
    "contentIndex": 0,
    "delta": "Hello world"
  },
  "message": { /* full accumulated message snapshot - IGNORED for streaming */ }
}
```

**toolcall_end (complete tool call info):**
```json
{
  "type": "message_update",
  "assistantMessageEvent": {
    "type": "toolcall_end",
    "contentIndex": 0,
    "toolCall": {
      "type": "toolCall",
      "id": "toolu_01BKzy4E5YAeFLdgwFKtNRqv",
      "name": "bash",
      "arguments": {"command": "echo hello"}
    }
  },
  "message": { /* full accumulated message snapshot */ }
}
```

### message_end

Message complete. Contains final message object.

**Assistant message_end (has full content and final usage):**
```json
{
  "type": "message_end",
  "message": {
    "role": "assistant",
    "content": [{"type": "text", "text": "Done. Output: hello."}],
    "api": "anthropic-messages",
    "provider": "anthropic",
    "model": "claude-opus-4-5",
    "usage": {
      "input": 1, "output": 14, "cacheRead": 8932, "cacheWrite": 70,
      "totalTokens": 9017,
      "cost": {"input": 0.000005, "output": 0.00035, "cacheRead": 0.00447, "cacheWrite": 0.00044, "total": 0.00526}
    },
    "stopReason": "stop",
    "timestamp": 1770259166907
  }
}
```

### tool_execution_start

Tool begins execution. Flat structure with tool info.

```json
{
  "type": "tool_execution_start",
  "toolCallId": "toolu_01BKzy4E5YAeFLdgwFKtNRqv",
  "toolName": "bash",
  "args": {"command": "echo hello"}
}
```

### tool_execution_update

Partial tool output (accumulated, not delta).

```json
{
  "type": "tool_execution_update",
  "toolCallId": "toolu_01BKzy4E5YAeFLdgwFKtNRqv",
  "toolName": "bash",
  "args": {"command": "echo hello"},
  "partialResult": {
    "content": [{"type": "text", "text": "hello\n"}],
    "details": {}
  }
}
```

### tool_execution_end

Tool complete. Has final result and error flag.

```json
{
  "type": "tool_execution_end",
  "toolCallId": "toolu_01BKzy4E5YAeFLdgwFKtNRqv",
  "toolName": "bash",
  "result": {
    "content": [{"type": "text", "text": "hello\n"}]
  },
  "isError": false
}
```

### turn_end

Turn complete. **This is where per-turn usage/cost lives.** Also the last event in print mode.

```json
{
  "type": "turn_end",
  "message": {
    "role": "assistant",
    "content": [...],
    "usage": {
      "input": 1, "output": 14, "cacheRead": 8932, "cacheWrite": 70,
      "totalTokens": 9017,
      "cost": {"input": 0.000005, "output": 0.00035, "cacheRead": 0.00447, "cacheWrite": 0.00044, "total": 0.00526}
    },
    "stopReason": "stop"
  },
  "toolResults": []
}
```

**`stopReason` values:**
- `"stop"` — natural completion
- `"toolUse"` — agent wants to call tools (more turns coming)
- `"length"` — hit token limit
- `"error"` — error occurred
- `"aborted"` — aborted

## Mapping to Ralph's StreamHandler

For the `PiStreamParser`, only a subset of events need handling:

| Pi event | Extract | StreamHandler call |
|----------|---------|-------------------|
| `message_update` (text_delta) | `assistantMessageEvent.delta` | `on_text(delta)` |
| `tool_execution_start` | `toolName`, `toolCallId`, `args` | `on_tool_call(name, id, args)` |
| `tool_execution_end` | `toolCallId`, `result.content[0].text` | `on_tool_result(id, output)` |
| `message_update` (error) | `assistantMessageEvent.reason` | `on_error(reason)` |
| `turn_end` (last one, `stopReason: "stop"`) | `message.usage.cost.total` | `on_complete(result)` |

**Events to ignore:** `session`, `agent_start`, `turn_start`, `message_start`, `message_end`, `message_update` (text_start, text_end, thinking_*, toolcall_start, toolcall_delta, toolcall_end, done), `tool_execution_update`.

**`extracted_text` accumulation:** Collect from `text_delta` events (same as `on_text` calls). This feeds Ralph's event parser for LOOP_COMPLETE detection.

## Cost Tracking

No single summary event like Claude's `result`. Instead:
1. Each `turn_end` has per-turn `message.usage.cost.total`
2. Accumulate across turns: `total_cost = sum(turn_end.message.usage.cost.total)`
3. For `on_complete()`: use accumulated totals from all `turn_end` events

**Turn count:** Count `turn_end` events.

**Duration:** Not provided by pi. Ralph must calculate from wall-clock time (already does this for non-Claude backends).

## Key Differences from Claude stream-json

| Aspect | Claude | Pi |
|--------|--------|-----|
| Text delivery | Complete text blocks | Character-level deltas |
| Tool calls in stream | Inside assistant content blocks | Separate `tool_execution_*` events |
| Session summary | Dedicated `result` event | No summary; accumulate from `turn_end` |
| Final event | `result` | `turn_end` (no `agent_end` in print mode) |
| Usage data | Per-assistant-turn `usage` | Per-turn in `turn_end.message.usage` |
| Cost format | `total_cost_usd` (float) | `usage.cost.total` (float, nested) |
| Duration | `duration_ms` in `result` | Not provided |
