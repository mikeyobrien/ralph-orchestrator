# ralph-cli

Binary entry point and CLI parsing.

## Overview

`ralph-cli` is the main binary that:

- Parses command-line arguments
- Routes to command handlers
- Configures runtime logging/output behavior

## Top-Level Commands

The `Commands` enum in `crates/ralph-cli/src/main.rs` currently includes:

- `run`
- `preflight`
- `hooks`
- `doctor`
- `tutorial`
- `events`
- `init`
- `clean`
- `emit`
- `plan`
- `code-task` (plus hidden legacy `task` alias)
- `tools`
- `loops`
- `hats`
- `tui`
- `web`
- `mcp`
- `bot`
- `completions`

For user-facing flags and examples, see the canonical CLI guide: `docs/guide/cli-reference.md`.

## MCP Server Mode (`ralph mcp`)

`ralph mcp serve` runs Ralph as a Model Context Protocol server over `stdio`.

Notes:

- Intended for MCP client configuration (non-interactive)
- Uses stdout for protocol messages and stderr for logs
- Exposes control-plane tools, including stream polling tools like `stream_next`

## Runtime Directories

Ralph runtime state is stored under `.ralph/` and remains ignored by default. Committed planning artifacts live in `.ralph/specs/`, and committed code task files live in `.ralph/tasks/`; legacy `.agent/` paths are not used for new artifacts.

## Command Dispatch

Dispatch is handled in `run()` via a `match` on `cli.command`, delegating to each submodule (for example `web::execute(args).await`, `mcp::execute(args).await`, `bot::execute(...)`).

## Global Options

Global CLI options include:

- `--config <PATH>`
- `--verbose`
- `--color <auto|always|never>`

## Shell Completions

`ralph completions <shell>` outputs completion scripts.

Example:

```bash
ralph completions bash > ~/.local/share/bash-completion/completions/ralph
```

## Exit Codes

Command handlers return process errors via `anyhow::Result`, surfaced by the binary entry point.
