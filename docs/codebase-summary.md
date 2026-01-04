# Ralph Orchestrator - Codebase Summary

## Quick Overview

Ralph Orchestrator is a 57-file Python application (149,954 tokens) organized into 6 major functional domains:

1. **Core Orchestration** - Main loop implementation
2. **Agent Adapters** - Multi-provider AI integration
3. **Output Formatters** - Rich console, JSON, and plain text output
4. **Terminal UI** - Interactive dashboard with Textual
5. **Web Server** - HTTP API and web interface
6. **Onboarding System** - Auto-detection and configuration

## Directory Structure

```
src/ralph_orchestrator/
├── __init__.py                 # Package initialization
├── __main__.py                 # CLI entry point (9,924 tokens)
├── main.py                     # Command handler and setup
├── orchestrator.py             # Core loop engine (9,135 tokens)
├── context.py                  # Context management and summarization
├── metrics.py                  # Performance metrics and cost tracking
├── safety.py                   # Limits and safety guards
├── logging_config.py           # Structured logging configuration
├── verbose_logger.py           # Enhanced logger (6,518 tokens)
├── error_formatter.py          # Error message formatting
├── async_logger.py             # Async file logging
├── security.py                 # API key masking and sanitization
│
├── adapters/                   # AI Provider Abstraction (18 files)
│   ├── __init__.py
│   ├── base.py                 # Base adapter interface
│   ├── claude.py               # Claude adapter implementation
│   ├── qchat.py                # Q Chat adapter
│   ├── gemini.py               # Gemini adapter
│   ├── acp.py                  # ACP protocol adapter
│   ├── acp_protocol.py         # ACP message protocol
│   ├── acp_client.py           # ACP subprocess manager
│   ├── acp_handlers.py         # ACP permission handling
│   ├── acp_models.py           # ACP data models
│   └── [4 support files]
│
├── output/                     # Output Formatting (7 files)
│   ├── __init__.py
│   ├── base.py                 # Base formatter interface
│   ├── console.py              # Rich console formatter (7,121 tokens)
│   ├── plain.py                # Plain text formatter
│   ├── json_formatter.py       # JSON output formatter
│   ├── rich_formatter.py       # Rich formatting utilities
│   └── content_detector.py     # File type detection
│
├── tui/                        # Terminal UI (12 files, 87% coverage)
│   ├── __init__.py
│   ├── app.py                  # TUI application shell
│   ├── connection.py           # WebSocket connection handler
│   ├── ralph.tcss              # Textual CSS styling
│   ├── screens/                # Screen implementations
│   │   ├── __init__.py
│   │   ├── help.py             # Help screen
│   │   ├── history.py          # History browser
│   │   └── [other screens]
│   └── widgets/                # Custom widgets
│       ├── __init__.py
│       ├── metrics.py          # Metrics display widget
│       ├── output.py           # Output display widget
│       ├── progress.py         # Progress widget
│       ├── tasks.py            # Task list widget
│       └── validation.py       # Validation UI widget
│
├── web/                        # Web Server (6 files)
│   ├── __init__.py
│   ├── __main__.py             # Web server entry point
│   ├── server.py               # FastAPI application
│   ├── auth.py                 # Authentication handler
│   ├── database.py             # SQLAlchemy models
│   ├── rate_limit.py           # Rate limiting middleware
│   └── static/                 # Web UI assets
│       ├── index.html          # Web dashboard (11,312 tokens)
│       └── login.html          # Login page
│
└── onboarding/                 # Onboarding System (7 files)
    ├── __init__.py
    ├── scanner.py              # Project scanner
    ├── history_analyzer.py     # Git history analysis
    ├── agent_analyzer.py       # Agent capability detection
    ├── pattern_extractor.py    # Pattern extraction
    ├── config_generator.py     # Config file generation
    └── settings_loader.py      # Settings persistence
```

## Module Descriptions

### Core Orchestration

#### `orchestrator.py` - Main Loop Engine (9,135 tokens)
**Purpose**: Implements the Ralph Wiggum pattern with enterprise enhancements

**Key Classes**:
- `RalphOrchestrator`: Main orchestration engine
  - Manages the core iteration loop
  - Handles agent execution and output processing
  - Implements safety guards and limits
  - Tracks metrics and costs
  - Manages git checkpointing

**Key Methods**:
- `arun()` - Async main loop
- `execute_iteration()` - Single loop iteration
- `evaluate_completion()` - Check if task is complete
- `handle_error()` - Error recovery logic
- `create_checkpoint()` - Git-based state saving

**Design Pattern**: Orchestrator pattern with state machine

#### `__main__.py` - CLI Entry Point (9,924 tokens)
**Purpose**: Command-line interface and argument parsing

**Key Components**:
- Argument parser for CLI options
- Configuration validation
- Agent auto-detection
- Mode routing (CLI, TUI, Web)
- Signal handling for graceful shutdown

**Entry Points**:
- `ralph run` - Execute orchestration
- `ralph tui` - Start terminal interface
- `ralph web` - Start web server
- `ralph validate` - Validation mode

