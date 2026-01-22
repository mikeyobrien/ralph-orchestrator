# Rust TUI Logging Best Practices

## Key Constraint

**TUI applications cannot log to stdout** — it would corrupt the terminal display. File-based logging is required.

## Recommended Stack

| Component | Crate | Purpose |
|-----------|-------|---------|
| Logging framework | `tracing` | Structured logging & spans |
| Subscriber | `tracing-subscriber` | Output formatting |
| TUI widget | `tui-logger` | Optional in-TUI log display |
| File output | `tracing-appender` | File writing with rotation |

## Best Practices

### 1. Use `#[instrument]` Macro

```rust
#[instrument(skip(self), fields(iteration = %self.iteration))]
async fn execute_hat(&mut self, hat: &Hat) -> Result<()> {
    // Function args automatically captured as structured fields
}
```

### 2. Structured Fields Over String Formatting

```rust
// Good
info!(iteration = %iter, hat = %hat.name, "Starting execution");

// Avoid
info!("Starting execution of {} at iteration {}", hat.name, iter);
```

### 3. Environment-Based Filtering

```rust
// Support RUST_LOG for granular control
let filter = EnvFilter::try_from_default_env()
    .unwrap_or_else(|_| EnvFilter::new("info"));
```

### 4. Log Levels

| Level | Use Case |
|-------|----------|
| `trace!` | Very detailed (function entry/exit) |
| `debug!` | Debugging info (too verbose for production) |
| `info!` | General operational messages |
| `warn!` | Non-fatal issues |
| `error!` | Serious issues |

### 5. File-Based Logging for TUI

```rust
// Write to file, not stdout
let file_appender = tracing_appender::rolling::daily(log_dir, "ralph.log");
let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);

tracing_subscriber::fmt()
    .with_writer(non_blocking)
    .with_ansi(false)  // No ANSI in files
    .init();
```

## Sources

- [Logging in Rust (2025) | Shuttle](https://www.shuttle.dev/blog/2023/09/20/logging-in-rust)
- [tui-logger documentation](https://docs.rs/tui-logger/latest/tui_logger/)
- [Tracing Rust Guide 2025](https://generalistprogrammer.com/tutorials/tracing-rust-crate-guide)
- [Setup Logging with tracing | Ratatui](https://ratatui.rs/recipes/apps/log-with-tracing/)
- [Structured Logs with tracing and OpenTelemetry](https://oneuptime.com/blog/post/2026-01-07-rust-tracing-structured-logs/view)
