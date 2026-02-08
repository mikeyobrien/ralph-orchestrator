# Agent Teams Feature Research

**Source:** <https://code.claude.com/docs/en/agent-teams>

## Overview

Agent Teams is an experimental Claude Code feature that coordinates multiple Claude instances working together. One session is the "team lead" that spawns teammates, assigns work, and synthesizes results.

## Enabling

- Environment variable: `CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS=1`
- Or in `settings.json`: `{ "env": { "CLAUDE_CODE_EXPERIMENTAL_AGENT_TEAMS": "1" } }`
- Disabled by default

## Architecture

| Component     | Role |
|:-------------|:-----|
| Team lead    | Main session that creates team, spawns teammates, coordinates |
| Teammates    | Separate Claude instances working on assigned tasks |
| Task list    | Shared work items at `~/.claude/tasks/{team-name}/` |
| Mailbox      | Messaging system between agents |

## Internal Tools Used

Agent Teams uses these Claude Code tools internally for coordination:

- `TaskCreate` — Create tasks in shared task list
- `TaskUpdate` — Update task status, assign to teammates
- `TaskList` — View all tasks and their status
- `TaskGet` — Get details of a specific task
- `TeamCreate` — Create a new team
- `SendMessage` — Inter-agent messaging

**Critical**: Ralph's `claude_interactive()` currently **disallows** `TaskCreate`, `TaskUpdate`, `TaskList`, `TaskGet`. These MUST be re-allowed when teams are enabled, or Agent Teams will be non-functional.

`TodoWrite` should remain disallowed (it's a different feature).

## Display Modes

- **In-process** (default): All teammates in main terminal. Shift+Up/Down to select.
- **Split panes**: Each teammate in own pane. Requires tmux or iTerm2.
- Controlled via `--teammate-mode` flag or `teammateMode` in settings.json

## Best Use Cases for PDD

1. **Parallel research** (Step 4): Multiple teammates investigate different topics simultaneously
2. **Adversarial design review** (Step 6): Critic teammate challenges the design
3. **Competing hypotheses**: Multiple approaches explored in parallel

## Limitations (Relevant)

- No session resumption with in-process teammates
- Task status can lag
- Shutdown can be slow
- One team per session
- No nested teams
- Significantly more token usage
- **Claude-only**: Other backends don't support Agent Teams

## Permissions

- Teammates inherit lead's permission settings
- If lead uses `--dangerously-skip-permissions`, all teammates do too
- Can't set per-teammate modes at spawn time

## Key Insight for Design

The SOP needs to give Claude **specific guidance** on when/how to spawn teams. Without it, Claude may or may not use teams effectively. The PDD SOP should:

- Explicitly instruct spawning teammates for research topics
- Instruct spawning a critic for design review
- Include synthesis steps where the lead consolidates teammate findings
- Keep user gates intact (teams don't change the user-driven flow)
