<!-- 2026-01-28 -->
# Ralph Orchestrator

[![License](https://img.shields.io/badge/license-MIT-blue)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-stable-orange)](https://www.rust-lang.org/)
[![Build](https://img.shields.io/github/actions/workflow/status/mikeyobrien/ralph-orchestrator/ci.yml?branch=main&label=CI)](https://github.com/mikeyobrien/ralph-orchestrator/actions)
[![Coverage](https://img.shields.io/endpoint?url=https://mikeyobrien.github.io/ralph-orchestrator/badges/coverage.json)](CONTRIBUTING.md#coverage)
[![Mentioned in Awesome Claude Code](https://awesome.re/mentioned-badge.svg)](https://github.com/hesreallyhim/awesome-claude-code)
[![Docs](https://img.shields.io/badge/docs-mkdocs-blue)](https://mikeyobrien.github.io/ralph-orchestrator/)
[![Discord](https://img.shields.io/discord/1482421188700667906?label=Discord&logo=discord&logoColor=white)](https://discord.gg/XWUyeUNffh)

A terminal frontend and coordination plane for autonomous coding loops powered by autoloop.

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

Ralph requires autoloop >= 0.10.0. The recommended npm installation installs
it automatically as a dependency. After installing Ralph via Cargo or the
GitHub Releases installer, just run `ralph run`: on the first interactive run,
Ralph offers to download the pinned standalone engine. For CI and other
non-interactive environments, opt in to first-run provisioning explicitly:

```bash
RALPH_AUTO_INSTALL_ENGINE=1 ralph run -p "your task"
```

You can instead install the npm engine yourself with
`npm install -g @mobrienv/autoloop`.

### No-Node install (vendored engine)

To install the standalone engine manually before the first run:

```bash
ralph doctor --install-engine
```

Ralph downloads the engine, verifies its SHA-256 checksum, and installs the
executable at `~/.ralph/engine/autoloop`. Set `RALPH_ENGINE_DIR` to install and
resolve the executable from another directory:

```bash
RALPH_ENGINE_DIR=/path/to/engine ralph doctor --install-engine
```

This global executable location is distinct from runtime state. For each
project, Ralph launches autoloop with a Ralph-owned state root at
`<workspace>/.ralph/autoloop`; users do not need to configure that path.

> Homebrew is not currently published from this repository's automated release flow. Prefer npm, Cargo, or the GitHub Releases installer.

## Quick Start

```bash
# 1. Initialize Ralph with your preferred backend
ralph init --backend claude

# 2. Plan your feature (interactive PDD session)
ralph plan "Add user authentication with JWT"
# The session writes its approved design and plan under .agents/planning/ by default.

# 3. Implement the approved plan
ralph run -p "Implement the approved authentication plan under .agents/planning/"
```

`ralph run` launches autoloop as the execution engine. Autoloop owns iteration
and completion judgment; Ralph observes its journal, event stream, and summary,
then coordinates the TUI, loop registry, worktrees, and merge queue.

For simpler tasks, skip planning and run directly:

```bash
ralph run -p "Add input validation to the /users endpoint"
```

## Web Dashboard (Alpha)

> **Alpha:** The web dashboard is under active development. Expect rough edges and breaking changes.
>
> **Live-state limitation:** The dashboard does **not** render live loop state
> under the v3 autoloop engine yet. Porting the autoloop event parser is tracked
> by `ga3-c4-dashboard-dead-svf`.

<img width="1513" height="1128" alt="image" src="https://github.com/user-attachments/assets/ce5f072f-3d81-44d8-8f2f-88b42b33a3be" />

Ralph retains the alpha web dashboard while its v3 live-state parser integration is pending.

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
- Node.js >= 22 + npm (for the frontend)

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

Ralph applies the [Ralph Wiggum technique](https://ghuntley.com/ralph/) as the
terminal frontend and observation/coordination plane around autoloop. Autoloop
is the execution engine and owns completion judgment; Ralph provides:

- **Multi-Backend Support** — Claude Code, Kiro, Gemini CLI, Codex, Forge, Amp, Copilot CLI, OpenCode
- **Hat System** — Specialized personas translated into autoloop roles and event routing
- **Observation** — TUI views over autoloop's journal, event stream, and summary contracts
- **Coordination** — Loop registry, worktrees, merge queue, and completion bookkeeping
- **Memories & Tasks** — Persistent learning and runtime work tracking
- **5 Supported Builtins** — `code-assist`, `debug`, `research`, `review`, and `pdd-to-code-assist`, with more patterns documented as examples

## RObot (Human-in-the-Loop)

**Telegram HITL is currently inactive for `ralph run` under the autoloop engine,
pending autoloop#345.**
The retained `ralph bot` commands can configure and test Telegram, but agent
questions and proactive guidance are not yet relayed into an autoloop-backed
run.

```bash
ralph bot onboard              # guided setup (token + chat id)
ralph bot status               # verify config
ralph bot test                 # send a test message
```

See the [Telegram guide](https://mikeyobrien.github.io/ralph-orchestrator/guide/telegram/)
for bot setup. Its in-loop relay workflow remains unavailable under autoloop
pending #345.

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
Ralph is the terminal frontend and observation/coordination plane for the
autoloop engine. It exposes hat-based workflows, TUI observation, persistent
state, worktrees, and merge coordination while autoloop executes roles and
judges completion.

**How is Ralph different from other AI coding tools?**
Ralph coordinates repeated, role-based agent work rather than making a
single-shot request. Backpressure and completion remain engine judgments;
Ralph observes their results and coordinates the surrounding developer
workflow.

### Installation & Setup

**What are the system requirements?**
- Current stable Rust (Edition 2024 workspace; no explicit MSRV is declared)
- Node.js >= 22 + npm (for the web dashboard frontend)
- An AI coding assistant CLI (Claude Code, Codex, Gemini CLI, etc.)

**Which installation method should I use?**
- **npm** (recommended for most users): `npm install -g @ralph-orchestrator/ralph-cli`
- **Cargo**: `cargo install ralph-cli` (best for Rust developers)
- **GitHub Releases installer**: One-link install with `curl ... | sh`

Cargo and installer users can run `ralph run` immediately; the first
interactive run offers to provision the autoloop engine. In CI or another
non-interactive environment, set `RALPH_AUTO_INSTALL_ENGINE=1` to opt in.

**Is Homebrew supported?**
Homebrew is not currently published from this repository's automated release flow. Prefer npm, Cargo, or the GitHub Releases installer.

### Usage

**How do I start a new project with Ralph?**
```bash
ralph init --backend claude
ralph plan "Add user authentication with JWT"
ralph run -p "Implement the approved authentication plan under .agents/planning/"
```

**What backends does Ralph support?**
Claude Code, Kiro, Gemini CLI, Codex, Forge, Amp, Copilot CLI, and OpenCode.

**What is the "hat system"?**
Ralph uses specialized personas (hats) that coordinate through events. Each hat has a specific role — code-assist, debug, research, review, and pdd-to-code-assist — enabling structured multi-step task execution.

### RObot (Human-in-the-Loop)

**What is RObot?**
RObot is the retained Telegram integration surface. Telegram relay into
`ralph run` is inactive under the autoloop engine pending autoloop#345.

**Can I configure Telegram now?**
Yes. The setup and test commands remain available, but they do not enable
in-loop questions or proactive guidance for autoloop-backed runs:
```bash
ralph bot onboard              # guided setup
ralph bot status               # verify config
ralph bot test                 # send a test message
```

### Web Dashboard

**How do I access the web dashboard?**
Run `ralph web` to start the Rust RPC API + frontend and open your browser. The
dashboard is currently in Alpha and does **not** render live loop state under
the v3 autoloop engine yet; the parser port is tracked by
`ga3-c4-dashboard-dead-svf`.

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
