# Ralph Orchestrator Source Code Architecture Scout Report

**Date**: 2026-01-04  
**Directory Scanned**: `/Users/nick/Desktop/ralph-orchestrator/src/ralph_orchestrator`  
**Total Python Files**: 54 files across 10 subdirectories

---

## Executive Summary

Ralph Orchestrator is a sophisticated AI agent orchestration framework implementing the "Ralph Wiggum technique" for multi-agent task orchestration. The architecture comprises:

1. **Core Orchestration Engine** - Main loop with metrics, safety, and recovery
2. **Multi-Adapter Support** - Claude SDK, Gemini CLI, Q Chat, and ACP protocol
3. **Web Dashboard** - FastAPI-based real-time monitoring
4. **Terminal UI** - Textual-based TUI for interactive control
5. **Onboarding System** - Intelligent project analysis and configuration
6. **Safety & Metrics** - Cost tracking, loop detection, and guardrails
7. **Output Formatting** - Rich terminal, JSON, and plain text formatters

---

## Core Architecture Files

### Main Entry Points

| File | Purpose | Key Classes/Functions |
|------|---------|----------------------|
| `__init__.py` | Package initialization and exports | `RalphOrchestrator`, `Metrics`, `CostTracker`, `ErrorMessage`, `VerboseLogger`, `DiffFormatter`, `RalphConsole` |
| `main.py` (executable) | CLI main loop and configuration validation | `ConfigValidator`, `AgentType` enum, argument parsing, checkpoint handling |
| `__main__.py` | Alternative entry point (46KB large file) | Complete implementation with CLI setup |

### Core Orchestration

| File | Purpose | Key Classes/Functions |
|------|---------|----------------------|
| `orchestrator.py` (48KB) | **CRITICAL** - Core orchestration loop implementing Ralph Wiggum technique | `RalphOrchestrator` - manages agent execution, iteration control, safety checks, metrics tracking, validation gates, checkpoint/rollback functionality |
| `context.py` | Prompt caching and context window management | `ContextManager` - handles prompt optimization, caching, summarization, dynamic context assembly |
| `metrics.py` | Performance and cost tracking | `Metrics`, `CostTracker`, `IterationStats`, `TriggerReason` enum - tracks iterations, costs, success rates, telemetry |
| `safety.py` | Safety guardrails and circuit breakers | `SafetyGuard`, `SafetyCheckResult` - enforces iteration/runtime/cost limits, consecutive failure tracking, loop detection using rapidfuzz |

---

## Adapter/Agent Implementations

The adapters module provides pluggable integration with different AI tools.

### Base Adapter

| File | Purpose | Key Classes/Functions |
|------|---------|----------------------|
| `adapters/base.py` | Abstract base class for all adapters | `ToolAdapter` ABC, `ToolResponse` dataclass - defines interface for `check_availability()`, `execute()`, `aexecute()` |

### Tool-Specific Adapters

| File | Purpose | Key Classes/Functions |
|------|---------|----------------------|
| `adapters/claude.py` | **PRIMARY** - Claude SDK integration | `ClaudeAdapter` - uses `claude_agent_sdk`, supports Claude Opus 4.5/Sonnet/Haiku, handles model pricing, inherits user's Claude Code settings (MCP servers, CLAUDE.md), max 10MB buffer |
| `adapters/gemini.py` | Fallback - Google Gemini CLI integration | `GeminiAdapter` - subprocess-based, supports model selection, enhanced prompt instructions, CLI wrapper |
| `adapters/qchat.py` | Alternative - Q Chat CLI integration | `QChatAdapter` - subprocess execution with signal handling, trust tools config, non-interactive mode, thread-safe locking |
| `adapters/acp.py` | Protocol - Agent Client Protocol implementation | `ACPAdapter` - ACP protocol v1, subprocess lifecycle management, initialization handshake, session routing, permission modes (auto_approve, ask, deny_all) |

### ACP (Agent Client Protocol) Support

