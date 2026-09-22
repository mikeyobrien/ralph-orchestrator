# Manual Live Harness Smoke Preset

**Status:** Approved and dogfooded  
**Approved:** 2026-07-20  
**Dogfood verdict:** Pass after clarifying native-journal completion verification

## Goal

Provide a bounded, intentionally manual release/dogfood smoke that proves Ralph 3.0 can drive one real agent turn through each selected live backend, observe one ordinary tool call and its result, receive a deterministic response, and advance through the full matrix.

Initial matrix, in order:

1. Claude (`claude-sdk` harness)
2. Codex (`command` harness)
3. OpenCode (`command` harness)
4. Pi (`pi` RPC harness)
5. Hermes (`acp` harness, provider `hermes`)
6. Kiro (`acp` harness, provider `kiro`)

This deliberately covers all four current autoloop process implementations (`claude-sdk`, `command`, `pi`, and `acp`) while also exercising two independent command providers and two independent ACP providers.

## Product shape

### Native multi-file preset

Add a manual-only native autoloop preset directory:

```text
presets/live-harness-smoke/
├── autoloops.toml
├── topology.toml
├── harness.md
└── roles/
    ├── claude.md
    ├── codex.md
    ├── opencode.md
    ├── pi.md
    ├── hermes.md
    └── kiro.md
```

The preset is native TOML rather than Ralph YAML because each role needs a distinct `backend_kind`, command, arguments, prompt mode, and ACP provider. Ralph's generated YAML-hat topology currently applies one global backend and cannot preserve this per-role matrix.

The preset is manual tooling, not a supported general-purpose builtin. It should not appear in the normal production builtin list until its cost and portability are proven.

### Manual runner

Add:

```text
tools/smoke-live-harnesses.sh
```

The runner:

1. Locates `ralph`, `autoloop`, `claude`, `codex`, `opencode`, `pi`, `hermes`, and `kiro-cli`.
2. Fails before spending tokens if any selected executable is missing. Missing selected providers are failures, not silent skips.
3. Creates a disposable temporary git repository.
4. Writes a temporary Ralph config using an absolute `core.autoloop_preset` path so per-role backend overrides reach autoloop unchanged.
5. Runs the matrix headlessly with strict iteration/runtime/cost bounds.
6. Reads autoloop's native journal and smoke evidence and prints a final PASS/FAIL table. Completion verification must inspect the native journal's emitted topic for the literal `smoke.complete`; Ralph's stop reason/summary alone cannot identify which event completed the run.
7. Preserves the temporary directory on failure and deletes it on success unless `KEEP_SMOKE_DIR=1` is set.

Supported overrides:

- `RALPH_BIN`
- `AUTOLOOP_BIN` or PATH
- `KEEP_SMOKE_DIR=1`
- `SMOKE_MAX_COST_USD`
- `SMOKE_TIMEOUT_SECONDS`

No provider/model override is required initially; each CLI uses its authenticated default model/profile.

## Probe contract

Each role receives a fresh context and must not inspect or modify the source checkout.

For backend `<id>`, the role must:

1. Make exactly one **ordinary agent tool call** using its shell/Bash tool.
2. Execute the provided fixed probe command, which appends and prints exactly:

   ```text
   HARNESS_SMOKE:<id>
   ```

   The evidence file lives under `$AUTOLOOP_STATE_DIR`, not in the source tree.
3. Wait for the tool result.
4. Respond with exactly:

   ```text
   HARNESS_OK:<id>:HARNESS_SMOKE:<id>
   ```

5. Use only the autoloop event tool after the ordinary probe to emit the next handoff event.

The event-tool invocation is control-plane signaling and does not count as the one ordinary probe tool call.

Roles must not use exploratory reads, searches, edits, web tools, or additional shell calls. A role that emits a handoff without the exact probe evidence is a failure.

### Final Kiro probe

Kiro's single ordinary shell call must both append its own sentinel and validate that the evidence file contains the six expected lines exactly once and in the required order. It may emit the completion event only when that command exits successfully.

## Topology

```text
loop.start
  → claude
  → smoke.claude.done
  → codex
  → smoke.codex.done
  → opencode
  → smoke.opencode.done
  → pi
  → smoke.pi.done
  → hermes
  → smoke.hermes.done
  → kiro
  → smoke.complete
```

