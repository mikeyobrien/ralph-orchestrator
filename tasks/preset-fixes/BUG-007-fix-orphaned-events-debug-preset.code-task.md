---
status: completed
created: 2026-01-15
started: 2026-01-15
completed: 2026-01-15
---
# Task: Fix Orphaned Events in Debug Preset (BUG-007)

## Description
Fix orphaned events in the debug.yml preset where two events (`hypothesis.confirmed` and `fix.failed`) had no hat handlers, breaking the debug workflow loop.

## Background
The debug preset follows a scientific method workflow for bug investigation:
1. Investigator forms hypotheses
2. Tester tests hypotheses (publishes `hypothesis.confirmed` or `hypothesis.rejected`)
3. Fixer implements fixes (publishes `fix.applied`)
4. Verifier validates fixes (publishes `fix.verified` or `fix.failed`)

Two events were orphaned (published but not handled):
- `hypothesis.confirmed` - when the tester confirms a hypothesis, no hat was triggered to propose a fix
- `fix.failed` - when the verifier reports a fix failed, no hat was triggered to retry

## Technical Requirements
1. Add `hypothesis.confirmed` to investigator's triggers so confirmed hypotheses lead to fix proposals
2. Add `fix.failed` to fixer's triggers so failed fixes can be retried

## Dependencies
- presets/debug.yml must exist
- Understanding of the debug workflow event flow

## Implementation Approach
1. Identify the investigator hat configuration
2. Add `hypothesis.confirmed` to its triggers array
3. Identify the fixer hat configuration
4. Add `fix.failed` to its triggers array
5. Verify YAML syntax is valid
6. Test that event flow is complete (no orphaned events)

## Acceptance Criteria

1. **Hypothesis Confirmed Triggers Investigator**
   - Given the tester publishes `hypothesis.confirmed` event
   - When the event is processed by the event loop
   - Then the investigator hat is triggered to propose a fix

2. **Fix Failed Triggers Fixer**
   - Given the verifier publishes `fix.failed` event
   - When the event is processed by the event loop
   - Then the fixer hat is triggered to retry the fix

3. **No Orphaned Events**
   - Given the complete debug preset configuration
   - When analyzing all published events against all triggers
   - Then every published event has at least one hat that triggers on it

4. **Valid YAML Syntax**
   - Given the modified debug.yml file
   - When parsed by a YAML parser
   - Then no syntax errors occur

## Resolution

This bug was fixed in commit `dbf3c3f1` with the following changes to `presets/debug.yml`:

**Investigator triggers (line 26):**
```yaml
triggers: ["debug.start", "hypothesis.rejected", "hypothesis.confirmed", "fix.verified"]
```

**Fixer triggers (line 103):**
```yaml
triggers: ["fix.propose", "fix.failed"]
```

## Event Flow After Fix

```
debug.start --> Investigator --> hypothesis.test --> Tester
                    ^                                   |
                    |                                   v
              fix.verified                    hypothesis.confirmed
                    ^                         hypothesis.rejected
                    |                                   |
                    |                                   v
               Verifier <-- fix.applied <-- Fixer <-- fix.propose
                    |                         ^
                    v                         |
               fix.failed --------------------+
```

## Metadata
- **Complexity**: Low
- **Labels**: Bug, Preset, Debug, Event-Flow, Orphaned-Events
- **Required Skills**: YAML, Event-driven architecture, Ralph presets
