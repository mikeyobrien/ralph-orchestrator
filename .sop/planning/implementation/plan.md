# ACP Adapter Implementation Plan

## Implementation Checklist

- [x] Step 1: Create ACPProtocol class for JSON-RPC 2.0 handling
- [x] Step 2: Create ACPMessage data models
- [x] Step 3: Create ACPClient subprocess manager
- [x] Step 4: Implement basic ACPAdapter with initialize/session flow
- [x] Step 5: Implement session/prompt and update handling
- [x] Step 6: Implement permission handler
- [x] Step 7: Implement file operation handlers
- [x] Step 8: Implement terminal handlers
- [x] Step 9: Add configuration support (ralph.yml)
- [x] Step 10: Add CLI integration
- [x] Step 11: Integration testing with Gemini CLI
- [ ] Step 12: Final integration with orchestrator loop

---

## Step 1: Create ACPProtocol class for JSON-RPC 2.0 handling

**Objective**: Implement the core JSON-RPC 2.0 protocol layer that serializes/deserializes messages.

**Implementation guidance**:
- Create `src/ralph_orchestrator/adapters/acp_protocol.py`
- Implement request ID generation (auto-incrementing integer)
- Implement `create_request(method, params)` → returns (id, json_string)
- Implement `create_notification(method, params)` → returns json_string
- Implement `parse_message(json_string)` → returns parsed dict with message type detection
- Implement `create_response(id, result)` and `create_error_response(id, code, message)`
- All messages must include `"jsonrpc": "2.0"`

**Test requirements**:
- Test request creation with various methods/params
- Test notification creation (no id field)
- Test parsing requests, responses, notifications, and errors
- Test error response creation with standard JSON-RPC error codes

**Integration**: Standalone module, no dependencies on other ACP components yet.

**Demo**: Run unit tests showing JSON-RPC message round-trip serialization.

---

## Step 2: Create ACPMessage data models

**Objective**: Define typed data classes for all ACP message types and session state.

**Implementation guidance**:
- Create `src/ralph_orchestrator/adapters/acp_models.py`
- Define `ACPRequest`, `ACPNotification`, `ACPResponse`, `ACPError` dataclasses
- Define `SessionUpdate`, `UpdatePayload` for session/update notifications
- Define `ToolCall` for tracking tool executions
- Define `ACPSession` for accumulating session state
- Define `ACPAdapterConfig` for adapter configuration
- Use `@dataclass` with type hints
- Include `from_dict()` class methods for parsing

**Test requirements**:
- Test dataclass creation and field access
- Test `from_dict()` parsing with valid and invalid data
- Test session update accumulation

**Integration**: Import into ACPProtocol for typed message handling.

**Demo**: Create and manipulate ACP message objects, show session state accumulation.

---

## Step 3: Create ACPClient subprocess manager

**Objective**: Implement subprocess lifecycle management and async message routing.

**Implementation guidance**:
- Create `src/ralph_orchestrator/adapters/acp_client.py`
- Use `asyncio.create_subprocess_exec` with stdin=PIPE, stdout=PIPE, stderr=PIPE
- Implement `start()` to spawn agent process
- Implement `stop()` with graceful shutdown (terminate → wait → kill)
- Implement `_read_loop()` to continuously read stdout and parse JSON-RPC messages
- Implement `_write_message(json_str)` to write to stdin with newline delimiter
- Implement `send_request(method, params)` returning Future for response
- Implement `send_notification(method, params)` for fire-and-forget
- Use `asyncio.Lock` for thread-safe writes
- Track pending requests by ID for response routing

**Test requirements**:
- Test subprocess spawn with mock command (e.g., `cat` or Python script)
- Test message write/read cycle
- Test graceful shutdown
- Test timeout handling

**Integration**: Uses ACPProtocol for message serialization.

**Demo**: Spawn a simple echo script, send request, receive response.

---

## Step 4: Implement basic ACPAdapter with initialize/session flow

**Objective**: Create the main adapter class with ACP initialization handshake.

