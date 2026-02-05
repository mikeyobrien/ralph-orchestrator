<!-- 2026-01-28 -->
# Hats

[![License](https://img.shields.io/badge/license-MIT-blue)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.75+-orange)](https://www.rust-lang.org/)
[![Build](https://img.shields.io/github/actions/workflow/status/mikeyobrien/hats/ci.yml?branch=main&label=CI)](https://github.com/mikeyobrien/hats/actions)
[![Coverage](https://img.shields.io/badge/coverage-65%25-yellowgreen)](coverage/index.html)
[![Mentioned in Awesome Claude Code](https://awesome.re/mentioned-badge.svg)](https://github.com/hesreallyhim/awesome-claude-code)
[![Docs](https://img.shields.io/badge/docs-mkdocs-blue)](https://mikeyobrien.github.io/hats/)

A hat-based orchestration framework that keeps AI agents in a loop until the task is done.

> "Me fail English? That's unpossible!" - Hats Wiggum

**[Documentation](https://mikeyobrien.github.io/hats/)** | **[Getting Started](https://mikeyobrien.github.io/hats/getting-started/quick-start/)** | **[Presets](https://mikeyobrien.github.io/hats/guide/presets/)**

## Installation

### Via npm (Recommended)

```bash
npm install -g @hats/hats-cli
```

### Via Homebrew (macOS)

```bash
brew install hats
```

### Via Cargo

```bash
cargo install hats-cli
```

## Quick Start

```bash
# 1. Initialize Hats with your preferred backend
hats init --backend claude

# 2. Plan your feature (interactive PDD session)
hats plan "Add user authentication with JWT"
# Creates: specs/user-authentication/requirements.md, design.md, implementation-plan.md

# 3. Implement the feature
hats run -p "Implement the feature in specs/user-authentication/"
```

Hats iterates until it outputs `LOOP_COMPLETE` or hits the iteration limit.

For simpler tasks, skip planning and run directly:

```bash
hats run -p "Add input validation to the /users endpoint"
```

## Web Dashboard (Alpha)

> **Alpha:** The web dashboard is under active development. Expect rough edges and breaking changes.

<img width="1513" height="1128" alt="image" src="https://github.com/user-attachments/assets/ce5f072f-3d81-44d8-8f2f-88b42b33a3be" />

Hats includes a web dashboard for monitoring and managing orchestration loops.

```bash
hats web                              # starts both servers + opens browser
hats web --no-open                    # skip browser auto-open
hats web --backend-port 4000          # custom backend port
hats web --frontend-port 8080         # custom frontend port
```

**Requirements:** Node.js >= 18 and npm. On first run, `hats web` will auto-detect missing `node_modules` and run `npm install` for you.

To set up Node.js:

```bash
# Option 1: nvm (recommended)
nvm install    # reads .nvmrc

# Option 2: direct install
# https://nodejs.org/
```

For development:

```bash
npm install          # install dependencies
npm run dev          # run both servers (backend:3000, frontend:5173)
npm run test:server  # backend tests
npm run test         # all tests
```

## What is Hats?

Hats implements the [Hats Wiggum technique](https://ghuntley.com/hats/) — autonomous task completion through continuous iteration. It supports:

- **Multi-Backend Support** — Claude Code, Kiro, Gemini CLI, Codex, Amp, Copilot CLI, OpenCode
- **Hat System** — Specialized personas coordinating through events
- **Backpressure** — Gates that reject incomplete work (tests, lint, typecheck)
- **Memories & Tasks** — Persistent learning and runtime work tracking
- **31 Presets** — TDD, spec-driven, debugging, and more

## RObot (Human-in-the-Loop)

Hats supports human interaction during orchestration via Telegram. Agents can ask questions and block until answered; humans can send proactive guidance at any time.

Quick onboarding (Telegram):

```bash
hats bot onboard --telegram   # guided setup (token + chat id)
hats bot status               # verify config
hats bot test                 # send a test message
hats run -c hats.bot.yml -p  "Help the human"
```

```yaml
# hats.yml
RObot:
  enabled: true
  telegram:
    bot_token: "your-token"  # Or HATS_TELEGRAM_BOT_TOKEN env var
```

- **Agent questions** — Agents emit `human.interact` events; the loop blocks until a response arrives or times out
- **Proactive guidance** — Send messages anytime to steer the agent mid-loop
- **Parallel loop routing** — Messages route via reply-to, `@loop-id` prefix, or default to primary
- **Telegram commands** — `/status`, `/tasks`, `/restart` for real-time loop visibility

See the [Telegram guide](https://mikeyobrien.github.io/hats/guide/telegram/) for setup instructions.

## Documentation

Full documentation is available at **[mikeyobrien.github.io/hats](https://mikeyobrien.github.io/hats/)**:

- [Installation](https://mikeyobrien.github.io/hats/getting-started/installation/)
- [Quick Start](https://mikeyobrien.github.io/hats/getting-started/quick-start/)
- [Configuration](https://mikeyobrien.github.io/hats/guide/configuration/)
- [CLI Reference](https://mikeyobrien.github.io/hats/guide/cli-reference/)
- [Presets](https://mikeyobrien.github.io/hats/guide/presets/)
- [Concepts: Hats & Events](https://mikeyobrien.github.io/hats/concepts/hats-and-events/)
- [Architecture](https://mikeyobrien.github.io/hats/advanced/architecture/)

## Contributing

Contributions are welcome! See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines and [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md) for community standards.

## License

MIT License — See [LICENSE](LICENSE) for details.

## Acknowledgments

- **[Geoffrey Huntley](https://ghuntley.com/hats/)** — Creator of the Hats Wiggum technique
- **[Strands Agents SOP](https://github.com/strands-agents/agent-sop)** — Agent SOP framework
- **[ratatui](https://ratatui.rs/)** — Terminal UI framework

---

*"I'm learnding!" - Hats Wiggum*
