# Ralph Hats Commands

## Create or Edit a Hats File

User-authored hat collections belong under `.ralph/hats/`:

```bash
mkdir -p .ralph/hats
$EDITOR .ralph/hats/my-workflow.yml
```

## Validate the Collection

```bash
ralph hats validate -c ralph.yml -H .ralph/hats/my-workflow.yml
```

This catches:

- reserved triggers
- ambiguous routing
- missing starting-event subscribers
- orphan published events

## Inspect the Topology

```bash
ralph hats graph -c ralph.yml -H .ralph/hats/my-workflow.yml --format ascii
ralph hats graph -c ralph.yml -H .ralph/hats/my-workflow.yml --format mermaid
ralph hats show -c ralph.yml -H .ralph/hats/my-workflow.yml planner
```

Use `graph` when the user wants a workflow explanation. Use `show` when one hat
needs closer inspection.

## Exercise the Workflow

```bash
ralph run -c ralph.yml -H .ralph/hats/my-workflow.yml -p "Add OAuth login"
```

For a quick inspection before running:

```bash
ralph run -c ralph.yml -H .ralph/hats/my-workflow.yml -p "Add OAuth login" --dry-run
```

## Improvement Loop

When refactoring an existing hats file:

1. read the current YAML
2. explain the current topology
3. propose the smallest structural improvement that fixes the problem
4. re-run `ralph hats validate`
5. if useful, re-render with `ralph hats graph`
