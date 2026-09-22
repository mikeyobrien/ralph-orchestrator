# Manual live harness smoke contract

This is a fixed harness check, not a coding task. Ignore the user's objective except as context for this smoke. Identify the active role from **Suggested next roles** and execute only that role's contract below.

For the active role:

1. Use the backend's ordinary shell/Bash tool exactly once with exactly the listed **ordinary probe** command. Do not inspect the prompt, files, environment, or state first. Do not use any other ordinary tool and do not retry or repair.
2. Wait for exit status 0 and verify the tool result is exactly that role's `HARNESS_SMOKE:<id>` line. A failure or different result is a hard failure: stop without a handoff or success response.
3. After a successful probe, invoke exactly the listed **control-plane handoff** once. This event-tool invocation is not an ordinary probe. Do not invoke `inspect`, memory, task, guide, or any other control-plane command.
4. After the handoff succeeds, make the final assistant response exactly the listed **final response**, with no other text. Never print or invoke a completion-promise fallback.

## Claude

- Ordinary probe: `printf '%s\n' 'HARNESS_SMOKE:claude' | tee -a '{{STATE_DIR}}/smoke-evidence.txt'`
- Control-plane handoff: `{{TOOL_PATH}} emit smoke.claude.done "HARNESS_OK:claude:HARNESS_SMOKE:claude"`
- Final response: `HARNESS_OK:claude:HARNESS_SMOKE:claude`

## Codex

- Ordinary probe: `printf '%s\n' 'HARNESS_SMOKE:codex' | tee -a '{{STATE_DIR}}/smoke-evidence.txt'`
- Control-plane handoff: `{{TOOL_PATH}} emit smoke.codex.done "HARNESS_OK:codex:HARNESS_SMOKE:codex"`
- Final response: `HARNESS_OK:codex:HARNESS_SMOKE:codex`

## OpenCode

- Ordinary probe: `printf '%s\n' 'HARNESS_SMOKE:opencode' | tee -a '{{STATE_DIR}}/smoke-evidence.txt'`
- Control-plane handoff: `{{TOOL_PATH}} emit smoke.opencode.done "HARNESS_OK:opencode:HARNESS_SMOKE:opencode"`
- Final response: `HARNESS_OK:opencode:HARNESS_SMOKE:opencode`

## Pi

- Ordinary probe: `printf '%s\n' 'HARNESS_SMOKE:pi' | tee -a '{{STATE_DIR}}/smoke-evidence.txt'`
- Control-plane handoff: `{{TOOL_PATH}} emit smoke.pi.done "HARNESS_OK:pi:HARNESS_SMOKE:pi"`
- Final response: `HARNESS_OK:pi:HARNESS_SMOKE:pi`

## Hermes

- Ordinary probe: `printf '%s\n' 'HARNESS_SMOKE:hermes' | tee -a '{{STATE_DIR}}/smoke-evidence.txt'`
- Control-plane handoff: `{{TOOL_PATH}} emit smoke.hermes.done "HARNESS_OK:hermes:HARNESS_SMOKE:hermes"`
- Final response: `HARNESS_OK:hermes:HARNESS_SMOKE:hermes`

## Kiro

- Ordinary probe: `evidence='{{STATE_DIR}}/smoke-evidence.txt'; printf '%s\n' 'HARNESS_SMOKE:kiro' >> "$evidence" && printf '%s\n' 'HARNESS_SMOKE:kiro' && expected=$(printf '%s\n' 'HARNESS_SMOKE:claude' 'HARNESS_SMOKE:codex' 'HARNESS_SMOKE:opencode' 'HARNESS_SMOKE:pi' 'HARNESS_SMOKE:hermes' 'HARNESS_SMOKE:kiro') && actual=$(cat "$evidence") && [ "$actual" = "$expected" ] && [ "$(wc -l < "$evidence" | tr -d '[:space:]')" = 6 ]`
- Control-plane handoff: `{{TOOL_PATH}} emit smoke.complete "HARNESS_OK:kiro:HARNESS_SMOKE:kiro"`
- Final response: `HARNESS_OK:kiro:HARNESS_SMOKE:kiro`

All paths above resolve into this run's disposable engine state. Never inspect or modify the source checkout or persistent provider configuration.
