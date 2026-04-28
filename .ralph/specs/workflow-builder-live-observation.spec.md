---
status: draft
gap_analysis: null
related:
  - workflow-builder-ui-edit-controls.spec.md
  - hat-collections.spec.md
---

# Workflow Builder Live Observation

## Overview

The Builder page (`/builder`) currently supports editing hat collection
workflows but has no awareness of running loops. This spec adds a live
observation mode: when a loop executes against a collection, the same
graph that the user designed lights up in real time, showing which hat
is active, which events are firing, and what iteration the loop is on.

Edit mode and observation mode share the same page and graph layout.
No separate route or tab is introduced.

## Non-Goals

- Streaming agent log output in the PropertiesPanel (separate feature)
- Historical replay of completed loops
- Cost or token tracking in the Builder
- Backpressure gate visualization
- Automatic config sync between collection and ralph.yml
- Supporting multiple simultaneous observed loops per Builder instance

## Design

### Backend: Event File Watcher (B1)

The orchestration loop writes every hat-to-hat event to a JSONL file.
Each fresh loop run creates a new timestamped file (e.g.
`.ralph/events-20260423-120000.jsonl`). A marker file
`.ralph/current-events` contains the relative path to the active file.

A new background task in `ralph-api` polls this file every 500ms:

1. Read `.ralph/current-events` to resolve the active events file path
2. If the path changed since last poll, reset the byte offset to 0
3. Read from the stored byte offset to EOF
4. Parse each new line as `EventRecord`. Both Ralph-emitted events (with
   `hat`, `iteration`, `triggered`) and agent-emitted events (only `ts`,
   `topic`, `payload`) are valid. Missing fields default to empty or 0
   via `#[serde(default)]`
5. Publish each record to the stream domain as a `loop.orchestration`
   event
6. If the file or marker does not exist, skip and retry next poll

No changes to `ralph-cli`. No subprocess piping. The file watcher runs
independently of the RPC handler (no mutex interaction).

### Frontend: Observation Overlay (B2)

`CollectionBuilder` gains an observation layer driven by a zustand store.
When observation is active, the graph renders visual overlays on nodes
and edges based on incoming `loop.orchestration` stream events.

State model (zustand, survives navigation):

```
active: boolean
loopId: string | null
iteration: number
activeHatId: string | null
nodeStates: Record<string, 'idle' | 'pending' | 'active' | 'completed'>
lastFiredEdgeId: string | null
lastFiredAt: number
```

Event-to-state mapping:

Ralph writes two kinds of events to `.ralph/events-*.jsonl` (see
`crates/ralph-core/src/event_logger.rs` `EventRecord`):

1. **Loop-level events** (written by Ralph itself): carry `hat`,
   `iteration`, `triggered`, and full context.
2. **Agent-level events** (written by the active agent via tool calls):
   carry only `ts`, `topic`, and `payload`. `hat` defaults to empty
   string via `#[serde(default)]`.

In a typical run, agent-level events dominate (empirically ~21 of 22
events in a single pong-game run). The observation overlay must handle
both shapes, inferring the transition from graph topology when the event
lacks explicit hat context.

| Event shape | State update |
|---|---|
| Loop-level with `hat` and `triggered` (e.g. `task.start`) | `triggered` hat → active; prior active → completed |
| Agent-level with `topic` matching an edge label | Edge's `source` hat → completed, `target` hat → active, edge → fired |
| Any event with `topic` matching an edge label | Set `lastFiredEdgeId` to that edge, reset timer |
| `iteration` increments | Update iteration counter |
| `topic` = loop completion promise (`LOOP_COMPLETE`, `loop.terminate`) | All nodes → completed, exit observation after 2s |

Events with `hat: "loop"` are meta-events from the framework itself, not
from a user-defined hat. They drive transitions (via `triggered`) but
never appear as the "active hat" badge in the toolbar.

Node visual states:

| State | Border | Animation |
|---|---|---|
| Idle | None | None |
| Pending | `!border-2 !border-sky-400` | Static |
| Active | `!border-2` | `animate-border-pulse` (red, border-color only) |
| Completed | `!border-2 !border-teal-400` | Static + ✓ badge |

Edge fired state: `stroke-dasharray` + `stroke-dashoffset` CSS animation
over 1.5s. If a new fire arrives before the animation completes, the
timer resets (previous animation cancelled). Only one edge animates at
a time (known limitation for MVP).

Edit mode vs observation mode:
- Observation: palette collapsed, properties panel hidden, drag/drop
  disabled, delete keys disabled, toolbar shows iteration counter +
  active hat name + Stop + inline legend
- Edit: current behavior + Run button

Observation toolbar layout:
- Left: `● Observing` badge, `Iteration N` counter, active-hat badge
  (filtered: `loop` is never shown as the active hat because it is a
  framework-level synthetic marker, not a user hat)
- Right: inline legend (`● pending`, `● active`, `● done ✓`) so the
  user can map ring colors without hunting for docs. The legend uses
  solid background swatches matching the node rings

