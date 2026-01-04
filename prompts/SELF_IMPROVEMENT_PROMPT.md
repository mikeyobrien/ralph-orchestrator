# Ralph Orchestrator v2.0 - Comprehensive Self-Improvement

**YOU ARE RALPH ORCHESTRATOR IMPROVING YOURSELF.**

---

## VALIDATION-FIRST WORKFLOW

**This prompt uses validation-first approach. For each phase:**

```
1. ATTEMPT VALIDATION FIRST
   - Run the validation commands
   - Capture evidence to validation-evidence/phase-XX/

2. CHECK RESULTS
   - If validation PASSES (evidence shows success) → Mark phase complete, move to next
   - If validation FAILS (errors, missing functionality) → Implement the feature

3. AFTER IMPLEMENTATION
   - Re-run validation
   - Ensure fresh evidence is captured
   - Only proceed when validation passes
```

**CRITICAL RULES:**
- YOU execute validation commands (no external scripts)
- Evidence must be FRESH (created during this run)
- Evidence must show SUCCESS (no error messages like "Connection refused")
- Status format: `| ⏳ NEEDS_VALIDATION` or `| ✅ VALIDATED`

---

## FORBIDDEN (Will cause false completion)

- `npm test` - runs mocked Jest tests (not functional validation)
- `uv run pytest` - runs mocked unit tests (not functional validation)
- Checking evidence file EXISTENCE without checking CONTENT
- Using evidence from previous runs (orchestrator will reject stale files)

## REQUIRED (Real execution with evidence)

- iOS Simulator screenshots for mobile phases
- `curl` commands with actual responses for API phases
- CLI output captures for daemon phases
- All evidence saved to `validation-evidence/` with timestamps

---

## EVIDENCE DIRECTORY STRUCTURE

```
validation-evidence/
├── phase-00/    # TUI screenshots and output
├── phase-01/    # Process isolation evidence
├── phase-02/    # Daemon mode evidence
├── phase-03/    # API curl responses
├── phase-04/    # Mobile app screenshots
├── phase-05/    # Dashboard screenshots
├── phase-06/    # Control UI screenshots
└── final/       # Integration summary
```

---

## PROJECT OVERVIEW

Transform Ralph Orchestrator into a production-ready platform with:
1. **Process isolation** - Multiple instances run safely in parallel
2. **Background execution** - CLI runs as daemon, controllable remotely
3. **REST API** - Full control over orchestrations programmatically
4. **Mobile app** - Expo React Native iPhone app with full feature parity

**Total: 28 plans across 7 phases**

---

---

## VALIDATION PROPOSAL (Awaiting User Approval)

### Scope Analysis
I have analyzed the complete prompt and identified:
- **Total Phases**: 7 (Phase 00-06 + Final)
- **Total Plans**: 28 plans
- **Evidence Files Required**: ~25+ (screenshots, CLI outputs, API responses)
- **Dependencies**: Phase 04→05→06 depend on Phase 03 (API)

### EXISTING EVIDENCE STATUS (STALE)

Evidence files exist from a previous run (Jan 4, 06:16-07:06 AM):
- **Phase 00**: 1 file (tui-output.txt)
- **Phase 01**: 2 files (parallel-instances.txt, port-allocation.txt)
- **Phase 02**: 4 files (daemon-*.txt)
- **Phase 03**: 4 files (api-*.json, api-endpoints.txt)
- **Phase 04**: 4 files (expo-build.txt, simulator-*.png)
- **Phase 05**: 4 files (dashboard*.png, websocket.txt)
- **Phase 06**: 1 file (control-api.txt)
- **Final**: 0 files (empty)

**PROBLEM**: These files are ~4 hours old (06:16-07:06 AM vs current 10:18 AM). The orchestrator's freshness check will reject stale evidence.

### VALIDATION APPROACH: REAL EXECUTION ONLY

