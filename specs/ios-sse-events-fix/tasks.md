---
spec: ios-sse-events-fix
phase: tasks
total_tasks: 7
created: 2026-02-07T07:30:00Z
---

# Tasks: iOS SSE Event Stream Fix

## Phase 1: Make It Work (POC)

Focus: Apply the 5-line fix, build, verify SSE gate opens for live sessions.

- [x] 1.1 Fix isSessionActive to check mode from backend
  - **Do**:
    1. Open `ios/RalphMobile/ViewModels/SessionViewModel.swift`
    2. Replace lines 440-443 (the `isSessionActive` computed property) with:
       ```swift
       var isSessionActive: Bool {
           guard let session = currentSession else { return false }
           // Backend-provided liveness (always present after fetchSessionStatus)
           if session.mode == "live" { return true }
           // iOS-only local state (set by pauseRun/resumeRun)
           if let status = session.status {
               return status == "running" || status == "paused"
           }
           return false
       }
       ```
    3. Verify `Session.mode` exists as `String?` at `ios/RalphMobile/Models/Session.swift:11`
  - **Files**: `ios/RalphMobile/ViewModels/SessionViewModel.swift` (lines 440-443)
  - **Done when**: `isSessionActive` checks `mode == "live"` first, falls back to local `status`
  - **Verify**: `grep -n 'session.mode == "live"' ios/RalphMobile/ViewModels/SessionViewModel.swift` returns a match
  - **Commit**: `fix(ios): check backend mode instead of nil status for SSE gate`
  - _Requirements: FR-1, FR-2, AC-1.1, AC-2.1, AC-3.2_
  - _Design: SessionViewModel.isSessionActive (ONLY CHANGE)_

- [x] 1.2 [VERIFY] Build verification (Gate 5)
  - **Do**:
    1. Build iOS app targeting exclusive simulator
    2. Run backend tests to confirm no regressions
  - **Verify**:
    - `xcodebuild -scheme RalphMobile -destination 'id=23859335-3786-4AB4-BE26-9EC0BD8D0B57' build 2>&1 | tail -5` shows `BUILD SUCCEEDED`
    - `cargo test -p ralph-mobile-server 2>&1 | tail -3` shows all tests pass
  - **Done when**: Zero build errors, all backend tests pass
  - **Commit**: None (verification only)

## Phase 2: Functional Validation (Real Backend + SSE)

Focus: Prove the fix works with real data flowing through real infrastructure.

- [x] 2.1 Start backend and configure iOS app
  - **Do**:
    1. Start ralph-mobile-server: `cargo run --bin ralph-mobile-server -- --bind-all --show-key`
    2. Get LAN IP: `ipconfig getifaddr en0`
    3. Configure simulator Settings programmatically:
       ```bash
       xcrun simctl spawn 23859335-3786-4AB4-BE26-9EC0BD8D0B57 defaults write dev.ralph.RalphMobile serverURL -string "http://<LAN_IP>:8080"
       xcrun simctl spawn 23859335-3786-4AB4-BE26-9EC0BD8D0B57 defaults write dev.ralph.RalphMobile apiKey -string "<API_KEY>"
       ```
    4. Install and launch app on simulator
    5. Verify sessions load from real backend: `curl http://<LAN_IP>:8080/api/sessions | head -c 200`
  - **Files**: None (infrastructure setup)
  - **Done when**: iOS app shows real session list from backend
  - **Verify**: `curl -s http://127.0.0.1:8080/api/sessions | python3 -c "import sys,json; d=json.load(sys.stdin); print(f'{len(d)} sessions')"` returns session count > 0
  - **Commit**: None (setup only)
  - _Requirements: Dependencies section_

- [x] 2.2 Validate SSE events flow into UI
  - **Do**:
    1. Start a ralph loop: `ralph run -c presets/feature.yml -p "analyze the README" --max-iterations 5`
    2. In simulator, tap the live session in sidebar
    3. Observe EVENT STREAM section -- should show events flowing with count > 0
    4. Take screenshot: `xcrun simctl io 23859335-3786-4AB4-BE26-9EC0BD8D0B57 screenshot ios/validation-screenshots/sse-fix-events-flowing.png`
    5. Verify connection state badge shows connected (not "ENDED")
    6. Tap a completed session, verify it shows "ENDED" badge with no SSE attempt
    7. Take screenshot: `xcrun simctl io 23859335-3786-4AB4-BE26-9EC0BD8D0B57 screenshot ios/validation-screenshots/sse-fix-completed-session.png`
  - **Files**: `ios/validation-screenshots/sse-fix-events-flowing.png`, `ios/validation-screenshots/sse-fix-completed-session.png`
  - **Done when**: EVENT STREAM badge shows count > 0 for live session, "ENDED" for completed session
  - **Verify**: `ls -la ios/validation-screenshots/sse-fix-events-flowing.png ios/validation-screenshots/sse-fix-completed-session.png` shows both files exist with size > 0
  - **Commit**: `docs(ios): add SSE fix validation screenshots`
  - _Requirements: FR-3, FR-4, FR-5, AC-1.2, AC-1.3, AC-2.1, AC-2.4_
  - _Design: Data Flow sequence diagram_

