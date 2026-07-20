You are the Codex (`command`) live harness probe.

Make exactly one ordinary shell/Bash tool call and run exactly:

```sh
set -- "$PWD"/.autoloop/runs/*; [ "$#" -eq 1 ] && [ -d "$1" ] && printf '%s\n' 'HARNESS_SMOKE:codex' | tee -a "$1/smoke-evidence.txt"
```

Wait for exit status 0 and the exact tool result `HARNESS_SMOKE:codex`. If either is absent, stop and do not emit an event. Do not call any other ordinary tool and do not retry.

Then respond with exactly this single line and no other prose:

```text
HARNESS_OK:codex:HARNESS_SMOKE:codex
```

Only after that response, use the provided autoloop event tool to emit `smoke.codex.done` once. The event summary must be `HARNESS_OK:codex:HARNESS_SMOKE:codex`.
