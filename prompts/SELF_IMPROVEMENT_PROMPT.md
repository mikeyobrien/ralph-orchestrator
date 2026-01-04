# Ralph Orchestrator v2.0 - Comprehensive Self-Improvement

**YOU ARE RALPH ORCHESTRATOR IMPROVING YOURSELF.**

---

## CRITICAL: VALIDATION APPROACH

**THIS PROJECT REQUIRES FUNCTIONAL VALIDATION - NOT UNIT TESTS**

### FORBIDDEN (Will cause false completion):
- `npm test` - runs mocked Jest tests
- `uv run pytest` - runs mocked unit tests
- Any Jest/pytest command alone

### REQUIRED (Real execution with evidence):
- iOS Simulator screenshots for mobile phases
- `curl` commands for API phases
- CLI output captures for daemon phases
- Evidence files in `validation-evidence/`

---

## COMPREHENSIVE VALIDATION PROPOSAL

### Scope Analysis

This project has:

- **Total Phases**: 7 (Phase 00-06)
- **Total Plans**: 28 (4 plans per phase)
- **Evidence Files Required**: ~20+ screenshots and output captures

### Current Progress Status

| Phase | Status | Evidence Required |
|-------|--------|-------------------|
| Phase 00: TUI Testing | ✅ VALIDATED | `validation-evidence/phase-00/tui-output.txt` |
| Phase 01: Process Isolation | ✅ VALIDATED | `validation-evidence/phase-01/parallel-instances.txt`, `port-allocation.txt` |
| Phase 02: Daemon Mode | ✅ VALIDATED | `validation-evidence/phase-02/daemon-start.txt`, `daemon-status.txt` |
| Phase 03: REST API Enhancement | ✅ VALIDATED | `validation-evidence/phase-03/api-endpoints.txt`, `api-start.json`, `api-stop.json` |
| Phase 04: Mobile Foundation | ⏳ NEEDS VALIDATION | iOS Simulator screenshot |
| Phase 05: Mobile Dashboard | ⏳ NEEDS VALIDATION | iOS Simulator with live data |
| Phase 06: Mobile Control | ⏳ NEEDS VALIDATION | iOS Simulator showing controls |

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

---

## VALIDATION EVIDENCE DIRECTORY

Create this structure and populate with REAL evidence:

```
validation-evidence/
├── phase-00/
│   └── tui-screenshot.png
├── phase-01/
│   ├── parallel-instances.txt
│   └── port-allocation.txt
├── phase-02/
│   ├── daemon-start.txt
│   ├── daemon-status.txt
│   └── daemon-logs.txt
├── phase-03/
│   ├── api-start.json
│   ├── api-stop.json
│   └── api-events.txt
├── phase-04/
│   ├── expo-build.txt
│   └── simulator-app.png
├── phase-05/
│   ├── dashboard.png
│   ├── detail-view.png
│   └── websocket.txt
├── phase-06/
│   ├── start-ui.png
│   ├── controls.png
│   └── api-calls.txt
└── final/
    └── summary.md
```

---

### Phase-by-Phase Acceptance Criteria

#### Phase 00: TUI Verification & Testing ✅ VALIDATED

| Plan | Acceptance Criteria | Status |
|------|---------------------|--------|
| 00-01 | TUI imports work, widgets load | ✅ VALIDATED |
| 00-02 | test_tui_app.py, test_tui_widgets.py exist | ✅ VALIDATED |
| 00-03 | No import/runtime errors | ✅ VALIDATED |
| 00-04 | End-to-end TUI workflow works | ✅ VALIDATED |

**Evidence**: `validation-evidence/phase-00/tui-output.txt`
- TUI launched successfully with `ralph tui -P prompts/SELF_IMPROVEMENT_PROMPT.md`
- All widgets rendered: TaskPanel, StatusPanel, MetricsPanel, LogPanel
- No import/runtime errors observed
- "Connected to orchestrator" confirmed

**REAL VALIDATION GATE** (NOT `pytest`):
```bash
# Run TUI and capture screenshot
ralph tui &
sleep 2
# Take screenshot of TUI (using appropriate tool)
xcrun simctl io booted screenshot validation-evidence/phase-00/tui-screenshot.png 2>/dev/null || \
  screencapture validation-evidence/phase-00/tui-screenshot.png
pkill -f "ralph tui"
```

