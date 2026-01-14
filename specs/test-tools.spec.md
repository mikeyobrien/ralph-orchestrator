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

### Key Design Patterns (Industry Research)

| Pattern | Description | Application |
|---------|-------------|-------------|
| **Record/Replay (VCR)** | Capture real LLM interactions once, replay deterministically forever | Zero API costs in CI, reproducible tests |
| **LLM-as-Judge** | Use an LLM to evaluate subjective criteria with rubrics | Code quality, tone, UX feel assessments |
| **Trace-Based Testing** | Assert on execution traces, not just outcomes | Catch integration bugs invisible to traditional assertions |
| **Multi-Level Evaluation** | Unit → Single-step → Full-turn → Multi-turn | Progressive confidence with cost optimization |

## Problem Statement

To validate Ralph Orchestrator behavior at scale, we need automated E2E testing. However:

1. Traditional test frameworks require imperative code—agents work better with declarative tools
2. Testing an orchestrator that runs agents creates meta-complexity (agent testing agent)
3. Session state spans multiple iterations—assertions need temporal awareness
4. Backend adapters have different behaviors—tests must isolate adapter-specific concerns
5. **LLM nondeterminism** makes tests flaky without record/replay patterns
6. **Subjective criteria** (code quality, plan coherence) need LLM-as-judge evaluation

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
│  ┌──────────┐ ┌──────────┐ ┌──────────┐ ┌──────────┐           │
│  │  record  │ │  replay  │ │ evaluate │ │  report  │           │
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
│  ┌─────────────┐  ┌─────────────┐                              │
│  │ cassettes/  │  │  reports/   │   ← Record/replay storage    │
│  │ (VCR tapes) │  │ (JUnit/TAP) │                              │
│  └─────────────┘  └─────────────┘                              │
└─────────────────────────────────────────────────────────────────┘
```

### Testing Modes

| Mode | Description | Use Case |
|------|-------------|----------|
| **Mock** | Scripted responses, no network | Fast unit-style E2E, CI pipelines |
| **Record** | Real LLM calls, save to cassette | Creating test fixtures |
| **Replay** | Serve responses from cassette | Deterministic regression tests |
| **Live** | Real LLM calls, no recording | Integration tests, debugging |

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

**Mock Response Object** (for `mock_responses` array):

| Name | Type | Required | Description |
|------|------|----------|-------------|
| `hat` | string | no | Only respond when this hat is active |
| `trigger_pattern` | string | no | Only respond when prompt matches regex |
| `output` | string | yes | The mock agent output |
| `exit_code` | int | no | Process exit code (default: 0) |
| `delay_ms` | int | no | Artificial delay before response |

**Returns:**

```json
{
  "exit_code": 0,
  "termination_reason": "CompletionPromise",
  "iterations": 3,
  "elapsed_secs": 45.2,
  "session_file": "/tmp/ralph-test-abc123/session.jsonl",
  "events_count": 12,
  "stdout": "Ralph completed successfully...",
  "stderr": "",
  "mock_responses_consumed": 3,
  "mock_responses_remaining": 0
}
```

**Behavior:**
- Runs `ralph` binary with workspace as cwd
- Records session to `session.jsonl` in workspace
- Captures stdout/stderr (returned in result)
- For mock backend: responses consumed in order (first match wins if trigger patterns used)
- Returns structured result for assertions

**Mock Backend Behavior:**
1. **Sequential Consumption:** Responses consumed in order per invocation
2. **Hat Awareness:** If `hat` specified, response only used when that hat is active
3. **Pattern Matching:** If `trigger_pattern` specified, response only used when prompt matches
4. **Exhaustion Handling:** If responses exhausted, returns error with consumed count
5. **Timing Simulation:** `delay_ms` enables testing timeout behavior

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
| `hat_sequence` | `hats: string[]` | Assert hats were activated in order (from `_meta.iteration` events) |
| `hat_transition` | `from: string, to: string` | Assert hat changed from one to another |
| `iteration_count` | `hat: string, min?: int, max?: int` | Assert how many times a specific hat ran |
| `stdout_contains` | `pattern: string` | Assert stdout contains regex pattern |
| `stderr_contains` | `pattern: string` | Assert stderr contains regex pattern |
| `cost_within` | `max_dollars: float` | Assert cumulative cost within budget (live tests only) |

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

### 6. `test_snapshot` *(Optional - defer to v2)*

> **Note:** Snapshot functionality can be achieved with `git diff` in the workspace. This tool is specified for convenience but may be deferred.



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

---

### 9. `test_record`

Records a real LLM session to a cassette file for later replay (VCR pattern).

**Parameters:**

| Name | Type | Required | Description |
|------|------|----------|-------------|
| `workspace_id` | string | yes | Target workspace |
| `cassette_name` | string | yes | Name for the cassette file |
| `task` | string | yes | Initial task/prompt |
| `backend` | string | yes | Real backend to use: `claude`, `kiro`, `gemini` |
| `max_iterations` | integer | no | Override max iterations (default: 10) |
| `redactions` | object | no | Patterns to redact from recording (see Redaction Patterns) |

**Returns:**

```json
{
  "cassette_path": "/tmp/ralph-test-abc123/cassettes/auth_flow.yaml",
  "cassette_name": "auth_flow",
  "interactions": 5,
  "total_tokens": 12500,
  "cost_dollars": 0.15,
  "duration_secs": 45.2
}
```

**Behavior:**
- Executes with real backend, capturing all request/response pairs
- Normalizes non-deterministic fields (tool call IDs, timestamps)
- Applies redactions to sensitive data (API keys, secrets)
- Saves in YAML format for human readability and editability

---

### 10. `test_replay`

Replays a recorded cassette for deterministic test execution.

**Parameters:**

| Name | Type | Required | Description |
|------|------|----------|-------------|
| `workspace_id` | string | yes | Target workspace |
| `cassette_name` | string | yes | Cassette to replay |
| `task` | string | yes | Initial task (must match recording) |
| `strict` | boolean | no | Fail if request doesn't match recording (default: true) |
| `allow_passthrough` | boolean | no | Allow real API calls for unmatched requests (default: false) |

**Returns:**

```json
{
  "exit_code": 0,
  "termination_reason": "CompletionPromise",
  "iterations": 5,
  "interactions_replayed": 5,
  "interactions_passthrough": 0,
  "cost_dollars": 0.00,
  "session_file": "/tmp/ralph-test-abc123/session.jsonl"
}
```

**Behavior:**
- Serves responses from cassette instead of real API
- Zero API costs, millisecond response times
- Validates requests match recording (when strict=true)
- Optional passthrough for evolving tests

---

### 11. `test_evaluate`

Uses LLM-as-judge to evaluate subjective criteria with rubrics. **Implemented via meta preset** - runs Ralph with a "judge" hat that evaluates the test artifacts.

**Implementation:** This tool spawns a Ralph run with a judge-specific preset/hat. The judge agent reads the target artifact (scratchpad, file, session) and scores against the rubric. This reuses existing backend infrastructure - no separate LLM API setup required.

**Parameters:**

| Name | Type | Required | Description |
|------|------|----------|-------------|
| `workspace_id` | string | yes | Target workspace |
| `evaluations` | array | yes | List of evaluation criteria (see below) |
| `judge_preset` | string | no | Meta preset to use for judging (default: `judge`) |
| `judge_backend` | string | no | Backend for judge runs (default: same as test, or `haiku` for cost efficiency) |
| `chain_of_thought` | boolean | no | Require reasoning before score (default: true) |

**Evaluation Object:**

| Name | Type | Required | Description |
|------|------|----------|-------------|
| `criterion` | string | yes | What to evaluate: `plan_quality`, `code_quality`, `task_completion`, `custom` |
| `target` | string | yes | What to evaluate: `scratchpad`, `file:<path>`, `session`, `output` |
| `rubric` | object | no | Scoring rubric with descriptions per level |
| `reference` | string | no | Gold-standard reference for comparison |
| `custom_prompt` | string | no | Custom evaluation prompt (for criterion=custom) |

**Rubric Format:**

```json
{
  "rubric": {
    "5": "Excellent: Clear, actionable, addresses all requirements",
    "4": "Good: Mostly clear, minor gaps",
    "3": "Acceptable: Understandable but has notable issues",
    "2": "Poor: Confusing or missing key elements",
    "1": "Unacceptable: Cannot be used as-is"
  }
}
```

**Returns:**

```json
{
  "evaluations": [
    {
      "criterion": "plan_quality",
      "score": 4,
      "max_score": 5,
      "reasoning": "The plan is well-structured with clear acceptance criteria, but...",
      "passed": true
    }
  ],
  "overall_score": 4.2,
  "all_passed": true
}
```

**Behavior:**
- Uses a different model than the one being tested (prevents self-evaluation bias)
- Requires chain-of-thought reasoning before scoring
- Decomposes complex criteria into sub-evaluations
- Binary pass/fail threshold configurable per criterion

**Built-in Criteria:**

| Criterion | Evaluates | Default Threshold |
|-----------|-----------|-------------------|
| `plan_quality` | Scratchpad plan clarity, actionability | ≥3 |
| `code_quality` | Generated code correctness, style | ≥3 |
| `task_completion` | Whether the original task was achieved | ≥4 |
| `efficiency` | Token usage, iteration count | ≥3 |
| `safety` | Adherence to guardrails, no dangerous operations | ≥5 |

---

### 12. `test_report`

Generates test results in CI/CD-compatible formats.

**Parameters:**

| Name | Type | Required | Description |
|------|------|----------|-------------|
| `workspace_id` | string | yes | Target workspace |
| `format` | string | yes | Output format: `junit`, `tap`, `json`, `ctrf` |
| `test_name` | string | yes | Name for the test suite |
| `include_traces` | boolean | no | Include session traces in report (default: false) |
| `output_path` | string | no | Custom output path (default: `reports/<format>.<ext>`) |

**Returns:**

```json
{
  "report_path": "/tmp/ralph-test-abc123/reports/junit.xml",
  "format": "junit",
  "tests": 5,
  "passed": 4,
  "failed": 1,
  "skipped": 0,
  "duration_secs": 45.2
}
```

**JUnit XML Output:**

```xml
<?xml version="1.0" encoding="UTF-8"?>
<testsuites name="ralph-e2e" tests="5" failures="1" time="45.2">
  <testsuite name="auth_flow" tests="5" failures="1">
    <testcase name="exit_code" classname="auth_flow" time="0.001"/>
    <testcase name="event_sequence" classname="auth_flow" time="0.002">
      <failure message="Expected [task.start, build.done] but got [task.start, build.blocked]"/>
    </testcase>
    <system-out>Session trace available at: session.jsonl</system-out>
  </testsuite>
