# Planning

Planning lets a person describe a piece of work and see that description saved as a session. The dashboard records the session. It does not ask a model a follow-up question by itself.

## Sub-features

- `planning-open` shows the start form and an empty session list.
- `planning-start` saves a prompt and opens the session.
- `planning-persist` shows the same prompt on the session page and in the session file.
- `planning-back` returns to the list, which now includes the session.

## How to get to it (user POV)

- Open `$DASHBOARD_URL/plan`. Planning is not in the sidebar.
- Type a prompt and choose **Start Planning**.
- Press Ctrl+Enter in the prompt to start. On this harness that key is `Control+Enter`.
- Choose **Back** to return to the list.

## Driving it with verify-ralph

Preconditions:

- `verify-ralph doctor` passed for this run.
- `$DATA_DIR/.ralph/planning-sessions/` has no session whose prompt is `Plan a release checklist`.

- **Open.** Go to Planning. Run `verify-ralph browser goto /plan`. The heading is `Planning`. The form title is `Start New Planning Session`. The empty copy is `No planning sessions yet. Start your first one above!`.
- **Start.** Fill the prompt and choose **Start Planning**. Run `verify-ralph browser fill --placeholder "What would you like to plan?" --value "Plan a release checklist"` and `verify-ralph browser click --role button --name "Start Planning"`. The session heading reads `Plan a release checklist`, the status badge reads `active`, and **Original Prompt** shows the same sentence.
- **File.** Find the new directory under `$DATA_DIR/.ralph/planning-sessions/`. Its `session.json` contains `"prompt": "Plan a release checklist"` and `"status": "active"`.
- **Back.** Choose **Back**. Run `verify-ralph browser click --role button --name "Back"`. The list shows `Plan a release checklist` and no longer shows `No planning sessions yet`.
- **Proof.** Capture the filled form and the open session. Run `verify-ralph browser screenshot --path "$EVIDENCE_DIR/planning-filled.png"`, `verify-ralph browser screenshot --path "$EVIDENCE_DIR/planning-session.png"`, and `verify-ralph browser snapshot --path "$EVIDENCE_DIR/planning-session.aria.txt"`. Copy `session.json` to `$EVIDENCE_DIR/planning-session.json`.

## Gotchas

- Starting a session does not run `ralph plan` and does not call a model. No clarifying question appears. An empty conversation is the expected session, not a timeout.
- The sidebar has no Planning link. `goto /plan` is the entry. A click on **Tasks**, **Builder**, or **Settings** leaves this page.
- The prompt box has no accessible name. Target the placeholder `What would you like to plan?`.
- Titles longer than 60 characters are shortened in the heading. Use a short prompt and assert that heading, not a truncated guess.