**Implementation guidance**:
- Create `src/ralph_orchestrator/adapters/acp.py`
- Extend `ToolAdapter` base class
- Implement `__init__` accepting agent_command, agent_args, permission_mode, etc.
- Implement `check_availability()` using `shutil.which(agent_command)`
- Implement `_initialize()` async method:
  1. Start ACPClient
  2. Send `initialize` request with protocol version and capabilities
  3. Receive and validate initialize response
  4. Send `session/new` request
  5. Store session_id
- Implement basic `aexecute()` that initializes and returns placeholder response
- Add signal handlers for graceful shutdown (like QChatAdapter)

**Test requirements**:
- Test availability check with existing/missing binary
- Test initialization sequence with mock agent
- Test session creation

**Integration**: Add to `adapters/__init__.py` exports.

**Demo**: Run `ralph run -a acp --dry-run` showing adapter initialization.

---

## Step 5: Implement session/prompt and update handling

**Objective**: Complete the prompt execution flow with streaming update processing.

**Implementation guidance**:
- In ACPAdapter, implement full `aexecute()`:
  1. Ensure initialized (call `_initialize()` if needed)
  2. Enhance prompt with `_enhance_prompt_with_instructions()`
  3. Send `session/prompt` request with messages array
  4. Set up notification handler for `session/update`
  5. Accumulate updates in ACPSession
  6. Wait for prompt response (stop_reason)
  7. Build and return ToolResponse
- Handle different update kinds:
  - `agent_message_chunk` → append to output
  - `agent_thought_chunk` → append to thoughts (verbose log)
  - `tool_call` → track in session
  - `tool_call_update` → update tool status
  - `plan` → log for debugging
- Implement sync `execute()` wrapper using `asyncio.run()`

**Test requirements**:
- Test prompt execution with streaming updates
- Test output accumulation from chunks
- Test tool call tracking
- Test response building

**Integration**: Now usable in orchestrator loop (without tool execution).

**Demo**: Execute a simple prompt with mock agent, show accumulated output.

---

## Step 6: Implement permission handler

**Objective**: Handle `session/request_permission` requests from agents.

**Implementation guidance**:
- Create `src/ralph_orchestrator/adapters/acp_handlers.py`
- Implement `ACPHandlers` class with permission_mode and allowlist
- Implement `handle_request_permission(params)`:
  - `auto_approve` mode → always return `{approved: true}`
  - `deny_all` mode → always return `{approved: false}`
  - `allowlist` mode → check operation against patterns
  - `interactive` mode → prompt user via stdin (if terminal available)
- Wire handler into ACPClient message routing
- Log all permission decisions

**Test requirements**:
- Test each permission mode
- Test allowlist pattern matching
- Test permission denial response format

**Integration**: Agents can now request and receive permission decisions.

**Demo**: Show agent requesting permission, Ralph approving/denying based on mode.

---

## Step 7: Implement file operation handlers

**Objective**: Implement `fs/read_text_file` and `fs/write_text_file` handlers.

**Implementation guidance**:
- In ACPHandlers, implement `handle_read_file(params)`:
  - Extract `path` from params (must be absolute)
  - Validate path exists and is readable
  - Read file content
  - Return `{content: "..."}`
  - Handle errors (not found, permission denied)
- Implement `handle_write_file(params)`:
  - Extract `path` and `content` from params
  - Validate path is writable
  - Write content to file
  - Return `{success: true}`
  - Handle errors
- Add security: optionally restrict to working directory

**Test requirements**:
- Test reading existing file
- Test reading non-existent file (error)
- Test writing new file
- Test writing to read-only location (error)
- Test path validation

**Integration**: Agents can now read/write files through Ralph.

**Demo**: Agent requests file read, Ralph serves content; agent writes file.

---

## Step 8: Implement terminal handlers

