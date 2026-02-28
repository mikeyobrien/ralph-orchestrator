# Research: merge_hats_overlay() Mechanics

## Function Behavior

`merge_hats_overlay(core: Value, hats: Value) -> Result<Value>` in `preflight.rs:498-540`

Three keys are handled with **different merge strategies**:

| Key | Strategy | Behavior |
|-----|----------|----------|
| `hats` | **Complete replacement** | `-H` hats entirely replace `-c` hats |
| `events` | **Complete replacement** | `-H` events entirely replace `-c` events |
| `event_loop` | **Field-level deep merge** | Individual fields from `-H` override `-c`; non-conflicting fields preserved |

## Important: Hats Are NOT Additively Merged

If `-c` defines `builder` and `-H` defines `reviewer`, the result only has `reviewer`. The hats overlay is a complete specification of which hats to use.

## Helper Functions

- `mapping_get(mapping, key)` — safe YAML mapping lookup
- `mapping_insert(mapping, key, value)` — safe YAML mapping insert
- `normalize_hats_source_value()` — validates only allowed top-level keys exist
- `extract_hat_overlay_from_preset()` — extracts hat keys from a full preset

## Import Resolution Integration Point

Import resolution slots in **before** `merge_hats_overlay()`:

```
load_core_value()     → core Value (may have hats with import: keys)
load_hats_value()     → hats Value (may have hats with import: keys)
                          ↓
resolve_imports(core)  ← NEW: resolve import: keys in core's hats
resolve_imports(hats)  ← NEW: resolve import: keys in hats' hats
                          ↓
merge_hats_overlay()   → merged Value (all imports already resolved)
                          ↓
serde_yaml::from_value() → RalphConfig
```

Each source resolves its own imports relative to its own file path before merging.
