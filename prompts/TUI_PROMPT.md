# Task: Real-Time Terminal User Interface (TUI)

Create a beautiful, live terminal interface for RALPH orchestrator that displays progress in real-time. While the web GUI exists, this TUI provides a native terminal experience similar to AMP code's functionality, allowing developers to watch, explore, and eventually interact with orchestration activities without leaving their terminal.

## Objective

Build a rich Terminal User Interface using Textual (Python's modern TUI framework) that:
1. **Displays real-time progress** - Shows current iteration, task, and agent activity as it happens
2. **Streams agent output** - Live-updates with Claude's responses and tool calls
3. **Visualizes metrics** - CPU, memory, cost, tokens in real-time charts
4. **Provides task navigation** - Browse task queue, completed tasks, and history
5. **Enables interaction** - Pause, resume, and eventually edit prompts live
6. **Supports exploration** - Drill into specific iterations, view tool call details

## Requirements

- [ ] Create `src/ralph_orchestrator/tui/` module
- [ ] Implement main TUI application using Textual
- [ ] Create real-time streaming connection to orchestrator
- [ ] Build dashboard with multiple panes (progress, output, metrics, tasks)
- [ ] Implement live-updating progress bars for iterations
- [ ] Create scrollable, syntax-highlighted output viewer
- [ ] Add sparkline/graph widgets for metrics visualization
- [ ] Implement task queue sidebar with status indicators
- [ ] Add keyboard navigation and shortcuts
- [ ] Create iteration history browser
- [ ] Implement tool call detail viewer with expand/collapse
- [ ] Add pause/resume controls
- [ ] Create CLI command: `ralph watch` or `ralph tui`
- [ ] Add connection to existing orchestrator via IPC or WebSocket
- [ ] Support both attached (running TUI and orchestrator together) and detached modes
- [ ] Create comprehensive test coverage
- [ ] Write user documentation

## Technical Specifications

### 1. TUI Application Structure (`src/ralph_orchestrator/tui/app.py`)

```python
from textual.app import App, ComposeResult
from textual.containers import Container, Horizontal, Vertical
from textual.widgets import Header, Footer, Static, Log, ProgressBar, DataTable
from textual.binding import Binding

class RalphTUI(App):
    """Real-time Terminal UI for RALPH Orchestrator."""
    
    CSS_PATH = "ralph.tcss"
    TITLE = "RALPH Orchestrator"
    
    BINDINGS = [
        Binding("q", "quit", "Quit"),
        Binding("p", "pause_resume", "Pause/Resume"),
        Binding("l", "toggle_logs", "Logs"),
        Binding("t", "toggle_tasks", "Tasks"),
        Binding("m", "toggle_metrics", "Metrics"),
        Binding("h", "show_history", "History"),
        Binding("d", "toggle_details", "Details"),
        Binding("?", "show_help", "Help"),
    ]
    
    def compose(self) -> ComposeResult:
        yield Header()
        with Horizontal():
            yield TaskSidebar(id="tasks")
            with Vertical(id="main"):
                yield ProgressPanel(id="progress")
                yield OutputViewer(id="output")
                yield MetricsPanel(id="metrics")
        yield Footer()
```

### 2. Core Widgets

#### Progress Panel (`src/ralph_orchestrator/tui/widgets/progress.py`)

```python
class ProgressPanel(Static):
    """Real-time progress display with iteration tracking."""
    
    def __init__(self):
        super().__init__()
        self.current_iteration = 0
        self.max_iterations = 100
        self.current_task = None
        self.status = "idle"
        self.start_time = None
    
    def compose(self) -> ComposeResult:
        yield Static(id="status_badge")  # Running/Paused/Complete
        yield Static(id="current_task")   # Current task description
        yield ProgressBar(id="iteration_progress")
        yield Static(id="timing_info")    # Runtime, ETA
        yield Static(id="cost_tracker")   # Current cost / max cost
```

#### Output Viewer (`src/ralph_orchestrator/tui/widgets/output.py`)

```python
class OutputViewer(RichLog):
    """Syntax-highlighted, scrollable output display."""
    
    def __init__(self):
        super().__init__(highlight=True, markup=True)
        self.auto_scroll = True
    
    def append_agent_output(self, text: str):
        """Append agent output with formatting."""
        self.write(Syntax(text, "markdown", theme="monokai"))
    
    def append_tool_call(self, tool_name: str, tool_input: dict, result: str):
        """Display tool call with collapsible details."""
        ...
    
    def append_error(self, error: str):
        """Display error with red highlighting."""
        self.write(f"[red bold]ERROR:[/] {error}")
```

#### Task Sidebar (`src/ralph_orchestrator/tui/widgets/tasks.py`)

```python
class TaskSidebar(Static):
    """Collapsible sidebar showing task queue."""
    
    def compose(self) -> ComposeResult:
        yield Static("📋 Tasks", classes="sidebar-title")
        yield TaskList(id="task_list")
        yield Collapsible(
            CompletedTaskList(id="completed"),
            title="✅ Completed",
            collapsed=True
        )

class TaskList(ListView):
    """Scrollable list of pending tasks."""
    
    def update_tasks(self, tasks: List[Task]):
        self.clear()
        for task in tasks:
            self.append(TaskItem(task))
```

#### Metrics Panel (`src/ralph_orchestrator/tui/widgets/metrics.py`)

```python
class MetricsPanel(Static):
    """Real-time metrics with sparkline visualizations."""
    
    def compose(self) -> ComposeResult:
        with Horizontal():
            yield MetricWidget("CPU", id="cpu", icon="🔥")
            yield MetricWidget("Memory", id="memory", icon="💾")
            yield MetricWidget("Tokens", id="tokens", icon="🎯")
            yield MetricWidget("Cost", id="cost", icon="💰")

class MetricWidget(Static):
    """Single metric with value and sparkline history."""
    
    def __init__(self, label: str, icon: str = ""):
        super().__init__()
        self.label = label
        self.icon = icon
        self.history = []
        self.max_history = 60  # Last 60 data points
    
    def compose(self) -> ComposeResult:
        yield Static(f"{self.icon} {self.label}", classes="metric-label")
        yield Static(id="value", classes="metric-value")
        yield Sparkline(id="sparkline")
```

### 3. Data Connection Layer (`src/ralph_orchestrator/tui/connection.py`)

```python
import asyncio
from typing import AsyncIterator
from dataclasses import dataclass

@dataclass
class TUIEvent:
    """Event from orchestrator to TUI."""
    type: str  # "iteration_start", "output", "tool_call", "metrics", "complete"
    data: dict
    timestamp: float

class OrchestratorConnection:
    """Connection to running orchestrator instance."""
    
    def __init__(self, mode: str = "attached"):
        self.mode = mode  # "attached" or "websocket"
        self._queue: asyncio.Queue[TUIEvent] = asyncio.Queue()
    
    async def connect(self, target: str = None) -> bool:
        """Connect to orchestrator (URL for websocket, None for attached)."""
        ...
    
    async def events(self) -> AsyncIterator[TUIEvent]:
        """Stream events from orchestrator."""
        while True:
            event = await self._queue.get()
            yield event
    
    async def send_command(self, command: str, **kwargs):
        """Send command to orchestrator (pause, resume, etc.)."""
        ...

class AttachedConnection(OrchestratorConnection):
    """Direct connection when running TUI with orchestrator."""
    
    def __init__(self, orchestrator: RalphOrchestrator):
        super().__init__(mode="attached")
        self.orchestrator = orchestrator
        self._setup_hooks()
    
    def _setup_hooks(self):
        """Hook into orchestrator events."""
        # Intercept console output
        # Hook into metrics updates
        # Capture tool call events
        ...

class WebSocketConnection(OrchestratorConnection):
    """WebSocket connection to remote orchestrator."""
    
    async def connect(self, url: str) -> bool:
        """Connect to orchestrator's WebSocket endpoint."""
        ...
```

### 4. Styling (`src/ralph_orchestrator/tui/ralph.tcss`)

```css
/* RALPH TUI Stylesheet - Cyberpunk/Terminal aesthetic */

Screen {
    background: $surface;
}

Header {
    dock: top;
    background: $primary;
    color: $text;
}

Footer {
    dock: bottom;
    background: $surface-darken-1;
}

#tasks {
    width: 30;
    background: $surface-darken-2;
    border-right: solid $primary;
}

#main {
    width: 1fr;
}

#progress {
    height: 8;
    background: $surface-darken-1;
    border-bottom: solid $primary-darken-2;
    padding: 1;
}

#output {
    height: 1fr;
    background: $surface;
    padding: 1;
}

#metrics {
    height: 5;
    background: $surface-darken-1;
    border-top: solid $primary-darken-2;
}

.status-running {
    color: $success;
    text-style: bold;
}

.status-paused {
    color: $warning;
    text-style: italic;
}

.status-error {
    color: $error;
    text-style: bold;
}

ProgressBar > .bar--complete {
    color: $success;
}

.tool-call {
    background: $surface-darken-2;
    border: solid $accent;
    margin: 1 0;
    padding: 1;
}

.metric-value {
    text-style: bold;
    color: $primary-lighten-2;
}

Sparkline {
    height: 1;
    color: $primary;
}
```

### 5. CLI Integration

```bash
# Watch a running orchestrator (detached mode via WebSocket)
ralph watch

# Watch specific orchestrator by ID or URL
ralph watch --url ws://localhost:8080/ws

# Run with TUI attached (starts orchestrator with TUI)
ralph tui -P PROMPT.md

# Run TUI with specific prompt
ralph tui --prompt tasks/feature.md

# TUI with custom theme
ralph tui --theme cyberpunk

# TUI in read-only mode (no controls)
ralph watch --readonly
```

### 6. Keyboard Shortcuts

| Key | Action | Description |
|-----|--------|-------------|
| `q` | Quit | Exit TUI (orchestrator continues in detached mode) |
| `p` | Pause/Resume | Toggle orchestrator pause state |
| `l` | Toggle Logs | Expand/collapse output panel |
| `t` | Toggle Tasks | Show/hide task sidebar |
| `m` | Toggle Metrics | Show/hide metrics panel |
| `h` | History | Browse iteration history |
| `d` | Details | Show detailed view of selected item |
| `↑/↓` | Navigate | Move through tasks/history |
| `Enter` | Select | View details of selected item |
| `Esc` | Back | Return to main view |
| `/` | Search | Search in output |
| `f` | Follow | Toggle auto-scroll in output |
| `?` | Help | Show keyboard shortcuts |

### 7. Screen Modes

#### Main Dashboard (Default)
```
┌─────────────────────────────────────────────────────────────┐
│ 🤖 RALPH Orchestrator                                   [?] │
├──────────────┬──────────────────────────────────────────────┤
│ 📋 Tasks     │  ▶ Running - Iteration 12/100             │
│              │  📝 Implementing user authentication         │
│ ○ Setup DB   │  ████████████████░░░░░░░░░░░░░░░░ 36%       │
│ ● Auth API   │  ⏱ 12m 34s | ETA: 22m | 💰 $2.34 / $50.00   │
│ ○ Frontend   ├──────────────────────────────────────────────┤
│ ○ Testing    │                                              │
│              │  Creating authentication middleware...       │
│ ✅ Completed │                                              │
│  • Init      │  ```python                                   │
│  • Config    │  from fastapi import Depends, HTTPException  │
│              │  from fastapi.security import OAuth2...      │
│              │  ```                                         │
│              │                                              │
│              │  🔧 TOOL: write_file                         │
│              │  📁 src/auth/middleware.py                   │
│              │  ✓ Success (234 bytes written)               │
│              │                                              │
├──────────────┴──────────────────────────────────────────────┤
│ 🔥 CPU: 23%  💾 MEM: 45%  🎯 Tokens: 45.2K  💰 Cost: $2.34  │
├─────────────────────────────────────────────────────────────┤
│ [q]uit [p]ause [l]ogs [t]asks [m]etrics [h]istory [?]help   │
└─────────────────────────────────────────────────────────────┘
```

#### History View
```
┌─────────────────────────────────────────────────────────────┐
│ 📜 Iteration History                              [Esc]Back │
├─────────────────────────────────────────────────────────────┤
│ #  │ Status │ Duration │ Task                    │ Cost     │
│────┼────────┼──────────┼─────────────────────────┼──────────│
│ 12 │ ●      │ 2m 14s   │ Auth middleware         │ $0.23    │
│ 11 │ ✓      │ 1m 45s   │ Database models         │ $0.18    │
│ 10 │ ✓      │ 3m 02s   │ Project setup           │ $0.31    │
│  9 │ ✗      │ 0m 32s   │ Config (retry)          │ $0.04    │
│  8 │ ✓      │ 1m 18s   │ Dependencies            │ $0.12    │
│ ...│        │          │                         │          │
├─────────────────────────────────────────────────────────────┤
│ [↑↓] Navigate  [Enter] Details  [f]ilter  [Esc] Back        │
└─────────────────────────────────────────────────────────────┘
```

## Implementation Steps

### Step 1: Core TUI Framework
- [ ] Create `src/ralph_orchestrator/tui/__init__.py`
- [ ] Set up Textual application structure
- [ ] Define base styling with TCSS
- [ ] Implement basic layout containers

### Step 2: Core Widgets
- [ ] Create ProgressPanel with iteration tracking
- [ ] Build OutputViewer with syntax highlighting
- [ ] Implement TaskSidebar with collapsible sections
- [ ] Add MetricsPanel with sparkline charts

### Step 3: Data Connection
- [ ] Create event streaming protocol
- [ ] Implement AttachedConnection for same-process mode
- [ ] Add WebSocket connection for remote mode
- [ ] Build event queue and dispatching

### Step 4: Interactivity
- [ ] Implement keyboard navigation
- [ ] Add pause/resume functionality
- [ ] Create history browser
- [ ] Build detail view overlays

### Step 5: CLI Integration
- [ ] Add `ralph tui` command
- [ ] Add `ralph watch` command
- [ ] Implement connection options
- [ ] Add theme configuration

### Step 6: Polish & Testing
- [ ] Create snapshot tests for TUI layouts
- [ ] Add integration tests with mock orchestrator
- [ ] Optimize rendering performance
- [ ] Write user documentation

## Success Criteria

- [ ] TUI launches with `ralph tui` command
- [ ] Real-time output streaming works without lag
- [ ] Metrics update at least every 2 seconds
- [ ] All keyboard shortcuts are functional
- [ ] Task sidebar accurately reflects queue state
- [ ] History view shows all past iterations
- [ ] Pause/resume controls work correctly
- [ ] WebSocket connection mode works for remote watching
- [ ] TUI handles network disconnections gracefully
- [ ] Memory usage remains stable over long sessions
- [ ] Tests achieve 85%+ coverage
- [ ] Documentation includes screenshots and usage examples

## Dependencies

Add to `pyproject.toml`:
```toml
[project.dependencies]
textual = ">=0.50.0"
rich = ">=13.0.0"  # Already included
websockets = ">=12.0"  # For remote connection
```

## Notes

- Textual is a modern Python TUI framework built on Rich
- The TUI should feel responsive - target <100ms for user interactions
- Use Textual's reactive properties for efficient updates
- Consider dark/light theme support via CSS variables
- The attached mode is simpler to implement first; WebSocket mode can follow

## Progress

### Status: NOT STARTED

### Next Steps:
1. Add Textual dependency
2. Create basic TUI application structure
3. Implement core widgets

---

**Completion Marker:** When all success criteria are met, add `- [x] TASK_COMPLETE` here.
