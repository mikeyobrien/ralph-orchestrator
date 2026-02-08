# Ralph v2.0 Roadmap

## Milestone: v2.0 - Production-Ready Orchestration Platform

### Phase 00: TUI Verification & Testing
**Goal**: Verify existing TUI code works and add tests

| Plan | Focus | Status |
|------|-------|--------|
| 00-01 | Verify TUI imports and launches | pending |
| 00-02 | Create TUI test suite | pending |
| 00-03 | Fix any TUI issues found | pending |
| 00-04 | End-to-end TUI test | pending |

**Verification**: TUI launches, tests pass (85%+ coverage)

---

### Phase 01: Process Isolation Foundation
**Goal**: Enable multiple ralph instances to run safely in parallel

| Plan | Focus | Status |
|------|-------|--------|
| 01-01 | Instance ID system (UUID-based identification) | pending |
| 01-02 | Per-instance state directories (`.agent-{id}/`) | pending |
| 01-03 | Dynamic port allocation for web monitor | pending |
| 01-04 | Instance-aware git branching | pending |

**Verification**: Run 2 instances simultaneously, both complete successfully

---

### Phase 02: Daemon Mode & Background Execution
**Goal**: CLI runs as background daemon, returns immediately

| Plan | Focus | Status |
|------|-------|--------|
| 02-01 | Process manager (`ralph daemon start/stop/status`) | pending |
| 02-02 | PID file management and cleanup | pending |
| 02-03 | IPC mechanism (Unix socket or HTTP) | pending |
| 02-04 | Log forwarding and persistence | pending |

**Verification**: `ralph run --daemon` returns immediately, orchestration continues

---

### Phase 03: REST API Enhancement
**Goal**: Full programmatic control over orchestrations

| Plan | Focus | Status |
|------|-------|--------|
| 03-01 | Start orchestration endpoint (`POST /api/orchestrators`) | pending |
| 03-02 | Stop/pause/resume endpoints | pending |
| 03-03 | Configuration API (update limits on-the-fly) | pending |
| 03-04 | Event streaming (SSE for real-time updates) | pending |

**Verification**: Start, control, and monitor orchestration entirely via API

---

### Phase 04: Mobile App - Foundation
**Goal**: Expo React Native project with core navigation and auth

| Plan | Focus | Status |
|------|-------|--------|
| 04-01 | Expo project initialization (TypeScript, NativeWind) | pending |
| 04-02 | Dark theme setup (matches Web UI) | pending |
| 04-03 | Navigation structure (tab-based) | pending |
| 04-04 | Authentication flow (JWT from REST API) | pending |

**Verification**: App builds, connects to REST API, authenticates

---

### Phase 05: Mobile App - Dashboard
**Goal**: View orchestration status and metrics on mobile

| Plan | Focus | Status |
|------|-------|--------|
| 05-01 | Orchestrator list view (active, history) | pending |
| 05-02 | Orchestrator detail view (progress, logs) | pending |
| 05-03 | Real-time updates (WebSocket or polling) | pending |
| 05-04 | System metrics display (CPU, memory charts) | pending |

**Verification**: View all orchestration data matching Web UI

---

### Phase 06: Mobile App - Control
**Goal**: Start, stop, and configure orchestrations from mobile

| Plan | Focus | Status |
|------|-------|--------|
| 06-01 | Start orchestration (select prompt, configure limits) | pending |
| 06-02 | Stop/pause/resume controls | pending |
| 06-03 | Edit prompt (inline text editor) | pending |
| 06-04 | Push notifications (optional, for completion) | pending |

**Verification**: Complete orchestration workflow from mobile

---

## Phase Dependencies

```
Phase 00 (TUI) - Independent, can run first
    ↓
Phase 01 (Isolation) - Required for daemon
    ↓
Phase 02 (Daemon) - Required for API
    ↓
Phase 03 (REST API) ←──────┐
    ↓                      │
Phase 04 (Mobile Foundation)
    ↓
Phase 05 (Mobile Dashboard)
    ↓
Phase 06 (Mobile Control)
```

## Estimated Scope

| Phase | Plans | Complexity |
|-------|-------|------------|
| 00 | 4 | Low (verification) |
| 01 | 4 | Medium (core changes) |
| 02 | 4 | High (process management) |
| 03 | 4 | Medium (extend existing) |
| 04 | 4 | Low (new project setup) |
| 05 | 4 | Medium (UI development) |
| 06 | 4 | Medium (control logic) |

**Total**: 28 plans across 7 phases

## Current State Summary

| Feature | Code | Tests | Status |
|---------|------|-------|--------|
| Onboarding | ✅ | ✅ 171 | COMPLETE |
| Web UI | ✅ | ✅ 73 | COMPLETE |
| Validation | ✅ | ✅ | COMPLETE |
| TUI | ✅ | ❌ 0 | CODE EXISTS, NEEDS TESTS |
| Process Isolation | ❌ | ❌ | NOT STARTED |
| Daemon Mode | ❌ | ❌ | NOT STARTED |
| REST API Control | ❌ | ❌ | NOT STARTED |
| Mobile App | ❌ | ❌ | NOT STARTED |
