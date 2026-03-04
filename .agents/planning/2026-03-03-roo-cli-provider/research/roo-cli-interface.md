# Research: roo-cli Interface & Stream Format

## CLI Binary & Version

- **Binary name**: `roo` (installed via npm at `/Users/skven/.local/share/mise/installs/node/20.20.0/bin/roo`)
- **Version**: 0.1.15
- **Source**: `/Users/skven/workplace/Roo-Code/apps/cli/`

## CLI Flags (from `roo --help`)

### Core Execution Flags
| Flag | Description |
|------|-------------|
| `prompt` (positional) | Your prompt |
| `-p, --print` | Print response and exit (non-interactive mode) |
| `--output-format <format>` | "text" (default), "json" (single result), or "stream-json" (realtime streaming) |
| `--ephemeral` | Run without persisting state (uses temporary storage) |
| `--oneshot` | Exit upon task completion |
| `-w, --workspace <path>` | Workspace directory path |
| `-a, --require-approval` | Require manual approval for actions (default: false) |
| `-e, --extension <path>` | Path to the extension bundle directory |
| `-d, --debug` | Enable debug output |

### Model/Provider Flags
| Flag | Description |
|------|-------------|
| `--provider <provider>` | API provider (roo, anthropic, openai, openrouter, etc.) |
| `-m, --model <model>` | Model to use (default: "anthropic/claude-opus-4.6") |
| `--max-tokens <n>` | Maximum output tokens |
| `--max-thinking-tokens <n>` | Maximum thinking/reasoning tokens |
| `-k, --api-key <key>` | API key for the LLM provider |

### AWS Bedrock Flags
| Flag | Description |
|------|-------------|
| `--aws-region <region>` | AWS region for Bedrock |
| `--aws-profile <profile>` | AWS profile name |
| `--aws-custom-arn <arn>` | Custom ARN for Bedrock inference |
| `--aws-inference-routing <mode>` | none, cross-region, or global |
| `--aws-bedrock-endpoint <url>` | Custom VPC endpoint URL |
| `--no-aws-prompt-cache` | Disable prompt caching for Bedrock |

### Mode & Behavior Flags
| Flag | Description |
|------|-------------|
| `--mode <mode>` | Mode to start in (code, architect, ask, debug, etc.) |
| `-r, --reasoning-effort <effort>` | Reasoning effort level |
| `--consecutive-mistake-limit <limit>` | Error/repetition limit |
| `--exit-on-error` | Exit on API request errors |

### Session Flags
| Flag | Description |
|------|-------------|
| `--session-id <task-id>` | Resume a specific task by task ID |
| `-c, --continue` | Resume the most recent task |
| `--prompt-file <path>` | Read prompt from a file |

### Stdin Streaming
| Flag | Description |
|------|-------------|
| `--stdin-prompt-stream` | Read NDJSON commands from stdin (requires --print and --output-format stream-json) |
| `--signal-only-exit` | Only terminate on SIGINT/SIGTERM (for stdin stream harnesses) |

## Stream-JSON Format

### Protocol
- Protocol: `roo-cli-stream`
- Schema version: 1
- NDJSON (newline-delimited JSON)

### Event Types (from source: `types/json-events.ts`)
| Type | Description |
|------|-------------|
| `system` | Init event, subtypes: `init` |
| `user` | User prompt echo |
| `assistant` | Assistant text deltas and complete messages |
| `thinking` | Reasoning/thinking deltas |
| `tool_use` | Tool invocation, subtypes: `tool`, `command`, `mcp` |
| `tool_result` | Tool result, subtypes: `command`, `mcp` |
| `error` | Error messages |
| `result` | Task completion (has `success` boolean and `cost` object) |
| `control` | Control flow events (ack, done, error) |
| `queue` | Queue management events |

