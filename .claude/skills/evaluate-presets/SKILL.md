---
name: evaluate-presets
description: Use when testing Ralph's hat collection presets, validating preset configurations, or auditing the preset library for bugs and UX issues.
---

# Evaluate Presets

## Overview

Systematically test all hat collection presets using shell scripts. Direct CLI invocation—no meta-orchestration complexity.

## When to Use

- Testing preset configurations after changes
- Auditing the preset library for quality
- Validating new presets work correctly
- After modifying hat routing logic

## Quick Start

**Evaluate a single preset:**
```bash
./tools/evaluate-preset.sh tdd-red-green claude
```

**Evaluate all presets:**
```bash
./tools/evaluate-all-presets.sh claude
```

**Arguments:**
- First arg: preset name (without `.yml` extension)
- Second arg: backend (`claude` or `kiro`, defaults to `claude`)

## Bash Tool Configuration

**IMPORTANT:** When invoking these scripts via the Bash tool, use these settings:

- **Single preset evaluation:** Use `timeout: 600000` (10 minutes max) and `run_in_background: true`
- **All presets evaluation:** Use `timeout: 600000` (10 minutes max) and `run_in_background: true`

Since preset evaluations can run for hours (especially the full suite), **always run in background mode** and use the `TaskOutput` tool to check progress periodically.

**Example invocation pattern:**
```
Bash tool with:
  command: "./tools/evaluate-preset.sh tdd-red-green claude"
  timeout: 600000
  run_in_background: true
```

After launching, use `TaskOutput` with `block: false` to check status without waiting for completion.

## What the Scripts Do

### `evaluate-preset.sh`

1. Loads test task from `tools/preset-test-tasks.yml` (if `yq` available)
2. Creates merged config with evaluation settings
3. Runs Ralph with `--record-session` for metrics capture
4. Captures output logs, exit codes, and timing
5. Extracts metrics: iterations, hats activated, events published

**Output structure:**
```
.eval/
├── logs/<preset>/<timestamp>/
│   ├── output.log          # Full stdout/stderr
│   ├── session.jsonl       # Recorded session
│   ├── metrics.json        # Extracted metrics
│   ├── environment.json    # Runtime environment
│   └── merged-config.yml   # Config used
└── logs/<preset>/latest -> <timestamp>
```

### `evaluate-all-presets.sh`

Runs all 12 presets sequentially and generates a summary:

```
.eval/results/<suite-id>/
├── SUMMARY.md              # Markdown report
├── <preset>.json           # Per-preset metrics
└── latest -> <suite-id>
```

## Presets Under Evaluation

| Preset | Test Task |
|--------|-----------|
| `tdd-red-green` | Add `is_palindrome()` function |
| `adversarial-review` | Review user input handler for security |
| `socratic-learning` | Understand `HatRegistry` |
| `spec-driven` | Specify and implement `StringUtils::truncate()` |
| `mob-programming` | Implement a `Stack` data structure |
| `scientific-method` | Debug failing mock test assertion |
| `code-archaeology` | Understand history of `config.rs` |
| `performance-optimization` | Profile hat matching |
| `api-design` | Design a `Cache` trait |
| `documentation-first` | Document `RateLimiter` |
| `incident-response` | Respond to "tests failing in CI" |
| `migration-safety` | Plan v1 to v2 config migration |

## Interpreting Results

**Exit codes from `evaluate-preset.sh`:**
- `0` — Success (LOOP_COMPLETE reached)
- `124` — Timeout (preset hung or took too long)
- Other — Failure (check `output.log`)

**Metrics in `metrics.json`:**
- `iterations` — How many event loop cycles
- `hats_activated` — Which hats were triggered
- `events_published` — Total events emitted
- `completed` — Whether completion promise was reached

## Hat Routing Performance

**Critical:** Validate that hats get fresh context per Tenet #1 ("Fresh Context Is Reliability").

### What Good Looks Like

Each hat should execute in its **own iteration**:
```
Iter 1: Ralph → publishes starting event → STOPS
Iter 2: Hat A → does work → publishes next event → STOPS
Iter 3: Hat B → does work → publishes next event → STOPS
Iter 4: Hat C → does work → LOOP_COMPLETE
```

### Red Flags (Same-Iteration Hat Switching)

**BAD:** Multiple hat personas in one iteration:
```
Iter 2: Ralph does Blue Team + Red Team + Fixer work
        ^^^ All in one bloated context!
```

### How to Check

**1. Count iterations vs events in `session.jsonl`:**
```bash
# Count iterations
grep -c "_meta.loop_start\|ITERATION" .eval/logs/<preset>/latest/output.log

# Count events published
grep -c "bus.publish" .eval/logs/<preset>/latest/session.jsonl
```

**Expected:** iterations ≈ events published (one event per iteration)
**Bad sign:** 2-3 iterations but 5+ events (all work in single iteration)

**2. Check for same-iteration hat switching in `output.log`:**
```bash
grep -E "ITERATION|Now I need to perform|Let me put on|I'll switch to" \
    .eval/logs/<preset>/latest/output.log
```

**Red flag:** Hat-switching phrases WITHOUT an ITERATION separator between them.

**3. Check event timestamps in `session.jsonl`:**
```bash
cat .eval/logs/<preset>/latest/session.jsonl | jq -r '.ts'
```

**Red flag:** Multiple events with identical timestamps (published in same iteration).

### Routing Performance Triage

| Pattern | Diagnosis | Action |
|---------|-----------|--------|
| iterations ≈ events | ✅ Good | Hat routing working |
| iterations << events | ⚠️ Same-iteration switching | Check prompt has STOP instruction |
| iterations >> events | ⚠️ Recovery loops | Agent not publishing required events |
| 0 events | ❌ Broken | Events not being read from JSONL |

### Root Cause Checklist

If hat routing is broken:

1. **Check workflow prompt** in `hatless_ralph.rs`:
   - Does it say "CRITICAL: STOP after publishing"?
   - Is the DELEGATE section clear about yielding control?

2. **Check hat instructions** propagation:
   - Does `HatInfo` include `instructions` field?
   - Are instructions rendered in the `## HATS` section?

3. **Check events context**:
   - Is `build_prompt(context)` using the context parameter?
   - Does prompt include `## PENDING EVENTS` section?

## Autonomous Fix Workflow

After evaluation, delegate fixes to subagents:

### Step 1: Triage Results

Read `.eval/results/latest/SUMMARY.md` and identify:
- `❌ FAIL` → Create code tasks for fixes
- `⏱️ TIMEOUT` → Investigate infinite loops
- `⚠️ PARTIAL` → Check for edge cases

### Step 2: Dispatch Task Creation

For each issue, spawn a Task agent:

```
"Use /code-task-generator to create a task for fixing: [issue from evaluation]
Output to: tasks/preset-fixes/"
```

### Step 3: Dispatch Implementation

For each created task:

```
"Use /code-assist to implement: tasks/preset-fixes/[task-file].code-task.md
Mode: auto"
```

### Step 4: Re-evaluate

```bash
./tools/evaluate-preset.sh <fixed-preset> claude
```

## Prerequisites

- **yq** (optional): For loading test tasks from YAML. Install: `brew install yq`
- **Cargo**: Must be able to build Ralph

## Related Files

- `tools/evaluate-preset.sh` — Single preset evaluation
- `tools/evaluate-all-presets.sh` — Full suite evaluation
- `tools/preset-test-tasks.yml` — Test task definitions
- `tools/preset-evaluation-findings.md` — Manual findings doc
- `presets/` — The preset collection being evaluated
