# Loop Runner & Event Loop Research

## Architecture Overview

The event loop (`crates/ralph-core/src/event_loop/mod.rs`) drives hat activation. The loop runner (`crates/ralph-cli/src/loop_runner.rs`) manages backend execution.

## Event Loop

```rust
pub struct EventLoop {
    config: RalphConfig,
    registry: HatRegistry,
    bus: EventBus,
    state: LoopState,
    instruction_builder: InstructionBuilder,
    ralph: HatlessRalph,
    robot_guidance: Vec<String>,
    event_reader: EventReader,
    diagnostics: DiagnosticsCollector,
    loop_context: Option<LoopContext>,
    skill_registry: SkillRegistry,
    robot_service: Option<Box<dyn RobotService>>,
}
```

## Main Loop Iteration Cycle

**File:** `crates/ralph-cli/src/loop_runner.rs:789`

```
loop {
    1. Check interrupt
    2. Drain guidance queue
    3. Check termination
    4. Get next hat              ← hat selection
    5. Build prompt              ← prompt construction
    6. Execute backend           ← SEQUENTIAL BOTTLENECK
    7. Process output
    8. Read events from JSONL
    9. Check completion/cancellation
    10. Cooldown delay
}
```

## Sequential Bottleneck

**Primary:** Backend execution (`loop_runner.rs:1214-1287`)
- `tokio::select!` waits for EITHER execution OR interrupt
- **One backend spawned per iteration** — no concurrent hat execution
- Must complete before output processing

**Secondary:** Event bus access — `take_pending()` mutates shared bus

## Hat Selection

**File:** `crates/ralph-core/src/event_loop/mod.rs:658-679`

- Solo mode: returns any hat with pending events
- Multi-hat mode: **always returns "ralph"** (Hatless Ralph architecture)
- Ralph coordinates all hat personas

## Backend Execution

Three modes:
1. **ACP** (`execute_acp`, lines 1558-1616) — Kiro backend, creates fresh AcpExecutor
2. **PTY** (`execute_pty`, lines 1618-1768) — reuses PtyExecutor if TUI connected
3. **CLI** — standard CliExecutor

Per-hat backend overrides supported via `hat.backend` config.

## Where Concurrency Would Be Introduced

1. **Backend Pool** — Replace single executor with concurrent spawn
2. **Event Bus** — Arc<Mutex<EventBus>> for shared state, or separate buses per wave instance
3. **Iteration Model** — New "wave iteration" type that spawns N backends, collects results, then proceeds
4. **State Tracking** — LoopState needs wave-aware activation counting

## Loop State

**File:** `crates/ralph-core/src/event_loop/loop_state.rs`

```rust
pub struct LoopState {
    pub iteration: u32,
    pub last_hat: Option<HatId>,
    pub last_emitted_topic: Option<String>,
    pub consecutive_same_topic: u32,
    pub seen_topics: HashSet<String>,
    pub hat_activation_counts: HashMap<HatId, u32>,
    // ... failures, costs, tasks
}
```

No correlation/wave tracking today.

## Termination

Key reasons: CompletionPromise, MaxIterations, MaxRuntime, MaxCost, ConsecutiveFailures, LoopThrashing, LoopStale, Interrupted, Cancelled.
