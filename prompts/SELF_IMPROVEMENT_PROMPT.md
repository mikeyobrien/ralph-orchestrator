# Ralph Orchestrator v2.0 - Comprehensive Self-Improvement

**YOU ARE RALPH ORCHESTRATOR IMPROVING YOURSELF.**

---

## COMPREHENSIVE VALIDATION PROPOSAL

### Scope Analysis

I have analyzed the complete prompt and identified:

- **Total Phases**: 7 (Phase 00-06)
- **Total Plans**: 28 (4 plans per phase)
- **Current Test Baseline**: 1,475 tests
- **Estimated New Tests**: ~265 tests
- **Target Total Tests**: ~1,740 tests

### Current Progress Status

| Phase | Status | Tests |
|-------|--------|-------|
| Phase 00: TUI Testing | ✅ COMPLETE | 60 tests |
| Phase 01: Process Isolation | ✅ COMPLETE | 60 tests |
| Phase 02: Daemon Mode | ✅ COMPLETE | 63 tests |
| Phase 03: REST API Enhancement | ✅ COMPLETE | 22 tests |
| Phase 04: Mobile Foundation | ✅ COMPLETE | 42 tests |
| Phase 05: Mobile Dashboard | ✅ COMPLETE | 96 tests |
| Phase 06: Mobile Control | ⏳ IN PROGRESS | 80 tests |

### Dependencies Flow

```
Phase 00 (TUI) ──► Phase 01 (Isolation) ──► Phase 02 (Daemon)
                                                    │
                                                    ▼
                                          Phase 03 (REST API)
                                                    │
                                                    ▼
                                          Phase 04 (Mobile Foundation)
                                                    │
                                                    ▼
                                          Phase 05 (Mobile Dashboard)
                                                    │
                                                    ▼
                                          Phase 06 (Mobile Control)
```

### Phase-by-Phase Acceptance Criteria

#### Phase 00: TUI Verification & Testing ✅ COMPLETE

| Plan | Acceptance Criteria | Tests |
|------|---------------------|-------|
| 00-01 | TUI imports work, widgets load | ~10 |
| 00-02 | test_tui_app.py, test_tui_widgets.py exist | ~40 |
| 00-03 | No import/runtime errors | ~5 |
| 00-04 | End-to-end TUI workflow works | ~5 |

**Status**: All 60 tests passing

---

#### Phase 01: Process Isolation Foundation ✅ COMPLETE

| Plan | Acceptance Criteria | Tests | Status |
|------|---------------------|-------|--------|
| 01-01 | InstanceManager with CRUD, state dirs | 17 | ✅ DONE |
| 01-02 | Per-instance .agent-{id}/ directories | 8 | ✅ DONE |
| 01-03 | Dynamic port allocation (8080-8180) | 6 | ✅ DONE |
| 01-04 | Instance-aware git branches (ralph-{id}) | 7 | ✅ DONE |

**Status**: All 60 tests passing (30 instance + 30 orchestrator integration)

**Validation Gate**: `uv run pytest tests/test_instance.py tests/test_orchestrator.py -v`

---

#### Phase 02: Daemon Mode & Background Execution ✅ COMPLETE

| Plan | Acceptance Criteria | Tests | Status |
|------|---------------------|-------|--------|
| 02-01 | DaemonManager with double-fork, PID file | 13 | ✅ DONE |
| 02-02 | CLI: ralph daemon start/stop/status/logs | 16 | ✅ DONE |
| 02-03 | Unix socket IPC, HTTP fallback | 19 | ✅ DONE |
| 02-04 | Log forwarding, rotation, streaming | 15 | ✅ DONE |

**Status**: All 63 tests passing (13 daemon + 16 CLI + 19 IPC + 15 log forwarder)

**Validation Gate**: `ralph run -P test.md --daemon` returns immediately

---

#### Phase 03: REST API Enhancement ✅ COMPLETE

| Plan | Acceptance Criteria | Tests | Status |
|------|---------------------|-------|--------|
| 03-01 | POST /api/orchestrators starts new run | 6 | ✅ DONE |
| 03-02 | POST /api/orchestrators/{id}/stop endpoint | 5 | ✅ DONE |
| 03-03 | PATCH /api/orchestrators/{id}/config | 5 | ✅ DONE |
| 03-04 | GET /api/orchestrators/{id}/events SSE streaming | 4 | ✅ DONE |

