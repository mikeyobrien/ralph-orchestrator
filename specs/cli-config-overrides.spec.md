---
status: implemented
created: 2026-01-24
updated: 2026-01-24
related:
  - https://github.com/mikeyobrien/ralph-orchestrator/issues/98
---

# CLI Config Overrides for Core Fields

## Goal

Enable users to override `core.*` configuration fields directly from the command line, allowing flexible organization of scratchpads and specs directories without multiple config files.

**Use case:** Organize Ralph workflows hierarchically:
```
.ralph/feature-one/scratchpad.md
.ralph/other-feature/scratchpad.md
```

## Configuration

Extend the `-c` flag to accept both file paths and `key=value` overrides:

```bash
ralph run -c ralph.yml -c core.scratchpad=".ralph/feature/scratchpad.md"
ralph run -c core.specs_dir="./my-specs/"
```

### Supported Fields

| Field | Description | Default |
|-------|-------------|---------|
| `core.scratchpad` | Path to scratchpad file | `.agent/scratchpad.md` |
| `core.specs_dir` | Path to specs directory | `./specs/` |

### Precedence

1. Default values (lowest)
2. Config file values
3. CLI overrides (highest)

## Schema Changes

### CLI Argument

**File:** `crates/ralph-cli/src/main.rs`

Change from single path to multiple values:

```rust
// Before
#[arg(short, long, default_value = "ralph.yml", global = true)]
config: PathBuf,

// After
#[arg(short, long, default_value = "ralph.yml", global = true, action = ArgAction::Append)]
config: Vec<String>,
```

### ConfigSource Enum

**File:** `crates/ralph-cli/src/main.rs`

Add `Override` variant:

```rust
#[derive(Debug, Clone)]
pub enum ConfigSource {
    File(PathBuf),
    Builtin(String),
    Remote(String),
    Override { key: String, value: String },  // NEW
}

impl ConfigSource {
    fn parse(s: &str) -> Self {
        // Check for core.* override pattern first (prevents false positives on paths with '=')
        // Only treat as override if it starts with "core." AND contains '='
        if s.starts_with("core.") {
            if let Some((key, value)) = s.split_once('=') {
                return ConfigSource::Override {
                    key: key.to_string(),
                    value: value.to_string(),
                };
            }
        }
        // Existing logic unchanged
        if let Some(name) = s.strip_prefix("builtin:") {
            ConfigSource::Builtin(name.to_string())
        } else if s.starts_with("http://") || s.starts_with("https://") {
            ConfigSource::Remote(s.to_string())
        } else {
            ConfigSource::File(PathBuf::from(s))
        }
    }
}
```

## Runtime Behavior

### Override Application

**File:** `crates/ralph-cli/src/main.rs` (adjacent to `ConfigSource` enum)

After config is loaded and normalized, apply overrides:

```rust
const KNOWN_CORE_FIELDS: &[&str] = &["scratchpad", "specs_dir"];

fn apply_config_overrides(
    config: &mut RalphConfig,
    overrides: &[ConfigSource],
) -> anyhow::Result<()> {
    for source in overrides {
        if let ConfigSource::Override { key, value } = source {
            match key.as_str() {
                "core.scratchpad" => {
                    config.core.scratchpad = value.clone();
                }
                "core.specs_dir" => {
                    config.core.specs_dir = value.clone();
                }
                other => {
                    // Note: with core.* prefix requirement in parse(), this branch
                    // only handles unknown core.* fields
                    let field = other.strip_prefix("core.").unwrap_or(other);
                    warn!(
                        "Unknown core field '{}'. Known fields: {}",
                        field,
                        KNOWN_CORE_FIELDS.join(", ")
                    );
                }
            }
        }
    }
    Ok(())
}
```

### Directory Auto-Creation

When scratchpad path specifies non-existent parent directories, create them automatically:

```rust
fn ensure_scratchpad_directory(config: &RalphConfig) -> anyhow::Result<()> {
    let scratchpad_path = config.core.resolve_path(&config.core.scratchpad);
    if let Some(parent) = scratchpad_path.parent()
        && !parent.exists()
    {
        info!("Creating scratchpad directory: {}", parent.display());
        std::fs::create_dir_all(parent)?;
    }
    Ok(())
}
```

### Updated run_command Flow

1. Parse all `-c` arguments into `Vec<ConfigSource>`
2. Partition into files/builtins/remotes and overrides
3. Load config:
   - If file/builtin/remote sources exist: load from first source
   - If only overrides: load `ralph.yml` as base (or defaults if not found)