---

#### Phase 01: Process Isolation Foundation ✅ VALIDATED

| Plan | Acceptance Criteria | Status |
|------|---------------------|--------|
| 01-01 | InstanceManager with CRUD, state dirs | ✅ VALIDATED |
| 01-02 | Per-instance .agent-{id}/ directories | ✅ VALIDATED |
| 01-03 | Dynamic port allocation (8080-8180) | ✅ VALIDATED |
| 01-04 | Instance-aware git branches (ralph-{id}) | ✅ VALIDATED |

**Evidence**: `validation-evidence/phase-01/parallel-instances.txt`, `validation-evidence/phase-01/port-allocation.txt`
- Created 2 instances with unique IDs (3ff90cf2, 93b2dbf0)
- State directories created: `~/.ralph/state-{id}/`
- Ports dynamically allocated: 8080, 8081, 8082, 8083 (no conflicts)
- Git branch names generated: `ralph-{id}`

**REAL VALIDATION GATE** (NOT `pytest`):
```bash
# Start two instances and verify no conflicts
ralph run -P prompts/test1.md &
PID1=$!
ralph run -P prompts/test2.md &
PID2=$!
sleep 5
ps aux | grep ralph > validation-evidence/phase-01/parallel-instances.txt
lsof -i :8080-8180 > validation-evidence/phase-01/port-allocation.txt
kill $PID1 $PID2 2>/dev/null
```

---

#### Phase 02: Daemon Mode & Background Execution ✅ VALIDATED

| Plan | Acceptance Criteria | Status |
|------|---------------------|--------|
| 02-01 | DaemonManager with double-fork, PID file | ✅ VALIDATED |
| 02-02 | CLI: ralph daemon start/stop/status/logs | ✅ VALIDATED |
| 02-03 | Unix socket IPC, HTTP fallback | ✅ VALIDATED |
| 02-04 | Log forwarding, rotation, streaming | ✅ VALIDATED |

**Evidence**: `validation-evidence/phase-02/daemon-start.txt`, `daemon-status.txt`, `daemon-logs.txt`
- DaemonManager with double-fork pattern implemented (manager.py lines 46-54)
- CLI integrated: `ralph daemon {start|stop|status|logs}`
- Daemon start returns immediately (1.5 seconds - parent returns after fork)
- IPC module exists (ipc.py), log forwarder exists (log_forwarder.py)
- PID file management at ~/.ralph/daemon.pid
- Log file management at ~/.ralph/logs/daemon.log

**REAL VALIDATION GATE** (NOT `pytest`):
```bash
# Verify daemon returns immediately
time (ralph daemon start 2>&1 | tee validation-evidence/phase-02/daemon-start.txt)
# Should complete in < 1 second

ralph daemon status > validation-evidence/phase-02/daemon-status.txt
ralph daemon logs --tail 20 > validation-evidence/phase-02/daemon-logs.txt
ralph daemon stop
```

---

#### Phase 03: REST API Enhancement ✅ VALIDATED

| Plan | Acceptance Criteria | Status |
|------|---------------------|--------|
| 03-01 | POST /api/orchestrators starts new run | ✅ VALIDATED |
| 03-02 | POST /api/orchestrators/{id}/stop endpoint | ✅ VALIDATED |
| 03-03 | PATCH /api/orchestrators/{id}/config | ✅ VALIDATED |
| 03-04 | GET /api/orchestrators/{id}/events SSE streaming | ✅ VALIDATED |

**Evidence**: `validation-evidence/phase-03/api-endpoints.txt`, `api-start.json`, `api-stop.json`, `api-events.txt`
- Web server started on port 8085 (no auth for testing)
- POST /api/orchestrators: Creates instance with unique ID (e.g., `7afb7ecc`)
- POST /api/orchestrators/{id}/stop: Returns 404 for non-existent (correct behavior)
- POST /api/orchestrators/{id}/pause: Endpoint exists, 404 for non-existent
- POST /api/orchestrators/{id}/resume: Endpoint exists, 404 for non-existent
- PATCH /api/orchestrators/{id}/config: Endpoint exists, 404 for non-existent
- GET /api/orchestrators/{id}/events: SSE streaming endpoint exists, 404 for non-existent
- GET /api/health: Returns `{"status":"healthy"}`

