---
status: draft
gap_analysis: 2026-01-14
related:
  - test-tools.spec.md
  - event-loop.spec.md
---

# Behavioral Verification Catalog

## Overview

This spec defines a systematic approach to evaluating Ralph Orchestrator's core functionality in a repeatable way. It establishes a **behavioral test catalog** - a comprehensive suite of tests that verify discrete, observable behaviors.

**Goal:** Answer "Does Ralph work correctly?" with automated, deterministic tests.

## The Bootstrapping Problem

**Critical Insight:** We can't test Ralph with Ralph-based tools if Ralph doesn't work.

The verification strategy must be **progressive** - each level assumes only that the previous level passed:

```
Level 0: Does it compile?           → cargo build
Level 1: Do units work?             → cargo test (Rust unit tests)
Level 2: Does the binary run?       → ralph --version
Level 3: Does it produce output?    → ralph + mock backend
Level 4: Do events route?           → Verify JSONL session file
Level 5: Do behaviors work?         → Full behavioral catalog
Level 6: Is quality acceptable?     → LLM-as-judge (meta preset)
```

**Each level gates the next.** If Level 2 fails, don't run Level 5.

## Verification Levels

### Level 0: Compilation (No Assumptions)

**Assumes:** Nothing. Source code exists.

```bash
cargo build --release
```

| Check | Command | Pass Criteria |
|-------|---------|---------------|
| Compiles | `cargo build` | Exit code 0 |
| No warnings | `cargo build 2>&1 | grep warning` | Empty output |
| All crates | `cargo build --workspace` | All crates compile |

**If this fails:** Fix compilation errors. Nothing else matters.

---

### Level 1: Unit Tests (Assumes: Compiles)

**Assumes:** Code compiles. Tests individual functions in isolation.

```bash
cargo test --workspace
```

These are **Rust unit tests** - no Ralph binary, no LLM, no orchestration:

| Component | Tests | What It Verifies |
|-----------|-------|------------------|
| `ralph-proto` | Event, Hat, Topic types | Data structures serialize/deserialize |
| `ralph-proto` | EventBus | Topic matching, subscription routing |
| `ralph-proto` | HatRegistry | Hat registration, lookup |
| `ralph-core` | LoopState | Termination checks, counter logic |
| `ralph-core` | InstructionBuilder | Prompt construction |
| `ralph-core` | SessionRecorder | JSONL serialization |
| `ralph-adapters` | CliBackend | Command construction, prompt modes |

**Example unit test (no Ralph needed):**

```rust
#[test]
fn test_topic_matches_glob() {
    let pattern = Topic::new("build.*");
    assert!(pattern.matches(&Topic::new("build.task")));
    assert!(pattern.matches(&Topic::new("build.done")));
    assert!(!pattern.matches(&Topic::new("task.start")));
}
```

**If this fails:** Fix the unit. The component is broken at the function level.

---

### Level 2: Binary Smoke Test (Assumes: Units Pass)

**Assumes:** Components work individually. Tests the compiled binary exists and runs.

```bash
./target/release/ralph --version
./target/release/ralph --help
```

| Check | Command | Pass Criteria |
|-------|---------|---------------|
| Binary exists | `test -f ./target/release/ralph` | File exists |
| Version flag | `ralph --version` | Outputs version, exit 0 |
| Help flag | `ralph --help` | Outputs help text, exit 0 |
| Config parsing | `ralph --config test.toml --dry-run` | Parses config, exit 0 |

**If this fails:** Binary linking or CLI parsing is broken.

---

### Level 3: Execution Smoke Test (Assumes: Binary Runs)

**Assumes:** Binary starts. Tests it can execute with a mock backend.

This is the **first test that actually runs Ralph**:

```bash
# Create minimal mock backend (shell script that echoes)
cat > /tmp/mock-backend.sh << 'EOF'
#!/bin/bash
echo "Mock response: task acknowledged"
exit 0
EOF
chmod +x /tmp/mock-backend.sh

# Run Ralph with mock
ralph --backend custom \
      --backend-command "/tmp/mock-backend.sh" \
      --max-iterations 1 \
      "Test task"
```

| Check | Pass Criteria |
|-------|---------------|
| Starts without crash | No segfault, no panic |
| Invokes backend | Mock script executed |
| Produces session file | `.agent/session.jsonl` created |
| Exits cleanly | Exit code is defined (0, 1, 2, or 130) |

**If this fails:** Event loop initialization, backend spawning, or signal handling is broken.