| File | Purpose | Key Classes/Functions |
|------|---------|----------------------|
| `adapters/acp_client.py` | ACP client library | `ACPClient` - manages ACP protocol communication, initialization, message routing |
| `adapters/acp_models.py` | ACP data models | `ACPAdapterConfig`, `ACPSession`, `UpdatePayload` - type definitions for ACP protocol |
| `adapters/acp_handlers.py` | ACP message handlers | `ACPHandlers` - processes ACP-specific messages and callbacks |
| `adapters/acp_protocol.py` | ACP protocol implementation details | Protocol version handling, message formats |
| `adapters/__init__.py` | Adapter module exports | Exposes all adapter classes |

---

## Web Server & Dashboard

FastAPI-based web monitoring with WebSocket real-time updates.

| File | Purpose | Key Classes/Functions |
|------|---------|----------------------|
| `web/server.py` (primary) | **CRITICAL** - FastAPI REST API and WebSocket server | `OrchestratorMonitor` - manages active orchestrators, execution history, WebSocket clients, metrics caching, system monitoring; `PromptUpdateRequest` - request models |
| `web/__main__.py` | Web server entry point | Uvicorn configuration, server startup |
| `web/auth.py` | Authentication and authorization | `auth_manager`, `LoginRequest`, `TokenResponse`, `get_current_user()`, `require_admin()` decorator |
| `web/database.py` | Database persistence for runs and metrics | `DatabaseManager` - stores execution history, iteration data, system metrics |
| `web/rate_limit.py` | Rate limiting middleware | `rate_limit_middleware()`, `setup_rate_limit_cleanup()` - prevents API abuse |
| `web/static/` | HTML frontend files | `index.html` (main dashboard), `login.html` (auth page) |

---

## Terminal User Interface (TUI)

Interactive Textual-based UI for real-time monitoring and control.

### Main App

| File | Purpose | Key Classes/Functions |
|------|---------|----------------------|
| `tui/app.py` | **CRITICAL** - Textual application main class | `RalphTUI` - reactive state (pause, iteration count, task, connection status), key bindings (q=quit, p=pause, l=logs, t=tasks, m=metrics, h=history, etc.), event handlers |
| `tui/connection.py` | WebSocket connection to orchestrator | `OrchestratorConnection` - manages WebSocket lifecycle, event handling, TUIEvent dataclass |

### TUI Screens

| File | Purpose | Key Classes/Functions |
|------|---------|----------------------|
| `tui/screens/help.py` | Help/documentation screen | Help content display |
| `tui/screens/history.py` | Conversation history browser | Browse previous iterations and outputs |
| `tui/screens/__init__.py` | Screens module exports | |

### TUI Widgets

| File | Purpose | Key Classes/Functions |
|------|---------|----------------------|
| `tui/widgets/progress.py` | Progress bar and iteration progress display | `ProgressPanel` - visual progress indicator |
| `tui/widgets/output.py` | Output viewer widget | `OutputViewer` - displays agent output in real-time |
| `tui/widgets/tasks.py` | Task queue sidebar | `TaskSidebar` - shows pending/running tasks |
| `tui/widgets/metrics.py` | Metrics display widget | `MetricsPanel` - visualizes cost, iterations, success rate |
| `tui/widgets/validation.py` | Validation gate widget | `ValidationPrompt` - user confirmation for validation decisions |
| `tui/widgets/__init__.py` | Widgets module exports | Exports `ProgressPanel`, `OutputViewer`, `TaskSidebar`, `MetricsPanel`, `ValidationPrompt` |
| `tui/ralph.tcss` | Textual CSS styling | TUI theme and layout styling |

---

## Onboarding Module

Intelligent project analysis system for automatic configuration generation.

| File | Purpose | Key Classes/Functions |
|------|---------|----------------------|
| `onboarding/__init__.py` | Module exports | Exposes all onboarding classes |
| `onboarding/scanner.py` | Project discovery and scanning | `ProjectScanner` - scans project directory, finds Claude history, CLAUDE.md files, MCP configs; `ProjectType` enum (nodejs, python, rust, go, expo, react, flutter, unknown) |
| `onboarding/history_analyzer.py` | Claude Code history analysis | `HistoryAnalyzer` - analyzes conversation history, `Conversation`, `ToolUsageStats`, `MCPServerStats`, `ToolChain` - extracts patterns from past interactions |
| `onboarding/agent_analyzer.py` | Agent capability analysis | `AgentAnalyzer`, `AnalysisResult` - determines best agent choice based on history and capabilities |
| `onboarding/pattern_extractor.py` | Workflow pattern extraction | `PatternExtractor`, `ProjectPatterns`, `Workflow` - identifies recurring workflow patterns |
| `onboarding/config_generator.py` | RALPH configuration generation | `ConfigGenerator` - generates optimized RALPH config based on analysis |
| `onboarding/settings_loader.py` | Settings and environment loading | `SettingsLoader` - loads project settings, environment vars, cached configs |

