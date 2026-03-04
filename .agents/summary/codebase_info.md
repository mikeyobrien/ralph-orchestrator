# Codebase Information

## Project Identity

| Field | Value |
|-------|-------|
| **Name** | Ralph Orchestrator |
| **Version** | 2.6.0 (Rust workspace) / 2.3.0 (npm workspace) |
| **License** | MIT |
| **Repository** | https://github.com/mikeyobrien/ralph-orchestrator |
| **Description** | Multi-agent orchestration framework for AI coding assistants |
| **Rust Edition** | 2024 |
| **Node.js** | ≥ 22.0.0 |

## Technology Stack

| Layer | Technology |
|-------|-----------|
| **Primary Language** | Rust (215 source files across 9 crates) |
| **Backend Web** | TypeScript/Node.js (Fastify + tRPC + SQLite) — 58 source files |
| **Frontend Web** | TypeScript/React (Vite + TailwindCSS + React Flow) — 70 source files |
| **Async Runtime** | Tokio (full features) |
| **TUI Framework** | ratatui 0.30 + crossterm 0.28 |
| **Serialization** | serde + serde_json + serde_yaml |
| **CLI Parsing** | clap 4 (derive mode) |
| **HTTP Client** | reqwest 0.12 (rustls-tls) |
| **WebSocket** | tokio-tungstenite 0.24 |
| **Telegram Bot** | teloxide 0.13 |
| **Error Handling** | thiserror 2 + anyhow 1 |
| **Logging** | tracing 0.1 + tracing-subscriber 0.3 |
| **PTY** | portable-pty 0.9 |
| **Keychain** | keyring 3 (apple-native, linux-native) |

## Workspace Structure

```
ralph-orchestrator/
├── Cargo.toml              # Rust workspace root
├── package.json            # npm workspace root
├── crates/                 # 9 Rust crates
│   ├── ralph-proto/        # Protocol definitions and shared types
│   ├── ralph-core/         # Orchestration logic, event loop, hats, memories, tasks
│   ├── ralph-adapters/     # Backend integrations (Claude, Kiro, Gemini, etc.)
│   ├── ralph-cli/          # CLI entry point and commands
│   ├── ralph-tui/          # Terminal UI (ratatui-based)
│   ├── ralph-telegram/     # Telegram bot for human-in-the-loop
│   ├── ralph-api/          # REST/WebSocket API server (Axum)
│   ├── ralph-e2e/          # End-to-end test framework
│   └── ralph-bench/        # Benchmarking
├── backend/                # Node.js web server
│   └── ralph-web-server/   # @ralph-web/server (Fastify + tRPC + SQLite)
├── frontend/               # React web dashboard
│   └── ralph-web/          # @ralph-web/dashboard (React + Vite + TailwindCSS)
├── presets/                 # Hat collection YAML presets
├── docs/                   # MkDocs documentation site
├── cassettes/              # Replay fixtures for smoke/E2E tests
├── scripts/                # Build and CI helper scripts
└── specs/                  # Design specifications
```

## Supported Platforms

| Platform | Support |
|----------|---------|
| macOS (aarch64) | ✅ Primary |
| macOS (x86_64) | ✅ Supported |
| Linux (aarch64) | ✅ Cross-compiled |
| Linux (x86_64) | ✅ Supported |
| Windows | ❌ Excluded (requires Unix PTY and signal handling) |

## Distribution

- **cargo-dist**: GitHub CI releases with shell and npm installers
- **npm scope**: `@ralph-orchestrator/ralph`
- **crates.io**: All 9 crates published