</testsuites>
```

**TAP Output:**

```tap
TAP version 14
1..5
ok 1 - exit_code: expected 0, got 0
ok 2 - termination_reason: expected CompletionPromise
not ok 3 - event_sequence: wrong order
  ---
  expected: [task.start, build.done]
  actual: [task.start, build.blocked]
  ...
ok 4 - file_exists: src/main.rs
ok 5 - duration: 45.2s < 60s max
```

**Behavior:**
- Aggregates all assertions from `test_assert` calls
- Includes evaluation results from `test_evaluate`
- JUnit XML for Jenkins/GitLab/GitHub Actions
- TAP for shell-based tooling
- CTRF (Common Test Report Format) for unified dashboards

## Redaction Patterns

Redactions mask sensitive or non-deterministic values in recordings and snapshots.

### Built-in Redaction Patterns

| Pattern | Description | Example |
|---------|-------------|---------|
| `$TIMESTAMP` | ISO timestamps | `2024-01-13T10:00:00Z` → `[TIMESTAMP]` |
| `$UUID` | UUIDs and tool call IDs | `call_abc123` → `[UUID]` |
| `$API_KEY` | API key patterns | `sk-...` → `[API_KEY]` |
| `$PATH` | Absolute paths | `/tmp/ralph-test-xxx` → `[WORKSPACE]` |
| `$DURATION` | Duration values | `45.234s` → `[DURATION]` |

### Custom Redaction

```json
{
  "redactions": {
    "patterns": {
      "user_id": "user_[a-z0-9]+",
      "session_token": "sess_[A-Za-z0-9]+"
    },
    "paths": [
      "$.response.headers.x-request-id",
      "$.data.created_at"
    ]
  }
}
```

## Cassette Format (VCR)

Cassettes store recorded LLM interactions in human-readable YAML.

```yaml
# cassette: auth_flow.yaml
# recorded: 2024-01-13T10:00:00Z
# backend: claude
# iterations: 3

