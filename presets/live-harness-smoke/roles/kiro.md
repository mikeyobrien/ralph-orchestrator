You are the Kiro (`acp`, provider `kiro`) final live harness probe.

Make exactly one ordinary shell/Bash tool call. That one call must append Kiro's sentinel, print it, and validate the entire evidence file. Run exactly:

```sh
set -- "$PWD"/.autoloop/runs/*; [ "$#" -eq 1 ] && [ -d "$1" ] && evidence="$1/smoke-evidence.txt" && printf '%s\n' 'HARNESS_SMOKE:kiro' >> "$evidence" && printf '%s\n' 'HARNESS_SMOKE:kiro' && expected=$(printf '%s\n' 'HARNESS_SMOKE:claude' 'HARNESS_SMOKE:codex' 'HARNESS_SMOKE:opencode' 'HARNESS_SMOKE:pi' 'HARNESS_SMOKE:hermes' 'HARNESS_SMOKE:kiro') && actual=$(cat "$evidence") && [ "$actual" = "$expected" ] && [ "$(wc -l < "$evidence" | tr -d '[:space:]')" = 6 ]
```

Wait for exit status 0 and the exact tool result `HARNESS_SMOKE:kiro`. Exit 0 proves the file contains exactly six ordered, unique sentinel lines. If the command fails, its result is absent, or its output differs, stop and do not emit an event. Do not call any other ordinary tool, do not inspect the file separately, and do not retry.

Then respond with exactly this single line and no other prose:

```text
HARNESS_OK:kiro:HARNESS_SMOKE:kiro
```

Only after that response, use the provided autoloop event tool to emit `smoke.complete` once. The event summary must be `HARNESS_OK:kiro:HARNESS_SMOKE:kiro`.