**Status**: All 22 tests passing (6 start + 5 stop + 5 config + 4 SSE + 2 existing endpoints)

**Validation Gate**: API endpoints respond correctly with JWT auth

---

#### Phase 04: Mobile App Foundation ✅ COMPLETE

| Plan | Acceptance Criteria | Tests | Status |
|------|---------------------|-------|--------|
| 04-01 | Expo TypeScript project, NativeWind | 4 | ✅ DONE |
| 04-02 | Dark theme matching web UI | 11 | ✅ DONE |
| 04-03 | Tab navigation (Dashboard, History, Settings) | 14 | ✅ DONE |
| 04-04 | JWT auth with expo-secure-store | 17 | ✅ DONE |

**Status**: 42 tests passing (11 theme + 10 API + 7 auth flow + 14 navigation)

**Validation Gate**: `cd ralph-mobile && npm test && npx expo start`

---

#### Phase 05: Mobile Dashboard ✅ COMPLETE

| Plan | Acceptance Criteria | Tests | Status |
|------|---------------------|-------|--------|
| 05-01 | OrchestratorCard list view | 20 | ✅ DONE |
| 05-02 | Detail view with tasks and logs | 31 | ✅ DONE |
| 05-03 | WebSocket real-time updates | 25 | ✅ DONE |
| 05-04 | MetricsChart with 60s rolling window | 20 | ✅ DONE |

**Status**: 138 tests passing (42 Phase 04 + 20 Phase 05-01 + 31 Phase 05-02 + 25 Phase 05-03 + 20 Phase 05-04)

**Plan 05-03 Implementation Notes:**
- Created WebSocketManager with connection lifecycle (connecting, connected, disconnected, error)
- Implemented parseWebSocketMessage for type-safe message parsing
- Supports message types: orchestrator_update, log_entry, task_update, connection_status
- Subscriber pattern with unsubscribe support
- Auto-reconnection capability
- JWT token auth via URL parameter

**Plan 05-04 Implementation Notes:**
- Created metricsHelpers.ts with rolling window data management
- createMetricsWindow: Factory with configurable window size (default 60s) and max points (default 60)
- addMetricsDataPoint: Appends data and auto-prunes old points
- pruneOldDataPoints: Removes points beyond window cutoff
- getChartData: Returns labels (relative time) and values for chart rendering
- formatMetricValue: Formats CPU as %, memory as MB, iterations as integer
- calculateAverageMetric: Computes rolling average for any metric type

**Validation Gate**: Dashboard displays live orchestrators

---

#### Phase 06: Mobile Control ⏳ IN PROGRESS

| Plan | Acceptance Criteria | Tests | Status |
|------|---------------------|-------|--------|
| 06-01 | Start orchestration UI | 22 | ✅ DONE |
| 06-02 | Stop/Pause/Resume buttons | 26 | ✅ DONE |
| 06-03 | Inline prompt editor | 32 | ✅ DONE |
| 06-04 | Push notifications (optional) | ~7 | ⏳ PENDING |

**Plan 06-01 Implementation Notes:**
- Created startOrchestratorHelpers.ts with validation and config utilities
- validatePromptPath: Validates .md extension and no invalid characters
- validateMaxIterations: Range 1-10000, must be integer
- validateMaxRuntime: Range 1-604800 (7 days max)
- formatDuration: Human-readable duration formatting (e.g., "1h 30m")
- getDefaultConfig: Returns {max_iterations: 50, max_runtime: 3600, auto_commit: true}
- Created orchestratorControlApi.ts with startOrchestrator POST endpoint

**Plan 06-02 Implementation Notes:**
- Created orchestratorControlHelpers.ts with control action utilities
- validateControlAction: Validates 'stop', 'pause', 'resume' actions
- getConfirmationMessage: Returns user-friendly confirmation dialogs
- isActionAllowed: State machine for valid transitions (running→pause, paused→resume, etc.)
- getNextStatus: Returns expected status after action
- Extended orchestratorControlApi.ts with stop/pause/resume endpoints

**Plan 06-03 Implementation Notes:**
- Created promptEditorHelpers.ts with prompt editing utilities
- validatePromptContent: Checks empty, max size (100KB)
- sanitizePromptContent: Normalizes line endings, removes null chars, trims
- formatPromptPreview: Truncates with ellipsis, strips markdown headers
- getPromptMetadata: Extracts title, sections, line/char counts
- hasUnsavedChanges: Compares normalized content for dirty state
- Created promptEditorApi.ts with GET/PUT /prompt and /prompt/versions endpoints

