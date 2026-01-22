# Technologies — Ralph Memories Implementation

## Available Dependencies (Workspace Cargo.toml)

### Already Available

| Crate | Version | Used For |
|-------|---------|----------|
| `serde` | 1 | Serialization with `derive` feature |
| `serde_json` | 1 | JSON serialization/deserialization |
| `chrono` | 0.4 | Timestamps with `serde` feature |
| `anyhow` | 1 | Error handling with context |
| `tracing` | 0.1 | Logging (warn for parse failures) |
| `clap` | 4 | CLI argument parsing with `derive` feature |
| `tempfile` | 3 | Test fixtures (dev-dependency) |

### Needs to be Added

| Crate | Version | Used For |
|-------|---------|----------|
| `rand` | 0.8 | Random ID generation (`rand::random::<u16>()`) |

**Note**: The design uses `rand::random::<u16>()` for unique ID generation. This crate needs to be added to the workspace dependencies and to `ralph-core/Cargo.toml`.

## Serialization Patterns

### Serde Attributes Used

```rust
#[derive(Serialize, Deserialize)]
pub struct Memory {
    pub id: String,
    pub created: String,  // ISO 8601 via Utc::now().to_rfc3339()

    #[serde(rename = "type")]  // JSON field name differs from Rust field
    pub memory_type: MemoryType,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]  // Optional field
    pub tags: Vec<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]  // Optional field
    pub source: Option<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]  // Pattern, Decision, Fix, Context → pattern, decision, fix, context
pub enum MemoryType {
    Pattern,
    Decision,
    Fix,
    Context,
}
```

### Clap ValueEnum Pattern

```rust
#[derive(Clone, Debug, ValueEnum)]
enum MemoryTypeArg {
    Pattern,
    Decision,
    Fix,
    Context,
}

impl From<MemoryTypeArg> for MemoryType {
    fn from(arg: MemoryTypeArg) -> Self {
        match arg {
            MemoryTypeArg::Pattern => MemoryType::Pattern,
            // ...
        }
    }
}
```

## File I/O Patterns

### Create Parent Directories

```rust
if let Some(parent) = self.path.parent() {
    fs::create_dir_all(parent)?;
}
```

### Append-Only Write

```rust
let mut file = OpenOptions::new()
    .create(true)
    .append(true)
    .open(&self.path)?;
writeln!(file, "{}", json)?;
```

### Buffered Read

```rust
let file = File::open(&self.path)?;
let reader = BufReader::new(file);
for line in reader.lines() {
    // ...
}
```

### Atomic Rewrite (for delete)

```rust
let mut file = File::create(&self.path)?;  // Truncates existing
for memory in memories {
    writeln!(file, "{}", serde_json::to_string(memory)?)?;
}
```

## Error Handling Patterns

### anyhow for CLI Commands

```rust
fn remember_command(color_mode: ColorMode, args: RememberArgs) -> Result<()> {
    // anyhow::Result for automatic error context
    let store = MemoryStore::default_path();
    store.append(&memory)?;  // Propagates io::Error
    Ok(())
}
```

### Graceful Failure for Reads

```rust
match serde_json::from_str(&line) {
    Ok(record) => records.push(record),
    Err(e) => {
        tracing::warn!("Failed to parse memory: {}", e);
        // Skip malformed line, continue processing
    }
}
```

## Timestamp Generation

```rust
use chrono::Utc;

// ISO 8601 timestamp
let created = Utc::now().to_rfc3339();  // "2024-01-15T10:23:45.123456789+00:00"

// Unix timestamp for ID
let timestamp = Utc::now().timestamp();  // 1705314225
```

## ID Generation Strategy

The design uses a hybrid timestamp + random approach:

```rust
let timestamp = Utc::now().timestamp();
let random_hex = format!("{:04x}", rand::random::<u16>());
let id = format!("mem-{}-{}", timestamp, random_hex);
// e.g., "mem-1705314225-a3f8"
```

This provides:
- **Sortability**: Timestamp prefix allows chronological ordering
- **Uniqueness**: Random suffix prevents collisions within same second
- **Readability**: Human-friendly format for debugging
