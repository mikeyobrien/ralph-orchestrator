# Ralph dashboard verification map

This directory is the maintained source for verifying the Ralph web dashboard. Read this index, then drive the feature file that matches the change. The harness is `verify-ralph`, documented in `../SKILL.md`.

## Baseline preconditions

- Launch with `.cursor/skills/verify-ralph/scripts/verify-ralph launch` and source the printed `run.env`.
- The dashboard URL is `$DASHBOARD_URL`. The API URL is `$API_URL`. Ports change every run.
- The disposable workspace is `$DATA_DIR`. It already contains `ralph.yml` from `ralph init --backend claude`. It contains no tasks and no hat collections.
- `$DATA_DIR` is outside the source checkout. Never point the API at the repo you are editing.
- Run `verify-ralph doctor` and require a pass before the first action.
- Never drive an instance that this run did not start. In particular, do not drive `ralph web` inside the source checkout.

## Driving conventions

- Start every recipe from the baseline state unless its preconditions say otherwise.
- Prefer the accessible names and placeholders listed in each feature file.
- Treat every command as literal. Keep quoted names and flags unchanged.
- Run browser actions through `verify-ralph browser`.
- The task list polls every 5 seconds. Wait for text on the page, not for the network to go idle.
- Restore nothing in the disposable workspace. The next launch creates a new one. Do not remove proof artifacts during cleanup.

## Proof and skip reporting

- Capture the user action and the resulting state, not only the final screen.
- UI proof is a screenshot plus an ARIA snapshot written under `$EVIDENCE_DIR`. The snapshot header includes the page URL and title `Ralphbot Mission Control`.
- Mutation proof includes a read of the file the dashboard wrote, after a reload when the feature is a list.
- Record the feature id in the artifact file name.
- Report an unreachable control with the command you ran and the missing name or precondition. Do not click by coordinates instead.
- Do not report a skipped entry point as verified through a different path.

## Feature entry contract

Each feature file starts with an H1 and one paragraph of user-visible behavior, then these four H2 sections in order: `Sub-features`, `How to get to it (user POV)`, `Driving it with verify-ralph`, and `Gotchas`.

## Features

- [Tasks](./tasks.md) covers the empty state, creating a task, pending status, reload, the on-disk task record, the closed-task toggle, and the detail page.
- [Hat collections](./hat-collections.md) covers the builder list, a saved collection, a blueprint, and YAML import.
- [Settings](./settings.md) covers reading `ralph.yml` in the editor and saving a change back to disk.
- [Planning](./planning.md) covers opening Planning and starting a session that shows the prompt again.