#### `context.py` - Context Management
**Purpose**: Manages prompt context and window summarization

**Key Responsibilities**:
- Load and parse PROMPT.md files
- Track context window usage
- Implement context summarization when window limit approached
- Archive prompt versions over iterations

#### `metrics.py` - Performance Tracking
**Purpose**: Collects and persists performance metrics

**Key Classes**:
- `Metrics`: Core metrics collection
- `CostTracker`: API cost tracking
- `IterationStats`: Per-iteration telemetry
- `TriggerReason`: Completion trigger classification

**Tracks**:
- Iteration count and duration
- Token usage (input, output, total)
- API costs per provider
- Error rates and retry attempts
- Memory usage and performance

#### `safety.py` - Safety Guards
**Purpose**: Enforces operational limits

**Key Mechanisms**:
- Iteration count limits
- Runtime duration limits
- Cost spending limits
- Token usage warnings
- Graceful shutdown triggers

### Agent Adapters

#### `adapters/base.py` - Adapter Interface
**Purpose**: Defines unified interface for all AI providers

**Key Interface**:
```python
class ToolAdapter(ABC):
    async def execute(prompt: str) -> ToolResponse
    async def validate() -> bool
    async def get_capabilities() -> dict
```

**Implementations**:
- `ClaudeAdapter` - Claude SDK integration
- `QChatAdapter` - Q Chat CLI tool
- `GeminiAdapter` - Gemini CLI tool
- `ACPAdapter` - Agent Client Protocol

#### `adapters/claude.py` - Claude Integration
**Purpose**: Primary Claude SDK adapter

**Features**:
- Web search capability
- Validation feature support
- Full API token tracking
- Error handling with Claude-specific recovery

#### `adapters/acp.py` - ACP Protocol Adapter
**Purpose**: Agent Client Protocol implementation

**Features**:
- JSON-RPC message protocol
- Subprocess lifecycle management
- Permission request handling
- Terminal I/O bridging

#### `adapters/acp_protocol.py` - ACP Protocol
**Purpose**: Message serialization and routing

**Message Types**:
- JSON-RPC requests/responses
- Notifications
- Permission requests
- Progress updates

### Output Formatters

#### `output/base.py` - Formatter Interface
**Purpose**: Defines output formatting contract

**Key Methods**:
- `format_output()` - Format agent response
- `format_error()` - Format error messages
- `format_metrics()` - Format metrics display
- `format_checkpoint()` - Format checkpoint info

#### `output/console.py` - Rich Terminal Output (7,121 tokens)
**Purpose**: Beautiful terminal formatting with Rich library

**Features**:
- Syntax highlighting for code
- Progress bars and spinners
- Colored output with themes
- Table formatting for metrics
- Panel rendering for sections

#### `output/json_formatter.py` - JSON Serialization
**Purpose**: Machine-readable output

**Serializes**:
- Agent responses
- Metrics and telemetry
- Checkpoint information
- Configuration states

### Terminal UI (TUI)

#### `tui/app.py` - Textual Application
**Purpose**: Main TUI application with Textual framework

**Screens**:
- Main orchestration view
- Metrics dashboard
- Output history browser
- Validation interface
- Help documentation

**Features**:
- Real-time updates via WebSocket
- Interactive task list
- Metrics graphs
- History browser
- Keyboard shortcuts

**Coverage**: 87% test coverage, 149+ tests

#### `tui/connection.py` - WebSocket Connection
**Purpose**: Real-time communication with web server

**Responsibilities**:
- Async WebSocket connection management
- Message serialization/deserialization
- Reconnection logic
- Connection state tracking

#### `tui/widgets/*` - Custom UI Widgets
**Purpose**: Reusable UI components

**Widgets**:
- `MetricsWidget` - Display metrics (iterations, costs, tokens)
- `OutputWidget` - Display agent output with syntax highlighting
- `ProgressWidget` - Show iteration progress and timers
- `TasksWidget` - List of tasks and status
- `ValidationWidget` - Validation interaction UI

### Web Server

#### `web/server.py` - FastAPI Application
**Purpose**: HTTP API and web interface

**Endpoints**:
- `GET /health` - Health check
- `POST /execute` - Start orchestration
- `GET /status` - Get current status
- `GET /metrics` - Retrieve metrics
- `WebSocket /ws` - Real-time updates
- `GET /history` - Get iteration history

**Middleware**:
- Authentication (JWT tokens)
- Rate limiting
- CORS handling
- Request logging

#### `web/auth.py` - Authentication
**Purpose**: Secure access control

**Mechanisms**:
- JWT token generation and validation
- User session management
- API key authentication
- Role-based access control

#### `web/database.py` - Data Persistence
**Purpose**: SQLAlchemy ORM models

**Tables**:
- `Executions` - Orchestration runs
- `Iterations` - Individual loop iterations
- `Metrics` - Performance metrics
- `Users` - User accounts and permissions

### Onboarding System

#### `onboarding/scanner.py` - Project Scanner
**Purpose**: Auto-detect project structure and AI tools