---

### Level 4: Event Routing Verification (Assumes: Execution Works)

**Assumes:** Ralph runs and produces output. Tests events are routed correctly.

**Key insight:** We verify by inspecting the session.jsonl file, NOT by trusting Ralph's behavior.

```bash
# Run Ralph with mock that produces events
ralph --backend mock \
      --mock-responses '[{"output": "<event topic=\"build.task\">Do thing</event>"}]' \
      --max-iterations 1 \
      "Start task"

# Verify events in session file (using jq, not Ralph)
jq -r 'select(.event == "bus.publish") | .data.topic' .agent/session.jsonl
```

| Check | Method | Pass Criteria |
|-------|--------|---------------|
| task.start published | `jq` on session.jsonl | Topic "task.start" exists |
| Routed to planner | `jq` on session.jsonl | `_meta.iteration` shows hat: "planner" |
| build.task published | `jq` on session.jsonl | Topic "build.task" exists |
| Events in order | `jq` on session.jsonl | task.start before build.task |

**Verification script (no Ralph-based tools):**

```bash
#!/bin/bash
# level4-verify.sh - Verify event routing from session file

SESSION=".agent/session.jsonl"

# Check task.start exists
if ! jq -e 'select(.event == "bus.publish" and .data.topic == "task.start")' "$SESSION" > /dev/null; then
    echo "FAIL: task.start not found"
    exit 1
fi

# Check build.task exists
if ! jq -e 'select(.event == "bus.publish" and .data.topic == "build.task")' "$SESSION" > /dev/null; then
    echo "FAIL: build.task not found"
    exit 1
fi

# Check ordering (task.start timestamp < build.task timestamp)
TASK_START_TS=$(jq -r 'select(.data.topic == "task.start") | .ts' "$SESSION" | head -1)
BUILD_TASK_TS=$(jq -r 'select(.data.topic == "build.task") | .ts' "$SESSION" | head -1)

if [[ "$TASK_START_TS" > "$BUILD_TASK_TS" ]]; then
    echo "FAIL: task.start after build.task"
    exit 1
fi

echo "PASS: Event routing verified"
```

**If this fails:** EventBus routing or session recording is broken.

---

### Level 5: Behavioral Verification (Assumes: Events Route)

**Assumes:** Events route correctly. Now we can use the full test-tools suite.

**THIS is where the behavioral catalog applies.**

At this level, we trust:
- Ralph starts ✓
- Events are recorded ✓
- Session files are parseable ✓

So we can use `test_run`, `test_assert`, `test_inspect`:

```bash
ralph-test verify --catalog ./specs/behavioral-verification.spec.md
```

See the **Behavioral Test Catalog** section below.

---

### Level 5b: Ralph-as-Verifier (Assumes: Events Route)

**The Meta Approach:** Use Ralph to verify Ralph's behavior.

Once Level 4 passes (events route correctly), Ralph is trustworthy enough to analyze its own artifacts. We add a **verifier hat** that reads behavioral specs and checks assertions against recorded sessions.

**Why this works:**
- Ralph excels at reading specs and checking implementations
- Session files are structured data Ralph can parse
- Verification is read-only (no risk of breaking things)
- LLM can reason about complex behavioral assertions

**Verifier Hat Configuration:**

```yaml
hats:
  verifier:
    triggers: ["verify.start"]
    publishes: ["verify.done"]
    instructions: |
      You are a behavioral verification agent for Ralph Orchestrator.

      ## Your Task
      Given a behavior specification and test artifacts, determine if the
      behavior was correctly exhibited.

      ## Inputs Available
      - Behavior spec: ID, description, expected assertions
      - Session file: .agent/session.jsonl (all events from the test run)
      - Workspace: files created/modified during the run
      - Scratchpad: .agent/scratchpad.md final state

      ## Verification Process
      1. Parse the session.jsonl file to extract events
      2. For each assertion in the spec:
         - Find relevant evidence in session/files/scratchpad
         - Determine if assertion holds
         - Document your reasoning
      3. Output structured verdict

      ## Output Format
      <event topic="verify.done">
      {
        "behavior_id": "PL-007",
        "verdict": "PASS" | "FAIL",
        "assertions": [
          {"assertion": "completion_promise_output", "result": "PASS", "evidence": "..."},
          {"assertion": "all_tasks_complete", "result": "PASS", "evidence": "..."}
        ],
        "reasoning": "Overall explanation of verification"
      }
      </event>

      ## Important
      - You are VERIFYING, not EXECUTING. Do not run any commands.
      - Base verdicts only on evidence in the artifacts.
      - If evidence is ambiguous, note this in reasoning.
      - A single failed assertion means overall FAIL.
```