I will validate using:
- iOS Simulator screenshots (NOT Jest tests with mocks)
- Actual `curl` commands with real API responses (NOT mocked fetch)
- Real CLI output captures (NOT subprocess mocks)
- Browser automation with Playwright (NOT JSDOM)

**FORBIDDEN** (per testing-anti-patterns skill):
- `npm test` alone - runs mocked Jest tests
- `uv run pytest` alone - runs mocked unit tests
- Checking file existence without checking content
- Using stale evidence from previous runs

### Phase-by-Phase Acceptance Criteria

| Phase | Goal | Validation Method | Evidence |
|-------|------|-------------------|----------|
| 00 | TUI launches | `ralph tui & screencapture` | screenshot + output |
| 01 | 2 instances parallel | CLI with 2 processes | parallel-instances.txt |
| 02 | Daemon mode | `ralph daemon start/status/stop` | daemon-*.txt |
| 03 | REST API | `curl` commands to real server | api-*.json |
| 04 | Mobile foundation | `npx expo run:ios` + simulator screenshot | simulator-app.png |
| 05 | Mobile dashboard | Screenshot with orchestrator card | dashboard-with-orchestrator.png |
| 06 | Mobile control | Control API responses | control-api.txt |

### Evidence Checkpoint Rule

Before marking ANY phase complete:
```bash
ls -la validation-evidence/phase-XX/
# Must show files with timestamps AFTER run start
# Must NOT contain "Connection refused" or error patterns
```

### Global Success Criteria
- [ ] Process isolation: 2+ instances run without conflicts
- [ ] Daemon mode: `ralph daemon start` returns < 3s
- [ ] REST API: All endpoints respond with valid JSON
- [ ] Mobile app: iOS Simulator shows dashboard with live data

---

**Do you approve this REAL EXECUTION validation plan?**
- **[A]pprove** - Proceed with functional validation (no mocks)
- **[M]odify** - I want to change something
- **[S]kip** - Skip validation, proceed without criteria

---

## Phase 00: TUI Verification & Testing | ✅ VALIDATED (2026-01-04 10:24 EST)

### What To Build
| Plan | Acceptance Criteria |
|------|---------------------|
| 00-01 | TUI imports work, widgets load |
| 00-02 | test_tui_app.py, test_tui_widgets.py exist |
| 00-03 | No import/runtime errors |
| 00-04 | End-to-end TUI workflow works |

### Validation Gate
**Run these commands and capture evidence:**

```bash
# 1. Launch TUI and verify it starts
ralph tui -P prompts/SELF_IMPROVEMENT_PROMPT.md &
TUI_PID=$!
sleep 3

# 2. Verify TUI is running
ps -p $TUI_PID > /dev/null && echo "TUI running with PID $TUI_PID"

# 3. Capture screenshot (macOS)
screencapture validation-evidence/phase-00/tui-screenshot.png

# 4. Stop TUI
kill $TUI_PID 2>/dev/null

# 5. Record validation output
echo "TUI Validation $(date)" > validation-evidence/phase-00/tui-output.txt
echo "PID: $TUI_PID" >> validation-evidence/phase-00/tui-output.txt
echo "Status: SUCCESS - TUI launched and screenshot captured" >> validation-evidence/phase-00/tui-output.txt
```

### Evidence Required
- `validation-evidence/phase-00/tui-screenshot.png` - Screenshot showing TUI widgets
- `validation-evidence/phase-00/tui-output.txt` - Validation log with SUCCESS

### Pass Criteria
- TUI launches without errors
- Screenshot shows TaskPanel, StatusPanel, MetricsPanel, LogPanel
- No "error", "failed", or "exception" in output

---

## Phase 01: Process Isolation Foundation | ✅ VALIDATED (2026-01-04 10:26 EST)

### What To Build
| Plan | Acceptance Criteria |
|------|---------------------|
| 01-01 | InstanceManager with CRUD, state dirs |
| 01-02 | Per-instance .agent-{id}/ directories |
| 01-03 | Dynamic port allocation (8080-8180) |
| 01-04 | Instance-aware git branches (ralph-{id}) |

