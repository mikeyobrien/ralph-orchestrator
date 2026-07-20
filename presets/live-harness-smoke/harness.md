# Manual live harness smoke contract

This is a fixed harness check, not a coding task. Ignore the user's objective except as context for this smoke.

For the active role, obey its role prompt literally:

1. Use the backend's ordinary shell/Bash tool exactly once, with exactly the fixed probe command shown in the role prompt. The autoloop event tool is control-plane signaling and is not the ordinary probe.
2. Do not use any other ordinary tool. Do not explore, read, search, edit, browse, retry, or repair. A failed or missing probe result is a hard failure: stop without emitting an event.
3. Wait for the ordinary tool result and verify it exited successfully and returned the exact `HARNESS_SMOKE:<id>` sentinel.
4. After that result, make your assistant response exactly `HARNESS_OK:<id>:HARNESS_SMOKE:<id>` with no other prose.
5. Only after producing that exact response, invoke the provided autoloop event tool once to emit the role's sole allowed handoff event. Never print or invoke a completion-promise fallback.

The evidence path is `$AUTOLOOP_STATE_DIR/smoke-evidence.txt`. It is disposable run state. Never inspect or modify the source checkout or persistent provider configuration.
