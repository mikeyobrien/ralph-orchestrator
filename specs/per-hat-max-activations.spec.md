---
status: implemented
created: 2026-01-20
related:
  - specs/event-loop/summary.md
  - https://github.com/mikeyobrien/ralph-orchestrator/issues/66
---

# Per-hat `max_activations`

## Goal

Prevent infinite feedback loops between hats (e.g., implementer -> reviewer) by allowing a per-hat cap on how many times a hat may be activated in a single run.

## Configuration

Add an optional field to each hat definition:

```yaml
hats:
  code_reviewer:
    name: "Code Reviewer"
    description: "Reviews changes and requests fixes"
    triggers: ["implementation.done"]
    publishes: ["review.changes_requested", "review.approved"]
    max_activations: 3
```

### Schema

- `max_activations: u32` (optional)
  - When omitted, the hat is unlimited.
  - When set to `N`, the hat may be activated at most `N` times.

## Definitions

- **Activation**: A hat is considered "activated" when it has pending events that would cause the orchestrator to include it as active for processing in an iteration.

## Runtime Behavior

Maintain a per-hat activation counter for the current loop run.

When the orchestrator is about to activate a hat and:

- `activation_count(hat_id) >= max_activations(hat_id)`

then:

1. Do **not** activate the hat.
2. Drop the pending events that would have activated it.
3. Publish a system event with topic: `<hat_id>.exhausted`.
   - Example: `code_reviewer.exhausted`
4. The `<hat_id>.exhausted` event payload MUST be actionable and include:
   - `hat_id`
   - `max_activations`
   - `activation_count`
   - the list of dropped event topics (one per pending event)

Notes:
- The exhaustion event is published by the orchestrator (not by an agent hat).
- The exhaustion event is intended to be handled by an optional "escalator" hat or by Ralph as fallback.
- The exhaustion event is published at most once per hat per run (to avoid flooding). Subsequent would-trigger events are dropped silently.

## Acceptance Criteria

1. **Config Parsing**
   - A config containing `hats.<id>.max_activations` loads successfully.

2. **Exhaustion Trigger**
   - Given `max_activations: 3`
   - When a 4th activation is attempted
   - Then `<hat_id>.exhausted` is published and the hat is not activated.

3. **Dropped Pending Events**
   - When exhaustion triggers, the pending events that would have activated the hat are dropped (not processed as that hat).

4. **Workflow Repro (Unit)**
   - A reviewer loop (implementation.done -> review.changes_requested) with `max_activations: 3`
   - A unit test reproduces the loop and asserts `code_reviewer.exhausted` is emitted and the hat is not activated beyond the limit.