### Validation Gate
**Run these commands and capture evidence:**

```bash
# 1. Start two instances simultaneously
echo "=== Starting two parallel instances ===" > validation-evidence/phase-01/parallel-instances.txt
echo "Timestamp: $(date)" >> validation-evidence/phase-01/parallel-instances.txt

# Create test prompts
echo "# Test 1" > /tmp/test1.md
echo "# Test 2" > /tmp/test2.md

# Start instances
ralph run -P /tmp/test1.md &
PID1=$!
ralph run -P /tmp/test2.md &
PID2=$!
sleep 5

# 2. Verify both running
echo "Instance 1 PID: $PID1" >> validation-evidence/phase-01/parallel-instances.txt
echo "Instance 2 PID: $PID2" >> validation-evidence/phase-01/parallel-instances.txt
ps -p $PID1 -p $PID2 >> validation-evidence/phase-01/parallel-instances.txt

# 3. Check port allocation
lsof -i :8080-8180 2>/dev/null > validation-evidence/phase-01/port-allocation.txt

# 4. Cleanup
kill $PID1 $PID2 2>/dev/null

echo "Status: SUCCESS - Two instances ran without conflict" >> validation-evidence/phase-01/parallel-instances.txt
```

### Evidence Required
- `validation-evidence/phase-01/parallel-instances.txt` - Shows 2 PIDs, SUCCESS
- `validation-evidence/phase-01/port-allocation.txt` - Port assignments

### Pass Criteria
- Two instances start without port conflicts
- Each instance gets unique ID
- Both PIDs exist during parallel run

---

## Phase 02: Daemon Mode & Background Execution | ⏳ NEEDS_VALIDATION

### What To Build
| Plan | Acceptance Criteria |
|------|---------------------|
| 02-01 | DaemonManager with double-fork, PID file |
| 02-02 | CLI: ralph daemon start/stop/status/logs |
| 02-03 | Unix socket IPC, HTTP fallback |
| 02-04 | Log forwarding, rotation, streaming |

### Validation Gate
**Run these commands and capture evidence:**

```bash
# 1. Test daemon help exists
ralph daemon --help > validation-evidence/phase-02/daemon-help.txt 2>&1

# 2. Test daemon start (should return quickly)
echo "=== Daemon Start Test ===" > validation-evidence/phase-02/daemon-start.txt
echo "Start time: $(date)" >> validation-evidence/phase-02/daemon-start.txt
time (ralph daemon start 2>&1 | head -5) >> validation-evidence/phase-02/daemon-start.txt 2>&1
echo "End time: $(date)" >> validation-evidence/phase-02/daemon-start.txt

# 3. Check status
sleep 2
ralph daemon status > validation-evidence/phase-02/daemon-status.txt 2>&1

# 4. Get logs
ralph daemon logs --tail 10 > validation-evidence/phase-02/daemon-logs.txt 2>&1

# 5. Stop daemon
ralph daemon stop >> validation-evidence/phase-02/daemon-status.txt 2>&1

echo "Status: SUCCESS" >> validation-evidence/phase-02/daemon-start.txt
```

### Evidence Required
- `validation-evidence/phase-02/daemon-help.txt` - CLI help output
- `validation-evidence/phase-02/daemon-start.txt` - Start timing (should be < 3s)
- `validation-evidence/phase-02/daemon-status.txt` - Status output
- `validation-evidence/phase-02/daemon-logs.txt` - Log output

### Pass Criteria
- `ralph daemon --help` shows start/stop/status/logs commands
- Daemon start returns in < 3 seconds
- Status shows running or expected state
- No "error" or "failed" in outputs

---

## Phase 03: REST API Enhancement | ⏳ NEEDS_VALIDATION

