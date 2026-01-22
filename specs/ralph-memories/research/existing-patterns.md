# Existing Patterns — Ralph Memories Implementation

## CLI Command Patterns

### Command Structure (main.rs:206-231)

Commands are defined as an enum with Parser derive:

```rust
#[derive(Subcommand, Debug)]
enum Commands {
    /// Doc comment becomes help text
    Remember(RememberArgs),
    Recall(RecallArgs),
    Forget(ForgetArgs),
    Memories(MemoriesArgs),
}
```

### Argument Structs Pattern (main.rs:233-436)

Each command has its own args struct with clap attributes:

```rust
#[derive(Parser, Debug)]
struct RememberArgs {
    /// The memory content to store
    #[arg(value_name = "CONTENT")]
    content: String,

    /// Memory type
    #[arg(short = 't', long, default_value = "pattern")]
    #[arg(value_enum)]
    memory_type: MemoryTypeArg,

    /// Comma-separated tags
    #[arg(long)]
    tags: Option<String>,
}
```

### Command Handler Pattern (main.rs:474-484, 832-903)

Commands are dispatched in main() with consistent signature:

```rust
match cli.command {
    Some(Commands::Remember(args)) => remember_command(cli.color, args),
    Some(Commands::Recall(args)) => recall_command(cli.color, args),
    // ...
}
```

Handler functions follow this pattern:

```rust
fn remember_command(color_mode: ColorMode, args: RememberArgs) -> Result<()> {
    let use_colors = color_mode.should_use_colors();
    // ... implementation
    Ok(())
}
```

## Color Output Patterns (main.rs:170-181)

ANSI codes are defined in a `colors` module:

```rust
mod colors {
    pub const RESET: &str = "\x1b[0m";
    pub const BOLD: &str = "\x1b[1m";
    pub const DIM: &str = "\x1b[2m";
    pub const GREEN: &str = "\x1b[32m";
    pub const YELLOW: &str = "\x1b[33m";
    pub const RED: &str = "\x1b[31m";
    pub const CYAN: &str = "\x1b[36m";
    pub const BLUE: &str = "\x1b[34m";
    pub const MAGENTA: &str = "\x1b[35m";
}
```

Usage pattern with conditional coloring:

```rust
if use_colors {
    println!("{}✓{} Memory stored: {}{}{}",
        colors::GREEN, colors::RESET,
        colors::CYAN, id, colors::RESET);
} else {
    println!("Memory stored: {}", id);
}
```

## JSONL Storage Patterns (event_logger.rs)

### File Structure

- Path: `.ralph/events-YYYYMMDD-HHMMSS.jsonl` (timestamped per run)
- Marker: `.ralph/current-events` (coordinates path between Ralph and agents)
- Format: One JSON object per line (JSONL)
- Operations: Append-only for writes, full read for queries

### Storage Implementation (event_logger.rs:128-197)

```rust
pub struct EventLogger {
    path: PathBuf,
    file: Option<File>,
}

impl EventLogger {
    pub const DEFAULT_PATH: &'static str = ".ralph/events.jsonl";

    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into(), file: None }
    }

    fn ensure_open(&mut self) -> std::io::Result<&mut File> {
        if self.file.is_none() {
            if let Some(parent) = self.path.parent() {
                fs::create_dir_all(parent)?;
            }
            let file = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&self.path)?;
            self.file = Some(file);
        }
        Ok(self.file.as_mut().unwrap())
    }

    pub fn log(&mut self, record: &EventRecord) -> std::io::Result<()> {
        let file = self.ensure_open()?;
        let json = serde_json::to_string(record)?;
        writeln!(file, "{}", json)?;
        file.flush()?;
        Ok(())
    }
}
```

### Reader Implementation (event_logger.rs:199-275)

```rust
pub struct EventHistory {
    path: PathBuf,
}

impl EventHistory {
    pub fn read_all(&self) -> std::io::Result<Vec<EventRecord>> {
        if !self.exists() {
            return Ok(Vec::new());
        }

        let file = File::open(&self.path)?;
        let reader = BufReader::new(file);
        let mut records = Vec::new();

        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            match serde_json::from_str(&line) {
                Ok(record) => records.push(record),
                Err(e) => {
                    tracing::warn!("Failed to parse record: {}", e);
                }
            }
        }

        Ok(records)
    }
}
```

### Key Pattern: Graceful Parse Failure

Malformed lines are logged with `tracing::warn!` and skipped, not treated as errors. This ensures partial corruption doesn't block access to valid data.

## OutputFormat Enum Pattern (main.rs:161-168)

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, ValueEnum)]
pub enum OutputFormat {
    #[default]
    Table,
    Json,
}
```

Used in RecallArgs and MemoriesArgs for consistent output format handling.

## Integration Test Patterns (integration_clean.rs)

### Test Structure

```rust
#[test]
fn test_remember_creates_memory() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let temp_path = temp_dir.path();

    // Create any necessary config files
    fs::write(temp_path.join("ralph.yml"), "...")?;

    // Run command
    let output = Command::new(env!("CARGO_BIN_EXE_ralph"))
        .args(["remember", "Test content"])
        .current_dir(temp_path)
        .output()?;

    // Assert success
    assert!(output.status.success());

    // Assert output content
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Memory stored:"));

    // Assert side effects (files created, etc.)
    let memories_path = temp_path.join(".agent/memories/memories.jsonl");
    assert!(memories_path.exists());

    Ok(())
}
```

### Color Testing Pattern

```rust
#[test]
fn test_color_output_never() -> Result<()> {
    // ... setup ...

    let output = Command::new(env!("CARGO_BIN_EXE_ralph"))
        .args(["command", "--color", "never"])
        .output()?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.contains("\x1b["), "Should not contain ANSI codes");

    Ok(())
}

#[test]
fn test_color_output_always() -> Result<()> {
    // ... setup ...

    let output = Command::new(env!("CARGO_BIN_EXE_ralph"))
        .args(["command", "--color", "always"])
        .output()?;

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("\x1b["), "Should contain ANSI codes");

    Ok(())
}
```

## Module Organization (ralph-core/src/lib.rs)

New modules are added to lib.rs with public re-exports:

```rust
mod memory;
mod memory_store;

pub use memory::{Memory, MemoryType};
pub use memory_store::MemoryStore;
```
