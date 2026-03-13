# Ralph Loop Commands

## Start a Run

```bash
ralph run -c ralph.yml -H .ralph/hats/my-workflow.yml -p "Add OAuth login"
```

Use `--dry-run` for a quick preflight:

```bash
ralph run -c ralph.yml -H .ralph/hats/my-workflow.yml -p "Add OAuth login" --dry-run
```

## Inspect Loops

```bash
ralph loops list
ralph loops list --json
ralph loops logs <id>
ralph loops logs <id> -f
ralph loops history <id>
ralph loops history <id> --json
ralph loops diff <id>
ralph loops diff <id> --stat
ralph loops attach <id>
```

Use `list --json` and `history --json` when the caller wants structured output.

## Merge Queue Operations

```bash
ralph loops merge <id>
ralph loops process
ralph loops retry <id>
ralph loops discard <id> -y
ralph loops merge-button-state <id>
```

Recommended flow for queued or `needs-review` work:

1. `ralph loops diff <id> --stat`
2. `ralph loops history <id>`
3. `ralph loops merge <id>` or `ralph loops retry <id>`
4. `ralph loops discard <id> -y` if the work should be abandoned

## Stop or Resume

```bash
ralph loops stop <id>
ralph loops stop <id> --force
ralph loops resume <id>
ralph loops prune
```

Use `resume` only when the loop is actually suspended. The command is idempotent
and writes the operator signal Ralph consumes at the suspension boundary.