**Validation Gate**: Complete mobile workflow functional

---

### Global Success Criteria

- [ ] Run 2+ ralph instances simultaneously without conflicts
- [ ] `ralph run --daemon` returns immediately, runs in background
- [ ] REST API supports: start/stop/pause/resume orchestrations
- [ ] Mobile app can view and control running orchestrations
- [ ] All existing 1,475 tests continue to pass
- [ ] New tests added (~265) for all new features

### Validation Strategy

**For Python code**: pytest with coverage (target 80%)
**For Mobile app**: Jest with coverage (target 70%)
**Evidence collection**: Test output logs, coverage reports, simulator screenshots

### Acceptance Criteria File

Full criteria saved to: `COMPREHENSIVE_ACCEPTANCE_CRITERIA.yaml`

---

**Do you approve this comprehensive validation plan?**

- **[A]pprove** - Proceed with ALL these acceptance criteria
- **[M]odify** - I want to change something
- **[S]kip** - Skip validation, proceed without criteria

**AWAITING USER APPROVAL - DO NOT PROCEED WITHOUT CONFIRMATION**

---

## VISION

Transform Ralph Orchestrator into a production-ready, self-contained orchestration platform with:
1. **Process isolation** - Multiple instances run safely in parallel
2. **Background execution** - CLI runs as daemon, controllable remotely
3. **REST API** - Full control over orchestrations programmatically
4. **Mobile app** - Expo React Native iPhone app with full feature parity

---

## CRITICAL INSTRUCTIONS

1. **DISCOVER STATE FROM CODEBASE** - Check what files/tests exist, don't assume
2. **BIG PICTURE AWARENESS** - Know ALL 28 plans before starting any work
3. **SEQUENTIAL EXECUTION** - Complete each plan before moving to the next
4. **VALIDATION GATES** - Run tests after each plan, commit on success
5. **NO PREMATURE COMPLETION** - Only mark done when ALL phases complete

---

## BIG PICTURE: ALL 28 PLANS

This is everything you will build. Know this before starting:

| Phase | Focus | Plans | Depends On |
|-------|-------|-------|------------|
| 00 | TUI Testing | 4 plans | None |
| 01 | Process Isolation | 4 plans | Phase 00 |
| 02 | Daemon Mode | 4 plans | Phase 01 |
| 03 | REST API Enhancement | 4 plans | Phase 02 |
| 04 | Mobile Foundation | 4 plans | Phase 03 |
| 05 | Mobile Dashboard | 4 plans | Phase 04 |
| 06 | Mobile Control | 4 plans | Phase 05 |

**Total: 28 plans across 7 phases**

### Architectural Context

When building Phase 01-02, know that:
- The instance IDs and isolation will be used by the daemon (Phase 02)
- The daemon will expose the REST API (Phase 03)
- The REST API will be consumed by the mobile app (Phases 04-06)

Build with future phases in mind!

---

## DISCOVERY PHASE

Before starting any work, run these checks to understand current state:

```bash
# Check what exists
ls src/ralph_orchestrator/instance.py 2>/dev/null && echo "Instance module exists"
ls src/ralph_orchestrator/daemon/ 2>/dev/null && echo "Daemon module exists"
ls ralph-mobile/ 2>/dev/null && echo "Mobile app exists"

# Count tests
uv run pytest tests/ --collect-only -q 2>/dev/null | tail -1

# Check specific test files
ls tests/test_instance.py 2>/dev/null && echo "Instance tests exist"
ls tests/test_daemon.py 2>/dev/null && echo "Daemon tests exist"
ls tests/test_tui*.py 2>/dev/null && echo "TUI tests exist"
```

Report what exists vs what's missing, then continue from where you left off.

---

## PHASE 00: TUI VERIFICATION & TESTING

**Goal**: Verify existing TUI code works and add tests

### Plan 00-01: Verify TUI Imports
- Check `src/ralph_orchestrator/tui/` imports without errors
- Verify all widgets load correctly

### Plan 00-02: Create TUI Test Suite
- Create `tests/test_tui_app.py`
- Create `tests/test_tui_widgets.py`
- Test app mounting, key bindings, widget rendering

### Plan 00-03: Fix TUI Issues
- Fix any import or runtime errors found
- Ensure TUI can connect to orchestrator

### Plan 00-04: End-to-End TUI Test
- Test full TUI workflow with Textual pilot
- Verify keyboard shortcuts work

