# Creating Custom Hats

!!! note "Documentation In Progress"
    This page is under development. Check back soon for comprehensive custom hat documentation.

## Overview

Custom hats allow you to extend Hats's orchestration capabilities by defining specialized behavioral modes for AI agents.

## Quick Start

```yaml
hats:
  my-custom-hat:
    emoji: "🎯"
    system_prompt: "You are a specialized agent for..."
    triggers:
      - pattern: "custom-trigger"
```

## See Also

- [Hats & Events](../concepts/hats-and-events.md) - Core concepts
- [Presets](../guide/presets.md) - Using built-in hat collections
- [Architecture](architecture.md) - System design overview