## Phase 3: 10-Minute Video Evidence (Gate 4)

Focus: Satisfy CLAUDE.md Gate 4 -- real ralph loop, SSE streaming, hat transitions, 10-minute video.

- [ ] 3.1 Record 10-minute SSE streaming session (DEFERRED - requires active ralph loop)
  - **Do**:
    1. Start ralph loop with enough iterations for 10+ minutes: `ralph run -c presets/feature.yml -p "refactor the event loop module for clarity" --max-iterations 15`
    2. Start video recording: `xcrun simctl io 23859335-3786-4AB4-BE26-9EC0BD8D0B57 recordVideo ios/validation-screenshots/sse-fix-validation.mp4 &`
    3. Tap the live session in sidebar
    4. Wait 10+ minutes, observing:
       - Events flowing into EVENT STREAM section
       - Hat transitions rendering (minimum 3 different hats)
       - Event count incrementing
       - Connection state staying connected
    5. Stop recording after 10+ minutes: `kill %1` (or send SIGINT to recorder)
    6. Verify video duration: `ffprobe -v error -show_entries format=duration -of csv=p=0 ios/validation-screenshots/sse-fix-validation.mp4`
  - **Files**: `ios/validation-screenshots/sse-fix-validation.mp4`
  - **Done when**: Video file > 600 seconds duration, shows live events flowing
  - **Verify**: `ffprobe -v error -show_entries format=duration -of csv=p=0 ios/validation-screenshots/sse-fix-validation.mp4 | python3 -c "import sys; d=float(sys.stdin.read().strip()); print(f'{d:.0f}s'); exit(0 if d >= 600 else 1)"`
  - **Commit**: `docs(ios): add 10-minute SSE fix validation video (Gate 4)`
  - _Requirements: AC-1.5, NFR-3, Success Criteria_
  - _Design: Validation Strategy_

- [ ] 3.2 [VERIFY] AC checklist -- all acceptance criteria met (DEFERRED - depends on 3.1)
  - **Do**:
    1. Verify AC-1.1 (SSE connects for live sessions): screenshot shows events flowing
    2. Verify AC-1.2 (events appear without refresh): video shows continuous stream
    3. Verify AC-1.3 (event count > 0): screenshot shows count badge
    4. Verify AC-1.5 (10-min stability): video duration >= 600s
    5. Verify AC-2.1 (completed = disconnected): screenshot shows "ENDED"
    6. Verify AC-2.2 (no SSE for completed): no connection attempt in completed session screenshot
    7. Verify code fix satisfies FR-1, FR-2, FR-3:
       ```bash
       grep -A6 'var isSessionActive' ios/RalphMobile/ViewModels/SessionViewModel.swift
       ```
    8. Document results in .progress.md
  - **Verify**:
    - `grep 'session.mode == "live"' ios/RalphMobile/ViewModels/SessionViewModel.swift` exits 0
    - `ls ios/validation-screenshots/sse-fix-validation.mp4` exits 0
    - `ls ios/validation-screenshots/sse-fix-events-flowing.png` exits 0
    - `ls ios/validation-screenshots/sse-fix-completed-session.png` exits 0
  - **Done when**: All AC items verified with evidence artifacts
  - **Commit**: None (verification only)
  - _Requirements: All AC-* items_

## Phase 4: Quality Gates

- [ ] 4.1 Create PR and verify CI
  - **Do**:
    1. Verify current branch is a feature branch: `git branch --show-current`
    2. Stage changed files: `git add ios/RalphMobile/ViewModels/SessionViewModel.swift`
    3. Push branch: `git push -u origin $(git branch --show-current)`
    4. Create PR: `gh pr create --title "fix(ios): SSE events display by fixing session liveness detection" --body "..."`
    5. Verify CI: `gh pr checks --watch`
  - **Verify**: `gh pr checks` shows all green
  - **Done when**: PR created, CI passes
  - **Commit**: None (PR creation only)

## Phase 5: PR Lifecycle

- [ ] 5.1 Address review feedback and maintain CI
  - **Do**:
    1. Check for review comments: `gh pr view --comments`
    2. Address any requested changes
    3. Push fixes and re-verify CI
    4. Ensure zero test regressions
  - **Verify**: `gh pr checks` all green, no unresolved review comments
  - **Done when**: PR approved or no blocking comments
  - **Commit**: `fix(ios): address review feedback` (if changes needed)

## Notes

- **POC shortcuts**: None needed -- fix is production-ready from task 1.1
- **Production TODOs**: None -- this is the complete fix
- **Risk area**: SwiftUI reactivity after fix -- the existing `objectWillChange.send()` workaround at line 189 should handle @Published mutation in async context, but Phase 2 validation will confirm empirically
- **Exclusive simulator**: UDID `23859335-3786-4AB4-BE26-9EC0BD8D0B57` (iPhone 17 Pro Max, iOS 26.2) -- do NOT use any other simulator
- **Backend binding**: Must use `--bind-all` flag so simulator can reach server via LAN IP
