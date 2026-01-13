# Known Issues

> **Note:** When a known issue is fixed, remove it from this file. An empty file means no known issues.

## Hat/Instructions Mismatch

**Severity:** Critical

**Symptom:** The iteration header displays the correct hat (e.g., `🎩 planner`) but the agent receives instructions for a different hat (e.g., builder instructions with "## BUILDER MODE").

**Example:**
```
═══════════════════════════════════════════════════════════════════════════════
 ITERATION 2 │ 🎩 planner │ 4m 21s elapsed │ 2/100
═══════════════════════════════════════════════════════════════════════════════
```
But agent output shows: "Since I'm in **Builder Mode**" and "❌ Create the scratchpad (planner does that)"

**Location:** The bug is suspected to be in the prompt building flow:
- `crates/ralph-cli/src/main.rs` - calls `event_loop.next_hat()` and `event_loop.build_prompt()`
- `crates/ralph-core/src/event_loop.rs:225` - `build_prompt()` matches on `hat_id.as_str()`

**Investigation Notes:**
- The same `hat_id` is used for both display and prompt building
- The match in `build_prompt()` should correctly route "planner" to `build_coordinator()` and "builder" to `build_ralph()`
- Need to add debug logging to trace the actual `hat_id` value at each step

## Loop Thrashing (Consequence of Hat Mismatch)

**Severity:** Critical

**Symptom:** The loop thrashes indefinitely, never reaching a terminal state. Each iteration emits `build.blocked` which triggers the next planner iteration, which also emits `build.blocked`.

**Root Cause:** When the planner receives builder instructions (due to the hat mismatch bug above):
1. Builder instructions say to pick a task from the scratchpad
2. Scratchpad doesn't exist (planner should create it)
3. Builder instructions say to emit `build.blocked` when stuck
4. `build.blocked` triggers the planner
5. Planner again receives builder instructions → goto step 1

**Flow:**
```
planner (wrong instructions) → build.blocked → triggers planner
                                    ↑                    │
                                    └────────────────────┘
```

**Note:** The `max_consecutive_failures` safeguard doesn't catch this because the iterations "succeed" (CLI exits 0), they're just logically stuck.

**Potential Fix:** Add detection for repeated `build.blocked` events from the same hat within N iterations.

