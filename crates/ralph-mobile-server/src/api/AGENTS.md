<!-- Parent: ../AGENTS.md -->
<!-- Generated: 2026-01-27 | Updated: 2026-01-27 -->

# api

## Purpose

REST API route handlers for ralph-mobile-server. Each module handles a specific resource type.

## Key Files

| File | Description |
|------|-------------|
| `mod.rs` | Route configuration and exports |
| `sessions.rs` | Session CRUD operations |
| `events.rs` | SSE event streaming |
| `configs.rs` | Config file listing |
| `prompts.rs` | Prompt file listing |
| `runner.rs` | Run control (start/stop Ralph) |

## For AI Agents

### Endpoints by Module

| Module | Endpoints |
|--------|-----------|
| `sessions.rs` | `GET /api/sessions`, `GET /api/sessions/:id` |
| `events.rs` | `GET /api/sessions/:id/events` (SSE) |
| `configs.rs` | `GET /api/configs` |
| `prompts.rs` | `GET /api/prompts` |
| `runner.rs` | `POST /api/runner/start`, `POST /api/runner/stop` |

### Working In This Directory
- Use `actix-web` extractors (`Path`, `Query`, `Json`)
- Return `actix_web::Result` from handlers
- SSE uses `actix_web::web::Bytes` streaming

<!-- MANUAL: Any manually added notes below this line are preserved on regeneration -->
