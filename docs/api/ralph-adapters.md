# hats-adapters

CLI backend integrations for various AI tools.

## Overview

`hats-adapters` provides:

- Backend definitions for Claude, Kiro, Gemini, and more
- PTY-based execution for real-time output
- Stream handlers for different output modes
- Auto-detection of available backends

## Supported Backends

| Backend | CLI | Status |
|---------|-----|--------|
| Claude Code | `claude` | Full support |
| Kiro | `kiro` | Full support |
| Gemini CLI | `gemini` | Full support |
| Codex | `codex` | Full support |
| Amp | `amp` | Full support |
| Copilot CLI | `copilot` | Full support |
| OpenCode | `opencode` | Full support |

## Key Components

### CliBackend

Backend definition.

```rust
pub struct CliBackend {
    pub name: String,
    pub command: String,
    pub prompt_mode: PromptMode,
    pub output_format: OutputFormat,
}

pub enum PromptMode {
    Arg,    // cli -p "prompt"
    Stdin,  // echo "prompt" | cli
}

pub enum OutputFormat {
    Text,
    Ndjson,
    Custom(Box<dyn Parser>),
}
```

**Built-in backends:**

```rust
use hats_adapters::backends;

let claude = backends::claude();
let kiro = backends::kiro();
let gemini = backends::gemini();
```

### Auto-Detection

Detect available backends.

```rust
use hats_adapters::auto_detect;

// Get first available backend
let backend = auto_detect::detect()?;

// Get all available backends
let backends = auto_detect::detect_all();

// Check specific backend
let available = auto_detect::is_available("claude");
```

**Detection order:**

1. Claude
2. Kiro
3. Gemini
4. Codex
5. Amp
6. Copilot
7. OpenCode

### PtyExecutor

PTY-based execution for real-time output.

```rust
use hats_adapters::pty_executor::PtyExecutor;

let executor = PtyExecutor::new();

// Execute with stream handler
let result = executor.execute(
    &backend,
    &prompt,
    Box::new(ConsoleStreamHandler::new()),
).await?;
```

### StreamHandler

Handle output from backends.

```rust
pub trait StreamHandler: Send {
    fn on_output(&mut self, chunk: &str);
    fn on_complete(&mut self);
    fn on_error(&mut self, error: &str);
}
```

**Built-in handlers:**

```rust
use hats_adapters::stream_handler::*;

// Console output (plain)
let handler = ConsoleStreamHandler::new();

// Pretty output (formatted)
let handler = PrettyStreamHandler::new();

// TUI mode
let handler = TuiStreamHandler::new(tx);

// Quiet (CI mode)
let handler = QuietStreamHandler::new();
```

### Claude Stream Parser

Parse Claude's NDJSON streaming output.

```rust
use hats_adapters::claude_stream::ClaudeStreamParser;

let parser = ClaudeStreamParser::new();

// Parse chunk
let events = parser.parse_chunk(chunk)?;

for event in events {
    match event {
        ClaudeEvent::Text(text) => println!("{}", text),
        ClaudeEvent::ToolCall(call) => println!("Tool: {}", call.name),
        ClaudeEvent::ToolResult(result) => println!("Result: {}", result),
        ClaudeEvent::Complete => break,
    }
}
```

## Custom Backends

Create custom backend definitions.

```rust
use hats_adapters::{CliBackend, PromptMode, OutputFormat};

let my_backend = CliBackend {
    name: "my-ai".to_string(),
    command: "my-ai-cli".to_string(),
    prompt_mode: PromptMode::Arg,
    output_format: OutputFormat::Text,
};
```

## Custom Stream Handlers

Implement the `StreamHandler` trait.

```rust
use hats_adapters::StreamHandler;

struct MyHandler {
    buffer: String,
}

impl StreamHandler for MyHandler {
    fn on_output(&mut self, chunk: &str) {
        self.buffer.push_str(chunk);
        // Custom processing
    }

    fn on_complete(&mut self) {
        println!("Done: {}", self.buffer);
    }

    fn on_error(&mut self, error: &str) {
        eprintln!("Error: {}", error);
    }
}
```

## Error Types

```rust
pub enum AdapterError {
    BackendNotFound(String),
    ExecutionError(String),
    ParseError(String),
    IoError(std::io::Error),
}
```

## Feature Flags

| Flag | Description |
|------|-------------|
| `default` | All backends |
| `claude` | Claude support only |
| `kiro` | Kiro support only |

## Example: Execute Backend

```rust
use hats_adapters::{backends, PtyExecutor, ConsoleStreamHandler};

#[tokio::main]
async fn main() -> Result<()> {
    // Get Claude backend
    let backend = backends::claude();

    // Create executor
    let executor = PtyExecutor::new();

    // Execute with prompt
    let result = executor.execute(
        &backend,
        "Write a hello world function",
        Box::new(ConsoleStreamHandler::new()),
    ).await?;

    println!("Exit code: {}", result.exit_code);
    println!("Output: {}", result.output);

    Ok(())
}
```
