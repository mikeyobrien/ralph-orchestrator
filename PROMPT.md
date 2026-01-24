# Mock E2E Adapter (Cost‑Free)

## Objective
Implement a cost‑free mock adapter for `ralph-e2e` that replays JSONL cassettes via a `mock-cli` subcommand and runs E2E scenarios without real API calls.

## Key Requirements
- Add `--mock` mode to `ralph-e2e` (opt‑in).
- Add `ralph-e2e mock-cli` subcommand:
  - Flags: `--cassette <path>`, `--speed <n>` (accelerated default), whitelist mechanism for local commands.
  - Supports `--version` with exit 0.
  - Replays `ux.terminal.write` from SessionRecorder JSONL to stdout.
- In mock mode, `ralph-e2e` writes `ralph.yml` with `cli.backend: custom` and `command` pointing to `ralph-e2e mock-cli`.
- Cassette naming: `cassettes/e2e/<scenario-id>-<backend>.jsonl` → fallback `cassettes/e2e/<scenario-id>.jsonl`; **fail fast** if missing.
- Bypass backend availability/auth checks in mock mode.
- Preserve per‑backend matrix reporting in mock mode.
- Execute **whitelisted** local commands (e.g., `ralph task add`, `ralph tools memory add`) for task/memory side effects.
- Do not inject iteration markers; replay output as‑is.

## Acceptance Criteria
- `ralph-e2e --mock` runs at least a small subset of scenarios with no real backend installed.
- Missing cassette causes a clear error and non‑zero exit.
- Task/memory scenarios succeed in mock mode due to whitelisted command execution.
- Non‑mock mode remains unchanged.

## References
- Detailed design: `specs/mock-adapter-e2e/design/detailed-design.md`
- Implementation plan: `specs/mock-adapter-e2e/implementation/plan.md`