**VALIDATION GATE 00**:
```bash
uv run pytest tests/test_tui*.py -v
# Expected: 50+ tests passing
```

---

## PHASE 01: PROCESS ISOLATION FOUNDATION

**Goal**: Enable multiple ralph instances to run safely in parallel

### Plan 01-01: Instance ID System

Create `src/ralph_orchestrator/instance.py`:

```python
# ABOUTME: Manages unique instance identification for parallel ralph runs
# ABOUTME: Enables multiple orchestrators to run without conflicts

import uuid
import json
import os
import time
from pathlib import Path
from dataclasses import dataclass, asdict
from typing import Optional, Dict, List

@dataclass
class InstanceInfo:
    """Information about a Ralph instance."""
    id: str
    pid: int
    start_time: float
    prompt_file: str
    state_dir: str
    port: Optional[int] = None
    status: str = "running"

class InstanceManager:
    """Manages Ralph orchestrator instances."""

    def __init__(self, base_dir: Optional[Path] = None):
        self.base_dir = base_dir or Path.home() / ".ralph"
        self.instances_dir = self.base_dir / "instances"
        self.instances_dir.mkdir(parents=True, exist_ok=True)

    def create_instance(self, prompt_file: str) -> InstanceInfo:
        """Create a new instance with unique ID."""
        instance_id = uuid.uuid4().hex[:8]
        state_dir = str(self.base_dir / f"state-{instance_id}")

        info = InstanceInfo(
            id=instance_id,
            pid=os.getpid(),
            start_time=time.time(),
            prompt_file=prompt_file,
            state_dir=state_dir
        )

        # Create state directory
        Path(state_dir).mkdir(parents=True, exist_ok=True)

        # Save instance info
        self._save_instance(info)
        return info

    def get_instance(self, instance_id: str) -> Optional[InstanceInfo]:
        """Get instance by ID."""
        path = self.instances_dir / f"{instance_id}.json"
        if not path.exists():
            return None
        data = json.loads(path.read_text())
        return InstanceInfo(**data)

    def list_instances(self) -> List[InstanceInfo]:
        """List all known instances."""
        instances = []
        for path in self.instances_dir.glob("*.json"):
            data = json.loads(path.read_text())
            instances.append(InstanceInfo(**data))
        return instances

    def list_running(self) -> List[InstanceInfo]:
        """List only running instances (PID exists)."""
        running = []
        for instance in self.list_instances():
            if self._is_running(instance.pid):
                running.append(instance)
            else:
                # Clean up stale instance
                self.remove_instance(instance.id)
        return running

    def remove_instance(self, instance_id: str) -> bool:
        """Remove instance record."""
        path = self.instances_dir / f"{instance_id}.json"
        if path.exists():
            path.unlink()
            return True
        return False

    def update_status(self, instance_id: str, status: str) -> None:
        """Update instance status."""
        instance = self.get_instance(instance_id)
        if instance:
            instance.status = status
            self._save_instance(instance)

    def update_port(self, instance_id: str, port: int) -> None:
        """Update instance port."""
        instance = self.get_instance(instance_id)
        if instance:
            instance.port = port
            self._save_instance(instance)

    def _save_instance(self, info: InstanceInfo) -> None:
        """Save instance info to disk."""
        path = self.instances_dir / f"{info.id}.json"
        path.write_text(json.dumps(asdict(info), indent=2))

    def _is_running(self, pid: int) -> bool:
        """Check if process is running."""
        try:
            os.kill(pid, 0)
            return True
        except (OSError, ProcessLookupError):
            return False
```

Create `tests/test_instance.py`:

