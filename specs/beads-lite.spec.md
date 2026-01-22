# Task Tracking for Ralph

**Status:** ✅ IMPLEMENTED (2026-01-22)

## Problem Statement

When memories are enabled, scratchpad is disabled. But the completion verification still requires scratchpad task state (`- [ ]` markers) to confirm loop termination. This causes infinite loops because:

1. Agent says `LOOP_COMPLETE`
2. Verification checks scratchpad → doesn't exist → resets confirmations to 0
3. Loop continues forever

More fundamentally, memories are for **long-term knowledge** (patterns, decisions, fixes, context) — not ephemeral task tracking. We need a separate system for tasks.

## Solution

Build a lightweight task tracking system inspired by [Steve Yegge's Beads](https://github.com/steveyegge/beads). This provides:

- **Structured task data** (JSONL, not markdown checkboxes)
- **Hash-based IDs** for multi-agent collision avoidance
- **Dependencies** for "ready" task detection
- **Git-backed persistence** (fits Ralph tenet: "Disk Is State, Git Is Memory")

Tasks and memories serve different purposes:
- **Memories:** Accumulated wisdom across sessions (persistent)
- **Tasks:** Current work items (ephemeral, closed when done)

## Data Model

### Task Structure

```rust
pub struct Task {
    /// Unique ID: task-{unix_timestamp}-{4_hex_chars}
    pub id: String,

    /// Short description
    pub title: String,

    /// Optional detailed description
    pub description: Option<String>,

    /// Current state
    pub status: TaskStatus,

    /// Priority 1-5 (1 = highest)
    pub priority: u8,

    /// Tasks that must complete before this one
    pub blocked_by: Vec<String>,

    /// Creation timestamp (ISO 8601)
    pub created: String,

    /// Completion timestamp (ISO 8601), if closed
    pub closed: Option<String>,
}

pub enum TaskStatus {
    Open,       // Not started
    InProgress, // Being worked on
    Closed,     // Complete
}
```

### Storage Format

Tasks stored in `.agent/tasks.jsonl` (one JSON object per line):

```jsonl
{"id":"task-1737372000-a1b2","title":"Build calculator CLI","status":"open","priority":2,"blocked_by":[],"created":"2025-01-20T10:00:00Z","closed":null}
{"id":"task-1737372100-c3d4","title":"Add tests","status":"open","priority":3,"blocked_by":["task-1737372000-a1b2"],"created":"2025-01-20T10:01:40Z","closed":null}
```

## Implementation

### Phase 1: Core Types

**File:** `crates/ralph-core/src/task.rs`

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Open,
    InProgress,
    Closed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    pub id: String,
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub status: TaskStatus,
    pub priority: u8,
    #[serde(default)]
    pub blocked_by: Vec<String>,
    pub created: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub closed: Option<String>,
}

impl Task {
    pub fn new(title: String, priority: u8) -> Self {
        Self {
            id: Self::generate_id(),
            title,
            description: None,
            status: TaskStatus::Open,
            priority: priority.clamp(1, 5),
            blocked_by: Vec::new(),
            created: chrono::Utc::now().to_rfc3339(),
            closed: None,
        }
    }

    pub fn generate_id() -> String {
        use std::time::{SystemTime, UNIX_EPOCH};
        let duration = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards");
        let timestamp = duration.as_secs();
        let hex_suffix = format!("{:04x}", duration.subsec_micros() % 0x10000);
        format!("task-{}-{}", timestamp, hex_suffix)
    }

    /// Returns true if this task is ready to work on (open + no blockers pending)
    pub fn is_ready(&self, all_tasks: &[Task]) -> bool {
        if self.status != TaskStatus::Open {
            return false;
        }
        self.blocked_by.iter().all(|blocker_id| {
            all_tasks
                .iter()
                .find(|t| &t.id == blocker_id)
                .is_some_and(|t| t.status == TaskStatus::Closed)
        })
    }
}
```

### Phase 2: Task Store

**File:** `crates/ralph-core/src/task_store.rs`

```rust
use crate::task::{Task, TaskStatus};
use std::path::Path;

pub struct TaskStore {
    path: std::path::PathBuf,
    tasks: Vec<Task>,
}

impl TaskStore {
    pub fn load(path: &Path) -> std::io::Result<Self> {
        let tasks = if path.exists() {
            let content = std::fs::read_to_string(path)?;
            content
                .lines()
                .filter(|line| !line.trim().is_empty())
                .filter_map(|line| serde_json::from_str(line).ok())
                .collect()
        } else {
            Vec::new()
        };
        Ok(Self {
            path: path.to_path_buf(),
            tasks,
        })
    }

