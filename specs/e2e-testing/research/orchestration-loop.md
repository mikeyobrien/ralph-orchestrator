# Ralph Orchestration Loop

## Core Architecture

Ralph is a **thin coordination layer**. The orchestrator acts as a state machine driven by events published to disk.

```
┌─────────────────────────────────────────────────────────┐
│ ITERATION CYCLE (repeats until termination)             │
├─────────────────────────────────────────────────────────┤
│ 1. Check termination conditions                         │
│ 2. Get next hat to execute                              │
│ 3. Build prompt with pending events & memories          │
│ 4. Execute prompt via backend (Claude, Kiro, etc)       │
│ 5. Parse events from agent output                       │
│ 6. Process output (publish events, track state)         │
│ 7. Read JSONL file for events agent may have written    │
│ 8. Loop back to step 1                                  │
└─────────────────────────────────────────────────────────┘
```

---

## Termination Conditions

| Condition | Exit Code | Description |
|-----------|-----------|-------------|
| `CompletionPromise` | 0 | Task completed successfully (LOOP_COMPLETE) |
| `MaxIterations` | 2 | Hit max_iterations config |
| `MaxRuntime` | 2 | Exceeded max_runtime_seconds |
| `MaxCost` | 2 | Spent more than max_cost_usd |
| `ConsecutiveFailures` | 1 | Too many failures |
| `LoopThrashing` | 1 | Same task blocked 3 times |
| `ValidationFailure` | 1 | 3+ consecutive malformed JSONL |
| `Interrupted` | 130 | SIGINT/SIGTERM |

---

## Event Flow

1. **Event Writing**: Agents emit via `<event topic="...">payload</event>` XML tags
2. **Event Storage**: Written to `.ralph/events-TIMESTAMP.jsonl`
3. **Event Reading**: `EventReader` tracks position, reads only new events
4. **Event Routing**: `EventBus` routes to subscribed hats

---

## E2E Testing Implications

### Must Validate:
1. **Iteration progression** - Loop advances correctly
2. **Event parsing** - XML tags extracted from output
3. **Termination detection** - All termination conditions work
4. **Completion promise** - Double confirmation required
5. **Backpressure** - build.done requires evidence (tests: pass, etc.)
6. **State persistence** - Scratchpad survives iterations
