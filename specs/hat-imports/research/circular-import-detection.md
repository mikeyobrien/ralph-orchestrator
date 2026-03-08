# Research: Circular Import Detection

## The Problem

If `a.yml` imports `b.yml` and `b.yml` imports `a.yml`, naive resolution loops forever.

## How Other Tools Handle It

| Tool | Strategy | Limit |
|------|----------|-------|
| Docker Compose | Error on detection | Explicit circular check |
| GitHub Actions | Depth limit | 10 levels max |
| Azure Pipelines | Multiple limits | 100 files, 100 nesting, 20MB |
| Ansible | Role deduplication | Roles processed once per play |
| Helm | DAG resolution | Implicit via dependency graph |

## Recommended Approach for Ralph

### Strategy: Import Stack with Depth Limit

Maintain a stack of file paths during resolution. Before processing each import:

1. **Canonicalize the path** (resolve symlinks, normalize `../`)
2. **Check if path is already on the stack** → if yes, error with full cycle trace
3. **Check depth** → if > 5 levels, error (even without cycles)
4. **Push path onto stack, resolve imports, pop**

```rust
fn resolve_imports(
    value: serde_yaml::Value,
    base_dir: &Path,
    import_stack: &mut Vec<PathBuf>,  // tracks the chain
) -> Result<serde_yaml::Value> {
    for (hat_id, hat_value) in hats_mapping {
        if let Some(import_path) = hat_value.get("import") {
            let canonical = base_dir.join(import_path).canonicalize()?;

            // Circular check
            if import_stack.contains(&canonical) {
                let cycle = import_stack.iter()
                    .map(|p| p.display().to_string())
                    .collect::<Vec<_>>()
                    .join(" -> ");
                bail!("Circular import detected: {} -> {}", cycle, canonical.display());
            }

            // Depth check
            if import_stack.len() >= 5 {
                bail!("Import depth limit exceeded (max 5). Chain: ...");
            }

            import_stack.push(canonical.clone());
            let imported = load_and_resolve(canonical, import_stack)?;
            import_stack.pop();

            // Merge imported fields with local overrides
            merge_hat(hat_value, imported);
        }
    }
}
```

### Why Depth 5 Is Sufficient

Hat imports are for reuse, not for building deep abstraction hierarchies. A realistic maximum chain:
- `my-workflow.yml` → imports `shared-hats/builder.yml` (depth 1)
- `shared-hats/builder.yml` → imports `base-hats/tdd-base.yml` (depth 2)

Anything deeper than 3-4 levels suggests over-engineering. A limit of 5 provides headroom while catching runaway chains.

### Error Messages

Good error messages are critical:

```
error: circular hat import detected
  my-workflow.yml
    -> shared-hats/builder.yml
    -> base-hats/common.yml
    -> shared-hats/builder.yml  <-- cycle
```

```
error: hat import depth limit exceeded (max 5)
  a.yml -> b.yml -> c.yml -> d.yml -> e.yml -> f.yml
  hint: consider flattening your import hierarchy
```

## Phase 1 Simplification

If we restrict Phase 1 to single-level imports only (imported files cannot themselves contain `import:`), then:
- No circular imports are possible
- No depth tracking needed
- Dramatically simpler implementation
- Still solves the core use case (shared hat definitions)

**Recommendation: Phase 1 = no transitive imports. Phase 2 = transitive imports with stack-based cycle detection.**