    pub fn save(&self) -> std::io::Result<()> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content: String = self
            .tasks
            .iter()
            .map(|t| serde_json::to_string(t).unwrap())
            .collect::<Vec<_>>()
            .join("\n");
        std::fs::write(&self.path, content + "\n")
    }

    pub fn add(&mut self, task: Task) -> &Task {
        self.tasks.push(task);
        self.tasks.last().unwrap()
    }

    pub fn get(&self, id: &str) -> Option<&Task> {
        self.tasks.iter().find(|t| t.id == id)
    }

    pub fn get_mut(&mut self, id: &str) -> Option<&mut Task> {
        self.tasks.iter_mut().find(|t| t.id == id)
    }

    pub fn close(&mut self, id: &str) -> Option<&Task> {
        if let Some(task) = self.get_mut(id) {
            task.status = TaskStatus::Closed;
            task.closed = Some(chrono::Utc::now().to_rfc3339());
            return self.get(id);
        }
        None
    }

    pub fn all(&self) -> &[Task] {
        &self.tasks
    }

    pub fn open(&self) -> Vec<&Task> {
        self.tasks
            .iter()
            .filter(|t| t.status != TaskStatus::Closed)
            .collect()
    }

    pub fn ready(&self) -> Vec<&Task> {
        self.tasks
            .iter()
            .filter(|t| t.is_ready(&self.tasks))
            .collect()
    }

    pub fn has_open_tasks(&self) -> bool {
        self.tasks.iter().any(|t| t.status != TaskStatus::Closed)
    }
}
```

### Phase 3: Update Completion Verification

**File:** `crates/ralph-core/src/event_loop.rs`

Replace scratchpad verification with task verification when memories enabled:

```rust
// Check for completion promise
if hat_id.as_str() == "ralph"
    && EventParser::contains_promise(output, &self.config.event_loop.completion_promise)
{
    // When memories enabled, check tasks instead of scratchpad
    let verification_result = if self.config.memories.enabled {
        self.verify_tasks_complete()
    } else {
        self.verify_scratchpad_complete()
    };

    match verification_result {
        Ok(true) => {
            self.state.completion_confirmations += 1;
            // ... rest unchanged
        }
        // ... rest unchanged
    }
}

fn verify_tasks_complete(&self) -> Result<bool, std::io::Error> {
    let tasks_path = Path::new(&self.workspace_root)
        .join(".agent")
        .join("tasks.jsonl");

    // No tasks file = no pending tasks = complete
    if !tasks_path.exists() {
        return Ok(true);
    }

    let store = TaskStore::load(&tasks_path)?;
    Ok(!store.has_open_tasks())
}
```

### Phase 4: CLI Commands

**File:** `crates/ralph-cli/src/main.rs`

Add task subcommand:

```
ralph task add "Build calculator" -p 2
ralph task add "Add tests" -p 3 --blocked-by task-xxx
ralph task list
ralph task ready
ralph task close task-xxx
ralph task show task-xxx
```

### Phase 5: Agent Instructions

**File:** `crates/ralph-core/src/hatless_ralph.rs`

When memories enabled, include task instructions instead of scratchpad:

```rust
if self.include_tasks {
    prompt.push_str(r#"
### 0b. TASKS
Track work in `.agent/tasks.jsonl`. Use `ralph task` CLI:

```bash
ralph task add "Title" -p 2           # Create task (priority 1-5)
ralph task add "X" --blocked-by Y     # Create with dependency
ralph task list                        # Show all tasks
ralph task ready                       # Show unblocked tasks
ralph task close <id>                  # Mark complete
```

Before saying LOOP_COMPLETE, close all tasks.
"#);
}
```

## Configuration

No new config needed. Tasks are enabled automatically when `memories.enabled: true`.

To use tasks without memories, add explicit flag:

```yaml
# ralph.yml
tasks:
  enabled: true  # Enable task tracking
```

## Acceptance Criteria

1. `ralph task add/list/ready/close` CLI commands work
2. Tasks stored in `.agent/tasks.jsonl` as JSONL
3. Hash IDs prevent collision: `task-{timestamp}-{hex}`
4. `ralph task ready` shows only unblocked open tasks
5. Loop completion checks tasks (not scratchpad) when memories enabled
6. Loop terminates correctly with consecutive LOOP_COMPLETE when no open tasks
7. `cargo test` passes
8. Smoke tests pass

## Test Plan

```bash
# Unit tests
cargo test -p ralph-core task

# Integration test: task lifecycle
TMPDIR=$(mktemp -d)
cd $TMPDIR
ralph task add "Test task" -p 1
ralph task list                    # Should show 1 open task
ralph task ready                   # Should show 1 ready task
ralph task close task-xxx
ralph task list                    # Should show 0 open tasks

# E2E test: loop termination with memories
cargo run --bin ralph -- run -c ralph.memory.yml -p "Create a task, complete it, say LOOP_COMPLETE"
# Should terminate in 2 iterations

# Run E2E test scenarios
cargo run -p ralph-e2e -- claude --filter task
```

## E2E Test Scenarios

Four E2E scenarios in `crates/ralph-e2e/src/scenarios/tasks.rs`:

| Scenario | Description |
|----------|-------------|
| `task-add` | Verifies `ralph task add` creates tasks in `.agent/tasks.jsonl` |
| `task-close` | Verifies `ralph task close` marks tasks as closed |
| `task-completion` | Verifies loop terminates with memories enabled and no open tasks |
| `task-ready` | Verifies `ralph task ready` shows only unblocked tasks |

Run with:
```bash
cargo run -p ralph-e2e -- claude
```

## Files Changed

- `crates/ralph-core/src/task.rs` — NEW: Task types
- `crates/ralph-core/src/task_store.rs` — NEW: JSONL persistence
- `crates/ralph-core/src/lib.rs` — Export task modules
- `crates/ralph-core/src/event_loop.rs` — Task verification for completion
- `crates/ralph-core/src/hatless_ralph.rs` — Task instructions in prompt
- `crates/ralph-cli/src/task_cli.rs` — NEW: Task CLI command handlers
- `crates/ralph-cli/src/main.rs` — Wire task CLI commands
- `crates/ralph-e2e/src/scenarios/tasks.rs` — NEW: E2E test scenarios
- `crates/ralph-e2e/src/scenarios/mod.rs` — Export task scenarios

**Note:** The previous `ralph task` command (code task generator) was renamed to `ralph code-task` to make room for task tracking.

## Future Enhancements

1. **Hierarchical tasks** — `task-xxx.1`, `task-xxx.1.1` (like Beads epics)
2. **Provenance links** — Track which agent created/closed tasks
3. **Task compaction** — Archive closed tasks to git history
4. **MCP server** — Expose tasks via MCP for other tools
