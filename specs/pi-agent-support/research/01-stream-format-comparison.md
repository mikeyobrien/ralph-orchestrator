# Research: Pi NDJSON Stream Format vs Claude stream-json

## Summary

Pi's `--mode json` NDJSON format and Claude's `--output-format stream-json` have fundamentally different schemas. A new parser is required; `ClaudeStreamParser` cannot be reused.

## Claude's stream-json Format

Claude emits 3-4 event types per session:

```json
{"type":"system","session_id":"abc123","model":"claude-opus","tools":[]}
{"type":"assistant","message":{"content":[{"type":"text","text":"Hello"},{"type":"tool_use","id":"id1","name":"Bash","input":{"command":"ls"}}]},"usage":{"input_tokens":100,"output_tokens":50}}
{"type":"user","message":{"content":[{"type":"tool_result","tool_use_id":"id1","content":"output"}]}}
{"type":"result","duration_ms":1500,"total_cost_usd":0.01,"num_turns":3,"is_error":false}
```

Key traits:
- **Coarse-grained**: Each `assistant` event contains the FULL message with all content blocks
- **Flat structure**: `type` discriminator at top level with `system`, `assistant`, `user`, `result`
- **Usage per assistant turn**: `usage` object with `input_tokens`, `output_tokens`
- **Session summary**: `result` event at end with duration, cost, turns

## Pi's --mode json Format

Pi emits fine-grained streaming events:

```json
{"type":"session","version":3,"id":"...","timestamp":"...","cwd":"..."}
{"type":"agent_start"}
{"type":"turn_start"}
{"type":"message_start","message":{"role":"user","content":[...],"timestamp":...}}
{"type":"message_end","message":{"role":"user","content":[...],"timestamp":...}}
{"type":"message_start","message":{"role":"assistant","content":[],...}}
{"type":"message_update","assistantMessageEvent":{"type":"text_start","contentIndex":0,...},"message":{...}}
{"type":"message_update","assistantMessageEvent":{"type":"text_delta","contentIndex":0,"delta":"Hello "},"message":{...}}
{"type":"message_update","assistantMessageEvent":{"type":"text_end","contentIndex":0,"content":"Hello"},"message":{...}}
{"type":"message_update","assistantMessageEvent":{"type":"thinking_start",...},"message":{...}}
{"type":"message_update","assistantMessageEvent":{"type":"thinking_delta","contentIndex":1,"delta":"..."},"message":{...}}
{"type":"message_update","assistantMessageEvent":{"type":"thinking_end",...},"message":{...}}
{"type":"message_update","assistantMessageEvent":{"type":"toolcall_start",...},"message":{...}}
{"type":"message_update","assistantMessageEvent":{"type":"toolcall_delta",...},"message":{...}}
{"type":"message_update","assistantMessageEvent":{"type":"toolcall_end",...},"message":{...}}
{"type":"message_end","message":{...}}
{"type":"tool_execution_start","toolCallId":"...","toolName":"bash","args":{"command":"ls"}}
{"type":"tool_execution_update","toolCallId":"...","toolName":"bash","partialResult":{...}}
{"type":"tool_execution_end","toolCallId":"...","toolName":"bash","result":{"content":[{"type":"text","text":"..."}]},"isError":false}
{"type":"turn_end","message":{...},"toolResults":[...]}
{"type":"agent_end","messages":[...]}
```

Key traits:
- **Fine-grained**: Individual delta events for text, thinking, tool calls
- **Nested `assistantMessageEvent`**: Streaming events wrapped in `message_update` with sub-types
- **Full message snapshots**: Each `message_update` includes the full accumulated `message` object
- **Lifecycle events**: `agent_start/end`, `turn_start/end`, `message_start/end`
- **Usage in message**: `usage` object inside `message` with `input`, `output`, `cacheRead`, `cacheWrite`, `cost`
- **Cost breakdown**: `cost` object with `input`, `output`, `cacheRead`, `cacheWrite`, `total`
- **No session summary event**: Cost/usage data is embedded in the final message's `usage` field

## Mapping Pi Events to StreamHandler

| StreamHandler method | Claude source | Pi source |
|---------------------|---------------|-----------|
| `on_text(text)` | `assistant.message.content[].text` | `message_update` where `assistantMessageEvent.type == "text_delta"` → use `delta` field |
| `on_tool_call(name, id, input)` | `assistant.message.content[].tool_use` | `tool_execution_start` → `toolName`, `toolCallId`, `args` |
| `on_tool_result(id, output)` | `user.message.content[].tool_result` | `tool_execution_end` → `toolCallId`, `result.content[0].text` |
| `on_error(error)` | `result.is_error` | `message_update` where `assistantMessageEvent.type == "error"` |
| `on_complete(result)` | `result` event | `agent_end` → synthesize from accumulated usage data |

## Key Differences

1. **Text extraction**: Claude gives complete text blocks; pi gives deltas that must be concatenated
2. **Tool calls**: Claude embeds tool_use in assistant content; pi has separate `tool_execution_start/end` events
3. **Cost tracking**: Claude has a dedicated `result` event; pi embeds cost in per-message `usage` fields
4. **Thinking**: Claude doesn't expose thinking in stream-json; pi streams thinking deltas (can be ignored)
5. **Event volume**: Pi emits ~10x more events for the same interaction (every delta is an event)

## Recommendation

Create a `PiStreamParser` (analogous to `ClaudeStreamParser`) that:
1. Parses pi's NDJSON events into a `PiStreamEvent` enum
2. Has a `dispatch_pi_stream_event()` function that maps to `StreamHandler` calls
3. Accumulates cost/usage data across events for the `on_complete()` call
4. Extracts text content for `extracted_text` (used by event loop for LOOP_COMPLETE detection)

The `OutputFormat` enum should get a new `PiStreamJson` variant (or reuse `StreamJson` since the dispatch logic is in the executor, not the format enum).
