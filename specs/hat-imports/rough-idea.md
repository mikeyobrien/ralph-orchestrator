# Hat Imports — Reusable Hat Definitions via Local/URL Import

Source: https://github.com/mikeyobrien/ralph-orchestrator/issues/209

## Use Case

As preset/hat-collection authors grow their configurations, they end up duplicating entire hat definitions across presets. A "Builder" hat, for example, might appear verbatim in three different workflow presets. When instructions or triggers change, every copy must be updated manually — this is error-prone and scales poorly.

Hat imports would let authors define a hat once in a standalone file and reference it from any preset, with optional field-level overrides.

## Proposed Solution

### Syntax

```yaml
# In a preset or hat collection file:
hats:
  builder:
    import: ./shared-hats/builder.yml       # local file (relative to this config)
    publishes: ["build.done", "security.scan"]  # overrides imported publishes
    max_activations: 3                          # overrides imported value
    # all other fields come from the imported file

  reviewer:
    import: https://example.com/hats/reviewer.yml  # URL import
    # no overrides — use as-is
```

### Imported hat file format (single hat per file)

```yaml
# shared-hats/builder.yml
name: "Builder"
description: "TDD builder — one task, one commit"
triggers: ["build.task"]
publishes: ["build.done", "build.blocked"]
default_publishes: "build.done"
instructions: |
  ## BUILDER MODE
  ...

# Optional: event metadata that gets merged into consuming preset
events:
  build.done:
    description: "Building completed successfully"
  build.blocked:
    description: "Building encountered a blocker"
```

### Override semantics

- **Field-level replacement** (not merge): if you specify `publishes:` alongside `import:`, it fully replaces the imported `publishes` list
- Fields not specified locally are inherited from the imported file
- The `import:` key itself is consumed during resolution and never reaches `HatConfig` deserialization

### Event metadata

- Imported hat files can include an `events:` section
- Imported events are merged into the preset's top-level `events:`
- Preset's own `events:` entries take priority over imported ones (override semantics)

## Alternatives Considered

- **YAML anchors / aliases**: only work within a single file, so they don't solve cross-preset reuse.
- **Copy-paste with conventions**: current approach; doesn't scale.
- **Preprocessing with external tools** (e.g., ytt, jsonnet): adds toolchain complexity and breaks the "just edit YAML" simplicity.

## Additional Context

This is a foundational building block for a hat ecosystem — community-shared hats, curated hat libraries, and organisation-internal hat registries all depend on a clean import mechanism.