**Objective**: Implement terminal/* operations for command execution.

**Implementation guidance**:
- In ACPHandlers, add `_terminals: dict[str, Terminal]` to track terminals
- Implement `handle_terminal_create(params)`:
  - Create subprocess for command
  - Generate terminal ID
  - Store in `_terminals`
  - Return `{terminalId: "..."}`
- Implement `handle_terminal_output(params)`:
  - Get terminal by ID
  - Read available stdout/stderr
  - Return `{output: "...", done: bool}`
- Implement `handle_terminal_wait_for_exit(params)`:
  - Wait for process completion
  - Return `{exitCode: int}`
- Implement `handle_terminal_kill(params)`:
  - Terminate process
  - Clean up terminal
- Implement `handle_terminal_release(params)`:
  - Clean up terminal without killing

**Test requirements**:
- Test terminal creation with simple command
- Test output reading (streaming)
- Test wait for exit
- Test kill and cleanup

**Integration**: Agents can now execute arbitrary commands.

**Demo**: Agent creates terminal, runs `ls`, receives output.

---

## Step 9: Add configuration support (ralph.yml)

**Objective**: Enable ACP adapter configuration via ralph.yml.

**Implementation guidance**:
- Update `src/ralph_orchestrator/main.py` to parse ACP config:
  ```yaml
  adapters:
    acp:
      enabled: true
      agent_command: gemini
      agent_args: []
      timeout: 300
      permission_mode: auto_approve
      permission_allowlist: []
  ```
- Create `ACPAdapterConfig` dataclass if not exists
- Pass config to ACPAdapter on initialization
- Support environment variable overrides:
  - `RALPH_ACP_AGENT` → agent_command
  - `RALPH_ACP_PERMISSION_MODE` → permission_mode
- Update `ralph init` to include ACP config template

**Test requirements**:
- Test config parsing from YAML
- Test environment variable overrides
- Test default values

**Integration**: Users can configure ACP via ralph.yml.

**Demo**: Show ralph.yml with ACP config, adapter using those values.

---

## Step 10: Add CLI integration

**Objective**: Expose ACP adapter through Ralph CLI.

**Implementation guidance**:
- Update `src/ralph_orchestrator/__main__.py`:
  - Add `'acp'` to agent choices in argparse
  - Add `--acp-agent` argument for specifying agent binary
  - Add `--acp-permission-mode` argument
- Update `_initialize_adapters()` in orchestrator.py:
  - Initialize ACPAdapter with config
  - Add to adapters dict with key `'acp'`
- Handle 'auto' agent selection to include ACP check

**Test requirements**:
- Test CLI argument parsing
- Test adapter selection via `-a acp`
- Test ACP-specific arguments

**Integration**: Full CLI access to ACP adapter.

**Demo**: `ralph run -a acp --acp-agent gemini -p "Hello"` executes successfully.

---

## Step 11: Integration testing with Gemini CLI

**Objective**: Validate full integration with real Gemini CLI.

**Implementation guidance**:
- Create `tests/test_acp_integration.py`
- Test requires `GOOGLE_API_KEY` environment variable
- Mark tests with `@pytest.mark.integration`
- Test cases:
  1. Basic prompt → response
  2. Multi-turn conversation
  3. File read request handling
  4. Tool execution flow
- Add CI skip for missing API key
- Document manual testing steps

**Test requirements**:
- Integration test with real Gemini CLI
- Verify streaming updates work
- Verify permission flow works
- Verify error handling

**Integration**: Validates real-world ACP compatibility.

**Demo**: Run integration test suite showing green results with Gemini CLI.

---

## Step 12: Final integration with orchestrator loop

**Objective**: Ensure ACP adapter works correctly in Ralph's iteration loop.

**Implementation guidance**:
- Verify cost tracking works (extract tokens if available from agent)
- Verify metrics recording captures ACP executions
- Verify checkpointing works with ACP responses
- Test multi-iteration scenarios
- Verify graceful shutdown during iteration
- Update documentation/README with ACP usage

**Test requirements**:
- Test full orchestrator loop with ACP adapter
- Test interrupt handling (Ctrl+C)
- Test max_iterations limit
- Test error recovery

**Integration**: ACP fully integrated into Ralph orchestration.

**Demo**: `ralph run -a acp -p "Build a simple Python calculator" --max-iterations 5` runs full loop with checkpoints.

---

## Summary

This implementation plan builds ACP support incrementally:

1. **Steps 1-3**: Core protocol infrastructure (JSON-RPC, subprocess)
2. **Steps 4-5**: Basic adapter with prompt execution
3. **Steps 6-8**: Client-side capability handlers
4. **Steps 9-10**: Configuration and CLI
5. **Steps 11-12**: Testing and integration

Each step produces working, testable code that builds on previous steps.
