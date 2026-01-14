---
status: draft
gap_analysis: null
related:
  - event-loop.spec.md
  - benchmark-harness.spec.md
---

# Test Tools Specification

## Overview

This spec defines custom tools for agent-driven end-to-end testing of Ralph Orchestrator. These tools enable an AI agent to set up test scenarios, execute orchestrator runs, inspect results, and assert on outcomes—all without human intervention.

**Design Philosophy:** Test tools are thin wrappers over existing primitives. The EventBus observer pattern already records sessions; these tools provide structured access and assertion capabilities.

## Problem Statement

To validate Ralph Orchestrator behavior at scale, we need automated E2E testing. However:

1. Traditional test frameworks require imperative code—agents work better with declarative tools
2. Testing an orchestrator that runs agents creates meta-complexity (agent testing agent)
3. Session state spans multiple iterations—assertions need temporal awareness
4. Backend adapters have different behaviors—tests must isolate adapter-specific concerns

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                        Test Agent                                │
│                    (wears "tester" hat)                         │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                       Test Tools                                 │
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐           │
│  │  setup   │ │   run    │ │  assert  │ │  inspect │           │
│  └──────────┘ └──────────┘ └──────────┘ └──────────┘           │
└─────────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────────┐
│                    Test Workspace                                │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐             │
│  │  fixtures/  │  │  session.   │  │  .agent/    │             │
│  │             │  │  jsonl      │  │  scratchpad │             │
│  └─────────────┘  └─────────────┘  └─────────────┘             │
└─────────────────────────────────────────────────────────────────┘
```

## Tool Definitions

### 1. `test_setup`

Creates an isolated test workspace with optional fixtures.

**Parameters:**

| Name | Type | Required | Description |
|------|------|----------|-------------|
| `workspace_id` | string | yes | Unique identifier for this test workspace |
| `fixtures` | object | no | Files to create in the workspace (path → content) |
| `config` | object | no | Ralph configuration overrides |
| `scratchpad` | string | no | Initial `.agent/scratchpad.md` content |

**Returns:**

```json
{
  "workspace_path": "/tmp/ralph-test-abc123",
  "workspace_id": "abc123",
  "created_files": ["fixtures/main.rs", ".agent/scratchpad.md"]
}
```

**Behavior:**
- Creates isolated directory under system temp
- Writes fixture files with specified content
- Initializes `.agent/` directory structure
- Stores config overrides for subsequent `test_run` calls

---

### 2. `test_run`

Executes Ralph Orchestrator in the test workspace and records the session.

**Parameters:**

| Name | Type | Required | Description |
|------|------|----------|-------------|
| `workspace_id` | string | yes | Target workspace from `test_setup` |
| `task` | string | yes | Initial task/prompt to send to orchestrator |
| `backend` | string | no | Backend adapter: `claude`, `kiro`, `gemini`, `mock` (default: `mock`) |
| `max_iterations` | integer | no | Override max iterations (default: 5) |
| `max_runtime_secs` | integer | no | Override max runtime (default: 300) |
| `env` | object | no | Environment variables to inject |
| `mock_responses` | array | no | Scripted responses for mock backend (required if backend=mock) |

**Returns:**

```json
{
  "exit_code": 0,
  "termination_reason": "CompletionPromise",
  "iterations": 3,
  "elapsed_secs": 45.2,
  "session_file": "/tmp/ralph-test-abc123/session.jsonl",
  "events_count": 12
}
```

**Behavior:**
- Runs `ralph` binary with workspace as cwd
- Records session to `session.jsonl` in workspace
- Captures stdout/stderr
- Returns structured result for assertions

---

### 3. `test_assert`

Validates conditions against the test run results and recorded session.

**Parameters:**

| Name | Type | Required | Description |
|------|------|----------|-------------|
| `workspace_id` | string | yes | Target workspace |
| `assertions` | array | yes | List of assertion objects (see below) |

**Assertion Types:**

| Type | Parameters | Description |
|------|------------|-------------|
| `exit_code` | `expected: int` | Assert process exit code |
| `termination_reason` | `expected: string` | Assert termination reason |
| `iterations` | `min?: int, max?: int, exact?: int` | Assert iteration count |
| `file_exists` | `path: string` | Assert file was created |
| `file_contains` | `path: string, pattern: string` | Assert file contains regex pattern |
| `file_not_contains` | `path: string, pattern: string` | Assert file does NOT contain pattern |
| `event_occurred` | `topic: string, payload_pattern?: string` | Assert event was published |
| `event_sequence` | `topics: string[]` | Assert events occurred in order |
| `event_count` | `topic: string, min?: int, max?: int, exact?: int` | Assert event occurrence count |
| `scratchpad_contains` | `pattern: string` | Assert scratchpad final state contains pattern |
| `no_event` | `topic: string` | Assert event topic never occurred |
| `duration` | `max_secs: int` | Assert total runtime within limit |

**Returns:**

```json
{
  "passed": true,
  "results": [
    {"assertion": "exit_code", "passed": true, "expected": 0, "actual": 0},
    {"assertion": "file_exists", "passed": true, "path": "src/main.rs"}
  ],
  "failed_count": 0
}
```

**Behavior:**
- Evaluates all assertions (does not short-circuit)
- Returns detailed results for each assertion
- Patterns use regex matching

---

### 4. `test_inspect`

Reads and filters session recording for debugging or complex assertions.

**Parameters:**

| Name | Type | Required | Description |
|------|------|----------|-------------|
| `workspace_id` | string | yes | Target workspace |
| `filter` | object | no | Filter criteria (see below) |
| `limit` | integer | no | Max records to return (default: 100) |
| `format` | string | no | Output format: `json`, `summary`, `timeline` (default: `json`) |

**Filter Options:**

| Name | Type | Description |
|------|------|-------------|
| `event_types` | string[] | Only these event types (`bus.publish`, `_meta.iteration`, etc.) |
| `topics` | string[] | Only events matching these topics (glob patterns supported) |
| `after_iteration` | int | Only events after this iteration |
| `before_iteration` | int | Only events before this iteration |
| `payload_pattern` | string | Only events where payload matches regex |

**Returns (format=json):**

```json
{
  "records": [
    {"ts": 1704067200000, "event": "bus.publish", "data": {"topic": "task.start", "payload": "..."}},
    {"ts": 1704067201000, "event": "_meta.iteration", "data": {"n": 1, "hat": "planner"}}
  ],
  "total_matched": 12,
  "truncated": false
}
```

**Returns (format=summary):**

```json
{
  "total_events": 45,
  "by_type": {"bus.publish": 30, "_meta.iteration": 5, "ux.terminal.write": 10},
  "by_topic": {"task.start": 1, "build.task": 3, "build.done": 2},
  "iterations": 5,
  "hats_used": ["planner", "builder"],
  "duration_secs": 120.5
}
```

**Returns (format=timeline):**

```json
{
  "timeline": [
    {"iteration": 1, "hat": "planner", "events": ["task.start → build.task"], "duration_ms": 15000},
    {"iteration": 2, "hat": "builder", "events": ["build.task → build.done"], "duration_ms": 30000}
  ]
}
```

---

### 5. `test_cleanup`

Removes test workspace and associated resources.

**Parameters:**

| Name | Type | Required | Description |
|------|------|----------|-------------|
| `workspace_id` | string | yes | Target workspace to clean up |
| `preserve_session` | boolean | no | Keep session.jsonl for debugging (default: false) |

**Returns:**

```json
{
  "deleted": true,
  "preserved_files": []
}
```

---

### 6. `test_mock_backend`

Configures a mock backend for deterministic testing without real LLM calls.

**Parameters:**

| Name | Type | Required | Description |
|------|------|----------|-------------|
| `workspace_id` | string | yes | Target workspace |
| `responses` | array | yes | Ordered list of mock response objects |

**Response Object:**

| Name | Type | Required | Description |
|------|------|----------|-------------|
| `hat` | string | no | Only respond when this hat is active |
| `trigger_pattern` | string | no | Only respond when prompt matches pattern |
| `output` | string | yes | The mock agent output |
| `exit_code` | int | no | Process exit code (default: 0) |
| `delay_ms` | int | no | Artificial delay before response |

**Returns:**

```json
{
  "configured": true,
  "response_count": 5
}
```

**Behavior:**
- Responses are consumed in order (first match wins if trigger patterns used)
- Unconsumed responses after run indicate test may need adjustment
- Supports simulating failures via exit_code

---

### 7. `test_snapshot`

Captures current workspace state for comparison or archival.

**Parameters:**

| Name | Type | Required | Description |
|------|------|----------|-------------|
| `workspace_id` | string | yes | Target workspace |
| `include_patterns` | string[] | no | Glob patterns of files to include (default: all) |
| `exclude_patterns` | string[] | no | Glob patterns to exclude |

**Returns:**

```json
{
  "snapshot_id": "snap_abc123_001",
  "files": {
    ".agent/scratchpad.md": "## Plan\n- Task 1\n...",
    "src/main.rs": "fn main() { ... }"
  },
  "file_count": 5,
  "total_bytes": 2048
}
```

---

### 8. `test_diff`

Compares two snapshots or a snapshot against current state.

**Parameters:**

| Name | Type | Required | Description |
|------|------|----------|-------------|
| `workspace_id` | string | yes | Target workspace |
| `baseline` | string | yes | Snapshot ID or "initial" for setup state |
| `compare_to` | string | no | Snapshot ID or "current" (default: "current") |

**Returns:**

```json
{
  "changed_files": ["src/main.rs", ".agent/scratchpad.md"],
  "added_files": ["src/utils.rs"],
  "deleted_files": [],
  "diffs": {
    "src/main.rs": {
      "additions": 15,
      "deletions": 3,
      "hunks": [{"start": 10, "end": 25, "content": "..."}]
    }
  }
}
```

## Mock Backend Specification

The mock backend enables deterministic E2E tests without real LLM calls.

### Mock Response Format

```json
{
  "responses": [
    {
      "output": "<event topic=\"build.task\">\n## Task\nImplement feature X\n</event>",
      "exit_code": 0
    },
    {
      "output": "<event topic=\"build.done\">\nCompleted implementation\n</event>",
      "exit_code": 0
    }
  ]
}
```

### Mock Backend Behavior

1. **Sequential Consumption:** Responses are consumed in order per invocation
2. **Hat Awareness:** If `hat` specified, response only used when that hat is active
3. **Pattern Matching:** If `trigger_pattern` specified, response only used when prompt matches
4. **Exhaustion Handling:** If responses exhausted, returns error (test should have enough responses)
5. **Timing Simulation:** `delay_ms` enables testing timeout behavior

## Example Test Scenario

**Scenario: Planner creates build.task on task.start**

```
1. test_setup
   - workspace_id: "planner_test_001"
   - scratchpad: "## Plan\n(empty)"