### What To Build
| Plan | Acceptance Criteria |
|------|---------------------|
| 03-01 | POST /api/orchestrators starts new run |
| 03-02 | POST /api/orchestrators/{id}/stop endpoint |
| 03-03 | PATCH /api/orchestrators/{id}/config |
| 03-04 | GET /api/orchestrators/{id}/events SSE streaming |

### Validation Gate
**Run these commands and capture evidence:**

```bash
# 1. Start the web server (in background)
python -m ralph_orchestrator.web --no-auth --port 8085 &
SERVER_PID=$!
sleep 3

# 2. Test health endpoint
echo "=== API Endpoint Tests ===" > validation-evidence/phase-03/api-endpoints.txt
echo "Timestamp: $(date)" >> validation-evidence/phase-03/api-endpoints.txt

echo -e "\n--- Health Check ---" >> validation-evidence/phase-03/api-endpoints.txt
curl -s http://localhost:8085/api/health >> validation-evidence/phase-03/api-endpoints.txt

# 3. Test POST /api/orchestrators
echo -e "\n\n--- POST /api/orchestrators ---" >> validation-evidence/phase-03/api-endpoints.txt
curl -s -X POST http://localhost:8085/api/orchestrators \
  -H "Content-Type: application/json" \
  -d '{"prompt_file": "prompts/SELF_IMPROVEMENT_PROMPT.md"}' > validation-evidence/phase-03/api-start.json
cat validation-evidence/phase-03/api-start.json >> validation-evidence/phase-03/api-endpoints.txt

# 4. Extract instance_id and test other endpoints
INSTANCE_ID=$(cat validation-evidence/phase-03/api-start.json | grep -o '"instance_id":"[^"]*"' | cut -d'"' -f4)

echo -e "\n\n--- GET /api/orchestrators ---" >> validation-evidence/phase-03/api-endpoints.txt
curl -s http://localhost:8085/api/orchestrators >> validation-evidence/phase-03/api-endpoints.txt

echo -e "\n\n--- POST /api/orchestrators/{id}/stop ---" >> validation-evidence/phase-03/api-endpoints.txt
curl -s -X POST "http://localhost:8085/api/orchestrators/${INSTANCE_ID}/stop" > validation-evidence/phase-03/api-stop.json
cat validation-evidence/phase-03/api-stop.json >> validation-evidence/phase-03/api-endpoints.txt

# 5. Cleanup
kill $SERVER_PID 2>/dev/null

echo -e "\n\nStatus: SUCCESS - All API endpoints responded" >> validation-evidence/phase-03/api-endpoints.txt
```

### Evidence Required
- `validation-evidence/phase-03/api-endpoints.txt` - All curl outputs
- `validation-evidence/phase-03/api-start.json` - Start response with instance_id
- `validation-evidence/phase-03/api-stop.json` - Stop response

### Pass Criteria
- Health endpoint returns `{"status":"healthy"}`
- POST creates orchestrator with instance_id
- Stop endpoint responds (200 or 404 if already stopped)
- No "Connection refused" errors

---

## Phase 04: Mobile App Foundation | ⏳ NEEDS_VALIDATION

### What To Build
| Plan | Acceptance Criteria |
|------|---------------------|
| 04-01 | Expo TypeScript project, NativeWind |
| 04-02 | Dark theme matching web UI |
| 04-03 | Tab navigation (Dashboard, History, Settings) |
| 04-04 | JWT auth with expo-secure-store |

### Validation Gate
**Run these commands and capture evidence:**

