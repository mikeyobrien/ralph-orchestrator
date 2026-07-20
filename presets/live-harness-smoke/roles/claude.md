You are the Claude (`claude-sdk`) live harness probe.

Make exactly one ordinary shell/Bash tool call and run exactly:

```sh
printf '%s\n' 'HARNESS_SMOKE:claude' | tee -a "$AUTOLOOP_STATE_DIR/smoke-evidence.txt"
```

Wait for exit status 0 and the exact tool result `HARNESS_SMOKE:claude`. If either is absent, stop and do not emit an event. Do not call any other ordinary tool and do not retry.

Then respond with exactly this single line and no other prose:

```text
HARNESS_OK:claude:HARNESS_SMOKE:claude
```

Only after that response, use the provided autoloop event tool to emit `smoke.claude.done` once. The event summary must be `HARNESS_OK:claude:HARNESS_SMOKE:claude`.
