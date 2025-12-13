# ACP Protocol Research (Updated)

## Critical Discovery: Two Different "ACP" Protocols

### 1. Agent Client Protocol (Zed/xenodium) - ACTIVE
- **Creator**: Zed Editor (launched August 2025)
- **Transport**: JSON-RPC 2.0 over stdin/stdout
- **Purpose**: Editor-to-agent communication (like LSP for AI)
- **Status**: Active, growing ecosystem
- **Docs**: https://agentclientprotocol.com

### 2. Agent Communication Protocol (IBM) - DEPRECATED
- **Creator**: IBM Research (BeeAI project)
- **Transport**: REST/HTTP
- **Purpose**: Agent-to-agent communication
- **Status**: **DEPRECATED September 2025** - merged into A2A protocol
- **Migration**: Users should migrate to A2A (Agent2Agent)

---

## Agent Client Protocol (Detailed Specification)

### Overview
Standardizes communication between code editors/IDEs and AI coding agents. Analogous to LSP for language servers.

### Transport
- JSON-RPC 2.0 over stdin/stdout
- Agents run as subprocesses of the editor
- Bidirectional, stateful, streaming communication

### Message Types
- **Methods**: Request-response pairs with `id` field
- **Notifications**: One-way messages without `id` field

### Lifecycle Phases

**1. Initialization**
- Client sends `initialize` request with protocol version
- Capability negotiation between editor and agent
- Optional `authenticate` method

**2. Session Management**
- `session/new` - Create new conversation session
- `session/load` - Resume existing session (optional)
- `session/set_mode` - Switch agent operating modes (optional)

**3. Prompt Turn**
- Client: `session/prompt` - Send user messages
- Agent: `session/update` notifications for progress
- Client: `session/cancel` - Interrupt operations
- Agent: Response with stop reason

### Core Methods

**Agent-side (baseline):**
- `initialize` - Version and capability negotiation
- `authenticate` - Optional authentication
- `session/new` - Create session
- `session/prompt` - Accept prompts

**Client-side (baseline):**
- `session/request_permission` - Authorize tool execution

**Client-side (optional):**
- File ops: `fs/read_text_file`, `fs/write_text_file`
- Terminal: `terminal/create`, `terminal/output`, `terminal/release`, `terminal/wait_for_exit`, `terminal/kill`

### Notification: session/update

The primary way agents stream responses. Contains `update` object with `kind`:

```json
{
  "jsonrpc": "2.0",
  "method": "session/update",
  "params": {
    "sessionId": "abc123",
    "update": {
      "kind": "agent_message_chunk",
      "content": "Here's my response..."
    }
  }
}
```

**Update kinds:**
- `agent_message_chunk` - Streaming response text
- `agent_thought_chunk` - AI reasoning/planning
- `tool_call` - Function call request with ID and arguments
- `tool_call_update` - Execution result or failure
- `plan` - Multi-step execution strategy
- `available_commands_update` - Executable operations

### Content Format
- Markdown default for human-readable text
- Reuses MCP JSON representations where possible
- Custom types for diff visualization

### Extensibility
- `_meta` field on all types for custom data
- Methods starting with `_` reserved for extensions
- Root-level fields reserved for future protocol versions

### Implementation Requirements
- All file paths must be absolute
- Line numbers use 1-based indexing
- Standard JSON-RPC error handling
- W3C trace context support via `_meta` field

---

## Current Implementations

### Editors
- **Zed Editor** - Native support (August 2025)
- **Emacs** - via agent-shell and acp.el (xenodium)
- **Neovim** - via Code Companion plugin

### Agents with ACP Support
- Google Gemini CLI (flagship reference)
- Claude Code (via wrapper packages)
- Goose
- Cursor
- OpenAI Codex
- Qwen Code

### SDKs
- **Rust**: docs.rs/agent-client-protocol
- **Python**: agent-client-protocol (PyPI)
- **TypeScript**: Available in official repos
- **Kotlin**: Available in official repos

---

## Protocol Relationships

Think of it as a protocol stack:
- **MCP**: Agent ↔ Tools (the "USB port")
- **ACP**: Editor ↔ Agent (the "LSP for AI")
- **A2A**: Agent ↔ Agent (the "HTTP of agents")

---

## Sources
- https://agentclientprotocol.com/overview/introduction
- https://agentclientprotocol.com/protocol/overview
- https://agentclientprotocol.com/protocol/extensibility
- https://github.com/xenodium/agent-shell
- https://github.com/xenodium/acp.el
- https://blog.promptlayer.com/agent-client-protocol-the-lsp-for-ai-coding-agents/