2. test_mock_backend
   - responses: [
       {hat: "planner", output: "<event topic=\"build.task\">...</event>"}
     ]

3. test_run
   - task: "Implement user authentication"
   - backend: "mock"
   - max_iterations: 1

4. test_assert
   - assertions: [
       {type: "exit_code", expected: 2},  // MaxIterations, not CompletionPromise
       {type: "event_occurred", topic: "build.task"},
       {type: "event_sequence", topics: ["task.start", "build.task"]}
     ]

5. test_inspect
   - format: "timeline"

6. test_cleanup
```

## Integration with Existing Infrastructure

### Session Recording

Test tools leverage existing `SessionRecorder` from `ralph-core`:

- `test_run` injects observer via `SessionRecorder::make_observer()`
- Session file uses existing JSONL format
- `test_inspect` parses standard record types

### EventBus Compatibility

Assertions understand EventBus semantics:

- Topic glob patterns work in filters
- Event source/target tracking available
- Hat subscriptions can be validated

### Backend Adapter Abstraction

`test_run` uses existing `CliBackend::from_config()`:

- Real backends available for integration tests
- Mock backend for unit-style E2E tests
- Same configuration surface as production

## Error Handling

| Scenario | Behavior |
|----------|----------|
| Invalid workspace_id | Return error with available workspace IDs |
| test_run on dirty workspace | Warn but proceed (idempotent re-runs) |
| Mock responses exhausted | Return error with consumed count |
| Assertion on missing file | Assertion fails with clear message |
| Session file corrupted | Return parse error with line number |
| Timeout during test_run | Kill process, record partial session |

## Acceptance Criteria

### Setup and Teardown

- **Given** no existing workspace
- **When** `test_setup` is called with `workspace_id: "test_001"`
- **Then** an isolated directory is created
- **And** the workspace is registered for subsequent operations

---

- **Given** a workspace with fixtures
- **When** `test_cleanup` is called
- **Then** all files are removed
- **And** the workspace ID is deregistered

---

- **Given** a workspace with `preserve_session: true`
- **When** `test_cleanup` is called
- **Then** only `session.jsonl` remains
- **And** all other files are removed

### Test Execution

- **Given** a configured mock backend with 3 responses
- **When** `test_run` executes 3 iterations
- **Then** all mock responses are consumed in order
- **And** session.jsonl contains all events

---

- **Given** a mock backend with `delay_ms: 5000`
- **When** `test_run` has `max_runtime_secs: 1`
- **Then** termination_reason is `MaxRuntime`
- **And** partial session is recorded

---

- **Given** a real backend (claude) with valid credentials
- **When** `test_run` executes
- **Then** actual LLM responses are recorded
- **And** session can be inspected for debugging

### Assertions

- **Given** a completed test run with exit_code 0
- **When** `test_assert` checks `{type: "exit_code", expected: 0}`
- **Then** assertion passes

---

- **Given** a session with events [task.start, build.task, build.done]
- **When** `test_assert` checks `{type: "event_sequence", topics: ["task.start", "build.done"]}`
- **Then** assertion passes (subsequence match)

---

- **Given** a session with events [task.start, build.task, build.done]
- **When** `test_assert` checks `{type: "event_sequence", topics: ["build.done", "task.start"]}`
- **Then** assertion fails (wrong order)

---

- **Given** a workspace where scratchpad contains "## Completed"
- **When** `test_assert` checks `{type: "scratchpad_contains", pattern: "Completed"}`
- **Then** assertion passes

---

- **Given** a test run that created `src/main.rs`
- **When** `test_assert` checks `{type: "file_exists", path: "src/main.rs"}`
- **Then** assertion passes

### Inspection

- **Given** a session with 50 events
- **When** `test_inspect` is called with `limit: 10`
- **Then** only first 10 events are returned
- **And** `truncated: true` is indicated

---

- **Given** a session with mixed event types
- **When** `test_inspect` filters by `topics: ["build.*"]`
- **Then** only `build.task`, `build.done`, `build.blocked` events returned

---

- **Given** a completed multi-iteration run
- **When** `test_inspect` uses `format: "timeline"`
- **Then** events are grouped by iteration
- **And** per-iteration durations are calculated

### Snapshots and Diffs

- **Given** initial workspace state
- **When** `test_snapshot` captures state
- **And** files are modified
- **And** `test_diff` compares to current
- **Then** changed files are identified
- **And** line-level diffs are provided

## Security Considerations

1. **Workspace Isolation:** Each test workspace is a separate directory under system temp
2. **No Network by Default:** Mock backend prevents unintended LLM calls
3. **Credential Isolation:** Real backend tests require explicit env configuration
4. **Cleanup Enforcement:** Stale workspaces are cleaned on test agent startup
5. **Path Traversal Prevention:** All paths validated to be within workspace

## Future Considerations

- **Parallel Test Execution:** Multiple workspaces can run concurrently
- **Test Recording Replay:** Re-run recorded sessions with mock backend for regression testing
- **Coverage Metrics:** Track which event topics/paths are covered by tests
- **Flakiness Detection:** Track assertion pass rates across runs
