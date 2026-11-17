# Tasks

Tasks lets a person type a piece of work, see it show up as Pending, and find the same title again after a reload.

## Sub-features

- `tasks-empty` shows an empty dashboard before any task exists.
- `tasks-create` saves a title from the Create Task button.
- `tasks-shortcut` saves a title with Ctrl+Enter.
- `tasks-persist` shows the same title after a reload and in the task file.
- `tasks-closed-toggle` switches Show closed and Hide closed once a task is on screen.
- `tasks-detail` opens the task from the list.

## How to get to it (user POV)

- Open `$DASHBOARD_URL/`. The app redirects to `/tasks`.
- Choose **Tasks** in the sidebar.
- After a task exists, choose its card to open the detail page.
- Press Ctrl+Enter in the task description to submit. On this harness that key is `Control+Enter`.

## Driving it with verify-ralph

Preconditions:

- `verify-ralph doctor` passed for this run.
- `$DATA_DIR/.ralph/api/tasks-v1.json` does not exist yet, or its `tasks` array is empty.
- No visible task is titled `Release checklist` or `Shortcut checklist`.

- **Empty state.** Open Tasks. Run `verify-ralph browser goto /tasks` and `verify-ralph browser text --contains "No tasks yet"`. The page also shows `Give Ralph something to do!` and **Create Task**. **Show closed** is not on this screen.
- **Button create.** Fill the description and choose **Create Task**. Run `verify-ralph browser fill --role textbox --name "Task description" --value "Release checklist"` and `verify-ralph browser click --role button --name "Create Task"`. The list shows `Release checklist` and `Pending`.
- **Reload.** Load Tasks again. Run `verify-ralph browser goto /tasks` and `verify-ralph browser text --contains "Release checklist"`. The title is still there.
- **File.** Read `$DATA_DIR/.ralph/api/tasks-v1.json`. The `tasks` array contains an object whose `title` is `Release checklist` and whose `status` is `pending`.
- **Shortcut create.** Fill a second title and press Ctrl+Enter in the description. Run `verify-ralph browser fill --role textbox --name "Task description" --value "Shortcut checklist"` and `verify-ralph browser press --role textbox --name "Task description" --key "Control+Enter"`. The list shows `Shortcut checklist`.
- **Closed toggle.** With a task visible, choose **Show closed**. Run `verify-ralph browser click --role button --name "Show closed"`. The button name becomes **Hide closed**. Choose it again with `verify-ralph browser click --role button --name "Hide closed"`. The name returns to **Show closed**.
- **Detail.** Choose the `Release checklist` card. Run `verify-ralph browser click --role button --name "Release checklist"`. The detail view shows that title and **Back to Tasks**.
- **Proof.** Capture the filled field before submit, the list after submit, and the list after reload. Run `verify-ralph browser screenshot --path "$EVIDENCE_DIR/tasks-filled.png"`, `verify-ralph browser screenshot --path "$EVIDENCE_DIR/tasks-list.png"`, `verify-ralph browser snapshot --path "$EVIDENCE_DIR/tasks-list.aria.txt"`, and `verify-ralph browser screenshot --path "$EVIDENCE_DIR/tasks-reloaded.png"`. Copy the task JSON to `$EVIDENCE_DIR/tasks-v1.json`.

## Gotchas

- **Create Task** queues the task. The badge reads **Pending**, not Open. Nothing in this step starts Claude or `ralph run`.
- The empty state returns before the toolbar is rendered. **Show closed** appears only after at least one task is visible. Do not fail the empty state for a missing toggle.
- The list polls every 5 seconds. Wait until the title is visible.
- The preset dropdown is UI state. The create request does not send the selected preset to `task.create`.
- Opening the card is a separate entry point from the list. The detail header's status map has no `pending` entry, so a just-created task throws `Cannot read properties of undefined (reading 'icon')` instead of showing **Back to Tasks**. Vite then paints an error overlay. Save that screenshot, leave `tasks-detail` unverified, and keep the reload plus `tasks-v1.json` as the proof of create. Do not describe the detail page as passed.
- `ralph tools task` and `ralph task` read `.ralph/agent/tasks.jsonl`. That file staying empty does not disprove a dashboard task.
