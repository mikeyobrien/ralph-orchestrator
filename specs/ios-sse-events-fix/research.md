---
spec: ios-sse-events-fix
phase: research
created: 2026-02-07T01:40:00Z
---

# Research: ios-sse-events-fix

## Executive Summary

The EVENT STREAM showing "0" events is caused by **two distinct bugs working in concert**. Bug #1 (PRIMARY, P0): The SSE stream is never started because `isSessionActive` always returns `false` -- the Rust backend's `SessionStatus` response has no `status` field, so iOS `Session.status` is always `nil`, failing the `== "running"` check. Bug #2 (SECONDARY, P1): Even if SSE were started, `@Published var events` mutations inside an async `for try await` loop from a `Task` block on `@MainActor` may not reliably trigger SwiftUI view updates due to run loop scheduling.

## Root Cause Analysis

### Bug #1: SSE Stream Never Starts (P0 -- PRIMARY CAUSE)

**Data flow trace:**

1. User taps session in sidebar
2. `SessionViewModel.connect(to:)` is called (line 137)
3. `fetchSessionStatus(id:)` calls `GET /api/sessions/{id}/status` (line 149)
4. Rust backend returns `SessionStatus` JSON:
   ```json
   {"id":"abc123","iteration":3,"total":null,"hat":"builder","elapsed_secs":42,"mode":"live"}
   ```
5. iOS decodes into `Session` model. The `Session.status` field maps to JSON key `"status"` -- **which does not exist in the response**
6. `Session.status` remains `nil`
7. `isSessionActive` (line 440-443) checks:
   ```swift
   guard let status = currentSession?.status else { return false }
   return status == "running" || status == "paused"
   ```
8. `status` is `nil`, guard fails, returns `false`
9. In `connect(to:)` line 151: `if isSessionActive { startEventStream(...) } else { connectionState = .disconnected }`
10. **SSE stream is NEVER started. Connection state is set to `.disconnected`.**

**Evidence:**

| Component | Field | Value |
|-----------|-------|-------|
| Rust `SessionStatus` struct (`sessions.rs:25-32`) | Fields | `id, iteration, total, hat, elapsed_secs, mode` |
| Rust `SessionStatus` struct | `status` field | **DOES NOT EXIST** |
| iOS `Session` model (`Session.swift:13`) | `var status: String?` | Decoded as `nil` (key absent in JSON) |
| iOS `isSessionActive` (`SessionViewModel.swift:440-443`) | Logic | `status == "running" \|\| status == "paused"` |
| iOS `connect(to:)` (`SessionViewModel.swift:151-156`) | Gate | `if isSessionActive { startSSE } else { disconnect }` |

**Conclusion:** The `status` field is an iOS-only concept that was never added to the Rust backend API. The backend uses `mode: "live" | "complete"` instead, but the iOS app checks `status` (expected values: "running", "paused", "stopped", "idle").

### Bug #2: Async Stream May Not Trigger SwiftUI Updates (P1 -- SECONDARY)

Even if the SSE stream were started, there is a potential reactivity issue:

