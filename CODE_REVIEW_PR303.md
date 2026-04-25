# Code Review: PR #303 — Remove `agent_wrote_events` guard from text completion fallback

**Reviewer:** Robby (fresh-context subagent)  
**Date:** 2026-04-19  
**Commit:** `pr-303` branch (11 insertions, 5 deletions, 1 file)

---

## Summary

The PR removes `!agent_wrote_events` from the text-based completion promise fallback in `loop_runner.rs`. Previously, this fallback only fired for backends that wrote no JSONL events at all (e.g., kiro-cli). Now it fires unconditionally when the final non-empty line of backend output matches the completion promise (e.g., `LOOP_COMPLETE`).

**Motivation:** OpenCode emits `LOOP_COMPLETE` via both JSONL event and stdout text, but the JSONL event path can reject completion (e.g., `required_events` validation), causing the loop to spin to `max_iterations`.

---

## Findings

### 🔴 CRITICAL: Text fallback bypasses `persistent` mode suppression

`check_completion_event()` in `event_loop/mod.rs` (line 658) explicitly suppresses completion when `event_loop.persistent` is true, injecting a `task.resume` event instead. This keeps the loop alive for continuous monitoring/watchdog use cases.

**The text fallback has no such check.** It terminates immediately. A persistent-mode loop will now terminate prematurely if any backend prints the completion promise as its final output line — even though the user configured it to run indefinitely.

**Reproduction path:**
1. Configure `event_loop.persistent: true` (watchdog/monitoring mode)
2. Backend completes a task cycle and prints `LOOP_COMPLETE` as final output
3. `check_completion_event()` correctly suppresses the JSONL event
4. Text fallback fires anyway → loop terminates
5. Persistent loop is dead

**Severity:** High. This silently breaks persistent mode for any backend that echoes the completion promise to stdout.

**Recommendation:** Add a `persistent` mode guard to the text fallback:
```rust
if !config.event_loop.persistent
    && EventParser::contains_promise(&output, &config.event_loop.completion_promise)
```

---

### 🔴 CRITICAL: Text fallback bypasses task verification (`memories/tasks` mode)

When `config.memories.enabled` is true, `check_completion_event()` (line 684) calls `verify_tasks_complete()` to ensure all runtime tasks are closed before accepting completion. If open tasks remain, completion is rejected and a `task.resume` event is injected with instructions to continue.

**The text fallback skips this entirely.** A backend that prints `LOOP_COMPLETE` as its final output line will terminate the loop even when runtime tasks remain open.

**Impact:** Half-completed task queues are abandoned silently. The user asked for multi-task completion; the loop stops after one task's backend happens to print the promise.

**Recommendation:** Either delegate through `check_completion_event()` instead of direct termination, or replicate the task verification logic in the fallback path. The cleanest approach would be to call `check_completion_event()` and only fall through to text detection if it returns `None`:

```rust
// Try event-based completion (respects required_events, persistent, tasks)
if let Some(reason) = event_loop.check_completion_event() {
    // ... existing event-based termination path ...
}

// Fallback: text-based (only for backends without ralph emit)
if !agent_wrote_events
    && EventParser::contains_promise(&output, &config.event_loop.completion_promise)
{
    // ... text fallback termination ...
}
```

Note: this is essentially **reverting the change**. See "Design Concern" below for the deeper issue.

---

### 🟡 MAJOR: Intentional bypass of `required_events` validation

The PR description explicitly acknowledges this: *"When the JSONL event path fails (e.g., required_events validation rejects the completion, or the event is not the last in the batch), the text fallback should still terminate."*

`required_events` is configured as `["review.passed"]` in `code-assist.yml`. This means the system architect's intent was: "Do not declare success until a code review has passed." By allowing text fallback to bypass this, the PR subverts that architectural guarantee.

**The PR's argument:** The backend declared completion — respect it.  
**The counter-argument:** `required_events` exists precisely because backends can declare premature completion. An LLM that prints `LOOP_COMPLETE` without running a review is a bug, not a legitimate completion signal.

**This is a design tradeoff, not a pure bug.** But it should be an explicit configuration decision, not a silent behavior change. If the intent is to make text fallback always win, document it prominently and consider a config flag like `text_fallback_overrides_required_events: true`.

---

### 🟡 MAJOR: Two redundant termination paths create maintenance risk

After this change, there are **two independent completion detection mechanisms** that can fire for the same iteration:

1. `check_completion_event()` — validates `required_events`, `persistent`, tasks
2. Text fallback — validates only "is promise the last non-empty line"

Both produce `TerminationReason::CompletionPromise` but take different validation paths. This means:
- Fixing a safety bug in one path may not fix it in the other (demonstrated by the `persistent` and `tasks` bypass above)
- The termination code is duplicated (~30 lines of hook dispatch + handle_termination in each branch)
- Future developers must reason about two code paths when adding completion safety features