`smoke.complete` is the sole completion event. The linear handoff topology establishes the required order, while `max_iterations = 6` is the fail-closed backstop rather than a separate sequence-enforcement primitive. Any backend process, probe, response, or handoff failure must prevent successful completion.

## Backend contracts

| Role | Kind | Provider/command | Required invocation shape |
|---|---|---|---|
| Claude | `claude-sdk` | `claude` | Agent SDK session, default model |
| Codex | `command` | `codex` | `codex exec --yolo <prompt>` |
| OpenCode | `command` | `opencode` | `opencode run <prompt>` |
| Pi | `pi` | `pi` | Persistent Pi RPC session |
| Hermes | `acp` | provider `hermes`, command `hermes` | `hermes acp`, default profile |
| Kiro | `acp` | provider `kiro`, command `kiro-cli` | `kiro-cli acp` |

All roles use bounded backend timeouts. Tool permissions are auto-approved for this disposable smoke workspace. The probe command is fixed and only writes inside the engine state directory.

## Result contract

The runner prints one row per provider:

```text
BACKEND   KIND         TOOL_SENTINEL   RESPONSE_SENTINEL   HANDOFF   RESULT
claude    claude-sdk   yes             yes                 yes       PASS
...
```

Overall success requires all six rows to pass and autoloop's native journal to contain the literal emitted topic `smoke.complete`. Ralph's generic `completion_event` stop reason is insufficient evidence. A zero exit code without complete native-journal evidence is a failure.

On failure, print:

- failing backend and harness kind;
- Ralph/autoloop exit code;
- run ID and retained temporary workspace;
- journal stop reason/detail;
- last relevant backend output;
- exact rerun command.

## Safety and cost bounds

- Disposable git workspace only.
- No source-tree mutation.
- Six iterations maximum.
- Per-backend timeout and total runtime cap.
- Configurable total cost cap with a conservative default.
- No retries inside the preset; one attempt per live provider keeps failures honest.
- No automatic auth/login flows.
- No release, publish, push, or persistent config mutation.

## Verification

### Static/integration checks

1. Load the native preset through autoloop and verify all six per-role backend overrides survive parsing.
2. Verify topology order and sole completion event.
3. Verify every role prompt contains the exact provider-specific sentinel and forbids additional ordinary tools.
4. Verify the runner fails before launch when any required executable is missing.
5. Verify journal/evidence parsing rejects:
   - missing provider rows;
   - duplicate/out-of-order sentinels;
   - a handoff without a response sentinel;
   - clean process exit without a literal `smoke.complete` emitted-topic record in autoloop's native journal.
6. Use fake provider fixtures for CI; CI must not call live paid models.

### Manual acceptance

Run the script against authenticated live CLIs and capture:

- all six PASS rows;
- the six ordered evidence lines;
- one tool-call/result pair from each backend stream/output;
- final `smoke.complete` summary;
- elapsed time and cost where available.

A prose claim or model response without tool evidence does not pass.

## Documentation

Document:

```bash
tools/smoke-live-harnesses.sh
KEEP_SMOKE_DIR=1 tools/smoke-live-harnesses.sh
```

Explain that this is a paid/manual smoke, which providers it covers, required authentication, expected cost, and how to diagnose one failing harness.

## Non-goals

- Testing every supported command provider in the first version.
- Running in normal CI against paid models.
- Benchmarking model quality or latency.
- Retrying transient provider failures automatically.
- Testing multi-tool workflows, file editing, web access, HITL, or resume.
- Treating provider availability as optional once it is selected in this matrix.

## Acceptance criteria

1. Claude, Codex, OpenCode, Pi, Hermes, and Kiro each execute exactly one ordinary probe tool call and return the exact response sentinel.
2. The six sentinels appear exactly once and in order under the run state directory.
3. All six event handoffs occur and `smoke.complete` is the sole successful terminal event.
4. Missing executables/auth, backend failure, timeout, malformed evidence, or false clean exit produce nonzero overall status and actionable diagnostics.
5. CI exercises the same runner/preset parsing and evidence logic with fake providers only.
6. Live manual evidence is captured before the feature is declared done.
7. `cargo test` passes before completion, per repository requirements.