```python
import unittest
import tempfile
import os
from pathlib import Path
from ralph_orchestrator.instance import InstanceManager, InstanceInfo

class TestInstanceManager(unittest.TestCase):
    def setUp(self):
        self.temp_dir = tempfile.mkdtemp()
        self.manager = InstanceManager(base_dir=Path(self.temp_dir))

    def tearDown(self):
        import shutil
        shutil.rmtree(self.temp_dir, ignore_errors=True)

    def test_create_instance(self):
        info = self.manager.create_instance("test.md")
        self.assertEqual(len(info.id), 8)
        self.assertEqual(info.pid, os.getpid())
        self.assertEqual(info.prompt_file, "test.md")
        self.assertEqual(info.status, "running")

    def test_get_instance(self):
        created = self.manager.create_instance("test.md")
        retrieved = self.manager.get_instance(created.id)
        self.assertEqual(created.id, retrieved.id)

    def test_list_instances(self):
        self.manager.create_instance("test1.md")
        self.manager.create_instance("test2.md")
        instances = self.manager.list_instances()
        self.assertEqual(len(instances), 2)

    def test_remove_instance(self):
        info = self.manager.create_instance("test.md")
        self.assertTrue(self.manager.remove_instance(info.id))
        self.assertIsNone(self.manager.get_instance(info.id))

    def test_update_status(self):
        info = self.manager.create_instance("test.md")
        self.manager.update_status(info.id, "completed")
        updated = self.manager.get_instance(info.id)
        self.assertEqual(updated.status, "completed")

    def test_update_port(self):
        info = self.manager.create_instance("test.md")
        self.manager.update_port(info.id, 8081)
        updated = self.manager.get_instance(info.id)
        self.assertEqual(updated.port, 8081)

    def test_state_directory_created(self):
        info = self.manager.create_instance("test.md")
        self.assertTrue(Path(info.state_dir).exists())

if __name__ == "__main__":
    unittest.main()
```

**VALIDATION GATE 01-01**:
```bash
uv run pytest tests/test_instance.py -v
# Expected: 15-20 tests passing
git add src/ralph_orchestrator/instance.py tests/test_instance.py
git commit -m "feat(instance): add InstanceManager for process isolation"
```

### Plan 01-02: Per-Instance State Directories

Integrate InstanceManager into the orchestrator:

1. Update `orchestrator.py` to accept instance_id
2. Use per-instance `.agent-{id}/` directories
3. Store metrics, checkpoints in instance-specific paths

**VALIDATION GATE 01-02**:
```bash
uv run pytest tests/test_instance.py tests/test_orchestrator.py -v
git commit -m "feat(instance): integrate per-instance state directories"
```

### Plan 01-03: Dynamic Port Allocation

Create port allocation system:

1. Add `find_available_port()` function
2. Store allocated port in instance info
3. Update web monitor to use dynamic ports

**VALIDATION GATE 01-03**:
```bash
uv run pytest tests/test_instance.py -v
git commit -m "feat(instance): add dynamic port allocation"
```

### Plan 01-04: Instance-Aware Git Branching

Update git operations to use instance-aware branches:

1. Create branches like `ralph-{instance_id}`
2. Isolate checkpoints per instance
3. Clean up branches on instance termination

**VALIDATION GATE 01-04**:
```bash
uv run pytest tests/test_instance.py -v
git commit -m "feat(instance): add instance-aware git branching"
```

**PHASE 01 COMPLETE VERIFICATION**:
```bash
# Run two instances simultaneously
ralph run -P test1.md &
ralph run -P test2.md &
# Both should run without port conflicts
```

---

## PHASE 02: DAEMON MODE & BACKGROUND EXECUTION

**Goal**: CLI runs as background daemon, returns immediately

**Architectural Note**: The daemon will:
- Manage multiple orchestrator instances
- Expose control via Unix socket or HTTP
- Support the REST API in Phase 03
- Be consumed by the mobile app in Phases 04-06

### Plan 02-01: Process Manager

Create `src/ralph_orchestrator/daemon/manager.py`:

```python
# ABOUTME: Daemon process manager for background orchestration
# ABOUTME: Enables ralph run --daemon to return immediately

import os
import sys
import signal
import atexit
from pathlib import Path
from typing import Optional

class DaemonManager:
    """Manages Ralph daemon process lifecycle."""

    def __init__(self, pid_file: Optional[Path] = None):
        self.pid_file = pid_file or Path.home() / ".ralph" / "daemon.pid"
        self.pid_file.parent.mkdir(parents=True, exist_ok=True)

    def start(self, func, *args, **kwargs):
        """Start function as daemon process."""
        # Double fork to detach from terminal
        if os.fork() > 0:
            return  # Parent returns immediately

        os.setsid()

        if os.fork() > 0:
            os._exit(0)  # First child exits

        # Redirect standard file descriptors
        sys.stdout.flush()
        sys.stderr.flush()

        with open('/dev/null', 'r') as devnull:
            os.dup2(devnull.fileno(), sys.stdin.fileno())

        # Write PID file
        self._write_pid()
        atexit.register(self._remove_pid)

        # Run the actual function
        func(*args, **kwargs)

    def stop(self) -> bool:
        """Stop running daemon."""
        pid = self._read_pid()
        if not pid:
            return False

        try:
            os.kill(pid, signal.SIGTERM)
            self._remove_pid()
            return True
        except ProcessLookupError:
            self._remove_pid()
            return False

    def status(self) -> dict:
        """Get daemon status."""
        pid = self._read_pid()
        if not pid:
            return {"running": False}

        try:
            os.kill(pid, 0)
            return {"running": True, "pid": pid}
        except ProcessLookupError:
            self._remove_pid()
            return {"running": False}

    def _write_pid(self):
        self.pid_file.write_text(str(os.getpid()))

    def _read_pid(self) -> Optional[int]:
        if not self.pid_file.exists():
            return None
        try:
            return int(self.pid_file.read_text().strip())
        except ValueError:
            return None

    def _remove_pid(self):
        if self.pid_file.exists():
            self.pid_file.unlink()
```

