# Requirements Clarification

This document captures the Q&A process to refine the ACP support feature requirements.

---

## Q1: What is the intended role for Ralph Orchestrator with ACP?

**Options:**
- **A) ACP Client**: Ralph connects to external ACP-compatible agents (Gemini CLI, Claude Code, etc.) via subprocess, acting as the "editor" side
- **B) ACP Server**: Ralph exposes itself as an ACP-compatible agent that editors (Zed, Emacs, Neovim) can connect to
- **C) Both**: Ralph can both consume and expose ACP

**Answer:** A) ACP Client

Ralph Orchestrator will act as an ACP client, connecting to external ACP-compatible agents (Gemini CLI, Claude Code, Goose, etc.) via subprocess stdin/stdout - similar to how Zed Editor or Emacs agent-shell connects to agents.

---

## Q2: How should ACP integrate with Ralph's existing adapter architecture?

**Options:**
- **A) New Adapter**: Create `ACPAdapter` as a new adapter type alongside Claude, QChat, Gemini - where you configure which ACP agent binary to use
- **B) Replace Existing**: Replace current CLI-based adapters with ACP protocol where supported (e.g., Gemini CLI already supports ACP)
- **C) Hybrid**: Add ACP as a transport option for existing adapters (e.g., `claude` adapter could use SDK or ACP)

**Answer:** A) New Adapter

Create a new `ACPAdapter` class that sits alongside the existing adapters. Users configure which ACP-compatible agent binary to spawn (e.g., `gemini`, `claude`, `goose`). This keeps existing adapters unchanged while adding ACP as a new option.

---

## Q3: Which ACP client-side capabilities should Ralph implement?

The ACP protocol defines baseline and optional client capabilities. Which should Ralph support?

**Baseline (required):**
- `session/request_permission` - Respond to agent permission requests

**Optional capabilities:**
- **A) Minimal**: Baseline only - auto-approve or deny all permissions
- **B) File Operations**: Add `fs/read_text_file`, `fs/write_text_file` - let agents read/write files
- **C) Terminal**: Add `terminal/*` operations - let agents execute commands
- **D) Full**: All of the above (files + terminal)

**Answer:** D) Full

Implement all ACP client capabilities:
- `session/request_permission` - Handle permission requests
- `fs/read_text_file`, `fs/write_text_file` - File operations
- `terminal/create`, `terminal/output`, `terminal/release`, `terminal/wait_for_exit`, `terminal/kill` - Terminal operations

---

## Q4: How should Ralph handle `session/request_permission` requests from agents?

When an agent wants to perform a sensitive operation, it sends a permission request. How should Ralph respond?

**Options:**
- **A) Auto-approve all**: Trust the agent completely (fastest, least secure)
- **B) Auto-approve with allowlist**: Auto-approve operations matching a configured allowlist, deny others
- **C) Interactive prompt**: Pause and ask the user for each permission (like agent-shell does)
- **D) Configurable**: Let users choose behavior via config (default: auto-approve)

**Answer:** D) Configurable

Permission handling will be configurable via ralph.yml:
- `auto_approve` (default): Trust agent completely
- `allowlist`: Auto-approve matching patterns, deny others
- `interactive`: Prompt user for each permission
- `deny_all`: Reject all permission requests

---

## Q5: How should Ralph handle the different `session/update` notification types?

ACP agents stream various update types during execution. How should Ralph process these?

**Update kinds:**
- `agent_message_chunk` - Main response text (streaming)
- `agent_thought_chunk` - AI reasoning/planning
- `tool_call` - Function execution request
- `tool_call_update` - Tool execution result
- `plan` - Multi-step execution strategy

**Options:**
- **A) Text only**: Only capture `agent_message_chunk`, ignore others
- **B) Text + Tools**: Capture message chunks and tool call/results (needed for full operation)
- **C) Full verbose**: Capture all types including thoughts and plans for debugging/logging
- **D) Configurable**: Let users choose verbosity level

**Answer:** C/D) Full verbose with configurability

Default to capturing all update types (full verbose) for comprehensive logging:
- `agent_message_chunk` → Main output
- `agent_thought_chunk` → Logged/displayed based on verbosity
- `tool_call` / `tool_call_update` → Tracked for tool execution flow
- `plan` → Logged for debugging

Verbosity configurable via ralph.yml or `--verbose` flag.

---

## Q6: Which ACP-compatible agents should be tested/supported initially?

Several agents already support ACP. Which should we prioritize for initial testing?

**Known ACP-compatible agents:**
- Gemini CLI (Google's flagship reference implementation)
- Claude Code (via wrapper)
- Goose
- Cursor
- Qwen Code

**Options:**
- **A) Gemini only**: Start with Gemini CLI as the reference implementation
- **B) Gemini + Claude**: Test with both major providers
- **C) All known**: Test with all ACP-compatible agents we can access
- **D) Generic**: Build generic ACP client, let users configure any agent binary

**Answer:** B) Gemini + Claude

Primary testing targets:
1. **Gemini CLI** - Google's reference ACP implementation
2. **Claude Code** - Via ACP wrapper/support

The adapter should be generic enough to work with any ACP-compatible agent, but these two will be the primary test targets.

---

## Q7: Any additional requirements or constraints?

Before we proceed to design, are there any other requirements to consider?

**Examples:**
- Specific configuration options needed?
- Integration with existing Ralph features (checkpointing, metrics, cost tracking)?
- Error handling preferences?
- Logging/debugging needs?
- Performance considerations?

**Answer:** No additional requirements. Proceed to design.

---

## Requirements Summary

1. **Role**: ACP Client - Ralph connects to ACP-compatible agents via subprocess
2. **Architecture**: New `ACPAdapter` alongside existing adapters
3. **Capabilities**: Full (permissions, file ops, terminal ops)
4. **Permissions**: Configurable (auto-approve, allowlist, interactive, deny-all)
5. **Updates**: Full verbose capture, configurable verbosity
6. **Test Targets**: Gemini CLI + Claude Code
