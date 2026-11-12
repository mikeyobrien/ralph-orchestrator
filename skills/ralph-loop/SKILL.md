---
name: ralph-loop
description: "Operate Ralph loops: run, monitor, measure, resume, merge, debug. Use for `ralph run` or `ralph loops`, headless runs, A/B comparing hat collections, suspended loops, or merge queue issues."
---

# Ralph Loop

Use this skill to operate Ralph loops from the outside.

## Use This Skill For

- Starting or continuing a Ralph run with the right `-c` and `-H` inputs
- Inspecting loop state, worktrees, logs, history, and diffs
- Resuming a hook-suspended loop
- Merging or discarding completed worktree loops
- Debugging unexpected loop behavior with current diagnostics files
- Running a loop headlessly, measuring its cost and iterations, or A/B comparing two hat collections

## Workflow

1. Start with `ralph loops list` or `ralph loops list --json` to establish the
   current state.
2. If the user wants execution, run `ralph run ...` with the right core config
   and hats source.
3. If the loop is stuck or suspicious, inspect `logs`, `history`, and `diff`
   before changing state.
4. If the loop is suspended, read `.ralph/suspend-state.json` and use
   `ralph loops resume <id>`.
5. If a loop is queued or in `needs-review`, inspect the diff first, then use
   `merge`, `process`, `retry`, or `discard` as appropriate.
6. Use diagnostics when you need detailed evidence about hats, events, tool
   calls, parse errors, or performance.
7. For an unattended run, or to report what a run did and cost, follow
   `references/headless-runs.md`: isolate the workspace, launch with
   `--autonomous`, watch the events file, and measure from the run log.

## Guardrails

- Prefer the CLI over direct edits to `.ralph` state files.
- Treat tasks and memories as the canonical runtime systems; do not center
  scratchpad as the primary state model.
- Inspect diffs before merging.
- Only remove lock or queue artifacts when the underlying process is confirmed
  dead.
- Manual edits under `.ralph/` are last-resort recovery steps and should be
  called out explicitly when used.

## Read These References When Needed

- For command recipes and operator flows: `references/commands.md`
- For diagnostics files and suspend-state details: `references/diagnostics.md`
- For headless launch, monitoring, measuring, A/B comparison, and when to step in: `references/headless-runs.md`
