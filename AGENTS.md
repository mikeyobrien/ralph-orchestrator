# AGENTS.md

> The orchestrator is a thin coordination layer, not a platform. Agents are smart; let them do the work.

## The Ralph Tenets

1. **Fresh Context Is Reliability** 
2. **Validation Over Prescription** 
3. **The Plan Is Disposable**
4. **Disk Is State, Git Is Memory** 
5. **Steer With Signals, Not Scripts** 

### Specifications

- Create specs in `specs/` — do NOT implement without an approved spec first

### Code Tasks

- Create code tasks in `tasks/` using `.code-task.md` extension
  - Tasks are self-contained implementation units with acceptance criteria

### Tools

- **Memories**: `ralph tools memory --help`
- **Tasks**: `ralph tools task --help`

## Build & Test

```bash
cargo build
cargo test
```

You the `playwright-mcp` tools to manually validate ralph-web functionality.

When running `ralph`, you should always use `RALPH_DIAGNOSTICS=1` to debug.

## IMPORTANT

- Run `cargo test` before declaring any task done (includes replay smoke tests)
- Backwards compatibility doesn't matter — it adds clutter for no reason
- Prefer replay-based smoke tests over live API calls for CI
- Run python tests, using a .venv
