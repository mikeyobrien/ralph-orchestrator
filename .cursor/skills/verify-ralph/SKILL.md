---
name: verify-ralph
description: "Drive the Ralph web dashboard the way a user does and capture evidence. Use when someone says verify it in the app, prove a dashboard change, or check Tasks, Builder, Settings, or Planning in the running UI instead of unit tests."
---

# Verify Ralph in the app

Ralph's user-facing app is the web dashboard (Tasks, Hat Builder, Settings, and Planning). This skill launches that app against a disposable workspace, drives it in a browser, and keeps the screenshots. `cargo test` and the Vitest suite do not satisfy it.

The CLI (`ralph run`, `ralph plan`) and the terminal UI are separate surfaces. `ralph tools task` writes `.ralph/agent/tasks.jsonl`. The dashboard writes `.ralph/api/tasks-v1.json`. A row in one file is not evidence for the other. `ralph run`, Builder **Run**, and `ralph plan` start a live model process. Do not click them while verifying.

## Launch

Requirements are the ones `ralph web` already documents: a Rust toolchain, Node.js 18 or newer, and npm. The repo `package.json` engines field asks for Node 22. Playwright Chromium is installed on first launch if it is missing.

From the repo root:

```bash
.cursor/skills/verify-ralph/scripts/verify-ralph launch
source /tmp/verify-ralph/<run-id>/run.env
```

`launch` prints the `source` line. Use that path. The command:

1. Builds `target/debug/ralph` and `target/debug/ralph-api` with `cargo build -p ralph-cli -p ralph-api`.
2. Creates `/tmp/verify-ralph/<run-id>/data`, runs `git init`, then `ralph init --backend claude`, and commits `ralph.yml`.
3. Starts `ralph-api` with `RALPH_API_HOST=127.0.0.1`, `RALPH_API_AUTH_MODE=trusted_local`, and `RALPH_API_WORKSPACE_ROOT` set to that data directory.
4. Starts the Vite app from `frontend/ralph-web` (`npm run dev`) with `RALPH_BACKEND_PORT` pointed at the API.
5. Opens headless Chromium (Playwright) on the Tasks page. Set `VERIFY_RALPH_HEADED=1` before launch to show the window. The browser is started with `--no-sandbox` so the same command works in a container.

Ready means all of these happened, not only that a process is listening:

- `api.log` contains `ralph-api listening`
- `curl -sf "$API_URL/health"` returns `"status":"ok"`
- `curl -sf "$DASHBOARD_URL/health"` returns `"status":"ok"` (the Vite proxy is up)
- the browser has rendered the Tasks page

Ports are chosen free at launch. Do not assume 3000 or 5173.

Do not verify with `ralph web` in this checkout. `ralph web` sets `RALPH_API_WORKSPACE_ROOT` to the source tree, so creating a task writes `.ralph/api/` into the repo you are working in. Two verification runs can run side by side only when each has its own run id, ports, and data directory. If more than one `run.env` exists, `source` the one you intend to drive. Refusing to drive a shared checkout beats writing into a developer's tasks.

Logs for this run:

- `$STATE_DIR/api.log`
- `$STATE_DIR/web.log`
- `$STATE_DIR/browser.log`

Read them before cleanup if something looks wrong. Cleanup deletes the state directory.

## Doctor

Run this before the first action, and again after a drive fails:

```bash
source /tmp/verify-ralph/<run-id>/run.env
.cursor/skills/verify-ralph/scripts/verify-ralph doctor
```

Doctor is read-only. It passes only when:

- the API, Vite, and browser PIDs recorded at launch are alive
- the API and dashboard ports are held by those processes (or their children)
- `/proc/<api-pid>/exe` is the `ralph-api` binary this run built
- `RALPH_API_WORKSPACE_ROOT` is the disposable data directory, which is outside the source checkout
- `ralph.yml` exists in that data directory
- both `/health` URLs return `"status":"ok"`
- the Tasks page shows the heading **Tasks**, the **Create Task** button, and the **RO** brand, and does not show `Error loading tasks`

A health JSON of `"status":"ok"` is not enough on its own. Doctor also requires the Tasks page to render.

## Drive

Source `run.env` first. Every browser command goes through the harness so the session, viewport, and evidence log stay attached to this run:

```bash
.cursor/skills/verify-ralph/scripts/verify-ralph browser goto /tasks
.cursor/skills/verify-ralph/scripts/verify-ralph browser click --role button --name "Create Task"
.cursor/skills/verify-ralph/scripts/verify-ralph browser fill --role textbox --name "Task description" --value "Release checklist"
.cursor/skills/verify-ralph/scripts/verify-ralph browser fill --placeholder "Collection name" --value "Release flow"
.cursor/skills/verify-ralph/scripts/verify-ralph browser fill --label "Name" --value "Imported flow"
.cursor/skills/verify-ralph/scripts/verify-ralph browser text --contains "Pending"
.cursor/skills/verify-ralph/scripts/verify-ralph browser absent --contains "Error loading tasks"
.cursor/skills/verify-ralph/scripts/verify-ralph browser screenshot --path "$EVIDENCE_DIR/tasks-list.png"
.cursor/skills/verify-ralph/scripts/verify-ralph browser snapshot --path "$EVIDENCE_DIR/tasks-list.aria.txt"
```

