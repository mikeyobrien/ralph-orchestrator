# PDD Summary: Claude Adapter Streaming Output

**Completed:** 2026-01-14

## Overview

This PDD process transformed the rough idea of "streaming output for non-interactive mode" into a detailed design and implementation plan for real-time visibility into Claude's progress during `ralph run`.

## Problem Statement

When running `ralph run -P PROMPT.md`, users see nothing until Claude completes. This is problematic because Ralph can be going in the wrong direction for long periods without user visibility.

## Solution

Enable Claude's `--output-format stream-json` flag and parse NDJSON events in real-time, displaying assistant text and tool invocations as they occur.

## Artifacts Created

```
specs/adapters/claude/
├── rough-idea.md              # Initial concept
├── idea-honing.md             # Requirements Q&A (8 decisions)
├── research/
│   ├── existing-spec.md       # Original claude.spec.md content
│   ├── implementation-gap-analysis.md  # Current vs required
│   └── adapter-streaming-analysis.md   # Extensibility research
├── design/
│   └── detailed-design.md     # Full technical design
├── implementation/
│   └── plan.md                # 9-step implementation plan
└── summary.md                 # This document
```

## Key Decisions

| # | Topic | Decision |
|---|-------|----------|
| 1 | Output | Assistant text + tool invocations (default); everything (verbose) |
| 2 | Format | Plain text streaming |
| 3 | Verbosity | CLI flag with precedence: CLI > env > config |
| 4 | Errors | Inline + stderr separation |
| 5 | Scope | Non-interactive, Claude-only |
| 6 | Summary | Verbose mode only |
| 7 | Malformed JSON | Skip silently, debug log |
| 8 | Enablement | Always-on with `--quiet` opt-out |

## Implementation Overview

### New Components

- `OutputFormat` enum — Adapter output format declaration
- `ClaudeStreamEvent` — Typed JSON event structures
- `ClaudeStreamParser` — NDJSON line parser
- `StreamHandler` trait — Event handling abstraction
- `ConsoleStreamHandler` — Terminal output formatter

### Files to Modify/Create

| File | Action |
|------|--------|
| `cli_backend.rs` | Add OutputFormat, update claude() |
| `claude_stream.rs` | New: event types and parser |
| `stream_handler.rs` | New: handler trait and impls |
| `pty_executor.rs` | Add run_observe_streaming() |
| `main.rs` | Wire verbosity and handler |

### Implementation Steps

1. Add OutputFormat enum to CliBackend
2. Create Claude stream event types
3. Implement JSON line parser
4. Create StreamHandler trait and ConsoleStreamHandler
5. Add streaming support to PTY executor
6. Add verbosity CLI flags to ralph run
7. Wire up streaming in main execution path
8. Add quiet mode support
9. Integration testing and polish

## Next Steps

1. Review the detailed design at `design/detailed-design.md`
2. Follow the implementation plan at `implementation/plan.md`
3. Use the checklist to track progress through each step
4. After implementation, update `specs/adapters/claude.spec.md` status to `implemented`

## Design Highlights

- **Extensible:** `OutputFormat` enum allows other adapters to adopt streaming later
- **Testable:** Trait-based handlers enable unit testing without real CLI
- **Backwards compatible:** Non-streaming adapters unchanged
- **Unix-friendly:** Errors to stderr, supports piping and log separation