---

## Output Formatting

Flexible output formatting system with multiple backends.

### Base Classes

| File | Purpose | Key Classes/Functions |
|------|---------|----------------------|
| `output/__init__.py` | Output module initialization and factory | `create_formatter()` factory function; exports `OutputFormatter`, `VerbosityLevel`, `ToolCallInfo`, `FormatContext`, `ContentDetector`, `ContentType`, legacy exports for backward compatibility |
| `output/base.py` | Abstract formatter base class | `OutputFormatter` ABC - defines `format_message()`, `format_tool_call()`, `format_error()` interface; `VerbosityLevel` enum, `TokenUsage`, `ToolCallInfo`, `MessageType`, `FormatContext` dataclasses |

### Formatter Implementations

| File | Purpose | Key Classes/Functions |
|------|---------|----------------------|
| `output/console.py` | Legacy Rich console utilities | `RalphConsole` - `print_status()`, `print_success()`, `print_error()`, `print_diff()` methods; `DiffFormatter`, `DiffStats` for git diff display |
| `output/plain.py` | Plain text formatter (no colors) | `PlainTextFormatter` - basic text output without styling |
| `output/rich_formatter.py` | Rich terminal formatter with colors/panels | `RichTerminalFormatter` - colored output, panels, syntax highlighting for code |
| `output/json_formatter.py` | JSON structured output | `JsonFormatter` - outputs valid JSON for programmatic consumption |
| `output/content_detector.py` | Content type detection | `ContentDetector`, `ContentType` enum (code, markdown, error, plain, diff) - auto-detects content type for smart formatting |

---

## Utility & Infrastructure Files

| File | Purpose | Key Classes/Functions |
|------|---------|----------------------|
| `error_formatter.py` | Error formatting and suggestions | `ClaudeErrorFormatter` - formats various error types with user-friendly suggestions; `ErrorMessage` dataclass with message + suggestion |
| `verbose_logger.py` (33KB) | Enhanced verbose logging | `VerboseLogger` - session metrics, emergency shutdown, re-entrancy protection, Rich output support, TextIOProxy for file capture |
| `async_logger.py` (16KB) | Async logging with rotation | Automatic 10MB log rotation with 3 backups, thread-safe rotation, unicode sanitization, security-aware logging, dual async/sync interface |
| `security.py` | Security validation utilities | `SecurityValidator` - input validation, command injection prevention, path traversal checks |
| `safety.py` | Safety mechanisms (duplicate entry in core section) | See Safety section above |
| `logging_config.py` | Logging configuration | Logger setup, RalphLogger factory with specialized loggers (orchestrator, adapter_qchat, etc.) |

---

## File Statistics & Directory Structure

