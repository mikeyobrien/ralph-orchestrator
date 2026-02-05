# Agents API Reference

## Overview

Hats treats agents as CLI backends. Backend selection lives in `hats_core::HatBackend` and
`hats_core::CliConfig`, while execution is handled by `hats-adapters`.

Key types:
- `hats_adapters::detect_backend_default`, `detect_backend`, `is_backend_available`
- `hats_adapters::CliBackend`, `CliExecutor`
- `hats_core::HatBackend`, `CliConfig`

## Backend Detection

Detect an available backend in PATH (Claude, Kiro, Gemini, Codex, Amp, Copilot, OpenCode):

```rust
use hats_adapters::{detect_backend_default, is_backend_available};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    if is_backend_available("claude") {
        println!("Claude CLI available");
    }

    let backend = detect_backend_default()?;
    println!("Selected backend: {backend}");

    Ok(())
}
```

## Build a Backend from Hat Configuration

`HatBackend` is the per-hat backend definition used in `hats.yml`. You can convert it into
a `CliBackend` for execution:

```rust
use hats_adapters::{CliBackend, CliExecutor};
use hats_core::HatBackend;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let hat_backend = HatBackend::NamedWithArgs {
        backend_type: "claude".to_string(),
        args: vec!["--model".to_string(), "claude-sonnet-4".to_string()],
    };

    let backend = CliBackend::from_hat_backend(&hat_backend)?;
    let executor = CliExecutor::new(backend);

    let result = executor.execute_capture("Summarize the task in 3 bullets.").await?;
    if result.success {
        println!("{}", result.output);
    }

    Ok(())
}
```

## Build a Backend from CLI Config

Use `CliConfig` when you want to start from the top-level config:

```rust
use hats_adapters::CliBackend;
use hats_core::CliConfig;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = CliConfig {
        backend: "gemini".to_string(),
        ..Default::default()
    };

    let backend = CliBackend::from_config(&config)?;
    let (cmd, args, _stdin, _temp) = backend.build_command("Hello", false);

    println!("Command: {cmd}");
    println!("Args: {args:?}");

    Ok(())
}
```

## Execution Results

`CliExecutor` returns `ExecutionResult`, which includes the aggregated output, exit code,
and timeout state.

```rust
use hats_adapters::{CliBackend, CliExecutor};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let backend = CliBackend::claude();
    let executor = CliExecutor::new(backend);

    let result = executor.execute_capture("List 5 project risks.").await?;
    println!("Success: {}", result.success);
    println!("Exit code: {:?}", result.exit_code);
    println!("Timed out: {}", result.timed_out);

    Ok(())
}
```
