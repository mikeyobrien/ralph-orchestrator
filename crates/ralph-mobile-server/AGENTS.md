<!-- Parent: ../AGENTS.md -->
<!-- Generated: 2026-01-27 | Updated: 2026-01-27 -->

# ralph-mobile-server

## Purpose

REST API and Server-Sent Events (SSE) server for mobile monitoring of Ralph orchestrator sessions. Powers the RalphMobile iOS app.

## Key Files

| File | Description |
|------|-------------|
| `src/main.rs` | Server entry point |
| `src/cli.rs` | CLI argument parsing |
| `src/auth.rs` | API key authentication |
| `src/session.rs` | Session management |
| `src/watcher.rs` | File system watcher for events |
| `Cargo.toml` | Crate manifest |

## Subdirectories

| Directory | Purpose |
|-----------|---------|
| `src/api/` | REST API route handlers |

## For AI Agents

### API Endpoints

| Endpoint | Method | Description |
|----------|--------|-------------|
| `/api/sessions` | GET | List active sessions |
| `/api/sessions/:id` | GET | Get session details |
| `/api/sessions/:id/events` | GET (SSE) | Stream events in real-time |
| `/api/configs` | GET | List available configs |
| `/api/prompts` | GET | List available prompts |
| `/api/runner/start` | POST | Start a new Ralph run |
| `/api/runner/stop` | POST | Stop a running session |

### API Modules

| Module | Purpose |
|--------|---------|
| `api/sessions.rs` | Session CRUD endpoints |
| `api/events.rs` | SSE event streaming |
| `api/configs.rs` | Config listing |
| `api/prompts.rs` | Prompt listing |
| `api/runner.rs` | Run control (start/stop) |

### Working In This Directory
- Depends on `ralph-core` only
- Uses `actix-web` for HTTP server
- Uses `notify` for file watching

### Testing Requirements
- Run: `cargo test -p ralph-mobile-server`
- Manual: `cargo run --bin ralph-mobile-server`

### Common Patterns
- Use `actix-web` extractors for request handling
- Use `tokio-stream` for SSE
- API key auth via `Authorization` header

<!-- MANUAL: Any manually added notes below this line are preserved on regeneration -->