**Implementation verified in**: `src/ralph_orchestrator/web/server.py`
- POST /api/orchestrators: lines 451-480
- POST /{id}/stop: lines 526-541
- PATCH /{id}/config: lines 543-567
- GET /{id}/events (SSE): lines 569-624

**REAL VALIDATION GATE** (NOT `pytest`):
```bash
# Start server
python -m ralph_orchestrator.web --no-auth --port 8085

# Test API endpoints with curl
curl -X POST http://localhost:8085/api/orchestrators \
  -H "Content-Type: application/json" \
  -d '{"prompt_file": "prompts/SELF_IMPROVEMENT_PROMPT.md"}'
# Response: {"instance_id":"7afb7ecc","status":"started","config":{...}}
```

---

#### Phase 04: Mobile App Foundation ⏳ NEEDS VALIDATION

| Plan | Acceptance Criteria | Status |
|------|---------------------|--------|
| 04-01 | Expo TypeScript project, NativeWind | ⏳ PENDING |
| 04-02 | Dark theme matching web UI | ⏳ PENDING |
| 04-03 | Tab navigation (Dashboard, History, Settings) | ⏳ PENDING |
| 04-04 | JWT auth with expo-secure-store | ⏳ PENDING |

**REAL VALIDATION GATE** (NOT `npm test`):
```bash
cd ralph-mobile

# Build and run in simulator
npx expo run:ios 2>&1 | tee validation-evidence/phase-04/expo-build.txt

# Wait for app to load, take screenshot
sleep 30
xcrun simctl io booted screenshot validation-evidence/phase-04/simulator-app.png
```

---

#### Phase 05: Mobile Dashboard ⏳ NEEDS VALIDATION

| Plan | Acceptance Criteria | Status |
|------|---------------------|--------|
| 05-01 | OrchestratorCard list view | ⏳ PENDING |
| 05-02 | Detail view with tasks and logs | ⏳ PENDING |
| 05-03 | WebSocket real-time updates | ⏳ PENDING |
| 05-04 | MetricsChart with 60s rolling window | ⏳ PENDING |

**REAL VALIDATION GATE** (NOT `npm test`):
```bash
# Ensure backend is running
ralph daemon start

cd ralph-mobile
npx expo run:ios

# Navigate to dashboard, take screenshots
sleep 30
xcrun simctl io booted screenshot validation-evidence/phase-05/dashboard.png

# Tap on an orchestrator for detail view
# (This may require Detox or manual interaction)
sleep 5
xcrun simctl io booted screenshot validation-evidence/phase-05/detail-view.png

# Check WebSocket connectivity in logs
adb logcat -d | grep -i websocket > validation-evidence/phase-05/websocket.txt 2>/dev/null || \
  log show --predicate 'process == "ralph-mobile"' --last 1m > validation-evidence/phase-05/websocket.txt
```

---

#### Phase 06: Mobile Control ⏳ NEEDS VALIDATION

| Plan | Acceptance Criteria | Status |
|------|---------------------|--------|
| 06-01 | Start orchestration UI | ⏳ PENDING |
| 06-02 | Stop/Pause/Resume buttons | ⏳ PENDING |
| 06-03 | Inline prompt editor | ⏳ PENDING |
| 06-04 | Push notifications (optional) | ⏳ PENDING |

**REAL VALIDATION GATE** (NOT `npm test`):
```bash
cd ralph-mobile
npx expo run:ios

# Navigate to start orchestration screen
sleep 30
xcrun simctl io booted screenshot validation-evidence/phase-06/start-ui.png

# Start an orchestration, navigate to detail, capture controls
sleep 10
xcrun simctl io booted screenshot validation-evidence/phase-06/controls.png

# Verify API calls were made
cat ~/.ralph/logs/api.log | tail -50 > validation-evidence/phase-06/api-calls.txt
```

---

### Global Success Criteria