interactions:
  - request:
      hat: planner
      prompt_hash: sha256:abc123...
      prompt_preview: "## Task\nImplement user auth..."
    response:
      output: |
        <event topic="build.task">
        ## Task
        Create authentication module
        </event>
      exit_code: 0
      tokens: 2500
      duration_ms: 3200

  - request:
      hat: builder
      prompt_hash: sha256:def456...
      prompt_preview: "## Build Task\nCreate auth..."
    response:
      output: |
        <event topic="build.done">
        Implemented OAuth2 flow
        </event>
      exit_code: 0
      tokens: 4200
      duration_ms: 5100

metadata:
  total_tokens: 12500
  total_cost_dollars: 0.15
  termination_reason: CompletionPromise
```

### Cassette Matching

During replay, requests are matched using:

1. **Hat match** - Same hat must be active
2. **Prompt hash** - SHA256 of normalized prompt (after redactions)
3. **Sequence position** - Fallback to positional matching if hashes differ slightly

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

## Example Test Scenarios

### Scenario 1: Basic Mock Test

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

---

### Scenario 2: Record/Replay for Regression Testing

**Scenario: Create deterministic regression test from real LLM interaction**

```
# Phase 1: Record (run once with real API)
1. test_setup
   - workspace_id: "auth_regression"
   - fixtures: {"specs/auth.spec.md": "...spec content..."}

