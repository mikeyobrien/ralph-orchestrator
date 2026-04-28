---
status: draft
gap_analysis: null
related:
  - hat-collections.spec.md
---

# Workflow Builder UI Edit Controls

## Overview

The web dashboard's Builder page (`/builder`) is a visual workflow editor for
hat collections. Users drag hats from a palette, connect them via event edges,
and save the resulting graph. Before this spec, the page supported creation
and export but had no controls for deleting individual edges, recovering from
accidentally-orphaned edges, importing YAML back into a collection, or guarding
against lost unsaved work.

This spec defines the edit controls the Builder must expose so a user can
maintain a collection end-to-end from the UI, plus the YAML round-trip that
completes the export → edit → re-import loop.

## Goals

1. **Deletable edges and nodes** — keyboard and mouse affordances for removing
   individual elements without re-saving the whole graph manually
2. **No orphaned edges** — when a user removes an event from a hat's
   `publishes` or `triggers`, the connected edges must be cleaned up
3. **YAML round-trip** — users can re-import a previously exported YAML file
   (or any compatible preset) via the UI
4. **Readable imports** — re-imported graphs land on a layered layout rather
   than a single vertical column
5. **Edit safety** — unsaved changes are detected and the user is asked to
   confirm before losing them

## Non-Goals

- Chat interface or conversational UI — Ralph is a loop runner, not a chat product
- Theme system or color customization — one dark theme is sufficient for alpha
- Live loop output streaming in the Builder — separate feature, different page
- Sidebar navigation blocking via `useBlocker` — requires router migration to
  `createBrowserRouter`, out of scope for edit controls
- Fixing the exported YAML hat key format (`planner-a1b2c3d4`) — pre-existing
  backend behavior

## Design

### Edge Controls

The Builder uses React Flow v12. Edge deletion is surfaced through
React Flow's built-in events; no custom delete affordance lives inside the
edge component itself.

- **Keyboard deletion** — `deleteKeyCode={["Backspace", "Delete"]}` on `<ReactFlow>`.
  Both keys are bound because Mac keyboards require `Fn + Backspace` to produce
  `Delete`, making a Delete-only binding unreachable for most Mac users. This is
  safe because the Builder's text inputs (toolbar, PropertiesPanel) are rendered
  outside the `<ReactFlow>` component tree — key events in those inputs do not
  bubble into React Flow's key handler.
- **Selection feedback** — the `OffsetEdge` component reads React Flow's
  `selected` prop and increases stroke width and glow opacity while selected
  so users see what keyboard deletion will affect.

### Node Deletion and Edge Cleanup

React Flow's `onNodesChange` removes nodes from state when the user presses
Delete, but it does **not** remove the connected edges. An `onNodesDelete`
handler on `<ReactFlow>` explicitly filters edges whose `source` or `target`
matches any deleted node's id.

The pre-existing `handleDeleteNode` path (the "Delete Hat" button in the
PropertiesPanel) already filters edges; the new `onNodesDelete` handler
covers the keyboard path so both entry points produce the same result.

### Edge Sync When Publishes or Triggers Change

The PropertiesPanel allows the user to edit a hat's `publishes` and
`triggersOn` arrays. Each trigger in `triggersOn` renders a target handle
with `id={triggerEvent}`; each event in `publishes` renders a source handle
with `id={publishEvent}`. Edges are stored with `sourceHandle` /
`targetHandle` equal to the event name.

When the user removes an event from either array, any edge whose
`sourceHandle` or `targetHandle` references the removed event becomes
invalid (it points at a handle that no longer renders). The Builder filters
those edges on the same change. A `console.info` reports the removal count
so developers can observe the cleanup.

### YAML Import

A new `ImportYamlDialog` component provides a modal with:
- Text area for pasting YAML
- File input that accepts `.yml` / `.yaml` and reads via `FileReader`
- Required `name` input (seeded from the uploaded filename when empty)
- Optional `description` input

The dialog delegates YAML parsing to the backend via the existing
`collection.import` RPC (no frontend YAML parser dependency is introduced).
Parse errors are surfaced inline using the same error banner style as
`SettingsPage`.

### Auto-Layout on Fresh Import

The Rust YAML importer produces a graph with every node at `x = 250` and
`y` increasing by 200 per node — a single vertical column. Opening that
graph in the Builder is ugly. A pure `autoLayout` function in a new
`layout.ts` module runs a Kahn-style BFS over the graph and assigns each
node to a left-to-right "level" based on its distance from the in-degree-0
roots, laying the graph out as a layered column-per-level grid.

Auto-layout fires exactly once: on mount of `CollectionBuilder` when the
component receives `justImported={true}` and the `needsLayout` heuristic
matches the importer's stacked output. Subsequent renders never re-layout.
The resulting graph is marked dirty so the user can persist the layout by
clicking Save.

Cycles and disconnected subgraphs are handled:
- Back-edges (cycles) are ignored via first-seen-wins level assignment so
  BFS always terminates
