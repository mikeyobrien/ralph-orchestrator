---
status: approved
created: 2026-07-21
updated: 2026-07-21
bead: ralph-orchestrator-v3-autoloops-backend-a7e.12
related:
  - v2-engine-autoloop-gap-analysis.md
---

# Ralph-owned Autoloop state

## Approved behavior

Ralph owns runtime state for every Ralph-launched Autoloop run beneath the
workspace's `.ralph/autoloop` directory. A single `ralph-core::engine_state`
layout seam supplies that root and composes validated run directories; TUI,
adapters, and CLI consumers receive paths from the seam rather than rebuilding
`runs/<run-id>`.

Ralph enforces this root for generated Ralph presets and explicit native
Autoloop presets. Engine journal, memory, tasks, events, and run-scoped streams
remain engine-owned and format-incompatible with Ralph's coordination stores.
Missing, empty, or unsafe run IDs fail closed: they create no provisional run
directory and never fall back to top-level `.autoloop`.

Standalone Autoloop retains its own default `.autoloop` location. This spec
changes only processes launched by Ralph.

## Privacy constraints

Normal and error output must not reveal prompts, credentials, tokens, arbitrary
override values, or physical workspace, preset, event, or state-store paths.
Spawn failures may identify the executable and fixed action being attempted,
but must not render user-controlled arguments. Advanced diagnostics may name
the logical Ralph-owned state location without exposing sensitive values.

## Non-goals

- Changing Autoloop's standalone default or modifying the upstream checkout.
- Sharing Ralph task or memory files with Autoloop.
- Making private backend stream files authoritative or inventing new filenames.
- Implementing resume, dashboard, HITL, or stable live-stream contracts tracked
  by other Beads.
- Backward-compatible fallback to top-level `.autoloop`.

## Upstream dependency

The supported engine contract is the accepted `core.state_dir` implementation
in `/Users/rook/.herdr/worktrees/autoloop/ralph-owned-state-dir` (accepted head
`a8ab85a`): the root is authoritative, omitted journal/memory/task paths derive
from it, explicit overrides remain supported, and custom roots work across run,
resume, control, dashboard, and chains. Ralph must fail closed if that contract
is unavailable rather than inspect additional private state.

## Verification matrix

All runtime cases use deterministic fake/local providers and assert that no
`<workspace>/.autoloop` file or directory is created, any engine state remains
beneath `<workspace>/.ralph/autoloop`, no `.autoloop/runs/<id>` fallback occurs,
and output excludes secret prompt markers and physical state paths.

| Case | Required evidence |
|---|---|
| Generated Ralph preset succeeds | CLI override sets the owned root; run streams resolve beneath it. |
| Explicit native preset succeeds | Ralph overrides preset state ownership identically. |
| Engine exits non-zero | Failure is bounded, private, and creates no fallback state. |
| Structured events are malformed | Observation fails closed with the same filesystem/privacy assertions. |
| Run ID is missing, empty, or absent | Headless/TUI observation creates no run directory or fallback path. |
| Run ID is safe | Core layout helper returns `.ralph/autoloop/runs/<id>` with unit coverage. |
| Run ID is unsafe | Empty, traversal, separator, and multi-component IDs are rejected. |
| Spawn fails | Direct command rendering and public error paths redact prompts, paths, secrets, and override values. |
| Store isolation | Engine task/memory files remain distinct from Ralph coordination stores. |
| Native contract | Non-skipped tests run against the accepted local upstream checkout when relevant. |

Completion also requires focused tests, `cargo fmt --check`, strict clippy with
warnings denied for touched crates and targets, full `cargo test`,
`git diff --check`, full baseline-diff inspection, and a clean artifact audit.

## Merge dependencies

This remediation is based on Ralph commits `8c08ed16`, `9b747027`, and
`a9a3c19d` after `54430971`, and depends on integration/release of the accepted
upstream `core.state_dir` contract. Keep it independently mergeable; pending
TUI-history and live-smoke work may conflict around observation paths and tests
and must be reconciled to this single layout seam rather than merged wholesale.