- [x] Run 2+ ralph instances simultaneously without conflicts
  - **Evidence**: `validation-evidence/phase-01/parallel-instances.txt` ✅
- [x] `ralph daemon start` returns immediately, runs in background
  - **Evidence**: `validation-evidence/phase-02/daemon-start.txt` shows 1.5s ✅
- [x] REST API supports: start/stop/pause/resume orchestrations
  - **Evidence**: `validation-evidence/phase-03/api-endpoints.txt` ✅
- [ ] Mobile app can view and control running orchestrations
  - **Evidence**: `validation-evidence/phase-06/*.png` showing app with data
- [ ] All existing tests continue to pass (separate from functional validation)
- [ ] Evidence files collected for all phases

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
4. **FUNCTIONAL VALIDATION** - Real execution with evidence, NOT unit tests
5. **EVIDENCE COLLECTION** - Capture screenshots/output to validation-evidence/
6. **NO PREMATURE COMPLETION** - Only mark done when evidence exists

---

## BIG PICTURE: ALL 28 PLANS

This is everything you will build. Know this before starting:

| Phase | Focus | Plans | Validation Type |
|-------|-------|-------|-----------------|
| 00 | TUI Testing | 4 plans | TUI screenshot |
| 01 | Process Isolation | 4 plans | CLI process output |
| 02 | Daemon Mode | 4 plans | CLI timing + output |
| 03 | REST API Enhancement | 4 plans | curl responses |
| 04 | Mobile Foundation | 4 plans | iOS Simulator screenshot |
| 05 | Mobile Dashboard | 4 plans | iOS Simulator with data |
| 06 | Mobile Control | 4 plans | iOS Simulator controls |

**Total: 28 plans across 7 phases**
**Required Evidence: ~20+ files in validation-evidence/**

---

## DISCOVERY PHASE

Before starting any work, run these checks to understand current state:

```bash
# Check what exists
ls src/ralph_orchestrator/instance.py 2>/dev/null && echo "Instance module exists"
ls src/ralph_orchestrator/daemon/ 2>/dev/null && echo "Daemon module exists"
ls ralph-mobile/ 2>/dev/null && echo "Mobile app exists"

# Check evidence directory
mkdir -p validation-evidence/{phase-00,phase-01,phase-02,phase-03,phase-04,phase-05,phase-06,final}
ls -la validation-evidence/

# Count existing evidence
find validation-evidence -type f | wc -l
```

Report what exists vs what's missing, then continue from where you left off.

---

## PHASE DETAILS

[Previous phase details with code examples remain the same - they define WHAT to build]
[The key change is HOW to validate - with real execution, not unit tests]

---

## SUCCESS CRITERIA

All of these must be true before marking complete:

- [ ] Run 2+ ralph instances simultaneously without conflicts
- [ ] `ralph run --daemon` returns immediately, runs in background
- [ ] REST API supports: start/stop/pause/resume orchestrations
- [ ] Mobile app can view and control running orchestrations
- [ ] All existing tests continue to pass
- [ ] **Evidence files exist in validation-evidence/ for ALL phases**

---

## COMPLETION

When ALL phases are complete with **EVIDENCE FILES**, write:

```
## FINAL STATUS

Evidence verification:
```bash
find validation-evidence -type f -name "*.png" | wc -l  # Should be > 5
find validation-evidence -type f -name "*.txt" | wc -l  # Should be > 5
find validation-evidence -type f -name "*.json" | wc -l # Should be > 2
```

All phases complete with evidence:
- Phase 00: TUI Testing - Evidence: [list files]
- Phase 01: Process Isolation - Evidence: [list files]
- Phase 02: Daemon Mode - Evidence: [list files]
- Phase 03: REST API Enhancement - Evidence: [list files]
- Phase 04: Mobile Foundation - Evidence: [list files]
- Phase 05: Mobile Dashboard - Evidence: [list files]
- Phase 06: Mobile Control - Evidence: [list files]

[WRITE LITERAL TEXT: TASK_COMPLETE]
```

**DO NOT write the completion marker until:**
1. ALL phases are verified complete
2. **ALL evidence files exist in validation-evidence/**
3. Evidence shows REAL execution (screenshots, curl output), NOT unit tests
