# Implement the approved manual live harness smoke preset

Approved spec: `.ralph/specs/manual-live-harness-smoke.spec.md`

## Mission

Implement and dogfood the approved manual smoke for this exact live provider sequence:

1. Claude (`claude-sdk`)
2. Codex (`command`)
3. OpenCode (`command`)
4. Pi (`pi` RPC)
5. Hermes (`acp`, provider `hermes`)
6. Kiro (`acp`, provider `kiro`)

The result must prove one ordinary shell tool call and its response for every provider, then fail closed unless all six ordered handoffs and the literal native-journal topic `smoke.complete` exist.

## Required implementation

- Native TOML preset under `presets/live-harness-smoke/` with six role prompts and per-role backend overrides.
- Manual runner `tools/smoke-live-harnesses.sh` that creates a disposable git repository, preflights every executable, generates an absolute explicit-preset Ralph config, runs the smoke, and prints a six-row PASS/FAIL table.
- Fake-provider integration coverage for CI. CI/tests must never call paid live models.
- Documentation for prerequisites, cost/safety bounds, reruns, retained failure artifacts, and environment overrides.
- Update preset documentation/index only as appropriate for a manual/internal preset; do not advertise it as a supported production builtin.

## Blocking acceptance gates

1. Each role makes exactly one ordinary probe tool call, receives its result, returns the exact provider sentinel, and only then uses the event tool for handoff.
2. The evidence file contains exactly six ordered, unique sentinel lines.
3. Kiro's single ordinary probe appends its own sentinel and validates the complete evidence file before it can emit `smoke.complete`.
4. Success parsing reads autoloop's native journal for the literal emitted topic `smoke.complete`; Ralph's generic `completion_event` stop reason is not sufficient.
5. Missing executable/auth, timeout, backend failure, malformed/duplicate evidence, missing response, missing handoff, or false clean exit produces nonzero overall status with actionable diagnostics.
6. The runner uses a disposable workspace and does not mutate the source checkout or persistent provider configuration.
7. No hidden retries, no silent skips, and no live paid calls from automated tests.
8. Run targeted tests during implementation and full `cargo test` before declaring done, as required by `AGENTS.md`.
9. Dogfood the finished runner against the six authenticated live CLIs. Capture the real six-row output, ordered evidence, completion topic, elapsed time, and available cost. If a provider fails, diagnose and fix the implementation or report the exact external blocker; do not fabricate a pass.
10. Commit small coherent slices and leave no generated/ephemeral files in git.

## Known implementation constraints

- Ralph YAML hat generation applies one global backend; the six-provider matrix must remain a native explicit autoloop preset.
- `core.autoloop_preset` absolute-path passthrough preserves per-role backend fields.
- Role ordering comes from the linear handoff topology; `max_iterations = 6` is only the fail-closed budget backstop.
- Current local executables exist for all six providers, but availability alone does not prove authentication.
- Use existing proven invocation shapes from `CliBackend` and autoloop ACP provider definitions rather than inventing arguments.

## Scope guardrails

- Do not alter unrelated builtins or backend semantics.
- Do not change public release/tag/package state.
- Do not make the preset a default workflow.
- Do not weaken tests or substitute source-only assertions for runtime behavior.
- Do not commit temporary smoke repositories, credentials, provider output, `.autoloop/`, `.ralph/` runtime state, or absolute machine paths.

Emit `task.complete` / `LOOP_COMPLETE` only after the full repository test gate and real six-provider manual smoke have completed with honest evidence, or return blocked with the precise external failure.
