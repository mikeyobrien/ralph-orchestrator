# Workflows

## Primary Orchestration Loop

The main `ralph run` workflow:

```mermaid
flowchart TD
    Start[ralph run -p prompt] --> LoadConfig[Load ralph.yml]
    LoadConfig --> Preflight[Preflight checks]
    Preflight --> AcquireLock{Acquire loop.lock?}
    AcquireLock -->|Yes| PrimaryLoop[Run as primary loop]
    AcquireLock -->|No| CreateWorktree[Create git worktree]
    CreateWorktree --> WorktreeLoop[Run as worktree loop]

    PrimaryLoop --> InitEventLoop[Initialize EventLoop]
    WorktreeLoop --> InitEventLoop

    InitEventLoop --> RegisterHats[Register hats from config]
    RegisterHats --> LoadSkills[Load skills from dirs + built-ins]
    LoadSkills --> LoadMemories[Load memories from .ralph/agent/memories.md]
    LoadMemories --> PublishStart[Publish starting_event]

    PublishStart --> IterationStart{Next iteration}
    IterationStart --> DrainEvents[Drain pending events from EventBus]
    DrainEvents --> SelectHat[Select hat for event topic]
    SelectHat --> BuildPrompt[Build prompt via HatlessRalph]
    BuildPrompt --> RunHooks[Run iteration.before hooks]
    RunHooks --> ExecuteAgent[Execute agent via adapter]
    ExecuteAgent --> ParseOutput[Parse agent output]
    ParseOutput --> ExtractEvents[Extract events from JSONL]
    ExtractEvents --> PublishEvents[Publish events to EventBus]
    PublishEvents --> RunAfterHooks[Run iteration.after hooks]
    RunAfterHooks --> CheckTermination{Termination condition?}

    CheckTermination -->|LOOP_COMPLETE| Complete[CompletionPromise]
    CheckTermination -->|Max iterations| MaxIter[MaxIterations]
    CheckTermination -->|Max runtime| MaxTime[MaxRuntime]
    CheckTermination -->|Max cost| MaxCost[MaxCost]
    CheckTermination -->|Failures| Fail[ConsecutiveFailures]
    CheckTermination -->|No| IterationStart

    Complete --> PostLoop[Post-loop: merge queue, cleanup]
    MaxIter --> PostLoop
    MaxTime --> PostLoop
    MaxCost --> PostLoop
    Fail --> PostLoop
```

## Planning Workflow (PDD)

Interactive planning via `ralph plan`:

```mermaid
sequenceDiagram
    participant User
    participant CLI as ralph plan
    participant SOP as SOP Runner
    participant Agent as AI Agent
    participant FS as Filesystem

    User->>CLI: ralph plan "Add JWT auth"
    CLI->>SOP: Load PDD SOP
    SOP->>Agent: Start planning session
    
    loop Conversation
        Agent-->>User: Ask clarifying question
        User->>Agent: Provide answer
    end

    Agent->>FS: Write specs/jwt-auth/requirements.md
    Agent->>FS: Write specs/jwt-auth/design.md
    Agent->>FS: Write specs/jwt-auth/implementation-plan.md
    Agent-->>User: Planning complete, run: ralph run -p "Implement specs/jwt-auth/"
```

## Task Lifecycle

```mermaid
stateDiagram-v2
    [*] --> Open: task.create
    Open --> InProgress: task.run / agent picks up
    InProgress --> Closed: task.close (success)
    InProgress --> Failed: task.close (failure)
    Failed --> Open: task.retry
    Open --> [*]: task.delete
    Closed --> [*]: task.archive
```

## Parallel Loop Workflow

```mermaid
sequenceDiagram
    participant T1 as Terminal 1
    participant Primary as Primary Loop
    participant Lock as loop.lock
    participant T2 as Terminal 2
    participant Worktree as Worktree Loop
    participant MQ as Merge Queue

    T1->>Primary: ralph run -p "Add header"
    Primary->>Lock: Acquire lock ✓
    Primary->>Primary: Run iterations...

    T2->>Worktree: ralph run -p "Add footer"
    Worktree->>Lock: Acquire lock ✗ (held)
    Worktree->>Worktree: Create git worktree
    Worktree->>Worktree: Run iterations...

    Worktree->>MQ: Queue merge on completion
    Worktree->>Worktree: Exit

    Primary->>Primary: Complete iterations
    Primary->>MQ: Process merge queue
    Primary->>Primary: Merge worktree branches
    Primary->>Lock: Release lock
```

## Human-in-the-Loop (RObot)

```mermaid
sequenceDiagram
    participant Agent as AI Agent
    participant EL as Event Loop
    participant Bot as Telegram Bot
    participant Human

    Note over Bot: Bot starts on primary loop only

    Agent->>EL: Emit human.interact event
    EL->>Bot: Send question via Telegram
    EL->>EL: Block waiting for response

    Human->>Bot: Reply to question
    Bot->>EL: Publish human.response event
    EL->>Agent: Inject response into next iteration

    Note over Human: Proactive guidance (anytime)
    Human->>Bot: Send guidance message
    Bot->>EL: Publish human.guidance event
    EL->>Agent: Inject as "## ROBOT GUIDANCE" in prompt
```

## Hook Lifecycle

```mermaid
flowchart LR
    LS[loop.start] --> IB[iteration.before]
    IB --> Agent[Agent Execution]
    Agent --> IA[iteration.after]
    IA --> IB
    IA --> LE[loop.end]
```

Each hook phase:
1. `HookEngine` resolves hooks for the phase-event
2. Builds JSON payload with iteration context, loop metadata, active hat info
3. `HookExecutor` runs each hook command, pipes payload to stdin
4. Handles errors per `on_error` config (fail, warn, ignore)
5. Supports suspend/resume for long-running hooks

## Web Dashboard Workflow

```mermaid
sequenceDiagram
    participant User
    participant CLI as ralph web
    participant API as ralph-api (Rust)
    participant FE as React Frontend
    participant Loop as Orchestration Loop

    User->>CLI: ralph web
    CLI->>API: Start Axum server (port 3000)
    CLI->>FE: Start Vite dev server (port 5173)
    CLI->>User: Open browser

    FE->>API: WebSocket connect
    API->>FE: Stream events (iterations, tasks, logs)

    User->>FE: Create task / start loop
    FE->>API: RPC call (task.create / task.run)
    API->>Loop: Spawn ralph run process
    Loop->>API: Stream output via RPC events
    API->>FE: Forward events via WebSocket
```

## Build & CI Workflow

```mermaid
flowchart TD
    Push[Push / PR] --> EmbeddedCheck[Check embedded files in sync]
    EmbeddedCheck --> Test[cargo test]
    EmbeddedCheck --> WebTest[npm test]
    EmbeddedCheck --> Lint[cargo fmt + clippy]
    Test --> HooksBDD[Hooks BDD gate]
    Test --> MockE2E[Mock E2E tests]
    Test --> PackageCheck[cargo package check]
    
    WebTest --> Done[CI Pass]
    Lint --> Done
    HooksBDD --> Done
    MockE2E --> Done
    PackageCheck --> Done
```

## Session Recording & Replay

For smoke tests and debugging:

```mermaid
flowchart LR
    Record[ralph run --record-session file.jsonl] --> JSONL[session.jsonl]
    JSONL --> Replay[Smoke test: replay_backend]
    Replay --> Verify[Verify event parsing, hat selection, termination]
```

Fixtures stored in `crates/ralph-core/tests/fixtures/`.