2. test_record
   - cassette_name: "auth_happy_path"
   - task: "Implement the auth spec"
   - backend: "claude"
   - redactions: {patterns: {"$TIMESTAMP": true, "$UUID": true}}

# Phase 2: Replay (run in CI, zero API cost)
3. test_replay
   - cassette_name: "auth_happy_path"
   - task: "Implement the auth spec"
   - strict: true

4. test_assert
   - assertions: [
       {type: "exit_code", expected: 0},
       {type: "termination_reason", expected: "CompletionPromise"},
       {type: "file_exists", path: "src/auth.rs"},
       {type: "cost_within", max_dollars: 0.00}  // Zero cost on replay!
     ]

5. test_report
   - format: "junit"
   - test_name: "auth_regression"

6. test_cleanup
```

---

### Scenario 3: LLM-as-Judge for Subjective Quality

**Scenario: Evaluate plan quality and code style**

```
1. test_setup
   - workspace_id: "quality_eval"
   - fixtures: {"specs/feature.spec.md": "..."}

2. test_run
   - task: "Plan and implement the feature spec"
   - backend: "mock"  # or "replay" with cassette
   - mock_responses: [...]

3. test_evaluate
   - evaluations: [
       {
         criterion: "plan_quality",
         target: "scratchpad",
         rubric: {
           5: "Clear, actionable, atomic tasks with acceptance criteria",
           4: "Good structure, minor gaps in detail",
           3: "Acceptable but missing key elements",
           2: "Confusing or overly vague",
           1: "Cannot be executed as-is"
         }
       },
       {
         criterion: "code_quality",
         target: "file:src/feature.rs",
         rubric: {
           5: "Clean, idiomatic, well-tested",
           4: "Good code with minor style issues",
           3: "Works but has notable problems",
           2: "Significant issues or bugs",
           1: "Does not compile or fundamentally broken"
         }
       },
       {
         criterion: "custom",
         target: "session",
         custom_prompt: "Did the agent follow the guardrails in AGENTS.md? Score 1-5."
       }
     ]
   - judge_model: "claude-3-haiku"  # Different from test backend
   - chain_of_thought: true

4. test_assert
   - assertions: [
       {type: "exit_code", expected: 0}
     ]
   # Evaluation results are automatically included in report

5. test_report
   - format: "junit"
   - test_name: "quality_evaluation"
   - include_traces: true

6. test_cleanup
```

---

### Scenario 4: Trace-Based Testing

**Scenario: Verify agent execution flow through traces**

```
1. test_setup
   - workspace_id: "trace_test"

2. test_run
   - task: "Add error handling to the API"
   - backend: "mock"
   - mock_responses: [...]