**Verification Command:**

```bash
# Execute test run (Phase 1)
ralph --backend mock \
      --mock-responses "$(cat cassettes/pl-007.yaml)" \
      --max-iterations 3 \
      --workspace /tmp/test-pl-007 \
      "Complete all tasks"

# Verify with Ralph (Phase 2)
ralph --hat verifier \
      --workspace /tmp/test-pl-007 \
      --verify-behavior PL-007 \
      --verify-spec ./specs/behavioral-verification.spec.md
```

**Example Verification Session:**

```
═══════════════════════════════════════════════════════════════════
 VERIFICATION │ 🔍 verifier │ Behavior PL-007
═══════════════════════════════════════════════════════════════════

Reading behavior spec PL-007: "Planner outputs completion promise when done"

Assertions to verify:
  1. All tasks marked [x] or [~] in scratchpad
  2. Output contains LOOP_COMPLETE
  3. Termination reason is CompletionPromise

Analyzing session.jsonl...
  - Found 3 iterations
  - Final iteration hat: planner
  - Events: task.start → build.task → build.done → build.task → build.done

Checking assertion 1: task completion markers
  ✓ Scratchpad shows: "- [x] Implement feature A"
  ✓ Scratchpad shows: "- [x] Implement feature B"
  ✓ No pending tasks found
  PASS

Checking assertion 2: completion promise in output
  ✓ Iteration 3 output contains "LOOP_COMPLETE"
  PASS

Checking assertion 3: termination reason
  ✓ _meta.termination shows reason: "CompletionPromise"
  PASS

<event topic="verify.done">
{"behavior_id": "PL-007", "verdict": "PASS", "assertions": [...]}
</event>
```

**Advantages of Ralph-as-Verifier:**

| Aspect | Benefit |
|--------|---------|
| **Complex assertions** | LLM can interpret nuanced behavioral requirements |
| **Readable output** | Natural language explanation of pass/fail |
| **Flexible matching** | Don't need exact string matches, can understand intent |
| **Self-documenting** | Verification reasoning is captured |
| **Spec-driven** | Same specs used for implementation guide verification |

**When to use Ralph-as-Verifier vs. Script-based:**

| Scenario | Approach |
|----------|----------|
| Exact string match | Script (faster, deterministic) |
| Event sequence check | Script (jq is sufficient) |
| "Did planner create reasonable tasks?" | Ralph (needs reasoning) |
| "Is the code style consistent?" | Ralph (subjective) |
| "Did builder follow the spec intent?" | Ralph (interpretation needed) |

---

### Level 6: Quality Evaluation (Assumes: Behaviors Work)

**Assumes:** Ralph behaves correctly. Evaluates subjective quality.

Uses LLM-as-judge via meta preset:

```bash
ralph-test evaluate \
    --criterion plan_quality \
    --target .agent/scratchpad.md \
    --judge-preset judge
```

**This level is optional for correctness** - it evaluates quality, not functionality.

---

## Batch Verification with Ralph

### The verify-behaviors Skill

Create a `/verify-behaviors` skill that orchestrates full catalog verification:

```bash
# Verify all behaviors in catalog
ralph /verify-behaviors

# Verify specific category
ralph /verify-behaviors --category planner

# Verify single behavior
ralph /verify-behaviors --id PL-007
```

**What happens internally:**

```
┌─────────────────────────────────────────────────────────────────┐
│                    /verify-behaviors                             │
└─────────────────────────────────────────────────────────────────┘
                              │
        ┌─────────────────────┼─────────────────────┐
        ▼                     ▼                     ▼
┌───────────────┐   ┌───────────────┐   ┌───────────────┐
│ Load Catalog  │   │ For each      │   │ Aggregate     │
│ from spec.md  │ → │ behavior:     │ → │ Results       │
│               │   │ - Setup       │   │               │
│               │   │ - Execute     │   │ 78 behaviors  │
│               │   │ - Verify      │   │ 75 PASS       │
│               │   │ - Cleanup     │   │ 3 FAIL        │
└───────────────┘   └───────────────┘   └───────────────┘
```

### Two-Phase Verification Loop

For each behavior in the catalog:

