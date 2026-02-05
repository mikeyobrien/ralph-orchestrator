# hats-cli

Binary entry point and CLI parsing.

## Overview

`hats-cli` is the main binary that:

- Parses command-line arguments
- Routes to appropriate commands
- Handles global options

## Commands

### hats run

Execute the orchestration loop.

```rust
pub struct RunCommand {
    pub prompt: Option<String>,
    pub prompt_file: Option<PathBuf>,
    pub max_iterations: Option<usize>,
    pub completion_promise: Option<String>,
    pub dry_run: bool,
    pub no_tui: bool,
    pub autonomous: bool,
    pub idle_timeout: Option<u64>,
    pub record_session: Option<PathBuf>,
    pub quiet: bool,
    pub continue_: bool,
}
```

### hats init

Initialize configuration file.

```rust
pub struct InitCommand {
    pub backend: Option<String>,
    pub preset: Option<String>,
    pub list_presets: bool,
    pub force: bool,
}
```

### hats plan

Start PDD planning session.

```rust
pub struct PlanCommand {
    pub idea: Option<String>,
    pub backend: Option<String>,
}
```

### hats task

Generate code task files.

```rust
pub struct TaskCommand {
    pub input: Option<String>,
    pub backend: Option<String>,
}
```

### hats events

View event history.

```rust
pub struct EventsCommand {
    pub limit: Option<usize>,
    pub format: OutputFormat,
}
```

### hats emit

Emit an event.

```rust
pub struct EmitCommand {
    pub topic: String,
    pub payload: Option<String>,
    pub json: Option<String>,
}
```

### hats clean

Clean up `.agent/` directory.

```rust
pub struct CleanCommand {
    pub diagnostics: bool,
    pub all: bool,
}
```

### hats tools

Runtime tools subcommands.

```rust
pub enum ToolsCommand {
    Memory(MemoryCommand),
    Task(TaskCliCommand),
}
```

## Global Options

```rust
pub struct GlobalOptions {
    pub config: PathBuf,
    pub verbose: bool,
    pub color: ColorMode,
}
```

## Implementation

### Command Dispatch

```rust
pub async fn run() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Command::Run(cmd) => run_command(cmd, &cli.global).await,
        Command::Init(cmd) => init_command(cmd, &cli.global).await,
        Command::Plan(cmd) => plan_command(cmd, &cli.global).await,
        Command::Task(cmd) => task_command(cmd, &cli.global).await,
        Command::Events(cmd) => events_command(cmd, &cli.global).await,
        Command::Emit(cmd) => emit_command(cmd, &cli.global).await,
        Command::Clean(cmd) => clean_command(cmd, &cli.global).await,
        Command::Tools(cmd) => tools_command(cmd, &cli.global).await,
    }
}
```

### Error Handling

```rust
fn main() {
    if let Err(e) = hats_cli::run() {
        eprintln!("Error: {}", e);
        std::process::exit(1);
    }
}
```

## Shell Completions

Generate shell completions:

```rust
pub fn generate_completions(shell: Shell) -> String {
    let mut cmd = Cli::command();
    let mut buf = Vec::new();
    generate(shell, &mut cmd, "hats", &mut buf);
    String::from_utf8(buf).unwrap()
}
```

**Usage:**

```bash
hats completions bash > ~/.local/share/bash-completion/completions/hats
hats completions zsh > ~/.zfunc/_hats
hats completions fish > ~/.config/fish/completions/hats.fish
```

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | General error |
| 2 | Configuration error |
| 3 | Backend not found |
| 4 | Interrupted |

## Example: Adding a New Command

1. Define command struct:

```rust
#[derive(Parser)]
pub struct MyCommand {
    #[arg(short, long)]
    pub option: Option<String>,
}
```

2. Add to command enum:

```rust
pub enum Command {
    // ...
    MyCommand(MyCommand),
}
```

3. Implement handler:

```rust
pub async fn my_command(cmd: MyCommand, global: &GlobalOptions) -> Result<()> {
    // Implementation
    Ok(())
}
```

4. Add to dispatch:

```rust
Command::MyCommand(cmd) => my_command(cmd, &cli.global).await,
```