3. test_assert
   - assertions: [
       # Traditional assertions
       {type: "exit_code", expected: 0},

       # Trace-based assertions (from research)
       {type: "hat_transition", from: "planner", to: "builder"},
       {type: "trace_span_sequence", names: ["plan", "implement", "verify"]},
       {type: "tool_called", tool: "write_file", args_pattern: ".*error.*"},

       # Event-based assertions
       {type: "event_sequence", topics: ["task.start", "build.task", "build.done"]},
       {type: "no_event", topic: "build.blocked"}  # No thrashing
     ]

4. test_inspect
   - format: "timeline"
   - filter: {topics: ["build.*"]}

5. test_cleanup
```

---

### Scenario 5: Snapshot Testing with Redactions

**Scenario: Compare workspace state against approved snapshot**

```
1. test_setup
   - workspace_id: "snapshot_test"
   - fixtures: {"src/main.rs": "fn main() {}"}

2. test_snapshot
   - snapshot_id: "initial"

3. test_run
   - task: "Add logging to main"
   - backend: "mock"
   - mock_responses: [...]

4. test_assert
   - assertions: [
       {
         type: "snapshot_matches",
         snapshot_id: "approved_state",  # Pre-approved golden snapshot
         redactions: {
           patterns: {"$TIMESTAMP": true},
           paths: ["$.files['.agent/scratchpad.md']"]  # Ignore scratchpad
         }
       }
     ]

5. test_diff
   - baseline: "initial"
   - compare_to: "current"
   # Returns structured diff for review

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

### Record/Replay (VCR Pattern)

- **Given** a workspace with valid backend credentials
- **When** `test_record` is called with `cassette_name: "auth_flow"`
- **Then** real LLM interactions are captured
- **And** cassette is saved in YAML format
- **And** non-deterministic values are normalized (tool call IDs, timestamps)

---

- **Given** a recorded cassette "auth_flow"
- **When** `test_replay` is called with matching task
- **Then** responses are served from cassette
- **And** no real API calls are made
- **And** cost_dollars is 0.00

---

- **Given** a cassette recorded with one prompt format
- **When** `test_replay` is called with `strict: true` and different prompt
- **Then** replay fails with mismatch error
- **And** expected vs actual prompt hashes are shown

---

- **Given** a cassette with `allow_passthrough: true`
- **When** replay encounters unmatched request
- **Then** real API is called for that request
- **And** passthrough is logged in results

### LLM-as-Judge Evaluation

- **Given** a completed test run with scratchpad content
- **When** `test_evaluate` checks `{criterion: "plan_quality", target: "scratchpad"}`
- **Then** judge model evaluates the plan
- **And** reasoning is provided before score
- **And** binary pass/fail is determined by threshold

---

- **Given** evaluation with custom rubric
- **When** rubric defines levels 1-5 with descriptions
- **Then** judge uses rubric for scoring
- **And** score reasoning references rubric levels

---

- **Given** evaluation with `chain_of_thought: true`
- **When** judge evaluates
- **Then** response includes reasoning field
- **And** reasoning explains score justification

---

- **Given** generated code in workspace
- **When** `test_evaluate` checks `{criterion: "code_quality", target: "file:src/main.rs"}`
- **Then** judge evaluates code correctness and style
- **And** provides actionable feedback

### CI/CD Reporting

- **Given** completed assertions and evaluations
- **When** `test_report` is called with `format: "junit"`
- **Then** JUnit XML is generated
- **And** GitLab/Jenkins/GitHub Actions can parse it

---

- **Given** failed assertions
- **When** `test_report` generates output
- **Then** failure messages include expected vs actual
- **And** relevant context is included (file paths, event topics)

---

- **Given** `include_traces: true`
- **When** report is generated
- **Then** session.jsonl path is included in output
- **And** trace can be used for debugging

### Trace-Based Assertions

- **Given** a session with multiple hat transitions
- **When** `test_assert` checks `{type: "hat_transition", from: "planner", to: "builder"}`
- **Then** assertion passes if transition occurred
- **And** fails with available transitions if not found

---

- **Given** a session where agent called tools
- **When** `test_assert` checks `{type: "tool_called", tool: "write_file"}`
- **Then** assertion passes if tool was invoked
- **And** args_pattern can filter by arguments

## Retry and Flakiness Handling

Test tools support retry configuration for handling inherent nondeterminism.

### Retry Configuration

```json
{
  "retry": {
    "max_attempts": 3,
    "backoff_ms": 1000,
    "retry_on": ["timeout", "rate_limit"],
    "no_retry_on": ["assertion_failure"]
  }
}
```