**Phase 1: Execute (Deterministic)**
```bash
# Run Ralph with mock/replay to produce artifacts
ralph --backend mock \
      --mock-responses "$(cat cassettes/${behavior_id}.yaml)" \
      --workspace /tmp/verify-${behavior_id} \
      --max-iterations ${max_iter} \
      "${task_prompt}"
```

**Phase 2: Verify (Ralph-as-Verifier)**
```bash
# Run Ralph with verifier hat to check artifacts
ralph --hat verifier \
      --workspace /tmp/verify-${behavior_id} \
      --verify-behavior ${behavior_id} \
      --verify-spec ./specs/behavioral-verification.spec.md \
      --output-format json
```

### Verification Report Structure

```json
{
  "catalog": "behavioral-verification.spec.md",
  "timestamp": "2026-01-13T10:30:00Z",
  "summary": {
    "total": 78,
    "passed": 75,
    "failed": 3,
    "skipped": 0
  },
  "by_category": {
    "event-routing": {"total": 10, "passed": 10, "failed": 0},
    "planner-behaviors": {"total": 15, "passed": 14, "failed": 1},
    "builder-behaviors": {"total": 15, "passed": 13, "failed": 2},
    "safeguards": {"total": 12, "passed": 12, "failed": 0},
    "completion-detection": {"total": 8, "passed": 8, "failed": 0},
    "core-behaviors": {"total": 8, "passed": 8, "failed": 0},
    "integration": {"total": 10, "passed": 10, "failed": 0}
  },
  "failures": [
    {
      "id": "PL-005",
      "description": "Planner cancels stuck tasks [~]",
      "verdict": "FAIL",
      "evidence": "After 3 build.blocked events, task still marked [ ] not [~]",
      "reasoning": "Planner did not detect consecutive blocks from same task"
    },
    {
      "id": "BU-002",
      "description": "Builder runs backpressure (tests)",
      "verdict": "FAIL",
      "evidence": "No test execution found in session before build.done",
      "reasoning": "Builder emitted build.done without running cargo test"
    }
  ],
  "behaviors": [
    {"id": "ER-001", "verdict": "PASS", "duration_ms": 1200},
    {"id": "ER-002", "verdict": "PASS", "duration_ms": 1100},
    // ... all 78 behaviors
  ]
}
```

### Integration with test-tools Spec

The `/verify-behaviors` skill uses test-tools under the hood:

```
┌─────────────────────────────────────────────────────────────────┐
│                    /verify-behaviors                             │
│                                                                  │
│  Uses:                                                           │
│  ├── test_setup()      - Create isolated workspace               │
│  ├── test_run()        - Execute with mock/replay                │
│  ├── test_inspect()    - Parse session for verifier              │
│  ├── test_evaluate()   - Ralph-as-verifier (judge preset)        │
│  ├── test_report()     - Generate JUnit/JSON output              │
│  └── test_cleanup()    - Remove workspace                        │
└─────────────────────────────────────────────────────────────────┘
```

### When Ralph Finds Failures

When verification fails, Ralph can help diagnose:

```bash
# Get detailed diagnosis for a failure
ralph /diagnose-failure --behavior PL-005

# Ralph analyzes:
# 1. The behavior spec (what should happen)
# 2. The session recording (what did happen)
# 3. The code implementation (why it might fail)
# 4. Suggests fixes
```

**Example diagnosis output:**

```
═══════════════════════════════════════════════════════════════════
 DIAGNOSIS │ PL-005: Planner cancels stuck tasks [~]
═══════════════════════════════════════════════════════════════════

## Expected Behavior
After 3 consecutive build.blocked events for the same task,
planner should mark it [~] (cancelled) in scratchpad.

## What Happened
Session shows:
  - Iteration 1: build.blocked (task: "Add auth")
  - Iteration 2: build.blocked (task: "Add auth")
  - Iteration 3: build.blocked (task: "Add auth")
  - Iteration 4: build.task (task: "Add auth")  ← Should have been cancelled

Scratchpad final state:
  - [ ] Add auth  ← Still pending, should be [~]

## Root Cause Analysis
Examining crates/ralph-core/src/instructions.rs:142

The planner instructions mention "after 3 consecutive blocks, cancel"
but there's no mechanism to:
  1. Track which task caused each block
  2. Count consecutive blocks per task
  3. Signal cancellation to the prompt

## Suggested Fix
Add to LoopState:
  - blocked_task_counts: HashMap<String, u32>
  - Increment on build.blocked with task identifier
  - Include count in planner prompt context
  - Reset count when task changes

## Files to Modify
  - crates/ralph-core/src/event_loop.rs (LoopState)
  - crates/ralph-core/src/instructions.rs (prompt injection)
```