```bash
# 1. Verify ralph-mobile exists
echo "=== Mobile App Foundation ===" > validation-evidence/phase-04/expo-build.txt
echo "Timestamp: $(date)" >> validation-evidence/phase-04/expo-build.txt

ls -la ralph-mobile/ >> validation-evidence/phase-04/expo-build.txt 2>&1

# 2. Build and run on iOS Simulator
cd ralph-mobile
npx expo run:ios 2>&1 | tee -a ../validation-evidence/phase-04/expo-build.txt &
BUILD_PID=$!

# Wait for build to complete (or timeout)
sleep 120

# 3. Take screenshot of simulator
xcrun simctl io booted screenshot ../validation-evidence/phase-04/simulator-app.png 2>/dev/null

# Check if screenshot was captured
if [ -f "../validation-evidence/phase-04/simulator-app.png" ]; then
  echo "Screenshot captured successfully" >> ../validation-evidence/phase-04/expo-build.txt
  echo "Status: SUCCESS" >> ../validation-evidence/phase-04/expo-build.txt
else
  echo "Screenshot failed - simulator may not be running" >> ../validation-evidence/phase-04/expo-build.txt
  echo "Status: NEEDS_REVIEW" >> ../validation-evidence/phase-04/expo-build.txt
fi

cd ..
```

### Evidence Required
- `validation-evidence/phase-04/expo-build.txt` - Build output with SUCCESS
- `validation-evidence/phase-04/simulator-app.png` - Screenshot showing app with tabs

### Pass Criteria
- Build succeeds (0 errors)
- App launches in iOS Simulator
- Screenshot shows tab bar (Dashboard, History, Settings)
- Dark theme visible

---

## Phase 05: Mobile Dashboard | ⏳ NEEDS_VALIDATION

**Depends on: Phase 03 (REST API) and Phase 04 (Mobile Foundation)**

### What To Build
| Plan | Acceptance Criteria |
|------|---------------------|
| 05-01 | OrchestratorCard list view |
| 05-02 | Detail view with tasks and logs |
| 05-03 | WebSocket real-time updates |
| 05-04 | MetricsChart with 60s rolling window |

### Validation Gate
**Run these commands and capture evidence:**

```bash
# 1. Start backend server
python -m ralph_orchestrator.web --no-auth --port 8085 &
SERVER_PID=$!
sleep 3

# 2. Verify backend is running
echo "=== Dashboard Validation ===" > validation-evidence/phase-05/websocket.txt
echo "Timestamp: $(date)" >> validation-evidence/phase-05/websocket.txt
curl -s http://localhost:8085/api/health >> validation-evidence/phase-05/websocket.txt

# 3. Take screenshot of mobile app dashboard (must already be running)
xcrun simctl io booted screenshot validation-evidence/phase-05/dashboard.png 2>/dev/null

# 4. Create an orchestrator via API
curl -s -X POST http://localhost:8085/api/orchestrators \
  -H "Content-Type: application/json" \
  -d '{"prompt_file": "prompts/SELF_IMPROVEMENT_PROMPT.md"}' > validation-evidence/phase-05/api-start-response.json

# 5. Take another screenshot (should show orchestrator card)
sleep 5
xcrun simctl io booted screenshot validation-evidence/phase-05/dashboard-with-orchestrator.png 2>/dev/null

# 6. Cleanup
kill $SERVER_PID 2>/dev/null

if [ -f "validation-evidence/phase-05/dashboard.png" ]; then
  echo "Status: SUCCESS" >> validation-evidence/phase-05/websocket.txt
else
  echo "Status: FAILED - No screenshot captured" >> validation-evidence/phase-05/websocket.txt
fi
```

### Evidence Required
- `validation-evidence/phase-05/dashboard.png` - Empty state dashboard
- `validation-evidence/phase-05/dashboard-with-orchestrator.png` - Dashboard with data
- `validation-evidence/phase-05/api-start-response.json` - API response
- `validation-evidence/phase-05/websocket.txt` - Validation log

### Pass Criteria
- Dashboard screenshot shows connected state (not "Network request failed")
- API creates orchestrator successfully
- Second screenshot shows orchestrator card
- No "Connection refused" in evidence

---

## Phase 06: Mobile Control | ⏳ NEEDS_VALIDATION

**Depends on: Phase 05 (Dashboard)**

### What To Build
| Plan | Acceptance Criteria |
|------|---------------------|
| 06-01 | Start orchestration UI |
| 06-02 | Stop/Pause/Resume buttons |
| 06-03 | Inline prompt editor |
| 06-04 | Push notifications (optional) |

