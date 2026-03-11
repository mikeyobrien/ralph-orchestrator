# Architecture

## System Overview

Ralph Orchestrator is a hat-based orchestration framework that runs AI agents in iterative loops until task completion. The architecture follows a layered design with clear separation between protocol definitions, core orchestration logic, backend adapters, and presentation layers.

```mermaid
graph TB
    subgraph CLI["ralph-cli (binary)"]
        Main[main.rs / clap]
        RunCmd[run command]
        PlanCmd[plan command]
        WebCmd[web command]
        McpCmd[mcp command]
        TaskCmd[task command]
        LoopsCmd[loops command]
    end

    subgraph Core["ralph-core (orchestration engine)"]
        EventLoop[Event Loop]
        HatlessRalph[HatlessRalph Coordinator]
        HatRegistry[Hat Registry]
        Config[Config Loader]
        InstructionBuilder[Instruction Builder]
        SkillRegistry[Skill Registry]
        Hooks[Hook Engine]
        MemoryStore[Memory Store]
        TaskStore[Task Store]
        Worktree[Worktree Manager]
        MergeQueue[Merge Queue]
        LoopRegistry[Loop Registry]
        PlanningSession[Planning Session]
    end

    subgraph Proto["ralph-proto (shared types)"]
        Event[Event]
        EventBus[EventBus]
        Hat[Hat / HatId]
        Topic[Topic]
        JsonRpc[JSON-RPC Protocol]
        UxEvent[UX Events]
        Robot[RobotService trait]
    end

    subgraph Adapters["ralph-adapters"]
        CliExecutor[CLI Executor]
        PtyExecutor[PTY Executor]
        AcpExecutor[ACP Executor]
        AutoDetect[Auto-Detect]
        StreamHandlers[Stream Handlers]
    end

    subgraph API["ralph-api (RPC + MCP)"]
        RpcRuntime[RPC Runtime]
        Transport[Axum Transport]
        McpServer[MCP Server]
        Domains[Domain Modules]
    end

    subgraph TUI["ralph-tui"]
        App[TUI App]
        RpcBridge[RPC Bridge]
        Widgets[Widgets]
    end

    subgraph Telegram["ralph-telegram"]
        Bot[Telegram Bot]
        TgHandler[Message Handler]
        TgService[Robot Service]
    end

    subgraph Web["Web Layer"]
        Frontend[React Dashboard]
        LegacyBackend[Node tRPC Server]
    end

    CLI --> Core
    CLI --> Adapters
    CLI --> TUI
    CLI --> API
    CLI --> Telegram
    Core --> Proto
    Adapters --> Proto
    Adapters --> Core
    API --> Core
    TUI --> Core
    TUI --> Proto
    Telegram --> Proto
    Frontend -.->|WebSocket/HTTP| API
    Frontend -.->|legacy tRPC| LegacyBackend
```

## Core Orchestration Flow

The event loop is the heart of Ralph. Each iteration: select a hat → build prompt → execute agent → parse events → route via EventBus → repeat.

```mermaid
sequenceDiagram
    participant CLI as ralph run
    participant EL as EventLoop
    participant HR as HatlessRalph
    participant HReg as HatRegistry
    participant EB as EventBus
    participant Adapter as CLI/PTY Executor
    participant Agent as AI Agent (Claude, etc.)

    CLI->>EL: start(config, prompt)
    EL->>HReg: register hats from config
    EL->>EB: register hats
    EL->>EL: publish starting_event

    loop Each Iteration
        EL->>EB: drain pending events
        EL->>HReg: select hat for event
        alt Hat found
            EL->>HR: build_prompt(hat, event, context)
            HR-->>EL: prompt with instructions + memories + skills
        else No hat (orphan)
            EL->>HR: build_prompt(ralph, event, context)
        end
        EL->>Adapter: execute(prompt)
        Adapter->>Agent: spawn CLI process
        Agent-->>Adapter: output stream
        Adapter-->>EL: parsed output
        EL->>EL: parse events from JSONL
        EL->>EB: publish parsed events
        EL->>EL: check termination conditions
    end

    EL-->>CLI: TerminationReason + exit code
```

## Hat System

Hats are specialized personas that coordinate through pub/sub events. Each hat subscribes to specific topics and publishes others, creating a directed workflow.

