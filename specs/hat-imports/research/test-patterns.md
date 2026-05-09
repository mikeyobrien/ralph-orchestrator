# Research: Existing Test Patterns for Config Loading

## Test Locations

| Type | Location | Count | Purpose |
|------|----------|-------|---------|
| Unit (parse) | `config.rs` inline tests | 40+ | YAML parsing, validation, normalization |
| Integration (CLI) | `crates/ralph-cli/tests/` | 10+ | CLI flag precedence, file loading |
| Smoke (replay) | `crates/ralph-core/tests/fixtures/` | 6+ | JSONL fixture-based scenario replay |

## Unit Test Pattern (config.rs)

```rust
#[test]
fn test_something() {
    let yaml = r#"
hats:
  builder:
    name: "Builder"
    triggers: ["build.task"]
    publishes: ["build.done"]
    instructions: "Build things"
"#;
    let config: RalphConfig = serde_yaml::from_str(yaml).unwrap();
    assert_eq!(config.hats.len(), 1);
}
```

## Integration Test Pattern (CLI tests)

```rust
fn run_ralph(temp_path: &Path, args: &[&str]) -> Command::Output {
    Command::new(env!("CARGO_BIN_EXE_ralph"))
        .args(args)
        .current_dir(temp_path)
        .output()
        .expect("execute ralph")
}

#[test]
fn test_something() {
    let temp_dir = TempDir::new().expect("temp dir");
    fs::write(temp_dir.path().join("ralph.yml"), yaml_content).unwrap();
    let output = run_ralph(temp_dir.path(), &["run", "--dry-run", ...]);
    assert!(output.status.success());
}
```

## Test Strategy for Hat Imports

**Unit tests** (in config.rs or a new test module):
- Parse YAML with `import:` keys at Value level
- Verify import resolution produces correct merged Value
- Test override semantics (local field replaces imported field)
- Test error cases (file not found, invalid YAML, transitive import rejected)

**Integration tests** (in crates/ralph-cli/tests/):
- Write preset + shared hat files to TempDir
- Run `ralph run --dry-run -c preset.yml` or `ralph preflight`
- Verify resolved config has correct hat definitions
- Test error messages for bad imports

## Remote URL Handling (for context)

Currently uses bare `reqwest::get(url)` with:
- No custom client config (no timeouts, no retries)
- HTTP status check (must be 2xx)
- No caching
- Fail-fast on any error

This is relevant for Phase 2 URL imports but not Phase 1.