## Progressive CI Pipeline

```yaml
name: Progressive Verification

jobs:
  level-0-compile:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - run: cargo build --workspace --release

  level-1-unit:
    needs: level-0-compile
    runs-on: ubuntu-latest
    steps:
      - run: cargo test --workspace

  level-2-smoke:
    needs: level-1-unit
    runs-on: ubuntu-latest
    steps:
      - run: ./target/release/ralph --version
      - run: ./target/release/ralph --help

  level-3-execution:
    needs: level-2-smoke
    runs-on: ubuntu-latest
    steps:
      - run: ./scripts/level3-smoke.sh

  level-4-routing:
    needs: level-3-execution
    runs-on: ubuntu-latest
    steps:
      - run: ./scripts/level4-verify.sh

  level-5-behavioral:
    needs: level-4-routing
    runs-on: ubuntu-latest
    steps:
      - run: ralph-test verify --catalog ./specs/behavioral-verification.spec.md
```

**Key:** Each job has `needs:` dependency on the previous level. If Level 2 fails, Levels 3-6 don't run.

## Design Principles

### 1. Behavior-Driven, Not Implementation-Driven

Each test verifies a **user-observable behavior**, not internal implementation details:

```
❌ Bad:  "EventBus.subscribers() returns correct list"
✅ Good: "Planner receives task.start event and produces build.task"
```

### 2. One Behavior Per Test

Each test has a single, clear assertion target:

```
❌ Bad:  "Planner handles all event types correctly"
✅ Good: "Planner produces build.task when receiving task.start"
✅ Good: "Planner produces build.task when receiving build.done"
✅ Good: "Planner produces build.task when receiving build.blocked"
```

### 3. Deterministic via Record/Replay

Tests use cassettes (VCR pattern) for determinism:

- **Record once** with real LLM (captures realistic behavior)
- **Replay forever** in CI (zero cost, millisecond execution)
- **Mock fallback** for edge cases that are hard to record

### 4. Hierarchical Organization

Tests organized by subsystem → behavior category → specific behavior:

```
ralph-behaviors/
├── event-routing/
│   ├── topic-matching/
│   └── hat-subscriptions/
├── hat-behaviors/
│   ├── planner/
│   └── builder/
├── safeguards/
│   ├── termination/
│   └── loop-detection/
└── integration/
    └── end-to-end/
```

## Behavioral Test Catalog

### Category 1: Event Routing (10 tests)

Tests that verify events flow correctly between hats.

| ID | Behavior | Setup | Assert |
|----|----------|-------|--------|
| ER-001 | task.start routes to planner | Publish task.start | Planner hat activated |
| ER-002 | build.task routes to builder | Publish build.task | Builder hat activated |
| ER-003 | build.done routes to planner | Publish build.done | Planner hat activated |
| ER-004 | build.blocked routes to planner | Publish build.blocked | Planner hat activated |
| ER-005 | Unknown topic produces no routing | Publish unknown.topic | No hat activated |
| ER-006 | Glob patterns match correctly | Hat subscribes to `build.*` | Receives build.task, build.done |
| ER-007 | Events don't route to source hat | Planner publishes build.task | Planner doesn't re-trigger |
| ER-008 | Direct target bypasses subscription | Event with target=builder | Builder receives regardless of topic |
| ER-009 | Observer receives all events | Any event published | Observer callback invoked |
| ER-010 | Multiple subscribers all receive | Two hats subscribe to same topic | Both activated |

**Test Template (ER-001):**

```yaml
behavior: "task.start routes to planner"
id: ER-001

setup:
  workspace_id: "er-001-task-start-routes"
  scratchpad: "## Plan\n- [ ] Implement feature"

run:
  task: "Implement the feature"
  backend: mock
  max_iterations: 1
  mock_responses:
    - hat: planner
      output: "<event topic=\"build.task\">Implement feature X</event>"

assert:
  - type: event_sequence
    topics: ["task.start", "build.task"]
  - type: hat_sequence
    hats: ["planner"]
  - type: iteration_count
    hat: planner
    exact: 1
```

---

### Category 2: Planner Hat Behaviors (15 tests)

Tests that verify the planner hat behaves correctly.