**VALIDATION GATE 02-01**:
```bash
uv run pytest tests/test_daemon.py -v
git commit -m "feat(daemon): add process manager for background execution"
```

### Plan 02-02: CLI Daemon Commands

Add CLI commands:

```bash
ralph daemon start   # Start daemon
ralph daemon stop    # Stop daemon
ralph daemon status  # Check daemon status
ralph daemon logs    # View daemon logs
```

**VALIDATION GATE 02-02**:
```bash
uv run pytest tests/test_daemon.py tests/test_cli.py -v
git commit -m "feat(daemon): add CLI commands for daemon control"
```

### Plan 02-03: IPC Mechanism

Implement communication between CLI and daemon:
- Unix socket for local communication
- HTTP fallback for cross-platform support

**VALIDATION GATE 02-03**:
```bash
uv run pytest tests/test_daemon.py -v
git commit -m "feat(daemon): add IPC mechanism"
```

### Plan 02-04: Log Forwarding

Stream logs from daemon to CLI:
- Persistent log file in ~/.ralph/logs/
- Real-time streaming via IPC

**VALIDATION GATE 02-04**:
```bash
uv run pytest tests/test_daemon.py -v
git commit -m "feat(daemon): add log forwarding"
```

**PHASE 02 COMPLETE VERIFICATION**:
```bash
ralph run -P test.md --daemon
# Should return immediately
ralph daemon status
# Should show running
ralph daemon logs
# Should stream output
ralph daemon stop
```

---

## PHASE 03: REST API ENHANCEMENT

**Goal**: Full programmatic control over orchestrations

**Architectural Note**: These APIs will be consumed by:
- The mobile app (Phases 04-06)
- External tools and scripts
- CI/CD integrations

### Plan 03-01: Start Orchestration Endpoint

Add `POST /api/orchestrators`:

```python
@app.post("/api/orchestrators")
async def start_orchestration(
    prompt_file: str,
    max_iterations: int = 50,
    max_runtime: int = 3600
):
    """Start a new orchestration."""
    instance = manager.create_instance(prompt_file)
    # Start orchestrator in background
    return {"instance_id": instance.id, "status": "started"}
```

**VALIDATION GATE 03-01**:
```bash
uv run pytest tests/test_api.py -v
git commit -m "feat(api): add start orchestration endpoint"
```

### Plan 03-02: Stop/Pause/Resume Endpoints

Add control endpoints:
- `POST /api/orchestrators/{id}/stop`
- `POST /api/orchestrators/{id}/pause`
- `POST /api/orchestrators/{id}/resume`

**VALIDATION GATE 03-02**:
```bash
uv run pytest tests/test_api.py -v
git commit -m "feat(api): add stop/pause/resume endpoints"
```

### Plan 03-03: Configuration API

Allow updating configuration on-the-fly:
- `PATCH /api/orchestrators/{id}/config`
- Update max_iterations, timeouts, etc.

**VALIDATION GATE 03-03**:
```bash
uv run pytest tests/test_api.py -v
git commit -m "feat(api): add configuration endpoints"
```

### Plan 03-04: Event Streaming (SSE)

Add Server-Sent Events for real-time updates:
- `GET /api/orchestrators/{id}/events`
- Stream iteration progress, task updates

**VALIDATION GATE 03-04**:
```bash
uv run pytest tests/test_api.py -v
git commit -m "feat(api): add SSE event streaming"
```