- In-degree-0 nodes are all placed at level 0, even with no outgoing edges
- Pure-cycle graphs (no roots at all) seed level 0 with the first node

### Role-Based Visual Identity

Each hat node displays an emoji and border color derived from its role
(planner, builder, reviewer, validator, confessor, custom). The mapping
lives in a shared `roles.ts` module consumed by both `HatNode` (border +
emoji) and `CollectionBuilder`'s `MiniMap` (node color), so the two views
cannot drift apart.

The role is extracted from the node's `data.key` field by stripping the
8-hex-character UUID suffix that `onDrop` appends when a hat is dragged
from the palette. Unknown roles fall back to a `custom` entry with a
neutral 🎩 emoji and the default border color. The fallback is mandatory:
`HatNode` reads `getRoleMeta(...).emoji` unconditionally, so losing the
fallback would crash the render.

### Dirty-State and Save Feedback

The dirty flag lives in `BuilderPage` (not in `CollectionBuilder`) so edits
to the Name and Description inputs in the page header flip dirty along
with graph edits. `CollectionBuilder` receives the flag as a prop and
calls an `onMarkDirty` callback on every internal change.

A `Badge` in the Builder toolbar shows "Unsaved changes" whenever the
flag is true. A `saveStatus: "idle" | "success" | "error"` prop drives
post-save feedback using the same `CheckCircle2` / `AlertCircle` icons
and three-second auto-clear as `SettingsPage`.

Unsaved-work protection:
- A `beforeunload` effect (gated on `isDirty`) prompts before tab close
  or reload
- `handleBack` in `BuilderPage` calls `confirm("Discard unsaved changes?")`
  before returning to the list view
- Sidebar navigation cannot be blocked (the app uses `<BrowserRouter>`
  rather than a data router, so `useBlocker` is not available); this is
  an accepted limitation

### Just-Imported One-Shot

A `justImportedId` ref in `BuilderPage` records the id of the last
imported collection. `CollectionBuilder` receives
`justImported = (selectedId === justImportedId.current)` as a prop. This
guard:
- Triggers auto-layout only for the freshly imported graph
- Naturally false when the user opens a different collection (since
  `selectedId` changes)
- Is also explicitly cleared in `handleBack` and in the update mutation's
  `onSuccess` so it does not fire a second time after a save or navigation

## Acceptance Criteria

### Edge Deletion — Keyboard

- **Given** an edge between two hats is selected on the canvas
- **When** the user presses `Backspace` or `Delete`
- **Then** the edge is removed and the dirty indicator appears

### Backspace Does Not Delete Canvas Elements When Input Is Focused

- **Given** the canvas contains a selected edge and the user's cursor is
  in the Collection Name input
- **When** the user presses `Backspace`
- **Then** a character is deleted from the input and the edge remains
  (because the input is outside the `<ReactFlow>` tree)

### Node Deletion Cleans Up Edges

- **Given** a hat node with incoming and outgoing edges is selected
- **When** the user presses `Delete`
- **Then** the node is removed and every edge referencing that node is
  also removed

### Edge Sync — Publish Removed

- **Given** a hat's `publishes` array contains `subtask.ready` and an
  outgoing edge labelled `subtask.ready` exists
- **When** the user removes `subtask.ready` from the hat's publishes
- **Then** the edge is filtered out of the graph

### Edge Sync — Trigger Removed

- **Given** a hat's `triggersOn` array contains `build.done` and an
  incoming edge labelled `build.done` exists
- **When** the user removes `build.done` from the hat's triggers
- **Then** the edge is filtered out of the graph

### Selection Feedback

- **Given** an edge exists on the canvas
- **When** the user clicks it to select it
- **Then** the edge renders with increased stroke width and glow
  opacity relative to the unselected state

### YAML Import — Paste

- **Given** the user opens the Import YAML dialog
- **When** the user pastes a valid preset YAML, enters a name, and
  clicks Import
- **Then** a new collection is created and the Builder opens it in
  edit view

### YAML Import — File Upload

- **Given** the user opens the Import YAML dialog
- **When** the user selects a local `.yml` file
- **Then** the file contents populate the textarea and the Name field
  is seeded from the filename (if still empty)

### YAML Import — Invalid YAML

- **Given** the user has pasted malformed YAML into the Import dialog
- **When** the user clicks Import
- **Then** the backend's error message is surfaced inline in the dialog
  and the dialog remains open

### Auto-Layout on Import

- **Given** a user imports a preset YAML with three hats `a → b → c`
- **When** the Builder mounts with the imported collection
- **Then** the three nodes are positioned in ascending x-columns
  (`a.x < b.x < c.x`) and the dirty indicator is set

### Auto-Layout Does Not Re-Run on Subsequent Opens

- **Given** a previously imported collection has been saved with its
  auto-laid-out positions
- **When** the user reopens the same collection from the list
- **Then** the graph uses the persisted positions without re-layout
  and the dirty indicator is not set

