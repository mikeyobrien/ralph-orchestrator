# xenodium/agent-shell Analysis

## Overview
Native Emacs shell enabling direct interaction with LLM agents via Agent Client Protocol (ACP). Bridges Emacs and multiple AI coding assistants.

## Architecture

```
agent-shell.el (Main Mode)
    ├── acp.el (Protocol Library - JSON-RPC 2.0)
    ├── shell-maker (Comint-based shell)
    ├── agent-shell-ui.el (Display/rendering)
    └── Provider modules (anthropic, google, openai, goose, cursor, qwen)
```

## Message Flow

```
User Input → agent-shell--handle → ACP Client → Agent Subprocess
                                       ↓
                            JSON-RPC Message Exchange
                                       ↓
Notification Handler ← agent-shell--on-notification ← Agent
Request Handler ← agent-shell--on-request ← Agent
Error Handler ← agent-shell--on-error ← Agent
```

## Key Code Patterns

### Notification Routing
```elisp
(pcase update-kind
  ("agent_message_chunk" (agent-shell--handle-message-chunk ...))
  ("tool_call" (agent-shell--handle-tool-call ...))
  ("tool_call_update" (agent-shell--handle-tool-result ...))
  ("plan" (agent-shell--handle-plan ...)))
```

### Request/Response Pattern (acp.el)
```
acp-send-request → JSON-RPC format → Subprocess stdout
                   (with callback handlers)
                                    ↓
Subprocess receives via stdin
                                    ↓
Response routes to callback in buffer context
```

### Session Lifecycle
1. ACP client creation with authentication
2. Event subscription setup
3. Protocol handshake (initialize)
4. Session establishment (session/new)
5. Model/mode configuration
6. Command transmission (session/prompt)

## Protocol Details (JSON-RPC 2.0)

### Agent → Client Notifications (no response)
- `session/update` containing:
  - `agent_message_chunk` - Streaming text
  - `agent_thought_chunk` - Reasoning
  - `tool_call` - Function call with ID/args
  - `tool_call_update` - Result/failure
  - `plan` - Execution strategy

### Agent → Client Requests (require response)
- `session/request_permission` - Approval dialogs
- `fs/read_text_file` - File reading
- `fs/write_text_file` - File writing
- `terminal/*` - Terminal management

### Client → Agent Requests
- `initialize` - Capability negotiation
- `session/new` - Create session
- `session/prompt` - Send input
- `session/set_mode` - Change modes

## Key Implementation Details

### State Management
Buffer-local `agent-shell--state` maintains:
- Client connection status
- Active session metadata (ID, mode, models)
- Tool call tracking with permission states
- Request/response metadata

### Provider Pattern
```elisp
agent-shell-{provider}-make-authentication  ; API key/login
agent-shell-{provider}-make-client          ; Spawn ACP client
```

### Transcript System
- Logs all interactions
- Timestamp and metadata tracking
- Configurable via `agent-shell--transcript-file-path-function`

## Sources
- https://github.com/xenodium/agent-shell
- https://github.com/xenodium/acp.el
- https://agentclientprotocol.com/protocol/overview
