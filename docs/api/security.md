# Security API Reference

## Overview

Ralph's security-related utilities are distributed across crates. Common safeguards:

- **Safe CLI execution** via `ralph_adapters::CliExecutor` (no shell invocation)
- **Secret masking** via `ralph_telegram::TelegramService::bot_token_masked`
- **Output escaping** via `ralph_telegram::escape_html`

## Safe CLI Execution

`CliExecutor` uses `tokio::process::Command` with explicit argument vectors, which
avoids shell interpolation and reduces injection risk for prompt content.

```rust
use ralph_adapters::{CliBackend, CliExecutor};
use ralph_core::CliConfig;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Configure a backend explicitly (no shell commands involved).
    let config = CliConfig {
        backend: "codex".to_string(),
        ..Default::default()
    };

    let backend = CliBackend::from_config(&config)?;
    let executor = CliExecutor::new(backend);

    let result = executor.execute_capture("Summarize this task.").await?;
    println!("success={} exit_code={:?}", result.success, result.exit_code);

    Ok(())
}
```

## Mask Secrets in Logs

When integrating Telegram, `TelegramService::bot_token_masked` keeps logs safe
by exposing only the prefix/suffix of the token.

```rust
use ralph_telegram::TelegramService;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let service = TelegramService::new(
        PathBuf::from("."),
        Some("1234567890:abcdefg_hijklmnop".to_string()),
        300,
        "loop-1".to_string(),
    )?;

    println!("token={}", service.bot_token_masked());
    Ok(())
}
```

## Escape HTML for Telegram Output

Telegram's HTML parse mode requires escaping special characters.

```rust
use ralph_telegram::escape_html;

fn main() {
    let raw = "<task> & details";
    let safe = escape_html(raw);
    assert_eq!(safe, "&lt;task&gt; &amp; details");
}
```