### Example Stream Output (observed)
```json
{"type":"system","subtype":"init","content":"Task started","schemaVersion":1,"protocol":"roo-cli-stream","capabilities":["stdin:start","stdin:message","stdin:cancel","stdin:ping","stdin:shutdown"]}
{"type":"user","id":1772593478580,"content":"Say hello in one sentence","done":true}
{"type":"assistant","id":1772593481508,"content":"Hello"}
{"type":"assistant","id":1772593481508,"content":"! Welcome"}
{"type":"assistant","id":1772593481508,"content":" to the"}
{"type":"assistant","id":1772593481508,"content":" ralph"}
{"type":"assistant","id":1772593481508,"content":"-orchest"}
{"type":"assistant","id":1772593481508,"content":"rator project"}
{"type":"assistant","id":1772593481508,"content":"."}
{"type":"assistant","id":1772593481508,"content":"Hello! Welcome to the ralph-orchestrator project.","done":true}
{"type":"result","id":1772593482878,"content":"Hello! I'm Roo, ready to help you with the ralph-orchestrator project.","done":true,"success":true}
```

### Key Schema Fields (from `JsonEvent` type)
```typescript
type JsonEvent = {
  type: JsonEventType          // Event discriminator
  id?: number                  // Message ID for correlation
  content?: string             // Text content
  done?: boolean               // Final message flag
  subtype?: string             // Categorization (e.g., "init", "tool", "command")
  schemaVersion?: number       // Protocol version (on system.init)
  protocol?: string            // Transport protocol (on system.init)
  capabilities?: string[]      // Supported capabilities (on system.init)
  taskId?: string              // Active task ID
  requestId?: string           // Correlation ID
  command?: string             // Command name for control events
  code?: string                // Machine-readable error code
  tool_use?: JsonEventToolUse  // Tool call info
  tool_result?: JsonEventToolResult  // Tool result info
  success?: boolean            // Task success (result events)
  cost?: JsonEventCost         // Token/cost usage (result events)
  queueDepth?: number          // Queue depth
  queue?: JsonEventQueueItem[] // Queue snapshots
}
```

### Cost Structure
```typescript
type JsonEventCost = {
  totalCost: number
  inputTokens: number
  outputTokens: number
  cacheWrites?: number
  cacheReads?: number
}
```

## Comparison with Claude Stream Format

| Feature | Claude (`--output-format stream-json`) | Roo (`--output-format stream-json`) |
|---------|--------------------------------------|--------------------------------------|
| Protocol | N/A (flat events) | `roo-cli-stream` v1 |
| Init event | `system` with session_id, model | `system.init` with capabilities |
| Text streaming | `assistant` with message.content[] | `assistant` with delta `content` |
| Tool use | Inside `assistant.message.content` | Separate `tool_use` event |
| Tool result | `user.message.content` | Separate `tool_result` event |
| Thinking | Inside `assistant.message.content` | Separate `thinking` event |
| Completion | `result` with cost, turns | `result` with success, cost |
| Delta vs Full | Full snapshots per turn | Streaming deltas with `done` flag |

### Key Differences
1. **Claude** emits full turns (`assistant`, `user`) with all content blocks
2. **Roo** emits streaming deltas (many small `assistant` events building up content, then a `done:true` final)
3. **Roo** has separate event types for tool_use and tool_result (Claude embeds in message content blocks)
4. **Roo** has `thinking` as a separate event type (Claude has it as a content block type)
5. **Roo** supports bidirectional stdin streaming (`--stdin-prompt-stream`)

## Headless Mode Command (confirmed working)
```bash
roo --provider bedrock --aws-profile roo-bedrock --aws-region us-east-1 \
    --model anthropic.claude-sonnet-4-6 --max-tokens 64000 \
    --print "Say hello in one sentence"
```

## Mapping to Ralph Adapter Pattern

For a **text-mode** (simplest) integration:
```
roo --print <prompt>
```

For a **stream-json** integration:
```
roo --print --output-format stream-json <prompt>
```

For **interactive** mode:
```
roo <prompt>  (no --print flag)
```