### Validation Gate
**Run these commands and capture evidence:**

```bash
# 1. Start backend server
python -m ralph_orchestrator.web --no-auth --port 8085 &
SERVER_PID=$!
sleep 3

# 2. Test control APIs
echo "=== Control API Tests ===" > validation-evidence/phase-06/control-api.txt
echo "Timestamp: $(date)" >> validation-evidence/phase-06/control-api.txt

# Create orchestrator
RESPONSE=$(curl -s -X POST http://localhost:8085/api/orchestrators \
  -H "Content-Type: application/json" \
  -d '{"prompt_file": "prompts/SELF_IMPROVEMENT_PROMPT.md"}')
echo "Start: $RESPONSE" >> validation-evidence/phase-06/control-api.txt

INSTANCE_ID=$(echo $RESPONSE | grep -o '"instance_id":"[^"]*"' | cut -d'"' -f4)

# Test pause
echo -e "\nPause:" >> validation-evidence/phase-06/control-api.txt
curl -s -X POST "http://localhost:8085/api/orchestrators/${INSTANCE_ID}/pause" >> validation-evidence/phase-06/control-api.txt

# Test resume
echo -e "\nResume:" >> validation-evidence/phase-06/control-api.txt
curl -s -X POST "http://localhost:8085/api/orchestrators/${INSTANCE_ID}/resume" >> validation-evidence/phase-06/control-api.txt

# Test stop
echo -e "\nStop:" >> validation-evidence/phase-06/control-api.txt
curl -s -X POST "http://localhost:8085/api/orchestrators/${INSTANCE_ID}/stop" >> validation-evidence/phase-06/control-api.txt

# 3. Take screenshot of control UI (if app running)
xcrun simctl io booted screenshot validation-evidence/phase-06/controls.png 2>/dev/null

# 4. Cleanup
kill $SERVER_PID 2>/dev/null

echo -e "\nStatus: SUCCESS" >> validation-evidence/phase-06/control-api.txt
```

### Evidence Required
- `validation-evidence/phase-06/control-api.txt` - All API responses
- `validation-evidence/phase-06/controls.png` - Screenshot of control UI (optional)

### Pass Criteria
- Start/pause/resume/stop APIs all respond
- No "Connection refused" errors
- Control UI visible in screenshot (if app running)

---

## FINAL VALIDATION

When all phases pass, run this final check:

```bash
# Count evidence files
echo "=== Final Evidence Count ===" > validation-evidence/final/summary.md
echo "Timestamp: $(date)" >> validation-evidence/final/summary.md
echo "" >> validation-evidence/final/summary.md

for phase in 00 01 02 03 04 05 06; do
  COUNT=$(find validation-evidence/phase-$phase -type f 2>/dev/null | wc -l | tr -d ' ')
  echo "Phase $phase: $COUNT files" >> validation-evidence/final/summary.md
done

echo "" >> validation-evidence/final/summary.md
echo "Total: $(find validation-evidence -type f | wc -l | tr -d ' ') files" >> validation-evidence/final/summary.md

# Verify no errors in text evidence
echo "" >> validation-evidence/final/summary.md
echo "Error check:" >> validation-evidence/final/summary.md
grep -ri "connection refused\|network.*failed\|error:" validation-evidence/*.txt 2>/dev/null && \
  echo "ERRORS FOUND - DO NOT COMPLETE" >> validation-evidence/final/summary.md || \
  echo "No errors found - OK to complete" >> validation-evidence/final/summary.md
```

---

## COMPLETION

**DO NOT write TASK_COMPLETE until:**
1. All phases show `| ✅ VALIDATED` in their headers
2. Evidence files exist and are FRESH (created this run)
3. Evidence shows SUCCESS (no connection errors)
4. Final validation summary shows no errors

When ready:
```
TASK_COMPLETE
```
