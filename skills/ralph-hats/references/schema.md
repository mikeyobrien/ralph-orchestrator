# Ralph Hats Schema Notes

This reference is for user-authored hats files used with:

```bash
ralph run -c ralph.yml -H .ralph/hats/<name>.yml -p "..."
```

## Supported Top-Level Shape

Keep hats files limited to:

```yaml
name: "Optional collection name"
description: "Optional collection description"

event_loop:
  starting_event: "work.start"
  completion_promise: "LOOP_COMPLETE"

events:
  work.start:
    description: "Delegated starting event"
    on_trigger: "Kick off the workflow"
    on_publish: "Emit when the coordinator should begin work"

hats:
  planner:
    name: "Planner"
    description: "Plans the work"
    triggers: ["work.start"]
    publishes: ["plan.ready"]
    default_publishes: "plan.ready"
    instructions: |
      Plan the task.
```

## Keep These In Core Config, Not Hats Files

Do not put general runtime config in a hats file. Keep settings like these in
the core `-c` config instead:

- `max_iterations`
- `max_runtime_seconds`
- `required_events`
- backend-wide CLI configuration
- memories/tasks/hooks settings

For hats files, the `event_loop` overlay is only for the keys Ralph currently
merges from hats overlays:

- `starting_event`
- `completion_promise`

## Current Hat Fields That Matter

Each hat should use the fields Ralph supports today:

- `name`
- `description`
- `triggers`
- `publishes`
- `instructions`
- `default_publishes`
- `extra_instructions`
- `backend`
- `backend_args`
- `effort`
- `max_activations`
- `disallowed_tools`

Notes:

- `description` is required in practice and should never be empty.
- `default_publishes` is a single string, not a list.
- `backend_args` also accepts the shorthand key `args`.
- `effort` is one of `low`, `medium`, `high`, `xhigh`, `max`, and falls back to
  `cli.effort` in the core config. It reaches Claude as `--effort`; other
  backends ignore it.

Per-hat model and effort:

```yaml
hats:
  builder:
    backend:
      type: "claude"
      args: ["--model", "claude-opus-5-5"]
    effort: "high"
  critic:
    backend:
      type: "codex"
      args: ["-m", "<codex-model-id>", "-c", 'model_reasoning_effort="high"']
```

Codex takes effort through its own `-c model_reasoning_effort=...` flag, so
`effort` is not set on Codex hats.

## Reserved Trigger Rule

Do not assign these as hat triggers:

- `task.start`
- `task.resume`

Ralph reserves them for the coordinator. Use a delegated semantic event instead,
then set `event_loop.starting_event` to that event.

Good examples:

- `work.start`
- `review.start`
- `research.start`
- `build.task`

## Event Metadata

Use `events:` when custom topics need explanation. Ralph supports:

- `description`
- `on_trigger`
- `on_publish`

This is especially helpful for custom topics like `plan.ready`,
`review.section`, or `investigation.blocked`.