**Recommendation:** Extract shared termination logic into a helper function. Consider having the text fallback set `self.state.completion_requested = true` and then call `check_completion_event()`, so all validation runs through a single gate.

---

### 🟢 MINOR: No test coverage for the new behavior

The PR removes a guard but adds no tests. While existing `contains_promise` tests cover the parser logic, there are no tests for:
- Text fallback firing when `agent_wrote_events` is true
- Text fallback NOT firing when `persistent` mode is active (currently broken)
- Text fallback NOT firing when open tasks exist (currently broken)
- Text fallback bypassing `required_events` rejection

The `test_builder_cannot_terminate_loop` test in `tests.rs` validates that `process_output` doesn't terminate for builder hats — but this test goes through `EventLoop::process_output()`, not the `loop_runner.rs` text fallback. The fallback path is untested at the integration level.

**Recommendation:** Add at minimum a test that verifies persistent mode suppresses the text fallback.

---

### 🟢 MINOR: `output` includes all backend stdout — slight false positive risk

`outcome.output` is the raw backend stdout, which may include command output, tool responses, ANSI escape codes, and other non-agent content. While `contains_promise()` strips `<event>` tags and requires the promise to be the final non-empty line, there is a narrow scenario:

A backend tool's stdout could end with `LOOP_COMPLETE` if, for example, the agent runs `cat` on a file whose last line is the promise string, or a test framework echoes completion tokens. The `strip_event_tags` safety check only strips `<event>` XML blocks, not arbitrary content.

This is unlikely in practice but not impossible. The risk is slightly elevated by removing the `!agent_wrote_events` guard, since now backends that DO use `ralph emit` (and thus have more complex output) are also eligible for text fallback.

---

### 🟢 MINOR: PR description mentions issue #187 (`default_publishes` cascading) implicitly

The `default_publishes` injection at line 2426 is still gated on `!agent_wrote_events`, which is correct — default publishes should only fire when the agent didn't write any events. Removing the guard from the text fallback doesn't change this, so #187 is not made worse by this PR specifically.

However, the interaction is worth noting: if a backend writes events (so `default_publishes` is suppressed) but the events don't trigger any hat (so the loop would stall), the text fallback could now terminate the loop instead. This is arguably the desired behavior (the backend declared completion), but it means `default_publishes` safety nets are less effective.

---

### ✅ CORRECT: `contains_promise` is a reasonable text detection mechanism

The implementation requiring the promise to be the **final non-empty line** after stripping event tags is well-designed. It prevents:
- Prompt echo false positives (promise in instructions)
- Mid-reasoning false positives (promise in agent thinking)
- Event payload false positives (promise inside `<event>` tags)

The test suite for `contains_promise` is thorough (tests at lines 765-834 of `event_parser.rs`).

---

### ✅ CORRECT: The `ralph emit` recovery heuristic is unaffected

The `output_mentions_ralph_emit` / `recover_expected_emit_after_output` heuristic (lines 2391-2404) still runs and can set `agent_wrote_events = true`. This logic correctly handles the race condition where JSONL events arrive slightly after output text. Removing the guard from the text fallback doesn't interfere with this.

---

### ✅ CORRECT: The observed problem (OpenCode spinning) is real

The described behavior — OpenCode emitting `LOOP_COMPLETE` every iteration for 72 remaining iterations — is a genuine user-facing issue. The fix direction is reasonable; the implementation just needs the safety guards mentioned above.

---

## Design Concern

The root tension in this PR is: **Should the text fallback be a first-class termination signal or a last-resort escape hatch?**

Currently, the code treats JSONL events as the primary, validated path and text as a fallback. The PR elevates text to "independent termination signal." This fundamentally changes the trust model:

- **Before:** Text fallback trusts backends that can't use `ralph emit` (simple tools)
- **After:** Text fallback trusts ALL backends, even those that can use `ralph emit` but whose events failed validation

A cleaner design might be:
1. **Keep the `!agent_wrote_events` guard** for the unconditional text fallback
2. **Add a new fallback for the OpenCode case**: if `agent_wrote_events` is true AND `check_completion_event()` rejected it AND `contains_promise` matches, log a warning and terminate with a distinct reason (e.g., `TerminationReason::TextFallbackCompletion`)

This preserves safety for normal backends while handling the OpenCode edge case explicitly.

---

## Verdict

The PR solves a real problem but introduces two critical regressions (persistent mode bypass, task verification bypass). The change should not merge without at minimum:
1. Adding `!config.event_loop.persistent` guard
2. Adding task verification or routing through `check_completion_event()`
3. Adding test coverage

The design question of whether text fallback should override `required_events` should be an explicit, documented decision — not a side effect of removing a guard.
