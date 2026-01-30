---
name: find-code-tasks
description: Lists all code tasks in the repository with their status, dates, and metadata. Useful for getting an overview of pending work or finding specific tasks.
type: anthropic-skill
version: "1.0"
---

# Find Code Tasks

## Overview

This skill finds and displays all code tasks (`.code-task.md` files) in the repository, showing their frontmatter status and metadata. Use it to get a quick overview of pending work, find tasks by status, or check the state of the task backlog.

## When to Use

- Starting a work session to see what tasks are available
- Checking status of tasks before/after running code-assist
- Finding tasks by status (pending, in_progress, completed)
- Getting a summary of task backlog
- Exporting task data for reporting

## Parameters

- **filter** (optional): Filter tasks by status
  - `pending` - Show only pending tasks
  - `in_progress` - Show only in-progress tasks
  - `completed` - Show only completed tasks
  - (none) - Show all tasks

- **format** (optional, default: "table"): Output format
  - `table` - Human-readable table with status symbols
  - `json` - JSON array for programmatic use
  - `summary` - Counts by status only

- **tasks_dir** (optional, default: ".ralph/tasks/"): Directory to search for tasks

## Usage Examples

```bash
# Show all tasks in table format
/find-code-tasks

# Show only pending tasks
/find-code-tasks filter:pending

# Get JSON output for tooling
/find-code-tasks format:json

# Quick summary of task counts
/find-code-tasks format:summary

# Search custom directory
/find-code-tasks tasks_dir:tools/
```

## Steps

### 1. Run Task Status Script

The script is colocated with this skill at `.claude/skills/find-code-tasks/task-status.sh`.

Execute it with appropriate arguments:

```bash
# Default: table format, all tasks
.claude/skills/find-code-tasks/task-status.sh

# With filter
.claude/skills/find-code-tasks/task-status.sh --pending
.claude/skills/find-code-tasks/task-status.sh --in_progress
.claude/skills/find-code-tasks/task-status.sh --completed

# With format
.claude/skills/find-code-tasks/task-status.sh --json
.claude/skills/find-code-tasks/task-status.sh --summary

# Custom tasks directory
TASKS_DIR=tools/ .claude/skills/find-code-tasks/task-status.sh
```

### 2. Present Results

Display the output to the user. For table format, the output includes:

| Symbol | Status |
|--------|--------|
| ○ | pending |
| ● | in_progress |
| ✓ | completed |
| ■ | blocked |

### 3. Suggest Next Actions

Based on the results, suggest relevant actions:

- If there are pending tasks: "Run `/code-assist .ralph/tasks/<task-name>.code-task.md` to start a task"
- If there are in_progress tasks: "There are tasks already in progress - consider completing those first"
- If all tasks are completed: "All tasks are done! Use `/code-task-generator` to create new tasks"

## Output Examples

### Table Format (default)

```
TASKS STATUS
════════════════════════════════════════════════════════════════
    TASK                                     STATUS       DATE
────────────────────────────────────────────────────────────────
○ add-task-frontmatter-tracking            pending      2025-01-15
○ enhance-headless-tool-output             pending      -
● fix-ctrl-c-freeze                        in_progress  2025-01-14
✓ replay-backend                           completed    2025-01-13
────────────────────────────────────────────────────────────────
Total: 4 tasks
```

### Summary Format

```
Task Summary
────────────
○ Pending:     10
● In Progress: 2
✓ Completed:   5
────────────
  Total:       17
```

### JSON Format

```json
[
  {"task": "add-task-frontmatter-tracking", "status": "pending", "created": "2025-01-15", "started": null, "completed": null},
  {"task": "fix-ctrl-c-freeze", "status": "in_progress", "created": "2025-01-14", "started": "2025-01-14", "completed": null}
]
```

## Frontmatter Schema

Tasks with frontmatter tracking have this structure:

```yaml
---
status: pending | in_progress | completed | blocked
created: YYYY-MM-DD    # Date task was created
started: YYYY-MM-DD    # Date work began (null if not started)
completed: YYYY-MM-DD  # Date work finished (null if not done)
---
```

Tasks without frontmatter are shown as `pending` with null dates.

## Integration with Other Skills

- **code-task-generator**: Creates new tasks with frontmatter
- **code-assist**: Updates task status when starting/completing work
- **ralph-code-assist**: Runs tasks through Ralph orchestrator

## Troubleshooting

### No Tasks Found

If no tasks are displayed:
- Verify the tasks directory exists: `ls .ralph/tasks/`
- Check file extension is `.code-task.md`
- Try specifying directory: `/find-code-tasks tasks_dir:./`

### Script Not Found

If the task-status.sh script is not found:
- Ensure you're in the repository root
- Check the script exists: `ls .claude/skills/find-code-tasks/task-status.sh`
- Make it executable: `chmod +x .claude/skills/find-code-tasks/task-status.sh`

### Frontmatter Not Parsed

If dates show as `-` for tasks with frontmatter:
- Ensure frontmatter starts with `---` on line 1
- Check YAML syntax is valid
- Verify field names match: `status`, `created`, `started`, `completed`