```
src/ralph_orchestrator/
├── __init__.py                          [25 lines]  - Package exports
├── __main__.py                          [46KB]     - Alternative entry point
├── main.py                              [22KB]     - CLI main loop (executable)
├── orchestrator.py                      [48KB]     - Core orchestration engine
├── context.py                           [8.8KB]    - Context management
├── metrics.py                           [11KB]     - Metrics & cost tracking
├── safety.py                            [5.1KB]    - Safety guardrails
├── error_formatter.py                   [8.5KB]    - Error formatting
├── verbose_logger.py                    [33KB]     - Verbose logging
├── async_logger.py                      [16KB]     - Async logging with rotation
├── security.py                          [15KB]     - Security validation
├── logging_config.py                    [7.6KB]    - Logging setup
│
├── adapters/                            [13 files] - AI tool integrations
│   ├── __init__.py
│   ├── base.py                          [Abstract adapter interface]
│   ├── claude.py                        [Claude SDK integration - PRIMARY]
│   ├── gemini.py                        [Gemini CLI integration]
│   ├── qchat.py                         [Q Chat CLI integration]
│   ├── acp.py                           [Agent Client Protocol]
│   ├── acp_client.py                    [ACP client library]
│   ├── acp_models.py                    [ACP data models]
│   ├── acp_handlers.py                  [ACP message handlers]
│   └── acp_protocol.py                  [ACP protocol implementation]
│
├── onboarding/                          [6 files]  - Project analysis & config
│   ├── __init__.py
│   ├── scanner.py                       [Project discovery]
│   ├── history_analyzer.py              [Claude Code history analysis]
│   ├── agent_analyzer.py                [Agent capability analysis]
│   ├── pattern_extractor.py             [Workflow pattern extraction]
│   ├── config_generator.py              [Config generation]
│   └── settings_loader.py               [Settings loading]
│
├── output/                              [8 files]  - Output formatting
│   ├── __init__.py                      [Factory & exports]
│   ├── base.py                          [Abstract formatter base]
│   ├── console.py                       [Legacy Rich console utilities]
│   ├── plain.py                         [Plain text formatter]
│   ├── rich_formatter.py                [Rich terminal formatter]
│   ├── json_formatter.py                [JSON formatter]
│   └── content_detector.py              [Content type detection]
│
├── tui/                                 [9 files]  - Terminal User Interface
│   ├── __init__.py
│   ├── app.py                           [Textual main app]
│   ├── connection.py                    [WebSocket connection]
│   ├── ralph.tcss                       [TUI styling]
│   ├── screens/                         [3 files]
│   │   ├── __init__.py
│   │   ├── help.py                      [Help screen]
│   │   └── history.py                   [History browser]
│   └── widgets/                         [6 files]
│       ├── __init__.py
│       ├── progress.py                  [Progress display]
│       ├── output.py                    [Output viewer]
│       ├── tasks.py                     [Task sidebar]
│       ├── metrics.py                   [Metrics display]
│       └── validation.py                [Validation gate]
│
└── web/                                 [6 files]  - Web Dashboard
    ├── __init__.py
    ├── __main__.py                      [Web server entry point]
    ├── server.py                        [FastAPI main server]
    ├── auth.py                          [Authentication]
    ├── database.py                      [Database persistence]
    ├── rate_limit.py                    [Rate limiting]
    └── static/
        ├── index.html                   [Main dashboard]
        └── login.html                   [Auth page]
```

---

## Architecture Highlights

### Ralph Wiggum Technique
- Iterative agent execution with continuous feedback loops
- Multi-tool support for agent diversity (Claude, Gemini, Q Chat, ACP)
- Checkpointing and rollback on failures
- Cost tracking and safety guardrails

### Key Design Patterns
1. **Adapter Pattern** - Multiple AI backends via common `ToolAdapter` interface
2. **Factory Pattern** - Output formatter creation via `create_formatter()`
3. **Observer Pattern** - TUI WebSocket connection for real-time updates
4. **Strategy Pattern** - Multiple formatting strategies (plain, rich, JSON)
5. **Guardian Pattern** - `SafetyGuard` enforces limits and detects loops

### Critical Dependencies
- **claude_agent_sdk** / **claude_code_sdk** - Claude integration
- **fastapi** + **uvicorn** - Web server
- **textual** - Terminal UI framework
- **rich** - Rich terminal output
- **pydantic** - Data validation
- **rapidfuzz** - Loop detection (optional)
- **psutil** - System metrics

---

## Configuration Entry Points

1. **CLI Args** (`main.py`) - Directly configurable parameters
2. **PROMPT.md** - Task definition file
3. **RalphConfig** object - Programmatic configuration
4. **Environment variables** - Adapter-specific settings (RALPH_QCHAT_*, etc.)
5. **Onboarding system** - Auto-generated configuration from project analysis

---

## Unresolved Questions

1. How is the `__main__.py` (46KB) different from `main.py` (22KB)? Possible legacy duplication?
2. Is loop detection via `rapidfuzz` actually used in production, given it's optional?
3. What is the relationship between `RalphConsole.print_diff()` and `DiffFormatter`? 
4. How does prompt caching in `ContextManager` interact with Claude SDK's built-in prompt caching?
5. Are all ACP protocol features (acp_protocol.py) fully utilized by the main adapter?

