# Workflows

## Primary Orchestration Loop

The main workflow executed by `ralph run`:

```mermaid
flowchart TD
    START([ralph run]) --> LOAD[Load Configuration]
    LOAD --> NORM[Normalize v1→v2 Config]
    NORM --> VALIDATE[Validate Config]
    VALIDATE --> DETECT{Backend = auto?}
    DETECT -->|Yes| AUTO[Auto-detect Backend]
    DETECT -->|No| LOCK
    AUTO --> LOCK{Acquire Loop Lock}
    
    LOCK -->|Success| PRIMARY[Run as Primary Loop]
    LOCK -->|Already Locked| PARALLEL{Parallel Enabled?}
    PARALLEL -->|Yes| WORKTREE[Create Git Worktree]
    PARALLEL -->|No| ERROR[Error: Loop Running]
    
    WORKTREE --> SYMLINK[Symlink Memories/Specs/Tasks]
    SYMLINK --> REGISTER[Register in Loop Registry]
    REGISTER --> PREFLIGHT
    
    PRIMARY --> PREFLIGHT{Preflight Checks}
    PREFLIGHT -->|Pass| INIT_LOOP[Initialize Event Loop]
    PREFLIGHT -->|Fail| ABORT[Abort with Error]
    
    INIT_LOOP --> BUILD_PROMPT[Build Prompt<br/>Guardrails + Hat Instructions + Memories + Skills]
    BUILD_PROMPT --> EXECUTE[Execute Agent Backend<br/>via PTY/CLI]
    EXECUTE --> PARSE[Parse Events from JSONL]
    PARSE --> PUBLISH[Publish Events to EventBus]
    PUBLISH --> CHECK_TERM{Termination?}
    
    CHECK_TERM -->|LOOP_COMPLETE| COMPLETE[Loop Completed]
    CHECK_TERM -->|Max Iterations| LIMIT[Limit Reached]
    CHECK_TERM -->|Error| FAIL[Handle Failure]
    CHECK_TERM -->|No| ROUTE[Route to Next Hat]
    
    ROUTE --> BUILD_PROMPT
    
    COMPLETE --> LANDING[Landing Handler<br/>Auto-commit, Merge Queue]
    LIMIT --> LANDING
```

## Event Loop Iteration Cycle

Detail of a single iteration within the loop:

```mermaid
sequenceDiagram
    participant Loop as Event Loop
    participant HR as HatlessRalph
    participant HE as Hook Engine
    participant Agent as AI Backend
    participant EB as EventBus
    participant TS as TaskStore
    participant MS as MemoryStore

    Note over Loop: Iteration N begins
    
    Loop->>HE: pre.iteration.start hooks
    HE-->>Loop: OK / Block / Suspend
    
    Loop->>MS: Load memories (if auto-inject)
    Loop->>TS: Load task status
    Loop->>HR: Build prompt (guardrails + hat + memories + skills + tasks)
    HR-->>Loop: Complete prompt string
    
    Loop->>Agent: Execute iteration (PTY/CLI)
    Agent-->>Loop: Agent output + events.jsonl
    
    Loop->>Loop: Parse JSONL events
    Loop->>EB: Publish parsed events
    
    alt human.interact event
        Loop->>Loop: Send question via RObot
        Loop->>Loop: Block waiting for response
    end
    
    EB-->>Loop: Route events to hat queues
    Loop->>Loop: Check termination conditions
    
    alt Completion Promise detected
        Loop->>HE: pre.loop.complete hooks
        Loop->>Loop: Verify required_events seen
        Loop-->>Loop: Terminate (exit 0)
    else Has pending events
        Loop->>Loop: Select next hat
        Note over Loop: Iteration N+1
    else No events, no completion
        Loop->>Loop: Continue with Ralph (fallback)
    end
```

## Parallel Loop Workflow

```mermaid
flowchart LR
    subgraph "Main Workspace"
        P[Primary Loop<br/>holds loop.lock]
        MQ[Merge Queue<br/>merge-queue.jsonl]
        REG[Loop Registry<br/>loops.json]
    end
    
    subgraph ".worktrees/fix-header/"
        W1[Worktree Loop 1]
        SYM1[Symlinks → main<br/>memories, specs, tasks]
    end
    
    subgraph ".worktrees/add-footer/"
        W2[Worktree Loop 2]
        SYM2[Symlinks → main<br/>memories, specs, tasks]
    end
    
    P -.->|spawns| W1
    P -.->|spawns| W2
    W1 -->|completes| MQ
    W2 -->|completes| MQ
    MQ -->|primary processes| P
    W1 --> REG
    W2 --> REG
```

## Telegram Human-in-the-Loop Flow

