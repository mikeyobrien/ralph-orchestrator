# ralph-web

Web dashboard for Ralph orchestrator - monitor, control, and start orchestration loops.

## Quick Start

```bash
# Start the backend server
cargo run -p ralph-web -- serve

# In another terminal, start the frontend dev server
cd crates/ralph-web/frontend
npm install
npm run dev
```

Open http://localhost:5173 in your browser.

## Features

- **Live Monitoring** - Real-time streaming output from running loops via WebSocket
- **Session Browser** - Browse past orchestration sessions and their outputs
- **Loop Control** - Start, stop, and manage multiple concurrent loops
- **Dark Mode** - System-aware theme with manual toggle

## Architecture

```
ralph-web/
├── src/              # Rust backend (Axum)
│   ├── lib.rs        # Server entry point
│   ├── routes/       # REST API endpoints
│   └── loop_manager.rs  # Process spawning and management
├── frontend/         # React frontend (Vite + TypeScript)
│   ├── src/
│   │   ├── components/  # Reusable UI components
│   │   ├── routes/      # Page components
│   │   ├── hooks/       # Custom React hooks
│   │   └── lib/         # Utilities and API client
│   └── e2e/          # Playwright E2E tests
└── tests/            # Rust integration tests
```

## API Endpoints

### Sessions
- `GET /api/sessions` - List all sessions
- `GET /api/sessions/:id` - Get session details

### Loops
- `GET /api/loops/active` - List active loops
- `POST /api/loops/start` - Start a new loop
- `POST /api/loops/:id/stop` - Stop a specific loop
- `POST /api/loops/stop` - Stop the active loop
- `GET /api/loops/:id` - Get loop status

### Configs
- `GET /api/configs` - List available config files

### WebSocket
- `WS /ws/loop/:session_id` - Stream loop output in real-time

## Development

### Backend

```bash
# Run backend with hot reload
cargo watch -x 'run -p ralph-web -- serve'

# Run backend tests
cargo test -p ralph-web
```

### Frontend

```bash
cd crates/ralph-web/frontend

# Install dependencies
npm install

# Development server
npm run dev

# Type checking
npm run typecheck

# Linting
npm run lint

# Unit tests
npm run test

# E2E tests (requires built frontend)
npm run build
npm run test:e2e
```

## Configuration

The backend serves on port 3001 by default. The frontend dev server proxies API requests to the backend.

Environment variables:
- `RALPH_WEB_PORT` - Backend port (default: 3001)
- `RALPH_DIAGNOSTICS` - Enable diagnostics for spawned loops
