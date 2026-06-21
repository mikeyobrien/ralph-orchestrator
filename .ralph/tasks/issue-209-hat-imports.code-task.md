---
status: completed
created: 2026-06-21
started: 2026-06-21
completed: 2026-06-21
---
# Task: Implement Phase 1 Hat Imports

## Description
Implement GitHub issue #209 Phase 1 hat imports from `specs/hat-imports/design.md`.

## Scope
- Resolve local file imports for hats at the `serde_yaml::Value` level.
- Resolve imports per file-based source before overlay merge so relative paths use the importing file directory.
- Use imported hat fields as the base and let local hat fields replace them at field level.
- Reject transitive imports, imported `events:`, non-string imports, missing files, invalid YAML, and imports from unsupported builtin/remote sources.
- Keep `HatConfig` unchanged.

## Verification
- Focused `ralph-cli` preflight unit/integration tests.
- `cargo test -p ralph-cli` if feasible.
