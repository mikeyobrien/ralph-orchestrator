# hats-tui

Terminal UI for real-time Hats monitoring.

## Overview

`hats-tui` provides:

- Real-time iteration display
- Hat and event status
- Agent output streaming
- Interactive controls

Built with [ratatui](https://ratatui.rs/).

## Features

### Header Display

Shows current orchestration state:

- Iteration count: `[iter 3]`
- Elapsed time: `00:02:15`
- Active hat emoji and name: `🔨 Builder`
- Mode indicator

### Content Area

Displays agent output with:

- Real-time streaming
- Syntax highlighting
- Scroll support

### Footer

Shows activity status:

- Activity indicator: `◉` (active), `◯` (idle), `■` (stopped)
- Current event topic
- Search display (if active)

## Usage

The TUI is enabled by default with `hats run`:

```bash
# TUI mode (default)
hats run

# Disable TUI
hats run --no-tui
```

## Key Bindings

| Key | Action |
|-----|--------|
| `q` | Quit |
| `↑`/`↓` | Scroll output |
| `PgUp`/`PgDn` | Page scroll |
| `Home`/`End` | Jump to start/end |
| `/` | Search |
| `n` | Next search result |
| `N` | Previous search result |

## Programmatic Use

### TUI Application

```rust
use hats_tui::TuiApp;

let app = TuiApp::new();
app.run().await?;
```

### TUI Stream Handler

For integration with adapters:

```rust
use hats_tui::TuiStreamHandler;
use tokio::sync::mpsc;

let (tx, rx) = mpsc::channel(100);
let handler = TuiStreamHandler::new(tx);

// Handler sends UxEvents to TUI
```

### UX Events

Events from the orchestrator to TUI:

```rust
use hats_proto::UxEvent;

enum UxEvent {
    TerminalWrite(String),
    Resize { width: u16, height: u16 },
    FrameCapture(Vec<u8>),
    IterationStart(usize),
    HatSelected(String),
    EventPublished(String),
}
```

## Widgets

### Header Widget

```rust
use hats_tui::widgets::Header;

let header = Header::new()
    .iteration(3)
    .elapsed(Duration::from_secs(135))
    .hat("🔨 Builder")
    .mode("hat-based");
```

### Footer Widget

```rust
use hats_tui::widgets::Footer;

let footer = Footer::new()
    .activity(Activity::Active)
    .event_topic("build.done")
    .search_query(None);
```

### Content Widget

```rust
use hats_tui::widgets::Content;

let content = Content::new()
    .text(&output)
    .scroll(scroll_position);
```

## Customization

### Colors

```rust
use hats_tui::theme::Theme;

let theme = Theme {
    header_bg: Color::Rgb(30, 30, 46),
    header_fg: Color::Rgb(205, 214, 244),
    active_indicator: Color::Green,
    // ...
};
```

### Layout

```rust
use hats_tui::Layout;

let layout = Layout {
    header_height: 3,
    footer_height: 2,
    // content fills remaining space
};
```

## Error Handling

```rust
pub enum TuiError {
    IoError(std::io::Error),
    RenderError(String),
    EventError(String),
}
```

## Example: Custom TUI Integration

```rust
use hats_tui::{TuiApp, TuiStreamHandler};
use hats_adapters::PtyExecutor;
use tokio::sync::mpsc;

#[tokio::main]
async fn main() -> Result<()> {
    // Create channel for TUI updates
    let (tx, rx) = mpsc::channel(100);

    // Create TUI handler for adapter
    let handler = TuiStreamHandler::new(tx);

    // Spawn TUI in separate task
    let tui_handle = tokio::spawn(async move {
        let app = TuiApp::with_receiver(rx);
        app.run().await
    });

    // Execute backend with TUI handler
    let executor = PtyExecutor::new();
    executor.execute(&backend, &prompt, Box::new(handler)).await?;

    // Wait for TUI to finish
    tui_handle.await??;

    Ok(())
}
```

## TUI Validation

For testing TUI rendering, use the `/tui-validate` skill:

```bash
/tui-validate file:output.txt criteria:hats-header
```

See [Testing & Validation](../advanced/testing.md) for details.