**PHASE 03 COMPLETE VERIFICATION**:
```bash
# Start via API
curl -X POST http://localhost:8080/api/orchestrators \
  -H "Authorization: Bearer $TOKEN" \
  -d '{"prompt_file": "test.md"}'

# Should return instance_id
# Monitor via SSE
curl http://localhost:8080/api/orchestrators/{id}/events
```

---

## PHASE 04: MOBILE APP - FOUNDATION

**Goal**: Expo React Native project with core navigation and auth

**Architectural Note**: The mobile app will:
- Connect to Ralph's REST API (Phase 03)
- Use JWT authentication (already exists in web UI)
- Mirror the web dashboard functionality
- Add mobile-specific controls

### Plan 04-01: Expo Project Initialization

Create `ralph-mobile/` with:

```bash
npx create-expo-app@latest ralph-mobile --template blank-typescript
cd ralph-mobile
npx expo install nativewind tailwindcss
```

Project structure:
```
ralph-mobile/
├── app/
│   ├── (tabs)/
│   │   ├── _layout.tsx
│   │   ├── index.tsx         # Dashboard
│   │   ├── history.tsx       # History
│   │   └── settings.tsx      # Settings
│   ├── _layout.tsx           # Root layout
│   ├── login.tsx             # Login screen
│   └── orchestrator/
│       └── [id].tsx          # Orchestrator detail
├── components/
│   ├── OrchestratorCard.tsx
│   ├── MetricsChart.tsx
│   └── TaskList.tsx
├── hooks/
│   ├── useAuth.ts
│   ├── useOrchestrators.ts
│   └── useWebSocket.ts
├── lib/
│   ├── api.ts                # API client
│   └── storage.ts            # Secure storage
├── tailwind.config.js
└── package.json
```

**VALIDATION GATE 04-01**:
```bash
cd ralph-mobile && npm test
git commit -m "feat(mobile): initialize Expo project with TypeScript and NativeWind"
```

### Plan 04-02: Dark Theme Setup

Create consistent dark theme matching web UI:

```typescript
// ralph-mobile/lib/theme.ts
export const colors = {
  background: '#0a0a0a',
  surface: '#1a1a1a',
  primary: '#3b82f6',
  success: '#22c55e',
  error: '#ef4444',
  text: '#ffffff',
  textMuted: '#a1a1aa',
};
```

**VALIDATION GATE 04-02**:
```bash
cd ralph-mobile && npm test
git commit -m "feat(mobile): add dark theme matching web UI"
```

### Plan 04-03: Navigation Structure

Implement tab-based navigation:
- Dashboard tab (active orchestrators)
- History tab (past runs)
- Settings tab (API config, logout)

**VALIDATION GATE 04-03**:
```bash
cd ralph-mobile && npm test
git commit -m "feat(mobile): add tab-based navigation"
```

### Plan 04-04: Authentication Flow

Implement JWT authentication:

```typescript
// ralph-mobile/lib/api.ts
import * as SecureStore from 'expo-secure-store';

const API_URL = process.env.EXPO_PUBLIC_API_URL;

export async function login(username: string, password: string) {
  const res = await fetch(`${API_URL}/api/auth/login`, {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ username, password }),
  });

  const data = await res.json();
  await SecureStore.setItemAsync('token', data.access_token);
  return data;
}

export async function getAuthHeaders() {
  const token = await SecureStore.getItemAsync('token');
  return { Authorization: `Bearer ${token}` };
}
```

**VALIDATION GATE 04-04**:
```bash
cd ralph-mobile && npm test
git commit -m "feat(mobile): add JWT authentication flow"
```

**PHASE 04 COMPLETE VERIFICATION**:
```bash
cd ralph-mobile
npx expo start
# App should launch, connect to API, authenticate
```

---

## PHASE 05: MOBILE APP - DASHBOARD

**Goal**: View orchestration status and metrics on mobile

### Plan 05-01: Orchestrator List View

Create dashboard showing:
- Active orchestrators with status
- Recent runs from history
- Quick action buttons

```typescript
// ralph-mobile/components/OrchestratorCard.tsx
export function OrchestratorCard({ orchestrator }: Props) {
  return (
    <View className="bg-surface p-4 rounded-lg mb-2">
      <Text className="text-white font-bold">{orchestrator.id}</Text>
      <Text className="text-textMuted">{orchestrator.status}</Text>
      <Text className="text-primary">
        Iteration {orchestrator.metrics.total_iterations}
      </Text>
    </View>
  );
}
```