### Auto-Layout Handles Cycles

- **Given** a graph `a → b → c → a`
- **When** `autoLayout` runs on it
- **Then** every node receives a finite position and the function
  terminates (no infinite loop)

### Role-Based Emoji and Border

- **Given** a user drops a `planner` template from the palette
- **When** the node renders
- **Then** its header shows `📋` and its border uses the violet
  role-specific class

### Unknown Role Fallback

- **Given** a hat node whose `data.key` does not match any known role
- **When** the node renders
- **Then** it shows the default `🎩` emoji and neutral border instead
  of crashing

### Dirty Flag — Graph Edit

- **Given** a saved collection with no pending changes
- **When** the user drags a node to a new position
- **Then** the "Unsaved changes" badge appears in the toolbar

### Dirty Flag — Metadata Edit

- **Given** a saved collection with no pending changes
- **When** the user edits the Collection Name input
- **Then** the "Unsaved changes" badge appears in the toolbar

### Save Feedback — Success

- **Given** the user has unsaved changes and clicks Save
- **When** the server responds successfully
- **Then** the badge disappears and "Saved" text with a check icon
  appears for three seconds before clearing

### Save Feedback — Error

- **Given** the user has unsaved changes and the Save mutation fails
- **When** the error comes back from the server
- **Then** "Error saving" text with an alert icon is shown and the
  dirty flag remains set

### Back Button Confirm

- **Given** the user has unsaved changes
- **When** the user clicks Back
- **Then** a confirm dialog asks whether to discard unsaved changes,
  and navigation only proceeds if the user confirms

### Tab Close Protection

- **Given** the user has unsaved changes
- **When** the user tries to close the tab or reload
- **Then** the browser's native `beforeunload` prompt appears

### Just-Imported One-Shot

- **Given** the user has imported a collection and saved it
- **When** the user navigates back to the list and reopens the same
  collection
- **Then** `justImported` is false, auto-layout does not re-run,
  and the dirty indicator is not set

## Implementation Notes

### Component Placement

| Component | File |
|-----------|------|
| `CollectionBuilder` | `frontend/ralph-web/src/components/builder/CollectionBuilder.tsx` |
| `HatNode` | `frontend/ralph-web/src/components/builder/HatNode.tsx` |
| `OffsetEdge` | `frontend/ralph-web/src/components/builder/OffsetEdge.tsx` |
| `ImportYamlDialog` | `frontend/ralph-web/src/components/builder/ImportYamlDialog.tsx` |
| `BuilderPage` | `frontend/ralph-web/src/pages/BuilderPage.tsx` |
| Role metadata | `frontend/ralph-web/src/components/builder/roles.ts` |
| Layout algorithm | `frontend/ralph-web/src/components/builder/layout.ts` |

### Dependencies

No new runtime or dev dependencies. The modal reuses the `fixed inset-0`
+ `Card` pattern from `PlanSession`. No dagre, no YAML parser, no toast
library.

### Test Coverage

Pure logic is unit-tested; UI interactions are manually verified:

| Module | Tests |
|--------|-------|
| `layout.ts` | `layout.test.ts` — linear chain, branching, cycle, disconnected nodes, empty input; `needsLayout` with stacked / varied / single / empty inputs |
| `roles.ts` | `roles.test.ts` — fallback returns the `custom` entry for unknown roles (protects against a crash in `HatNode`) |
| Builder UI flows | Covered by manual verification against the acceptance criteria above |

### Backend Contract

No backend changes. The existing `collection.import` and
`collection.export` RPCs on `ralph-api` carry the full round-trip. The
Rust YAML serializer's stacked output (`x = 250`, `y += 200`) is
compensated for client-side by `autoLayout`.

### Known Limitations

- Sidebar navigation with unsaved changes does not prompt because the
  app uses `<BrowserRouter>` rather than a data router. The Back button
  and `beforeunload` cover the most common exit paths.
- Exported YAML uses the node's full id (e.g. `planner-a1b2c3d4`) as the
  hat dictionary key because the drop handler overwrites `data.key` with
  the node id. Re-importable but cosmetically ugly. Pre-existing
  behavior; not addressed by this spec.
- Reroute nodes saved with empty `data` will fail backend schema
  validation on save because the backend's `HatNodeData` schema is
  required. Pre-existing behavior; not addressed by this spec.

## Future Considerations

- **Auto-layout toolbar button** — expose `autoLayout` as a manual action
  for users who want to tidy up existing messy collections, not just imports
- **Toast notifications** — replace `console.info` edge-removal feedback
  with a visible toast so users notice the cleanup without opening DevTools
- **Router migration** — moving from `<BrowserRouter>` to `createBrowserRouter`
  would enable `useBlocker` for full in-app navigation guarding
- **Inline edge label editing** — double-click an edge label to rename the
  event; deferred because it creates invariant violations with the hat's
  `publishes` / `triggersOn` arrays (the label must stay in sync with the
  handle ids)