| ID | Behavior | Setup | Assert |
|----|----------|-------|--------|
| PL-001 | Planner reads specs directory | Specs exist in ./specs/ | Planner references spec content |
| PL-002 | Planner creates scratchpad if missing | No scratchpad exists | .agent/scratchpad.md created |
| PL-003 | Planner dispatches ONE task per iteration | Multiple pending tasks | Only one build.task emitted |
| PL-004 | Planner marks completed tasks [x] | build.done received | Scratchpad shows [x] |
| PL-005 | Planner cancels stuck tasks [~] | 3 consecutive build.blocked | Scratchpad shows [~] |
| PL-006 | Planner prioritizes ISSUES.md fixes | ISSUES.md and spec tasks exist | ISSUES.md task dispatched first |
| PL-007 | Planner outputs completion promise when done | All tasks [x] or [~] | Output contains LOOP_COMPLETE |
| PL-008 | Planner validates work matches spec | build.done with implementation | Planner checks against spec |
| PL-009 | Planner handles empty task list | No pending tasks | Outputs completion promise |
| PL-010 | Planner does NOT implement code | Task requires implementation | No file writes, only build.task |
| PL-011 | Planner re-plans on build.blocked | build.blocked received | Updates scratchpad, new build.task |
| PL-012 | Planner respects task dependencies | Task B depends on Task A | Task A dispatched first |
| PL-013 | Planner updates scratchpad atomically | Concurrent access | No corruption |
| PL-014 | Planner includes context in build.task | Complex task | build.task payload has sufficient context |
| PL-015 | Planner handles malformed scratchpad | Invalid markdown | Recovers gracefully |

---

### Category 3: Builder Hat Behaviors (15 tests)

Tests that verify the builder hat behaves correctly.

| ID | Behavior | Setup | Assert |
|----|----------|-------|--------|
| BU-001 | Builder implements ONE task per iteration | build.task received | Only one build.done emitted |
| BU-002 | Builder runs backpressure (tests) | Code written | Tests executed before build.done |
| BU-003 | Builder runs backpressure (lint) | Code written | Lint executed before build.done |
| BU-004 | Builder runs backpressure (typecheck) | Code written | Typecheck executed before build.done |
| BU-005 | Builder commits on success | Task completed | Git commit created |
| BU-006 | Builder marks task [x] in scratchpad | Task completed | Scratchpad updated |
| BU-007 | Builder emits build.blocked when stuck | Cannot complete task | build.blocked with reason |
| BU-008 | Builder does NOT plan work | build.task received | No scratchpad planning changes |
| BU-009 | Builder does NOT output completion promise | Task completed | No LOOP_COMPLETE in output |
| BU-010 | Builder follows existing code patterns | Codebase has patterns | Generated code matches style |
| BU-011 | Builder handles missing files gracefully | File reference doesn't exist | build.blocked, not crash |
| BU-012 | Builder provides unblock recommendation | build.blocked emitted | Payload includes suggestion |
| BU-013 | Builder searches before assuming missing | Task mentions feature | Grep/search executed first |
| BU-014 | Builder handles test failures | Tests fail | Fixes or emits build.blocked |
| BU-015 | Builder respects file boundaries | Task scoped to one file | Only that file modified |

---

### Category 4: Safeguards & Termination (12 tests)

Tests that verify safety mechanisms work correctly.

| ID | Behavior | Setup | Assert |
|----|----------|-------|--------|
| SF-001 | CompletionPromise exits with code 0 | Planner outputs LOOP_COMPLETE | exit_code: 0 |
| SF-002 | MaxIterations exits with code 2 | iteration >= max_iterations | exit_code: 2, reason: MaxIterations |
| SF-003 | MaxRuntime exits with code 2 | elapsed >= max_runtime_secs | exit_code: 2, reason: MaxRuntime |
| SF-004 | MaxCost exits with code 2 | cost >= max_cost_usd | exit_code: 2, reason: MaxCost |
| SF-005 | ConsecutiveFailures exits with code 1 | failures >= threshold | exit_code: 1, reason: ConsecutiveFailures |
| SF-006 | LoopThrashing exits with code 1 | 3+ consecutive build.blocked | exit_code: 1, reason: LoopThrashing |
| SF-007 | SIGINT exits with code 130 | Send SIGINT | exit_code: 130, clean shutdown |
| SF-008 | Failure counter resets on success | Failure then success | consecutive_failures: 0 |
| SF-009 | Loop detection triggers on similarity | 90%+ similar outputs | Detected as loop |
| SF-010 | Loop detection ignores dissimilar | <90% similar outputs | Not detected as loop |
| SF-011 | Safeguards checked every iteration | Any iteration | LoopState.check_termination() called |
| SF-012 | Partial session recorded on termination | MaxIterations hit | session.jsonl has all events |