Locator flags are `--role` plus `--name` (substring of the accessible name), `--placeholder`, `--label`, or `--text`. `--value-file` fills from a file when the value is multiline. `--timeout` is milliseconds. `browser dialog --action accept` answers the next `window.confirm` with OK. The default is to dismiss, which cancels a delete.

`browser goto` waits until the document has text. It does not wait for `networkidle`. The task list polls every 5 seconds, so the network never goes idle.

Stable handles:

| What the user sees | How to target it |
| --- | --- |
| Tasks, Builder, Settings | `--role link --name "Tasks"` (also `Builder`, `Settings`) |
| Task description | `--role textbox --name "Task description"` |
| Preset | `--role combobox --name "Preset"` |
| Create Task | `--role button --name "Create Task"` |
| Show closed / Hide closed | `--role button --name "Show closed"` or `"Hide closed"` |
| New Collection, Import YAML, Save, Create Task | `--role button --name "<label>"` |
| Collection name, Description | `--placeholder "Collection name"`, `--placeholder "Description"` |
| Settings editor | `--placeholder "# Ralph configuration"` |
| Active Collection | `--label "Active Collection"` |
| Import name | `--label "Name"` |
| Close import | `--role button --name "Close"` |
| Planning prompt | `--placeholder "What would you like to plan?"` |
| Start Planning | `--role button --name "Start Planning"` |

The page title is `Ralphbot Mission Control`. The sidebar shows **RO** and **Alpha**.

Keyboard submit on Tasks and Planning is Ctrl+Enter in this environment (`--key "Control+Enter"` on the textbox). The button is the path to script first.

Follow `features/README.md`, then the feature file. A passing screenshot of a different page does not cover a feature you did not drive.

## Evidence

Write proof under `$EVIDENCE_DIR` (`/tmp/verify-ralph-evidence/<run-id>`). The harness appends every browser command to `$EVIDENCE_DIR/commands.log`. Cleanup copies the session recording to `$EVIDENCE_DIR/session.webm` and leaves the directory in place.

For each feature, capture:

- the user action (filled form, or the click that submits it) and the resulting screen, not only the final screen
- an ARIA snapshot of the resulting screen (`.aria.txt`). The snapshot file starts with the page URL and `Ralphbot Mission Control`
- the side effect on disk, in a second view that is not the same screen: the tasks file, the collections file, `ralph.yml`, or the planning session file

A task is not verified because the button spinner stopped. Reload the page and find the title in `.ralph/api/tasks-v1.json`. A settings save is not verified because the green **Saved** label appeared. Read `ralph.yml`. Do not mock `/rpc/v1`. The dashboard must be talking to the `ralph-api` this run started.

`ralph init --backend claude` only writes config. It does not call Claude. Creating a task marks it **Pending** and does not start a model. Starting a planning session writes a session file and does not start a model. Builder **Run** does start `ralph run`. Leave it alone.

## Cleanup

Stop only the processes this run started:

```bash
source /tmp/verify-ralph/<run-id>/run.env
.cursor/skills/verify-ralph/scripts/verify-ralph cleanup
test -d "$EVIDENCE_DIR"
```

Cleanup sends SIGTERM to the browser, Vite, and API process groups recorded in `run.env`, then deletes `$STATE_DIR` (the data directory and the logs). It does not delete `$EVIDENCE_DIR`. If `test -d "$EVIDENCE_DIR"` fails, cleanup is wrong. Run cleanup after a failed launch too, so the next attempt does not inherit a stray port. Do not kill `ralph-api`, `vite`, or `chrome` by name.

## Helpers

The harness is `.cursor/skills/verify-ralph/scripts/verify-ralph`. It is the only supported way to start and click the dashboard for this skill.

```bash
.cursor/skills/verify-ralph/scripts/verify-ralph launch
.cursor/skills/verify-ralph/scripts/verify-ralph doctor
.cursor/skills/verify-ralph/scripts/verify-ralph browser goto /tasks
.cursor/skills/verify-ralph/scripts/verify-ralph cleanup
```

`scripts/browser.mjs` is the Playwright session `verify-ralph browser` talks to. Call it through `verify-ralph browser`, which loads `run.env`.

## Maintenance

When the dashboard changes, update this skill with `/maintain-verification-skill` instead of patching the map from memory.
