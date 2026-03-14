---
name: ralph-hats
description: Create, inspect, validate, explain, and improve Ralph hat collections. Use this skill whenever the user asks to make or refine a `.ralph/hats/*.yml` workflow, debug hat routing, explain event topology, or tune a multi-hat Ralph run.
---

# Ralph Hats

Use this skill to operate the full Ralph hat lifecycle for user-authored hat
collections.

## Use This Skill For

- Creating a new hat collection in `.ralph/hats/`
- Inspecting an existing hat collection and explaining its topology
- Validating trigger routing, event flow, and completion behavior
- Improving or refactoring hats for clearer roles and safer routing
- Recommending better orchestration patterns for a Ralph workflow

## Core Assumptions

- Core runtime config already lives in `ralph.yml` or another `-c` source.
- User-authored hats are stored separately and passed with `-H`.
- This skill operates public hat collections, not Ralph built-in presets.

## Workflow

1. If a hats file already exists, read it first and explain the current
   topology before proposing changes.
2. If creating a new workflow, write it to `.ralph/hats/<name>.yml`.
3. Keep the hats file focused on hats-only data. Leave runtime limits and other
   core config in the main config file.
4. Validate with `ralph hats validate`.
5. Visualize topology with `ralph hats graph` when the event flow is not
   trivial.
6. Use `ralph hats show <hat>` when you need to inspect one hat's effective
   configuration.
7. When the user wants stronger confidence, run a targeted `ralph run -c ... -H
   ... -p "..."` exercise or provide the exact test command.

## Guardrails

- Only use hats-file top-level keys that Ralph accepts today:
  `name`, `description`, `events`, `event_loop`, `hats`.
- In a hats file, `event_loop` is only for hats overlay keys such as
  `starting_event` and `completion_promise`.
- Never use `task.start` or `task.resume` as hat triggers. Ralph reserves those
  for coordination. Use semantic delegated events like `work.start`,
  `review.start`, or `research.start`.
- Each trigger must route to exactly one hat.
- Keep `description` populated on every hat.
- Prefer `events:` metadata when custom event names would otherwise be opaque.
- Do not write user workflows into `presets/` from this skill.

## Output Expectations

- When editing or creating hats, produce the file changes and the validation
  result.
- When only inspecting, produce a concise topology summary, the main risks, and
  concrete improvement options.

## Read These References When Needed

- For current hats schema and supported fields: `references/schema.md`
- For command recipes and validation workflow: `references/commands.md`
- For pattern and file examples: `references/examples.md`
