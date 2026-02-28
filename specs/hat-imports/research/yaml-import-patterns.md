# Research: YAML Import Patterns Across Tools

## Comparison Matrix

| Dimension | Docker Compose | GitHub Actions | Helm | Azure Pipelines | Ansible | ESLint |
|---|---|---|---|---|---|---|
| **Mechanism** | `extends`, `include`, `-f` merge | `uses` (atomic call) | Dependencies + values overlay | `template:` (include/extends) | `import_*/include_*` | `extends` array |
| **Merge granularity** | Field-level (maps merge, lists append, scalars replace) | None (parameterize only) | Field-level (maps merge, lists replace) | None (parameter slots only) | Full replace default | Property-specific |
| **Override direction** | Extending overrides base | Caller cannot override callee | Parent overrides subchart | Template controls; caller fills params | Higher precedence replaces | Later overrides earlier |
| **Remote refs** | OCI registries | GitHub repo refs only | Helm repos, OCI, `file://` | Cross-repo via resources | No (use ansible-galaxy) | No |
| **Circular prevention** | Error on detection | 10-level depth limit | Implicit DAG | 100 files, 100 levels, 20MB cap | Role dedup | JS runtime |
| **Path resolution** | Relative to file | Repo root | URL/OCI/file:// | Relative to file or repo root | roles/ dir | JS modules |

## Key Trade-offs

### Parameterized Call (GitHub Actions, Azure Pipelines)
Callee defines slots, caller fills them. No merging, no surprises, but no flexibility to tweak internals. Best for security-sensitive contexts.

### Field-Level Merge (Docker Compose, Helm, Ansible)
More flexible but more complex. Need to understand which fields merge vs. replace.

### Array-of-Configs Composition (ESLint)
Flat, explicit, no file-level indirection. Property-specific merge behavior.

## Critical Insight: List/Array Handling

No consensus across tools:
- Docker Compose: **appends** lists
- Helm: **replaces** lists entirely
- Ansible: **replaces** lists
- ESLint: **replaces** lists

**Recommendation for Ralph: Field-level REPLACEMENT (not merge).** This matches the issue proposal and is the simplest, most predictable semantic. If you specify `publishes:` alongside `import:`, it fully replaces the imported `publishes` list.

## Most Relevant Pattern: Docker Compose `extends`

The closest analogue to Ralph hat imports:
- Single-entity import (one service) with field-level override
- Relative path resolution from the importing file
- Base fields inherited unless overridden
- Clear override semantics (scalars replace, maps merge, lists append)

Ralph's proposed approach is simpler: **all fields replace** (no deep merge). This is the safest choice.
