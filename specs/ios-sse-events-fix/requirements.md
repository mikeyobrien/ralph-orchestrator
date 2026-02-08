---
spec: ios-sse-events-fix
phase: requirements
created: 2026-02-07T01:45:00Z
---

# Requirements: iOS SSE Event Stream Fix

## Goal
Fix SSE event rendering in iOS app by correcting session liveness detection. Events flow from Rust backend to iOS but don't appear in UI because `isSessionActive` checks nonexistent `status` field instead of API-provided `mode` field.

## User Stories

### US-1: Event Stream Displays Live Events
**As a** Ralph Mobile user monitoring a live session
**I want to** see SSE events appear in real-time in the EVENT STREAM section
**So that** I can track orchestration progress without checking logs

**Acceptance Criteria:**
- [ ] AC-1.1: Tapping a live session (`mode == "live"`) establishes SSE connection within 2 seconds
- [ ] AC-1.2: Events appear in EVENT STREAM section as they arrive (no manual refresh)
- [ ] AC-1.3: Event count badge shows correct number (not "0")
- [ ] AC-1.4: New events insert at top (newest first)
- [ ] AC-1.5: SSE connection survives 10+ minute sessions without dropping events

### US-2: Completed Sessions Show Read-Only State
**As a** user browsing historical sessions
**I want to** completed sessions to show disconnected state
**So that** I understand no new events will arrive

**Acceptance Criteria:**
- [ ] AC-2.1: Tapping completed session (`mode == "complete"`) sets `connectionState = .disconnected`
- [ ] AC-2.2: No SSE connection attempt for completed sessions
- [ ] AC-2.3: Existing events (if any) remain visible in read-only mode
- [ ] AC-2.4: Connection indicator shows "Disconnected" badge

### US-3: Local Pause/Resume State Preserved
**As a** user pausing/resuming via iOS controls
**I want to** pause/resume actions to maintain SSE connection
**So that** I don't miss events during brief pauses

**Acceptance Criteria:**
- [ ] AC-3.1: Pause button sets `session.status = "paused"` locally
- [ ] AC-3.2: `isSessionActive` returns `true` for paused sessions
- [ ] AC-3.3: SSE connection remains active during pause state
- [ ] AC-3.4: Resume button clears paused state

## Functional Requirements

| ID | Requirement | Priority | Acceptance Criteria |
|----|-------------|----------|---------------------|
| FR-1 | Modify `isSessionActive` to check `mode == "live"` from backend | P0 | Returns `true` when `currentSession.mode == "live"` |
| FR-2 | Preserve local `status` field for pause/resume | P0 | Returns `true` when `status == "paused"` even if `mode` is nil |
| FR-3 | SSE stream starts automatically for live sessions | P0 | `startEventStream()` called when `isSessionActive == true` |
| FR-4 | Events propagate to SwiftUI views | P1 | `@Published var events` mutations trigger view updates |
| FR-5 | Connection state reflects SSE lifecycle | P1 | `.connecting` → `.connected` → `.disconnected` transitions visible in UI |

## Non-Functional Requirements

| ID | Requirement | Metric | Target |
|----|-------------|--------|--------|
| NFR-1 | SSE connection latency | Time to first event | < 2 seconds |
| NFR-2 | Event rendering latency | Event arrival to UI update | < 100ms |
| NFR-3 | Session stability | No dropped events | 10-minute continuous session |
| NFR-4 | Memory safety | No retain cycles in async stream | Zero leaks in Instruments |
| NFR-5 | Build compatibility | Zero errors/warnings | iPhone 16 Pro, iOS 18.2+ |

## User Decisions

### Interview Responses

**Q: Who are the primary users?**
**A:** Both internal developers (testing Ralph loops) and external users (monitoring production orchestration runs).

**Q: Priority tradeoffs?**
**A:** Speed of delivery. Root cause is well-understood (5-line change), focus on fast validation over exhaustive edge case coverage.

**Q: Success criteria?**
**A:** Full 10-minute SSE validation with video evidence per CLAUDE.md Gate 4. Must show:
- Real ralph loop running (`ralph run -c presets/feature.yml -p "test" --max-iterations 10`)
- Events flowing into iOS app
- Hat transitions rendering correctly
- Video recording of full session

## Glossary

- **SSE (Server-Sent Events)**: HTTP streaming protocol for unidirectional server-to-client push
- **Session liveness**: Whether a ralph loop is actively running (`mode == "live"`) vs completed
- **`isSessionActive`**: Computed property that gates SSE connection establishment
- **`mode` field**: Backend-provided session state (`"live"` | `"complete"`), always present in API responses
- **`status` field**: iOS-only local state for pause/resume UI (`"running"` | `"paused"`), NOT provided by backend
- **Gate 4**: CLAUDE.md validation requirement for real loop testing with video evidence

## Out of Scope

- Adding `status` field to Rust backend API (Option B from research — unnecessary)
- Refactoring event stream architecture
- Performance optimization beyond bug fix
- UI/UX polish of event rendering
- Adding new event types or filtering
- Backend changes of any kind

## Dependencies

- **Zero backend dependencies** — fix is iOS-only
- Rust backend must be running: `cargo run --bin ralph-mobile-server -- --bind-all`
- Active ralph loop for SSE testing: `ralph run -c <config> -p <prompt> --max-iterations 10`
- Simulator: iPhone 17 Pro Max (UDID: `23859335-3786-4AB4-BE26-9EC0BD8D0B57`)

## Success Criteria

- [ ] Build succeeds with zero errors: `xcodebuild -scheme RalphMobile -destination 'id=23859335-3786-4AB4-BE26-9EC0BD8D0B57' build`
- [ ] Backend tests pass: `cargo test -p ralph-mobile-server`
- [ ] EVENT STREAM section shows events > 0 for live sessions
- [ ] 10-minute video evidence shows continuous event streaming
- [ ] Hat transitions visible in video (minimum 3 different hats)
- [ ] No SSE connection attempts for completed sessions
- [ ] `connectionState` transitions observable in UI

## Unresolved Questions

- After fixing `isSessionActive`, will the existing `objectWillChange.send()` workaround (line 189) be sufficient for SwiftUI reactivity, or will additional view refresh triggers be needed? (Empirical validation required.)
- Should `isSessionActive` also handle future backend states like `"initializing"` or `"shutting_down"`? (Defer to future spec if backend adds these states.)

## Next Steps

1. Implement `isSessionActive` fix in `SessionViewModel.swift` (lines 440-443)
2. Build iOS app and verify zero errors
3. Start ralph-mobile-server with `--bind-all` flag
4. Configure iOS Settings with LAN IP (`http://10.128.252.91:8080`)
5. Start real ralph loop: `ralph run -c presets/feature.yml -p "test prompt" --max-iterations 10`
6. Tap session in iOS app, verify SSE connection establishes
7. Record 10-minute video showing continuous event streaming
8. Verify hat transitions render correctly
9. Save video to `ios/validation-screenshots/sse-fix-validation.mp4`
10. Mark spec complete with evidence
