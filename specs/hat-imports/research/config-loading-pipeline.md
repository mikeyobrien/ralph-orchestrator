# Research: Config Loading Pipeline

## The 5-Stage Pipeline

```
CLI arg (String)
    |
ConfigSource::File(PathBuf) / HatsSource::Builtin(String) / HatsSource::Remote(String)
    |
File content (String)
    |
serde_yaml::Value          <-- BEST INSERTION POINT FOR IMPORTS
    |
RalphConfig with HashMap<String, HatConfig>
    |
HatRegistry with BTreeMap<HatId, Hat>
```

## Key Files

| Component | File | Function | Lines |
|-----------|------|----------|-------|
| Load config | `preflight.rs` | `load_config_for_preflight()` | 196-230 |
| Load core YAML | `preflight.rs` | `load_core_value()` | 256-329 |
| Load hats YAML | `preflight.rs` | `load_hats_value()` | 332-379 |
| Parse YAML text | `preflight.rs` | `parse_yaml_value()` | 381-382 |
| **Merge hats** | `preflight.rs` | `merge_hats_overlay()` | 498-531 |
| Deserialize | `preflight.rs` | (inline in load_config) | 220-221 |
| Normalize | `config.rs` | `RalphConfig::normalize()` | 272-355 |
| Validate | `config.rs` | `RalphConfig::validate()` | 366-494 |
| Create registry | `hat_registry.rs` | `HatRegistry::from_config()` | 24-46 |

## Split Config Architecture

Two separate flags:
- **`-c/--config`**: Core config (backend, workspace, guardrails). Can be file, URL, or override.
- **`-H/--hats`**: Hat collection (hat definitions, events, event_loop overrides). Can be `builtin:name`, file, or URL.

## Three Import Resolution Windows

### Window 1: Raw YAML Text (before parse)
- Location: After `read_to_string()`, before `parse_yaml_value()`
- Type: `String`
- Pros: Full text control
- Cons: YAML structure not yet validated

### Window 2: serde_yaml::Value (RECOMMENDED)
- Location: After YAML parse, before serde deserialization
- Type: `serde_yaml::Value` / `serde_yaml::Mapping`
- Pros: YAML validated; same types as `merge_hats_overlay()`; clean injection point
- Cons: Need to walk the Value tree

### Window 3: Post-Deserialization
- Location: After `serde_yaml::from_value()`
- Type: `RalphConfig` / `HatConfig`
- Cons: Would need custom Deserialize impl or post-processing; serde already validated

## Recommended Approach

Insert import resolution in `preflight.rs` between `load_hats_value()` and `merge_hats_overlay()`:

```rust
let hats_value = load_hats_value(source).await?;
let resolved_value = resolve_imports(hats_value, source_dir)?;  // NEW
let merged = merge_hats_overlay(core_value, resolved_value)?;
```

This works with validated YAML structure, integrates with existing overlay merging, and keeps imports processed before deserialization.

## Current State: No Cross-Preset References

- No YAML anchors/aliases across files
- No `import`, `include`, `extends` keywords in any preset
- Each preset is fully self-contained
- YAML anchors are used WITHIN single files (e.g., shared instruction fragments)