```mermaid
graph LR
    Start((work.start)) --> Planner
    Planner["📋 Planner"] -->|subtask.ready| Builder
    Builder["⚡ Builder"] -->|subtask.done| Planner
    Planner -->|all_steps.done| Reviewer
    Builder -->|implementation.done| Reviewer
    Reviewer["👀 Reviewer"] -->|review.approved| Finalizer
    Reviewer -->|review.changes_requested| Builder
    Finalizer["📝 Finalizer"] -->|LOOP_COMPLETE| End((Done))
```

## Event System

Events flow through the `EventBus` which routes by topic pattern matching. Each event has a topic, payload, optional source hat, and optional target hat.

```mermaid
classDiagram
    class Event {
        +Topic topic
        +String payload
        +Option~HatId~ source
        +Option~HatId~ target
    }

    class EventBus {
        -BTreeMap~HatId, Hat~ hats
        -BTreeMap~HatId, Vec~Event~~ pending
        -Vec~Event~ human_pending
        -Vec~Observer~ observers
        +register(Hat)
        +publish(Event) Vec~HatId~
        +drain(HatId) Vec~Event~
    }

    class Hat {
        +HatId id
        +String name
        +String description
        +Vec~Topic~ subscriptions
        +Vec~Topic~ publishes
        +String instructions
    }

    class Topic {
        +String pattern
        +matches(other) bool
    }

    EventBus --> "*" Hat
    EventBus --> "*" Event
    Hat --> "*" Topic
    Event --> Topic
```

## Execution Modes

Ralph supports multiple execution strategies for agent backends:

| Mode | Module | Description |
|------|--------|-------------|
| CLI | `cli_executor.rs` | Spawns agent CLI as subprocess, captures stdout |
| PTY | `pty_executor.rs` | Pseudo-terminal for rich TUI output (colors, spinners) |
| ACP | `acp_executor.rs` | Agent Communication Protocol for structured I/O |
| Stream | `stream_handler.rs` | Handles streaming output (Claude, Pi parsers) |

## Parallel Loops Architecture

Multiple loops run concurrently via git worktrees. The primary loop holds `.ralph/loop.lock` and processes the merge queue.

```mermaid
graph TB
    subgraph Primary["Primary Loop (main workspace)"]
        PL[Event Loop]
        Lock[loop.lock]
        MQ[Merge Queue]
    end

    subgraph WT1["Worktree Loop A (.worktrees/loop-a/)"]
        WL1[Event Loop]
        Sym1[Symlinked: memories, specs, tasks]
    end

    subgraph WT2["Worktree Loop B (.worktrees/loop-b/)"]
        WL2[Event Loop]
        Sym2[Symlinked: memories, specs, tasks]
    end

    WL1 -->|queue merge| MQ
    WL2 -->|queue merge| MQ
    PL -->|process queue| MQ
    PL --> Lock
```

## Hook Lifecycle

Hooks execute at defined phase-events during the orchestration lifecycle (e.g., `iteration.before`, `iteration.after`, `loop.start`, `loop.end`). The `HookEngine` resolves hooks per phase-event, builds JSON payloads, and the `HookExecutor` runs them.

## API Architecture

The `ralph-api` crate provides a Rust-native RPC API and MCP server:

```mermaid
graph TB
    subgraph Clients
        WebUI[Web Dashboard]
        TUI[TUI via RPC Bridge]
        McpClient[MCP Client]
    end

    subgraph API["ralph-api"]
        Transport[Axum HTTP/WS Transport]
        McpStdio[MCP stdio Server]
        Runtime[RPC Runtime]
        Auth[Authenticator]
        Idempotency[Idempotency Store]
    end

    subgraph Domains
        TaskDomain[task.*]
        LoopDomain[loop.*]
        PlanningDomain[planning.*]
        ConfigDomain[config.*]
        CollectionDomain[collection.*]
        PresetDomain[preset.*]
        StreamDomain[stream.*]
    end

    WebUI -->|HTTP/WS| Transport
    TUI -->|WS| Transport
    McpClient -->|stdio| McpStdio
    Transport --> Runtime
    McpStdio --> Runtime
    Runtime --> Auth
    Runtime --> Idempotency
    Runtime --> Domains
```
