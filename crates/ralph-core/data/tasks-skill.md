---
name: tasks
description: Runtime work tracking via CLI for managing tasks across iterations
---

# Ralph Tasks

Runtime work tracking via CLI (preferred over scratchpad markers):

```bash
ralph tools task add 'Title' -p 2           # Create (priority 1-5, 1=highest)
ralph tools task add 'X' --blocked-by Y     # With dependency
ralph tools task ready                       # Unblocked tasks only
ralph tools task close <id>                  # Mark complete (ONLY after verification)
```

**CRITICAL:** Only close tasks after verification (tests pass, build succeeds).