4. Call `config.normalize()` (v1 → v2 migration)
5. Apply existing CLI overrides (`--backend`, `--max-iterations`, etc.)
6. Apply `-c core.*=value` overrides (highest precedence)
7. Validate config
8. Auto-create scratchpad directory if needed
9. Continue with orchestration

### Override-Only Behavior

When only overrides are specified (e.g., `ralph run -c core.scratchpad=path`):
- Load `ralph.yml` as the base config (if it exists)
- Fall back to `RalphConfig::default()` if `ralph.yml` not found
- Apply the overrides

This enables minimal invocations like:
```bash
ralph run -c core.scratchpad=".custom/scratch.md" -p "do work"
```

## Error Handling

| Scenario | Behavior |
|----------|----------|
| Unknown `core.*` field | Warn, continue |
| Non-`core.*` with `=` | Treated as file path (not override) |
| Invalid path characters | OS-level error on directory creation |
| Permission denied (mkdir) | Error with clear message |
| Multiple file configs | Use first file, warn about others |

## Acceptance Criteria

1. **Backward Compatibility**
   - `ralph run -c ralph.yml` works unchanged
   - `ralph run` uses default `ralph.yml`
   - `ralph run -c builtin:tdd` works unchanged

2. **Override Parsing**
   - `ralph run -c core.scratchpad=path` sets scratchpad
   - `ralph run -c ralph.yml -c core.scratchpad=path` loads file then overrides

3. **Precedence**
   - CLI override beats config file value
   - Given config file with `scratchpad: foo` and `-c core.scratchpad=bar`
   - Then scratchpad is `bar`

4. **Directory Creation**
   - Given `-c core.scratchpad=".ralph/new/scratchpad.md"`
   - When `.ralph/new/` doesn't exist
   - Then directory is created automatically

5. **Unknown Field Warning**
   - Given `-c core.unknown=value`
   - Then warning is logged
   - And execution continues

6. **Non-Core Override Treated as File**
   - Given `-c event_loop.max_iterations=5`
   - Then it's treated as a file path (not an override)
   - And file-not-found error occurs (expected behavior)

7. **Override-Only Mode**
   - Given `ralph run -c core.scratchpad=".custom/scratch.md" -p "prompt"`
   - Then `ralph.yml` is loaded as base config
   - And scratchpad override is applied
   - And execution proceeds normally

## Testing Strategy

### Unit Tests

1. **ConfigSource::parse**
   - `"ralph.yml"` → `File`
   - `"core.scratchpad=path"` → `Override { key: "core.scratchpad", value: "path" }`
   - `"core.specs_dir=./specs"` → `Override { key: "core.specs_dir", value: "./specs" }`
   - `"builtin:default"` → `Builtin`
   - `"https://..."` → `Remote`
   - `"path/with=equals.yml"` → `File` (not override - no `core.` prefix)
   - `"core.field"` (no `=`) → `File` (treated as path, will fail to load)

2. **apply_config_overrides**
   - Known field updates config
   - Unknown core field logs warning, doesn't error
   - Multiple overrides applied in order

3. **ensure_scratchpad_directory**
   - Creates missing parent directories
   - Rejects paths deeper than 5 levels
   - No-op for existing directories

### Integration Tests

1. **Dry-run verification**
   ```bash
   ralph run -c ralph.yml -c core.scratchpad=".ralph/test/scratchpad.md" --dry-run
   ```
   Verify scratchpad path in output.

2. **Directory creation**
   - Run with non-existent directory path
   - Verify directory created

3. **Multiple config files**
   ```bash
   ralph run -c a.yml -c b.yml
   ```
   - First file (a.yml) is used
   - Warning logged about b.yml being ignored

4. **Override before file**
   ```bash
   ralph run -c core.scratchpad=x -c ralph.yml
   ```
   - Config loaded from ralph.yml
   - Override still applied (order doesn't matter for override precedence)

5. **Override-only (no explicit config file)**
   ```bash
   ralph run -c core.scratchpad=".custom/scratch.md" -p "test prompt"
   ```
   - `ralph.yml` loaded as base
   - Override applied
   - Execution proceeds

## Alternatives Considered

1. **Dedicated `--scratchpad` flag** - Rejected: requires new flag per field, less extensible
2. **Full variable substitution (`{feature}` syntax)** - Deferred: over-engineered for current need
3. **Strict validation (error on unknown)** - Rejected: breaks forward compatibility
4. **Generic `key=value` override syntax** - Rejected: parsing ambiguity with file paths containing `=`
5. **Separate `--override` flag** - Rejected: `-c` reuse is more ergonomic and consistent
