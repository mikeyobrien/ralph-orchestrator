<!-- 2026-01-28 -->
# Ralph Orchestrator

[![License](https://img.shields.io/badge/license-MIT-blue)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.75+-orange)](https://www.rust-lang.org/)
[![Build](https://img.shields.io/github/actions/workflow/status/mikeyobrien/ralph-orchestrator/ci.yml?branch=main&label=CI)](https://github.com/mikeyobrien/ralph-orchestrator/actions)
[![Coverage](https://img.shields.io/endpoint?url=https://mikeyobrien.github.io/ralph-orchestrator/badges/coverage.json)](CONTRIBUTING.md#coverage)
[![Mentioned in Awesome Claude Code](https://awesome.re/mentioned-badge.svg)](https://github.com/hesreallyhim/awesome-claude-code)
[![Docs](https://img.shields.io/badge/docs-mkdocs-blue)](https://mikeyobrien.github.io/ralph-orchestrator/)
[![Discord](https://img.shields.io/discord/1482421188700667906?label=Discord&logo=discord&logoColor=white)](https://discord.gg/XWUyeUNffh)

A hat-based orchestration framework that keeps AI agents in a loop until the task is done.

> "Me fail English? That's unpossible!" - Ralph Wiggum

**[Documentation](https://mikeyobrien.github.io/ralph-orchestrator/)** | **[Getting Started](https://mikeyobrien.github.io/ralph-orchestrator/getting-started/quick-start/)** | **[Presets](https://mikeyobrien.github.io/ralph-orchestrator/guide/presets/)**

## Installation

### Via npm (Recommended)

```bash
npm install -g @ralph-orchestrator/ralph-cli
```

### Via GitHub Releases installer

```bash
curl --proto '=https' --tlsv1.2 -LsSf \
  https://github.com/mikeyobrien/ralph-orchestrator/releases/latest/download/ralph-cli-installer.sh | sh
```

### Via Cargo

```bash
cargo install ralph-cli
```

> Homebrew is not currently published from this repository's automated release flow. Prefer npm, Cargo, or the GitHub Releases installer.

## Quick Start

```bash
# 1. Initialize Ralph with your preferred backend
ralph init --backend claude

# 2. Plan your feature (interactive PDD session)
ralph plan "Add user authentication with JWT"
# Creates: specs/user-authentication/requirements.md, design.md, implementation-plan.md

# 3. Implement the feature
ralph run -p "Implement the feature in specs/user-authentication/"
```

Ralph iterates until it outputs `LOOP_COMPLETE` or hits the iteration limit.

For simpler tasks, skip planning and run directly:

```bash
ralph run -p "Add input validation to the /users endpoint"
```

## Web Dashboard (Alpha)

> **Alpha:** The web dashboard is under active development. Expect rough edges and breaking changes.

<img width="1513" height="1128" alt="image" src="https://github.com/user-attachments/assets/ce5f072f-3d81-44d8-8f2f-88b42b33a3be" />

Ralph includes a web dashboard for monitoring and managing orchestration loops.

```bash
ralph web                              # starts Rust RPC API + frontend + opens browser
ralph web --no-open                    # skip browser auto-open
ralph web --backend-port 4000          # custom RPC API port
ralph web --frontend-port 8080         # custom frontend port
ralph web --legacy-node-api            # opt into deprecated Node tRPC backend
```

### MCP Server Workspace Scope

`ralph mcp serve` is scoped to a single workspace root per server instance.

```bash
ralph mcp serve --workspace-root /path/to/repo
```

Precedence is:

1. `--workspace-root`
2. `RALPH_API_WORKSPACE_ROOT`
3. current working directory

For multi-repo use, run one MCP server instance per repo/workspace. Ralph's current
control-plane APIs persist config, tasks, loops, planning sessions, and collections
under a single workspace root, so server-per-workspace is the deterministic model.

**Requirements:**
- Rust toolchain (for `ralph-api`)
- Node.js >= 18 + npm (for the frontend)

On first run, `ralph web` auto-detects missing `node_modules` and runs `npm install`.

To set up Node.js:

```bash
# Option 1: nvm (recommended)
nvm install    # reads .nvmrc

# Option 2: direct install
# https://nodejs.org/
```

For development:

```bash
npm install              # install frontend + legacy backend deps
npm run dev:api          # Rust RPC API (port 3000)
npm run dev:web          # frontend (port 5173)
npm run dev              # frontend only (default)
npm run dev:legacy-server  # deprecated Node backend (optional)
npm run test             # all frontend/backend workspace tests
```

## MCP Server Mode

Ralph can run as an MCP server over stdio for MCP-compatible clients:

```bash
ralph mcp serve
```

Use this mode from an MCP client configuration rather than an interactive terminal workflow.

## What is Ralph?

Ralph implements the [Ralph Wiggum technique](https://ghuntley.com/ralph/) — autonomous task completion through continuous iteration. It supports:

- **Multi-Backend Support** — Claude Code, Kiro, Gemini CLI, Codex, Amp, Copilot CLI, OpenCode
- **Hat System** — Specialized personas coordinating through events
- **Backpressure** — Gates that reject incomplete work (tests, lint, typecheck)
- **Memories & Tasks** — Persistent learning and runtime work tracking
- **5 Supported Builtins** — `code-assist`, `debug`, `research`, `review`, and `pdd-to-code-assist`, with more patterns documented as examples

## RObot (Human-in-the-Loop)

Ralph supports human interaction during orchestration via Telegram. Agents can ask questions and block until answered; humans can send proactive guidance at any time.

Quick onboarding (Telegram):

```bash
ralph bot onboard --telegram   # guided setup (token + chat id)
ralph bot status               # verify config
ralph bot test                 # send a test message
ralph run -c ralph.bot.yml -p  "Help the human"
```

```yaml
# ralph.yml
RObot:
  enabled: true
  telegram:
    bot_token: "your-token"  # Or RALPH_TELEGRAM_BOT_TOKEN env var
```

- **Agent questions** — Agents emit `human.interact` events; the loop blocks until a response arrives or times out
- **Proactive guidance** — Send messages anytime to steer the agent mid-loop
- **Parallel loop routing** — Messages route via reply-to, `@loop-id` prefix, or default to primary
- **Telegram commands** — `/status`, `/tasks`, `/restart` for real-time loop visibility

See the [Telegram guide](https://mikeyobrien.github.io/ralph-orchestrator/guide/telegram/) for setup instructions.

## Documentation

Full documentation is available at **[mikeyobrien.github.io/ralph-orchestrator](https://mikeyobrien.github.io/ralph-orchestrator/)**:

- [Installation](https://mikeyobrien.github.io/ralph-orchestrator/getting-started/installation/)
- [Quick Start](https://mikeyobrien.github.io/ralph-orchestrator/getting-started/quick-start/)
- [Configuration](https://mikeyobrien.github.io/ralph-orchestrator/guide/configuration/)
- [CLI Reference](https://mikeyobrien.github.io/ralph-orchestrator/guide/cli-reference/)
- [Presets](https://mikeyobrien.github.io/ralph-orchestrator/guide/presets/)
- [Concepts: Hats & Events](https://mikeyobrien.github.io/ralph-orchestrator/concepts/hats-and-events/)
- [Architecture](https://mikeyobrien.github.io/ralph-orchestrator/advanced/architecture/)


## FAQ

### General

**What is Ralph Orchestrator?**
Ralph is a hat-based orchestration framework that implements the Ralph Wiggum technique — autonomous task completion through continuous iteration. It keeps AI agents in a loop until the task is done, supporting multiple backends like Claude Code, Gemini CLI, Codex, and more.

**How is Ralph different from other AI coding tools?**
Unlike single-shot AI assistants, Ralph iterates until completion using a "hat system" with specialized personas. It includes backpressure gates (tests, lint, typecheck) that reject incomplete work, plus persistent memories and tasks for continuous learning.

### Installation & Setup

**What are the system requirements?**
- Rust 1.75+ (for the `ralph-api` component)
- Node.js >= 18 + npm (for the web dashboard frontend)
- An AI coding assistant CLI (Claude Code, Codex, Gemini CLI, etc.)

**Which installation method should I use?**
- **npm** (recommended for most users): `npm install -g @ralph-orchestrator/ralph-cli`
- **Cargo**: `cargo install ralph-cli` (best for Rust developers)
- **GitHub Releases installer**: One-link install with `curl ... | sh`

**Is Homebrew supported?**
Homebrew is not currently published from this repository's automated release flow. Prefer npm, Cargo, or the GitHub Releases installer.

### Usage

**How do I start a new project with Ralph?**
```bash
ralph init --backend claude
ralph plan "Add user authentication with JWT"
ralph run -p "Implement the feature in specs/user-authentication/"
```

**What backends does Ralph support?**
Claude Code, Kiro, Gemini CLI, Codex, Amp, Copilot CLI, and OpenCode.

**What is the "hat system"?**
Ralph uses specialized personas (hats) that coordinate through events. Each hat has a specific role — code-assist, debug, research, review, and pdd-to-code-assist — enabling structured multi-step task execution.

### RObot (Human-in-the-Loop)

**What is RObot?**
RObot enables human interaction during orchestration via Telegram. Agents can ask questions and block until answered; humans can send proactive guidance mid-loop.

**How do I set up Telegram integration?**
```bash
ralph bot onboard --telegram   # guided setup
ralph bot status               # verify config
ralph bot test                 # send a test message
```

### Web Dashboard

**How do I access the web dashboard?**
Run `ralph web` to start the Rust RPC API + frontend and open your browser. The dashboard is currently in Alpha — expect rough edges and breaking changes.

**Can I customize the dashboard ports?**
Yes: `ralph web --backend-port 4000 --frontend-port 8080`

### MCP Server

**How do I run Ralph as an MCP server?**
```bash
ralph mcp serve --workspace-root /path/to/repo
```
Each MCP server instance is scoped to a single workspace root. For multi-repo use, run one instance per workspace.

### Troubleshooting

**Ralph fails to start with "node_modules not found"**
Run `npm install` in the project directory, or let `ralph web` auto-detect and install on first run.

**How do I set up Node.js if not installed?**
Use nvm (recommended): `nvm install` (reads `.nvmrc`), or install directly from https://nodejs.org/

**Where can I get help?**
- Join our [Discord server](https://discord.gg/XWUyeUNffh)
- Report bugs on the [Issue Tracker](https://github.com/mikeyobrien/ralph-orchestrator/issues)
- Read full documentation at [mikeyobrien.github.io/ralph-orchestrator](https://mikeyobrien.github.io/ralph-orchestrator/)

## Contributing

Contributions are welcome! See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines and [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md) for community standards.

## License

MIT License — See [LICENSE](LICENSE) for details.

## 💬 Community & Support

Join the **ralph-orchestrator** community to discuss AI agent patterns, get help with your implementation, or contribute to the roadmap.

* **Discord**: [Join our server](https://discord.gg/XWUyeUNffh) to chat with the maintainers and other users in real-time.
* **GitHub Issues**: For bug reports and formal feature requests, please use the [Issue Tracker](https://github.com/mikeyobrien/ralph-orchestrator/issues).

## Acknowledgments

- **[Geoffrey Huntley](https://ghuntley.com/ralph/)** — Creator of the Ralph Wiggum technique
- **[Strands Agents SOP](https://github.com/strands-agents/agent-sop)** — Agent SOP framework
- **[ratatui](https://ratatui.rs/)** — Terminal UI framework

---

*"I'm learnding!" - Ralph Wiggum*
