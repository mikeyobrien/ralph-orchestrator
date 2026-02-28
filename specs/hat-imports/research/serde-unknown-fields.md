# Research: HatConfig Serde Behavior with Unknown Fields

## Critical Finding

Neither `HatConfig` nor `RalphConfig` uses `#[serde(deny_unknown_fields)]`. Unknown fields are **silently ignored** during deserialization.

## Implications for Import Resolution

1. **We MUST process `import:` at the serde_yaml::Value level** — serde will silently drop the `import:` key during deserialization, so it won't be available after conversion to `HatConfig`.

2. **Processing after deserialization won't work** — `HatConfig` has no `import` field, and adding one would be a leaky abstraction.

3. **No custom Deserialize impls exist** — code relies on `#[derive(Deserialize)]`, giving us full control over the Value-level transformation.

4. **The existing `test_unknown_fields_ignored()` test confirms this behavior** — forward compatibility is intentional.

## Recommended Approach

Process imports at the `serde_yaml::Value` stage:

```rust
fn resolve_hat_imports(hats_mapping: &mut Mapping, base_dir: &Path) -> Result<()> {
    for (hat_id, hat_value) in hats_mapping.iter_mut() {
        if let Some(import_path) = hat_value.get("import") {
            // 1. Load imported file
            // 2. Merge imported fields (imported = base, local = override)
            // 3. Remove the "import" key
        }
    }
    Ok(())
}
```

The `import:` key is consumed during resolution and never reaches `HatConfig` deserialization.