- `SessionViewModel` is `@MainActor`
- `startEventStream` creates a `Task` (inherits `@MainActor`)
- Inside the task: `for try await event in stream { self.events.insert(event, at: 0) }`
- The code already has a manual `objectWillChange.send()` before mutation (line 189), which was added as a "CRITICAL FIX" comment, suggesting this issue was previously encountered
- Per [Apple Developer Forums](https://developer.apple.com/forums/thread/682675), `@Published` mutations in tight async loops can coalesce updates, causing the view to not reflect intermediate states
- The existing `objectWillChange.send()` call should mitigate this, but needs validation once Bug #1 is fixed

**Risk:** LOW after Bug #1 fix. The manual `objectWillChange.send()` at line 189 likely handles this. Needs real-device verification.

### Bug #3: SSE Event Type Name (NON-ISSUE)

- Backend sends: `event: workflow\ndata: {...}\n\n`
- `SSEParser` stores event type but `decodeEvent()` only uses the `data` field
- Event type name is irrelevant for parsing -- **NOT a bug**

### Bug #4: JSON Field Mapping (NON-ISSUE)

- Backend sends: `{"topic":"...","ts":"...","payload":"..."}`
- iOS `Event.CodingKeys`: `timestamp = "ts"`, `topic`, `payload`
- All other fields are optional with defaults
- Custom decoder handles missing fields gracefully -- **NOT a bug**

## Codebase Analysis

### Data Flow Architecture

```
Rust Backend                          iOS Client
-----------                          ----------
events.jsonl
    |
EventWatcher (notify crate)
    |
broadcast::Sender<Event>
    |
SSE endpoint (events.rs)
event: workflow
data: {"topic":"x","ts":"y"}         EventStreamService.connect()
    |                                    |
    +---(HTTP SSE)----->             URLSession.bytes.lines
                                         |
                                     SSEParser.parse(line:)
                                         |
                                     SSEParser.decodeEvent()
                                         |
                                     continuation.yield(event)
                                         |
                              [BLOCKED]  SessionViewModel.connect()
                              [HERE]         if isSessionActive {  // <-- ALWAYS FALSE
                                                 startEventStream()
                                             } else {
                                                 .disconnected  // <-- ALWAYS TAKES THIS PATH
                                             }
```

### Existing Patterns

- `Session.mode` (`"live"` | `"complete"`) is the correct backend field for determining session liveness
- `isSessionActive` should check `mode == "live"` instead of `status == "running"`
- The `status` field is set manually in `pauseRun()` (line 310) and `resumeRun()` (line 321) but never populated from API responses

### Dependencies

- `ralph_core::Event` struct: `{topic: String, payload: Option<String>, ts: String}` -- 3 fields only
- `SessionBroadcast` uses `tokio::sync::broadcast` with capacity 100
- iOS `EventStreamService` is an `actor` (thread-safe)
- `URLSession.bytes.lines` strips empty lines (SSEParser handles this with immediate emit on `data:` line)

### Constraints

- Cannot add `status` field to Rust `SessionStatus` without defining what "running" means server-side (currently there's no process state tracking for discovered sessions)
- The simpler fix is to use the existing `mode` field on the iOS side
- `mode == "live"` is the semantic equivalent of "running" for discovered sessions

## Feasibility Assessment

| Aspect | Assessment | Notes |
|--------|------------|-------|
| Technical Viability | **High** | Fix is a 2-line change in `isSessionActive` |
| Effort Estimate | **S** (Small) | Primary fix + validation < 1 hour |
| Risk Level | **Low** | Minimal code change, well-understood root cause |

## Recommended Fix

### Option A: Fix iOS `isSessionActive` to use `mode` (RECOMMENDED)

Change `SessionViewModel.swift` lines 440-443:

```swift
// BEFORE (broken):
var isSessionActive: Bool {
    guard let status = currentSession?.status else { return false }
    return status == "running" || status == "paused"
}

// AFTER (fixed):
var isSessionActive: Bool {
    guard let session = currentSession else { return false }
    // Check mode from backend API (always present in status response)
    if session.mode == "live" { return true }
    // Check status for locally-managed state (pause/resume)
    if let status = session.status {
        return status == "running" || status == "paused"
    }
    return false
}
```

**Pros:** No backend changes needed, uses existing API contract, backward compatible.
**Cons:** None significant.

### Option B: Add `status` field to Rust `SessionStatus`

Add a `status: String` field to `SessionStatus` in `sessions.rs` derived from process manager state.

**Pros:** Makes API more explicit.
**Cons:** Requires backend changes, process state tracking for discovered sessions is complex, not all sessions have process manager entries.

### Option C: Both A and B

Best long-term but unnecessary for this bug fix.

**Recommendation: Option A.** Minimal change, zero risk, fixes the root cause.

### Additional Validation Steps

After fixing `isSessionActive`:

1. Verify SSE connection establishes (check `connectionState` transitions to `.connected`)
2. Verify events flow into `viewModel.events` array
3. Verify `VerboseEventStreamView` renders events (check event count badge)
4. If Bug #2 manifests (view doesn't update despite events in array), add explicit view refresh trigger
5. Run 10-minute SSE validation per CLAUDE.md Gate 4

## Related Specs

| Spec | Relevance | `mayNeedUpdate` | Notes |
|------|-----------|-----------------|-------|
| `api-coverage` | **High** | false | Adds 20 more API routes; SSE fix is orthogonal but validates the event stream infrastructure they'll build on |
| `ios-app-polish-v2` | **Medium** | false | Completed. Validated all 9 screens but didn't catch SSE issue because sessions were not actively running during validation |
| `ios-app-polish` | **Low** | false | Earlier polish round, superseded by v2 |

## Quality Commands

| Type | Command | Source |
|------|---------|--------|
| Build (Rust) | `cargo build` | CLAUDE.md |
| Test (Rust) | `cargo test -p ralph-mobile-server` | CLAUDE.md |
| Build (iOS) | `xcodebuild -scheme RalphMobile -destination 'platform=iOS Simulator,name=iPhone 16 Pro' build` | CLAUDE.md |
| Lint | `npm run lint` | package.json |
| Test (all) | `npm test` | package.json |
| Build (web) | `npm run build` | package.json |

**Local CI**: `cargo test -p ralph-mobile-server && xcodebuild -scheme RalphMobile -destination 'id=23859335-3786-4AB4-BE26-9EC0BD8D0B57' build`

## Open Questions

1. Should `isSessionActive` also handle future states like "initializing" or "shutting_down"?
2. Should the Rust backend eventually return a `status` field in `SessionStatus` for consistency?
3. After fixing the gating issue, will the `objectWillChange.send()` workaround at line 189 be sufficient, or will further SwiftUI reactivity fixes be needed? (Must verify empirically.)

## Sources

- `/Users/nick/Desktop/ralph-orchestrator/ios/RalphMobile/ViewModels/SessionViewModel.swift` -- `isSessionActive` (lines 440-443), `connect(to:)` (lines 137-158), `startEventStream` (lines 169-209)
- `/Users/nick/Desktop/ralph-orchestrator/crates/ralph-mobile-server/src/api/sessions.rs` -- `SessionStatus` struct (lines 25-32), `get_session_status` handler (lines 66-115)
- `/Users/nick/Desktop/ralph-orchestrator/crates/ralph-mobile-server/src/api/events.rs` -- SSE stream format (line 121)
- `/Users/nick/Desktop/ralph-orchestrator/crates/ralph-core/src/event_reader.rs` -- `Event` struct (lines 86-95)
- `/Users/nick/Desktop/ralph-orchestrator/ios/RalphMobile/Models/Session.swift` -- `Session` model, `status` field (line 13)
- `/Users/nick/Desktop/ralph-orchestrator/ios/RalphMobile/Models/Event.swift` -- Event model with CodingKeys (lines 217-230)
- `/Users/nick/Desktop/ralph-orchestrator/ios/RalphMobile/Services/EventStreamService.swift` -- SSE connection (lines 31-83)
- `/Users/nick/Desktop/ralph-orchestrator/ios/RalphMobile/Utilities/SSEParser.swift` -- SSE line parsing (lines 21-79)
- `/Users/nick/Desktop/ralph-orchestrator/ios/RalphMobile/Views/Ralph/UnifiedRalphView.swift` -- Event stream section (lines 490-521)
- [Apple Developer Forums: View doesn't update during async stream](https://developer.apple.com/forums/thread/682675)
- [Hacking with Swift: View not updating with @Published](https://www.hackingwithswift.com/forums/swiftui/view-not-updating-after-change-atpublished-property/11885)
