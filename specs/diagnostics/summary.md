# Project Summary: Diagnostic Logging for Ralph

## Overview

This PDD session transformed the rough idea of "diagnostic logging for Ralph" into a comprehensive design with an actionable implementation plan. The feature will provide complete visibility into Ralph's operation for debugging purposes.

## Artifacts Created

```
specs/diagnostics/
├── rough-idea.md                    # Initial concept
├── idea-honing.md                   # Requirements Q&A (9 questions)
├── research/
│   ├── summary.md                   # Research executive summary
│   ├── existing-infrastructure.md   # Current logging state
│   ├── architecture.md              # TUI vs non-TUI architecture
│   ├── event-system.md              # Event bus and flow
│   └── best-practices.md            # Rust TUI logging best practices
├── design/
│   └── detailed-design.md           # Comprehensive design document
├── implementation/
│   └── plan.md                      # 11-step implementation plan
└── summary.md                       # This document
```

## Key Design Decisions

| Decision | Choice |
|----------|--------|
| **What to capture** | Everything (agent output, events, orchestration, tracing, performance, errors) |
| **File format** | Separate JSONL files by type |
| **Location** | `.ralph/diagnostics/<timestamp>/` |
| **Activation** | Opt-in via `RALPH_DIAGNOSTICS=1` |
| **Agent output detail** | Stripped text + parsed JSON events |
| **CLI for queries** | None (use `jq`/`grep`) |
| **Cleanup** | Manual via `ralph clean --diagnostics` |
| **Crash safety** | Incremental flush (no buffering) |

## Architecture

```
┌─────────────────────────────────────────────┐
│           DiagnosticsCollector              │
│  (enabled via RALPH_DIAGNOSTICS=1)          │
└──────────────────┬──────────────────────────┘
                   │
    ┌──────────────┼──────────────┐
    ▼              ▼              ▼
AgentOutput   Orchestration    Trace
  Logger         Logger        Layer
    │              │              │
    ▼              ▼              ▼
┌─────────────────────────────────────────────┐
│  .ralph/diagnostics/2024-01-15T10-23-45/    │
│  ├── agent-output.jsonl                     │
│  ├── orchestration.jsonl                    │
│  ├── trace.jsonl                            │
│  ├── performance.jsonl                      │
│  └── errors.jsonl                           │
└─────────────────────────────────────────────┘
```

## Implementation Summary

**11 incremental steps:**

1. Create diagnostics module + DiagnosticsCollector
2. Implement AgentOutputLogger
3. Implement DiagnosticStreamHandler wrapper
4. Implement OrchestrationLogger
5. Integrate into event loop
6. Implement TraceLogger (tracing Layer)
7. Integrate trace layer into subscriber
8. Implement PerformanceLogger
9. Implement ErrorLogger
10. Add `ralph clean --diagnostics` command
11. End-to-end integration tests

Each step produces working, testable functionality.

## Next Steps

1. **Review** the detailed design at `specs/diagnostics/design/detailed-design.md`
2. **Check** the implementation plan at `specs/diagnostics/implementation/plan.md`
3. **Begin implementation** following the step-by-step checklist

## Usage After Implementation

```bash
# Enable diagnostics for a run
RALPH_DIAGNOSTICS=1 ralph run -p "your task"

# Query results with jq
jq 'select(.type == "tool_call")' .ralph/diagnostics/*/agent-output.jsonl
jq 'select(.error_type)' .ralph/diagnostics/*/errors.jsonl
jq 'select(.metric == "iteration_duration")' .ralph/diagnostics/*/performance.jsonl

# Clean up
ralph clean --diagnostics
```
