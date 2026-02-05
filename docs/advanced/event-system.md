# Event System Design

!!! note "Documentation In Progress"
    This page is under development. Check back soon for comprehensive event system documentation.

## Overview

Hats's event system provides the communication backbone for hat orchestration, enabling agents to emit signals that trigger hat switches and backpressure mechanisms.

## Event Types

| Event | Description |
|-------|-------------|
| `plan:complete` | Planning phase finished |
| `code:complete` | Implementation phase finished |
| `test:pass` | Tests passed |
| `test:fail` | Tests failed |
| `LOOP_COMPLETE` | Task fully complete |

## Emitting Events

```bash
hats emit plan:complete
hats emit test:pass
```

## See Also

- [Hats & Events](../concepts/hats-and-events.md) - Core concepts
- [Backpressure](../concepts/backpressure.md) - Backpressure mechanisms
- [Creating Custom Hats](custom-hats.md) - Custom hat development