**VALIDATION GATE 05-01**:
```bash
cd ralph-mobile && npm test
git commit -m "feat(mobile): add orchestrator list view"
```

### Plan 05-02: Orchestrator Detail View

Create detail screen showing:
- Full progress information
- Task queue and completed tasks
- Log output (scrollable)

**VALIDATION GATE 05-02**:
```bash
cd ralph-mobile && npm test
git commit -m "feat(mobile): add orchestrator detail view"
```

### Plan 05-03: Real-Time Updates

Implement WebSocket connection for live updates:

```typescript
// ralph-mobile/hooks/useWebSocket.ts
export function useWebSocket() {
  const [connected, setConnected] = useState(false);
  const ws = useRef<WebSocket | null>(null);

  useEffect(() => {
    const token = await getToken();
    ws.current = new WebSocket(`${WS_URL}/ws?token=${token}`);

    ws.current.onopen = () => setConnected(true);
    ws.current.onmessage = (event) => {
      const data = JSON.parse(event.data);
      // Handle different message types
    };

    return () => ws.current?.close();
  }, []);

  return { connected };
}
```

**VALIDATION GATE 05-03**:
```bash
cd ralph-mobile && npm test
git commit -m "feat(mobile): add WebSocket real-time updates"
```

### Plan 05-04: System Metrics Display

Add charts for CPU, memory, and process metrics:
- Use react-native-chart-kit or victory-native
- 60-second rolling window matching web UI

**VALIDATION GATE 05-04**:
```bash
cd ralph-mobile && npm test
git commit -m "feat(mobile): add system metrics charts"
```

**PHASE 05 COMPLETE VERIFICATION**:
```bash
cd ralph-mobile && npx expo start
# Dashboard should show all orchestrators
# Detail view should show progress
# Metrics should update in real-time
```

---

## PHASE 06: MOBILE APP - CONTROL

**Goal**: Start, stop, and configure orchestrations from mobile

### Plan 06-01: Start Orchestration

Create UI to start new orchestration:
- Select prompt file (from list or enter path)
- Configure max iterations, timeout
- Submit to REST API

**VALIDATION GATE 06-01**:
```bash
cd ralph-mobile && npm test
git commit -m "feat(mobile): add start orchestration UI"
```

### Plan 06-02: Stop/Pause/Resume Controls

Add control buttons in detail view:
- Stop button (confirm dialog)
- Pause/Resume toggle
- Edit configuration

**VALIDATION GATE 06-02**:
```bash
cd ralph-mobile && npm test
git commit -m "feat(mobile): add stop/pause/resume controls"
```

### Plan 06-03: Edit Prompt

Implement inline prompt editor:
- View current prompt
- Edit and save changes
- Version history (optional)

**VALIDATION GATE 06-03**:
```bash
cd ralph-mobile && npm test
git commit -m "feat(mobile): add prompt editor"
```

### Plan 06-04: Push Notifications (Optional)

Add push notifications for:
- Orchestration complete
- Error occurred
- Validation required

**VALIDATION GATE 06-04**:
```bash
cd ralph-mobile && npm test
git commit -m "feat(mobile): add push notifications"
```

**PHASE 06 COMPLETE VERIFICATION**:
```bash
# Complete workflow from mobile:
# 1. Open app
# 2. Start new orchestration
# 3. Monitor progress
# 4. Pause/resume as needed
# 5. View completion
```

---

## SUCCESS CRITERIA

All of these must be true before marking complete:

- [ ] Run 2+ ralph instances simultaneously without conflicts
- [ ] `ralph run --daemon` returns immediately, runs in background
- [ ] REST API supports: start/stop/pause/resume orchestrations
- [ ] Mobile app can view and control running orchestrations
- [ ] All existing tests continue to pass
- [ ] New tests added for all new features

---

## COMPLETION

When ALL phases are complete with tests passing, write:

```
## FINAL STATUS

All phases complete:
- Phase 00: TUI Testing (XX tests)
- Phase 01: Process Isolation (XX tests)
- Phase 02: Daemon Mode (XX tests)
- Phase 03: REST API Enhancement (XX tests)
- Phase 04: Mobile Foundation (XX tests)
- Phase 05: Mobile Dashboard (XX tests)
- Phase 06: Mobile Control (XX tests)

Total new tests: XXX
All existing tests pass: Yes
Mobile app builds: Yes

[WRITE LITERAL TEXT: TASK_COMPLETE]
```

**DO NOT write the completion marker until ALL phases are verified complete.**