The toolbar uses `flex-wrap` so the legend gracefully drops to a second
line on narrow viewports.

### Frontend: Run Button (B3)

A Run button in the Builder toolbar starts a loop:

1. If dirty, Run is disabled; the user must save first (to avoid
   desync between the graph shown and the graph being run)
2. On click, open a prompt dialog asking what the workflow should do
3. On prompt submit, call `collection.run` with the collection id and
   the prompt
4. The RPC exports the collection as a hats-only YAML and spawns
   `ralph run -H <yaml> -a -p <prompt>` in the workspace. `-a`
   (autonomous) is required because the API spawns ralph as a
   background subprocess with no controlling tty
5. Enter observation mode for the loop. The RPC response includes
   `startingHat` (the hat that will activate first, derived from the
   graph topology). The frontend calls `setHatActive(startingHat)`
   immediately, eliminating the timing race between the RPC return
   and the first WebSocket event.
6. Toolbar switches to: `Observing` badge, iteration counter, active
   hat badge, Stop button, inline legend
7. Stop calls `loop.stop` (which reads `.ralph/loop.lock` for the PID
   and signals the process), exits observation, returns to edit mode
8. Loop completion exits observation automatically after a 2s delay
9. If the spawned ralph exits non-zero within the first 500ms, the
   RPC returns an error containing ralph's verbatim stderr. The
   Builder shows a dismissable error banner with that text

MVP limitation: the Run button uses the user's existing `ralph.yml` for
backend/max_iterations/backpressure config. The collection only provides
hats and events. If the configured backend is not installed or
misconfigured, ralph exits non-zero and the Builder surfaces the stderr
verbatim in a dismissable banner; the user is expected to fix their
config and retry.

## Acceptance Criteria

### Event Watcher Publishes to Stream

- **Given** a loop is running and writing to `.ralph/events-*.jsonl`
- **When** a new event line is appended to the file
- **Then** within 500ms, a `loop.orchestration` event appears on the
  stream WebSocket with the correct `hat`, `iteration`, `topic`, and
  `triggered` fields

### Watcher Handles Missing File

- **Given** no loop is running and `.ralph/current-events` does not exist
- **When** the watcher polls
- **Then** it skips without error and retries on the next poll

### Watcher Switches on New Loop

- **Given** the watcher is tailing `events-A.jsonl`
- **When** a new loop starts and `.ralph/current-events` changes to
  `events-B.jsonl`
- **Then** the watcher resets its offset and begins tailing the new file

### Node Lights Up When Hat Activates

- **Given** the Builder is in observation mode showing a collection with
  a planner and builder hat connected by an edge labelled `subtask.ready`
- **When** a `loop.orchestration` event arrives with either
  - (a) `hat: "builder"` (Ralph-emitted), or
  - (b) `topic: "subtask.ready"` without a `hat` field (agent-emitted)
- **Then** the builder node shows an indigo ring (active), the planner
  node transitions to completed (teal ring + ✓), and the edge pulses

### Edge Pulses When Event Fires

- **Given** the Builder is in observation mode with an edge labeled
  `build.done`
- **When** a `loop.orchestration` event arrives with `topic: "build.done"`
- **Then** the edge renders a dash-offset pulse animation for 1.5s

### Iteration Counter Updates

- **Given** the Builder is in observation mode
- **When** events arrive with incrementing `iteration` values
- **Then** the toolbar displays the current iteration number

### Run Button Starts Loop

- **Given** a saved collection is open in the Builder with no unsaved
  changes
- **When** the user clicks Run, enters a prompt in the dialog, and
  submits
- **Then** `collection.run` is called with the collection id and prompt;
  on success the Builder enters observation mode

### Run Button Disabled While Dirty

- **Given** the collection has unsaved changes
- **When** the user looks at the Run button
- **Then** the button is disabled with a tooltip explaining to save first

### Run Error Feedback

- **Given** the configured backend binary (from `ralph.yml`) is not
  installed on PATH
- **When** the user clicks Run and submits a prompt
- **Then** ralph's verbatim stderr is shown in a dismissable banner
  prefixed with the exit code (e.g. `ralph run exited with exit code 1:
  kiro-acp: No such file or directory`)

### Stop Button Exits Observation

- **Given** the Builder is in observation mode
- **When** the user clicks Stop
- **Then** `loop.stop` is called and the Builder returns to edit mode

### Loop Completion Exits Observation

- **Given** the Builder is in observation mode
- **When** the loop emits the completion promise event
- **Then** all nodes show completed state and observation mode exits
  after a brief delay

### Observation Survives Navigation

- **Given** the Builder is in observation mode
- **When** the user navigates to /tasks and back to /builder
- **Then** observation mode is still active with the current state
  (zustand persistence + WebSocket reconnect with replay)

### Editing Disabled During Observation

- **Given** the Builder is in observation mode
- **When** the user tries to drag a node, delete an edge, or drop a
  hat from the palette