### Flakiness Detection

When a test passes after retry, it's flagged as potentially flaky:

```json
{
  "passed": true,
  "flaky": true,
  "attempts": 2,
  "failure_history": [
    {"attempt": 1, "error": "timeout after 30s"}
  ]
}
```

### Best Practices for Reducing Flakiness

1. **Use record/replay** - Eliminates LLM nondeterminism entirely
2. **Assert on patterns, not exact values** - Use regex for variable output
3. **Use redactions** - Mask timestamps, IDs, paths in comparisons
4. **Prefer event assertions over output assertions** - Events are structured and predictable
5. **Set appropriate timeouts** - Don't fail on normal latency variation

## Security Considerations

1. **Workspace Isolation:** Each test workspace is a separate directory under system temp
2. **No Network by Default:** Mock backend prevents unintended LLM calls
3. **Credential Isolation:** Real backend tests require explicit env configuration
4. **Cleanup Enforcement:** Stale workspaces are cleaned on test agent startup
5. **Path Traversal Prevention:** All paths validated to be within workspace

## Future Considerations

### Near-Term Enhancements

- **Parallel Test Execution:** Multiple workspaces can run concurrently with isolated state
- **Snapshot Update Mode:** Interactive mode to accept/reject snapshot changes (like `cargo insta review`)
- **Coverage Metrics:** Track which event topics/hats/code paths are covered by tests
- **Cost Budgets:** Fail tests that exceed token/dollar budgets

### Advanced Patterns (From Industry Research)

| Pattern | Description | Benefit |
|---------|-------------|---------|
| **Multi-Level Evaluation** | Unit → Single-step → Full-turn → Multi-turn testing | Progressive confidence with cost optimization |
| **Trace-Based Testing** | Assert on OpenTelemetry-style traces, not just outcomes | Catches integration bugs invisible to traditional assertions |
| **Golden File Testing** | Compare entire output against approved "golden" files | Catch unexpected regressions in any output |
| **Chaos Testing** | Inject failures (timeouts, errors) to test resilience | Validate graceful degradation |
| **Contract Testing** | Validate event schemas and payload formats | Prevent integration drift |

### Observability Integration

- **OpenTelemetry Export:** Emit spans for each test step, correlate with production traces
- **Structured Event Streaming:** Real-time NDJSON output for live dashboards
- **Correlation IDs:** Thread test_run_id through all operations for debugging

### Potential Tool Additions

| Tool | Purpose |
|------|---------|
| `test_chaos` | Inject failures (timeouts, errors, malformed responses) |
| `test_benchmark` | Run N iterations, collect performance statistics |
| `test_compare` | Compare two cassettes or test runs side-by-side |
| `test_coverage` | Report which events/hats/paths were exercised |

## References

Industry patterns and tools that informed this specification:

### Agent Testing Frameworks
- [DeepEval](https://github.com/confident-ai/deepeval) - Pytest-style LLM evaluation with 14+ metrics
- [Promptfoo](https://www.promptfoo.dev/) - Declarative YAML test configuration
- [LangChain AgentEvals](https://docs.langchain.com/) - Trajectory matching and LLM-as-judge

### Record/Replay Pattern
- [VCR (Ruby)](https://github.com/vcr/vcr) - Original record/replay library
- [Docker cagent](https://www.docker.com/blog/deterministic-ai-testing-with-session-recording-in-cagent/) - Deterministic AI testing with session recording

### LLM-as-Judge
- [G-Eval Framework](https://www.confident-ai.com/blog/g-eval-the-definitive-guide) - Chain-of-thought evaluation
- [Anthropic Safety Evaluations](https://alignment.anthropic.com/) - Automated alignment auditing

### E2E Testing
- [Playwright](https://playwright.dev/) - Auto-waiting assertions, trace viewer
- [Insta](https://insta.rs/) - Rust snapshot testing with redactions
- [BATS](https://github.com/bats-core/bats-core) - Bash Automated Testing System

### Observability
- [Tracetest](https://tracetest.io/) - OpenTelemetry trace-based testing
- [JUnit XML Format](https://github.com/testmoapp/junitxml) - De facto CI/CD standard
- [TAP (Test Anything Protocol)](https://testanything.org/) - Simple text-based results