---

### Category 5: Completion Detection (8 tests)

Tests that verify completion is detected correctly.

| ID | Behavior | Setup | Assert |
|----|----------|-------|--------|
| CD-001 | Checkbox marker detected | `- [x] TASK_COMPLETE` | Completion detected |
| CD-002 | Unchecked marker NOT detected | `- [ ] TASK_COMPLETE` | Completion NOT detected |
| CD-003 | Promise in output detected | `LOOP_COMPLETE` in stdout | Completion detected |
| CD-004 | Promise in event payload NOT detected | `<event>LOOP_COMPLETE</event>` | Completion NOT detected |
| CD-005 | Promise is case-insensitive | `loop_complete` | Completion detected |
| CD-006 | Custom promise works | Custom string configured | That string detected |
| CD-007 | Partial match NOT detected | `LOOP_COMPLETE_EXTRA` | Completion NOT detected |
| CD-008 | Whitespace around promise OK | `  LOOP_COMPLETE  ` | Completion detected |

---

### Category 6: Core Behaviors (8 tests)

Tests that verify always-present behaviors.

| ID | Behavior | Setup | Assert |
|----|----------|-------|--------|
| CB-001 | Scratchpad persists across iterations | Write in iter 1, read in iter 2 | Content preserved |
| CB-002 | Fresh context each iteration | Complex iter 1 | Iter 2 prompt doesn't reference iter 1 |
| CB-003 | Guardrails injected for planner | Planner prompt | Contains search-first, backpressure |
| CB-004 | Guardrails injected for builder | Builder prompt | Contains search-first, backpressure |
| CB-005 | Specs directory referenced | Specs exist | Prompt mentions ./specs/ |
| CB-006 | Custom guardrails injected | Config has custom guardrails | Appear in prompt |
| CB-007 | Less-is-more principle applied | Large task | Atomic subtask dispatched |
| CB-008 | Event history logged | Events occur | .ralph/events-*.jsonl populated |

---

### Category 7: Integration / End-to-End (10 tests)

Full workflow tests using recorded cassettes.

| ID | Behavior | Setup | Assert |
|----|----------|-------|--------|
| E2E-001 | Happy path: plan → build → complete | Simple spec | exit_code: 0, file created |
| E2E-002 | Multi-iteration workflow | Spec with 3 tasks | 3+ iterations, all [x] |
| E2E-003 | Build blocked triggers re-plan | Builder can't complete | Planner re-dispatches |
| E2E-004 | Recovery from failure | First attempt fails | Second attempt succeeds |
| E2E-005 | Custom hat integration | Reviewer hat configured | Reviewer triggers on build.done |
| E2E-006 | Large prompt handling | Prompt > 7000 chars | Claude backend uses temp file |
| E2E-007 | Interactive mode behavior | TTY attached | Permission prompts work |
| E2E-008 | Headless mode behavior | No TTY | Runs autonomously |
| E2E-009 | Resume from checkpoint | Previous run interrupted | Resumes from scratchpad state |
| E2E-010 | Cost tracking accuracy | Multi-iteration | Cumulative cost matches expected |

---

## Cassette Recording Strategy

### When to Use Mock vs. Record/Replay

| Scenario | Strategy | Rationale |
|----------|----------|-----------|
| Event routing tests | Mock | Deterministic, no LLM needed |
| Safeguard tests | Mock | Need to control termination conditions |
| Completion detection | Mock | Need exact output patterns |
| Hat behavior tests | Record/Replay | Realistic agent responses |
| E2E integration tests | Record/Replay | Full realistic workflows |

### Cassette Naming Convention

```
cassettes/
├── planner/
│   ├── pl-001-reads-specs.yaml
│   ├── pl-007-completion-promise.yaml
│   └── ...
├── builder/
│   ├── bu-001-implements-one-task.yaml
│   └── ...
└── e2e/
    ├── e2e-001-happy-path.yaml
    └── ...
```

### Recording Checklist

Before recording a cassette:

1. [ ] Clean workspace (no leftover state)
2. [ ] Minimal fixtures (only what's needed)
3. [ ] Redactions configured (timestamps, UUIDs, paths)
4. [ ] Backend credentials available
5. [ ] Max iterations set appropriately

After recording:

1. [ ] Review cassette for sensitive data
2. [ ] Verify replay produces same result
3. [ ] Add to version control
4. [ ] Document any quirks in cassette header

---

## Evaluation Workflow

### Initial Recording (One-Time Setup)

```bash
# For each test that needs cassettes
ralph-test record \
  --workspace "pl-007-workspace" \
  --cassette "planner/pl-007-completion-promise" \
  --task "Complete all tasks in scratchpad" \
  --backend claude \
  --redactions '$TIMESTAMP,$UUID,$PATH'
```

### CI Pipeline (Repeatable)

```yaml
# .github/workflows/behavioral-verification.yml
name: Behavioral Verification

on: [push, pull_request]

jobs:
  verify:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4

      - name: Run Behavioral Tests
        run: |
          ralph-test replay-all \
            --cassettes ./cassettes \
            --report junit \
            --output ./reports/behavioral.xml

      - name: Upload Results
        uses: actions/upload-artifact@v4
        with:
          name: test-results
          path: ./reports/

      - name: Publish to CI
        uses: mikepenz/action-junit-report@v4
        with:
          report_paths: './reports/*.xml'
```

### Local Development

```bash
# Run all behavioral tests
ralph-test verify --catalog ./specs/behavioral-verification.spec.md

# Run specific category
ralph-test verify --category safeguards

# Run single test
ralph-test verify --id SF-001

# Re-record a stale cassette
ralph-test record --id PL-007 --force
```

---

## LLM-as-Judge Evaluations

Some behaviors require subjective evaluation. Use `test_evaluate` with meta preset:

| Test | Criterion | Target | Threshold |
|------|-----------|--------|-----------|
| PL-014 | context_quality | build.task payload | ≥3 |
| BU-010 | code_style | Generated files | ≥3 |
| BU-012 | recommendation_quality | build.blocked payload | ≥3 |
| E2E-001 | task_completion | Final workspace state | ≥4 |

**Evaluation Rubric (context_quality):**

```yaml
rubric:
  5: "Complete context: task goal, acceptance criteria, relevant file paths, constraints"
  4: "Good context: task goal clear, most relevant info included"
  3: "Acceptable: task understandable but missing some context"
  2: "Poor: task vague, builder would need to ask questions"
  1: "Unacceptable: task cannot be understood without clarification"
```

---

## Coverage Metrics

Track which behaviors are verified:

| Category | Total | Implemented | Coverage |
|----------|-------|-------------|----------|
| Event Routing | 10 | 0 | 0% |
| Planner Behaviors | 15 | 0 | 0% |
| Builder Behaviors | 15 | 0 | 0% |
| Safeguards | 12 | 0 | 0% |
| Completion Detection | 8 | 0 | 0% |
| Core Behaviors | 8 | 0 | 0% |
| End-to-End | 10 | 0 | 0% |
| **Total** | **78** | **0** | **0%** |

Target: 100% coverage of documented behaviors before v1.0 release.

---

## Acceptance Criteria

### Catalog Completeness

- **Given** the behavioral verification catalog
- **When** all tests pass
- **Then** Ralph's core functionality is verified

---

### Determinism

- **Given** a recorded cassette
- **When** replayed 100 times
- **Then** all 100 runs produce identical results

---

### CI Integration

- **Given** behavioral tests in CI pipeline
- **When** a PR breaks a behavior
- **Then** CI fails with clear indication of which behavior broke

---

### Coverage Tracking

- **Given** the coverage metrics table
- **When** a new behavior is added to Ralph
- **Then** a corresponding test is added to the catalog

---

## Maintenance

### When to Update Catalog

1. **New feature added** → Add tests for new behaviors
2. **Bug fixed** → Add regression test for the bug
3. **Behavior changed** → Update existing test assertions
4. **Test flaky** → Re-record cassette or convert to mock

### Cassette Staleness

Cassettes may become stale when:
- Backend API changes response format
- Ralph's prompt format changes significantly
- Redaction patterns miss new dynamic values

**Staleness detection:**
```bash
# Check if cassettes are older than spec changes
ralph-test check-staleness --cassettes ./cassettes --specs ./specs
```

### Quarterly Review

Every quarter:
1. [ ] Review coverage metrics
2. [ ] Identify untested behaviors
3. [ ] Re-record cassettes with latest backend
4. [ ] Update rubrics based on observed patterns