- **Then** the action is blocked (palette hidden, properties panel
  hidden, delete keys disabled, drag/drop disabled)

## Implementation Notes

### Files Changed (B1: Rust)

| File | Change |
|---|---|
| `crates/ralph-api/src/protocol.rs` | Add `loop.orchestration` to `STREAM_TOPICS`; add `collection.run` types |
| `crates/ralph-api/src/event_watcher.rs` (new) | File-polling watcher that tails events JSONL |
| `crates/ralph-api/src/transport.rs` | Spawn watcher task on server startup with shutdown wiring |
| `crates/ralph-api/src/collection_domain.rs` | Add `run()` method that spawns `ralph run -H ... -a -p ...`, surfaces stderr on failure, detaches a reaper thread so successful runs don't leave zombies |
| `crates/ralph-api/src/runtime/dispatch.rs` | Dispatch `collection.run` to the domain |
| `crates/ralph-api/src/mcp.rs` | Tool description for `collection.run` |
| `crates/ralph-api/data/rpc-v1-schema.json` | Schema entries for `collectionRunParams` / `collectionRunResult` |
| `crates/ralph-api/src/lib.rs` | Re-export `event_watcher` module |
| `crates/ralph-api/Cargo.toml` | Depend on `ralph-core` for `EventRecord`, on `tempfile` for tests |

### Files Changed (B2: Frontend)

| File | Change |
|---|---|
| `frontend/ralph-web/src/stores/observationStore.ts` (new) | Zustand store for observation state; exposes itself on `window.__observationStore` in dev builds for Playwright |
| `frontend/ralph-web/src/hooks/useLoopObservation.ts` (new) | Hook that subscribes to `loop.orchestration` stream and updates the store |
| `frontend/ralph-web/src/components/builder/CollectionBuilder.tsx` | Conditional rendering based on observation state; inline `ObservationLegend` in observation toolbar; filter `loop` out of active-hat badge |
| `frontend/ralph-web/src/components/builder/HatNode.tsx` | Accept `observationState`, render ring + `data-hat-key` + `data-observation-state` attributes |
| `frontend/ralph-web/src/components/builder/OffsetEdge.tsx` | Accept `fired` data prop, render pulse animation; expose `data-edge-id` + `data-fired` |
| `frontend/ralph-web/src/components/layout/AppShell.tsx` | Add `h-full` to the outlet wrapper so the Builder's flex tree can claim full viewport height |
| `frontend/ralph-web/src/index.css` | Add `@keyframes edgePulse` |
| `frontend/ralph-web/src/vite-env.d.ts` (new) | `/// <reference types="vite/client" />` for `import.meta.env` typing |

### Files Changed (B3: Frontend)

| File | Change |
|---|---|
| `frontend/ralph-web/src/pages/BuilderPage.tsx` | Run/Stop buttons, prompt dialog integration, error banner, blueprint click handler |
| `frontend/ralph-web/src/components/builder/RunPromptDialog.tsx` (new) | Modal asking what the workflow should do before starting the loop |
| `frontend/ralph-web/src/components/builder/blueprints.ts` (new) | Pre-built workflow templates (starter simple versions, not full shipped presets) |
| `frontend/ralph-web/src/trpc.ts` | Add `collection.run` and `loops.stop` mutation wrappers |
| `frontend/ralph-web/src/rpc/client.ts` | Add `collection.run` to `MUTATING_METHODS` for idempotency-key handling |

### Dependencies

No new runtime dependencies in either Rust or TypeScript.

The `ralph-core` crate is added as a dependency of `ralph-api` for the
`EventRecord` type. This is a workspace-internal dependency, not external.

### Test Coverage

| Module | Tests |
|---|---|
| `event_watcher.rs` | Unit tests: parse valid/invalid lines, handle missing file, detect marker change, offset tracking |
| `collection_domain.rs` | Unit tests: `run()` surfaces stderr on non-zero exit; `run()` surfaces a clear error when the ralph binary itself is missing |
| `observationStore.ts` | Covered via Playwright because the observation UI drives the store through `window.__observationStore`; node-state transitions asserted via `data-observation-state` attributes |
| Playwright e2e (`builder.spec.ts`) | 30 tests covering Collection List, New Collection, Save/Edit Flow, Import YAML, Run + Observation (including full 4-hat chain state machine with mocked RPC, edge-fire toggle, legend visibility, properties-panel toggle), Node deletion via Backspace and Delete, Blueprint cards, and Run Error Feedback |
| UI flows | Manual verification against acceptance criteria |

### Known Limitations

- 500ms polling latency: events appear with up to half a second delay
- Single-edge animation: rapid successive events cancel the previous
  edge's animation
- Navigation replay gap: returning mid-loop may show some nodes as
  idle if the replay buffer doesn't cover full history
- Config mismatch: Run button uses existing ralph.yml, not the
  collection's YAML directly
- No streaming log output in PropertiesPanel during observation
