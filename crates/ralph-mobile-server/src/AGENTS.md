<!-- Parent: ../AGENTS.md -->
<!-- Generated: 2026-01-27 | Updated: 2026-01-27 -->

# src

## Purpose

Source code for ralph-mobile-server. Contains the HTTP server, API routes, authentication, and file watching.

## Key Files

| File | Description |
|------|-------------|
| `main.rs` | Server entry point and setup |
| `cli.rs` | CLI argument parsing with clap |
| `auth.rs` | API key authentication middleware |
| `session.rs` | Session state management |
| `watcher.rs` | File system watcher for events |

## Subdirectories

| Directory | Purpose |
|-----------|---------|
| `api/` | REST API route handlers |

## For AI Agents

### Server Architecture
```
main.rs
  ↓ configures
actix-web server
  ↓ routes to
api/*.rs handlers
  ↓ uses
session.rs (state)
watcher.rs (file events)
auth.rs (middleware)
```

### Working In This Directory
- Use `actix-web` patterns
- All routes require API key auth
- SSE streams use `tokio-stream`

<!-- MANUAL: Any manually added notes below this line are preserved on regeneration -->
