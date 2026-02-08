# Ralph Orchestrator v2.0 - Comprehensive Improvement Brief

## Vision

Transform Ralph Orchestrator into a production-ready, self-contained orchestration platform with:
1. **Process isolation** - Multiple instances run safely in parallel
2. **Background execution** - CLI runs as daemon, controllable remotely
3. **REST API** - Full control over orchestrations programmatically
4. **Mobile app** - Expo React Native iPhone app with full feature parity

## Current State

### What Works
- Core orchestration loop (`orchestrator.py`)
- Multiple agent adapters (Claude, Gemini, Q, ACP)
- Web monitoring dashboard (FastAPI + WebSocket)
- TUI interface (Textual)
- Onboarding feature (`ralph onboard`) - **COMPLETE**
- Completion detection - **FIXED** (now supports multiple formats)

### Known Issues

| Issue | Impact | Root Cause |
|-------|--------|------------|
| Port collisions | Can't run 2 instances on same machine | Hardcoded default ports |
| State directory conflicts | `.agent/` shared between instances | Single global state path |
| Git checkpoint conflicts | Both instances commit to same branch | No instance-aware branching |
| Web monitor singleton | Only one monitor per process | Global `_web_monitor` variable |

### Architecture Gaps
- No instance ID/isolation mechanism
- No daemon mode (foreground only)
- REST API exists for monitoring but not control
- No mobile client

## Success Criteria

- [ ] Run 2+ ralph instances simultaneously without conflicts
- [ ] `ralph run --daemon` returns immediately, runs in background
- [ ] REST API supports: start/stop/pause/resume orchestrations
- [ ] Mobile app can view and control running orchestrations
- [ ] All existing tests continue to pass

## Tech Stack

| Layer | Technology | Purpose |
|-------|------------|---------|
| CLI | Python Click/Typer | Background process management |
| IPC | Unix sockets/HTTP | Daemon communication |
| REST API | FastAPI (existing) | Remote control |
| Auth | JWT (existing) | API security |
| Mobile | Expo + React Native | iOS client |
| UI | NativeWind (Tailwind) | Dark theme styling |

## Constraints

- **Backward compatible**: Existing `ralph run` behavior preserved
- **Self-contained**: No external services required (Redis, etc.)
- **Cross-platform**: Works on macOS, Linux, Windows (WSL)
- **Resource efficient**: Daemon mode uses minimal memory/CPU when idle

## Out of Scope (v2.0)

- Android app (iOS first)
- Cloud deployment features
- Multi-user authentication
- Distributed orchestration across machines