```mermaid
sequenceDiagram
    participant Agent as AI Agent
    participant Loop as Event Loop
    participant Bot as Telegram Bot
    participant Human as Human (Telegram)

    Note over Agent: Agent encounters ambiguity
    Agent->>Loop: Emit human.interact event
    Loop->>Bot: Send question via Telegram
    Bot->>Human: "❓ Agent asks: ..."
    
    Loop->>Loop: Block (wait for response or timeout)
    
    Human->>Bot: Reply with answer
    Bot->>Loop: Write human.response to events.jsonl
    Loop->>Agent: Inject response into next iteration
    
    Note over Human: Proactive guidance
    Human->>Bot: Send message without question
    Bot->>Loop: Write human.guidance to events.jsonl
    Loop->>Agent: Inject as "## ROBOT GUIDANCE" in prompt
```

## PDD Planning Session

```mermaid
flowchart TD
    START([ralph plan]) --> LOAD[Load Config]
    LOAD --> BACKEND[Resolve Backend]
    BACKEND --> SPAWN[Spawn AI Backend<br/>with PDD SOP prompt]
    SPAWN --> INTERACT[Interactive Session<br/>Clarify requirements]
    INTERACT --> RESEARCH[Research Phase<br/>Explore codebase]
    RESEARCH --> DESIGN[Design Phase<br/>Architecture decisions]
    DESIGN --> PLAN[Implementation Plan<br/>Phased steps]
    PLAN --> OUTPUT[Write plan to .ralph/specs/]
```

## Code Task Generation

```mermaid
flowchart TD
    START([ralph code-task]) --> INPUT{Input Type}
    INPUT -->|Description| DESC[Parse description text]
    INPUT -->|PDD Plan| PLAN[Parse plan file steps]
    DESC --> GEN[Generate .code-task.md]
    PLAN --> GEN
    GEN --> CRITERIA[Given-When-Then<br/>Acceptance Criteria]
    CRITERIA --> OUTPUT[Write to .ralph/tasks/]
```

## Subprocess TUI Mode

The default execution mode spawns a two-process architecture:

```mermaid
flowchart LR
    subgraph "Parent Process"
        TUI[TUI Render Loop<br/>ratatui]
        INPUT[Keyboard Input]
        WRITER[RPC Writer<br/>stdin pipe]
        READER[Event Reader<br/>stdout pipe]
    end
    
    subgraph "Child Process (ralph run --rpc)"
        RPC_IN[RPC stdin reader]
        LOOP[Event Loop]
        RPC_OUT[JSON-lines stdout]
    end
    
    INPUT --> WRITER
    WRITER -->|JSON commands| RPC_IN
    RPC_IN --> LOOP
    LOOP --> RPC_OUT
    RPC_OUT -->|JSON events| READER
    READER --> TUI
```

## Web Dashboard Workflow

```mermaid
flowchart TD
    subgraph "Frontend (React + Vite)"
        UI[Web Dashboard]
        TRPC_C[tRPC Client]
        WS_C[WebSocket Client]
    end
    
    subgraph "Backend (Fastify + tRPC)"
        API[tRPC Router]
        RUNNER[Ralph Runner]
        QUEUE[Task Queue]
        DB[(SQLite)]
    end
    
    subgraph "Ralph Process"
        RALPH[ralph run --rpc]
    end
    
    UI --> TRPC_C
    TRPC_C --> API
    UI --> WS_C
    WS_C --> API
    API --> RUNNER
    RUNNER --> RALPH
    API --> QUEUE
    QUEUE --> DB
```

## Hook Execution Flow

```mermaid
flowchart TD
    EVENT[Lifecycle Event<br/>e.g., pre.iteration.start] --> ENABLED{Hooks Enabled?}
    ENABLED -->|No| SKIP[Skip]
    ENABLED -->|Yes| LOOKUP[Lookup hooks for phase-event]
    LOOKUP --> EXEC[Execute hooks sequentially]
    
    EXEC --> RESULT{Hook Result}
    RESULT -->|Success| NEXT{More hooks?}
    NEXT -->|Yes| EXEC
    NEXT -->|No| CONTINUE[Continue Loop]
    
    RESULT -->|Failure| ON_ERR{on_error}
    ON_ERR -->|warn| LOG[Log Warning] --> NEXT
    ON_ERR -->|block| BLOCK[Block Loop Action]
    ON_ERR -->|suspend| SUSPEND[Suspend Loop<br/>Await Recovery]
    
    SUSPEND --> MODE{suspend_mode}
    MODE -->|wait_for_resume| WAIT[Wait for operator]
    MODE -->|retry_backoff| RETRY[Retry with backoff]
    MODE -->|wait_then_retry| WAIT_RETRY[Wait then retry once]
```

## Session Recording & Replay

```mermaid
flowchart LR
    REC[ralph run --record-session file.jsonl] --> OBSERVER[Session Recorder<br/>EventBus Observer]
    OBSERVER --> JSONL[(file.jsonl)]
    
    JSONL --> REPLAY[Smoke Test Runner]
    REPLAY --> MOCK[Replay Backend<br/>Serves recorded responses]
    MOCK --> TEST[Automated Test<br/>Validates behavior]
```