**Detects**:
- Available AI CLI tools (Claude, Q Chat, Gemini)
- Programming languages in codebase
- Framework usage (React, Django, etc.)
- Build system type
- Test framework in use

#### `onboarding/agent_analyzer.py` - Agent Capability Detection
**Purpose**: Determine what each agent can do

**Analyzes**:
- Claude SDK version and features
- Q Chat capabilities
- Gemini CLI features
- ACP-compliant agents

#### `onboarding/config_generator.py` - Configuration Generation
**Purpose**: Auto-generate ralph.yaml configuration

**Generates**:
- Default agent selection
- Iteration limits based on project size
- Cost limits based on project complexity
- Checkpoint intervals
- Output format preferences

## Key Files and Purposes

| File | Tokens | Purpose |
|------|--------|---------|
| web/static/index.html | 11,312 | Web dashboard UI |
| __main__.py | 9,924 | CLI entry point |
| orchestrator.py | 9,135 | Core loop engine |
| output/console.py | 7,121 | Rich terminal output |
| verbose_logger.py | 6,518 | Enhanced logging |
| error_formatter.py | ~4,000 | Error formatting |
| context.py | ~3,500 | Context management |
| safety.py | ~3,000 | Safety mechanisms |

## Dependency Overview

### Internal Dependencies
```
__main__.py
├── orchestrator.py
│   ├── adapters/* (all adapters)
│   ├── metrics.py
│   ├── safety.py
│   ├── context.py
│   └── output/* (formatters)
├── tui/app.py
└── web/server.py
```

### External Dependencies
- **anthropic** - Claude SDK
- **fastapi** - Web framework
- **textual** - TUI framework
- **sqlalchemy** - ORM
- **pydantic** - Data validation
- **rich** - Terminal formatting
- **asyncio** - Async runtime

### Subprocess Dependencies
- **claude** - Claude AI CLI
- **qchat** - Q Chat CLI
- **gemini** - Gemini CLI
- **git** - Version control

## Code Statistics

- **Total Files**: 57
- **Total Tokens**: 149,954
- **Total Characters**: 744,575
- **Test Files**: 50
- **Test Cases**: 295+
- **Test Lines**: 19,130+
- **Languages**: Python 3.10+
- **Security**: No suspicious files detected

## Architectural Patterns

### 1. Adapter Pattern
All AI providers implement `ToolAdapter` interface for seamless swapping.

### 2. Orchestrator Pattern
Central `RalphOrchestrator` manages lifecycle of orchestration process.

### 3. Factory Pattern
`_initialize_adapters()` creates appropriate adapter instances.

### 4. Observer Pattern
Metrics collection observes iteration lifecycle events.

### 5. Strategy Pattern
Different output formatters implement formatting strategy.

## Error Handling Strategy

1. **Try-Catch Blocks**: Wrap external calls (subprocess, API, file I/O)
2. **Exponential Backoff**: Retry with increasing delays (1s, 2s, 4s, etc.)
3. **Graceful Degradation**: Fall back to alternative formatters/outputs
4. **State Checkpointing**: Save state before risky operations
5. **Logging**: Log all errors with full context for debugging

## Async/Concurrency Model

- **Event Loop**: asyncio main loop in `arun()`
- **Non-Blocking I/O**: Git operations, file writes
- **Subprocess Execution**: Agent execution in asyncio subprocess
- **Real-Time Updates**: WebSocket for TUI live updates
- **Concurrent Safety**: Lock-based synchronization for shared state

## Configuration

Ralph stores configuration in YAML format:

```yaml
agent: claude           # or: qchat, gemini, acp
max_iterations: 100
max_runtime: 14400      # seconds
max_cost: 10.0          # dollars
checkpoint_interval: 5
enable_validation: false
verbose: false
```

## Extension Points

### Adding New AI Provider
1. Create adapter in `adapters/myagent.py`
2. Implement `ToolAdapter` interface
3. Register in `_initialize_adapters()`
4. Add tests in `tests/adapters/`

### Adding New Output Format
1. Create formatter in `output/myformat.py`
2. Implement `OutputFormatter` interface
3. Register in output module
4. Add to CLI options

### Adding New TUI Widget
1. Create widget in `tui/widgets/mywidget.py`
2. Inherit from Textual widget base class
3. Add to main screen layout
4. Implement event handlers

## Testing Coverage

- **Unit Tests**: Individual component testing
- **Integration Tests**: Multi-component flows
- **Async Tests**: AsyncIO-specific tests
- **ACP Protocol Tests**: 80+ test cases
- **Onboarding Tests**: 59 test cases
- **Output Formatters**: 24 test cases
- **Logging**: 25 test cases

## Deployment

- **Docker Image**: Multi-stage build with Python 3.11-slim
- **Services**: Ralph, Redis, PostgreSQL, Prometheus, Grafana, Docs
- **CI/CD**: GitHub Actions workflows
- **Documentation**: MkDocs integration

## Conclusion

Ralph Orchestrator is a well-structured, thoroughly tested application that successfully combines simplicity with enterprise-grade features. Its modular design, comprehensive testing, and clear separation of concerns make it maintainable and extensible for future enhancements.
