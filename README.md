# Ralph Orchestrator

[![License](https://img.shields.io/badge/license-MIT-blue)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.75+-orange)](https://www.rust-lang.org/)
[![Build](https://img.shields.io/github/actions/workflow/status/mikeyobrien/ralph-orchestrator/ci.yml?branch=main&label=CI)](https://github.com/mikeyobrien/ralph-orchestrator/actions)
[![Coverage](https://img.shields.io/badge/coverage-65%25-yellowgreen)](coverage/index.html)
[![Mentioned in Awesome Claude Code](https://awesome.re/mentioned-badge.svg)](https://github.com/hesreallyhim/awesome-claude-code)
[![Docs](https://img.shields.io/badge/docs-mkdocs-blue)](https://mikeyobrien.github.io/ralph-orchestrator/)

A hat-based orchestration framework that keeps AI agents in a loop until the task is done.

> "Me fail English? That's unpossible!" - Ralph Wiggum

**[Documentation](https://mikeyobrien.github.io/ralph-orchestrator/)** | **[Getting Started](https://mikeyobrien.github.io/ralph-orchestrator/getting-started/quick-start/)** | **[Presets](https://mikeyobrien.github.io/ralph-orchestrator/guide/presets/)**

## Installation

### Via npm (Recommended)

```bash
npm install -g @ralph-orchestrator/ralph-cli
```

### Via Homebrew (macOS)

```bash
brew install ralph-orchestrator
```

### Via Cargo

```bash
cargo install ralph-cli
```

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

## What is Ralph?

Ralph implements the [Ralph Wiggum technique](https://ghuntley.com/ralph/) — autonomous task completion through continuous iteration. It supports:

- **Multi-Backend Support** — Claude Code, Kiro, Gemini CLI, Codex, Amp, Copilot CLI, OpenCode
- **Hat System** — Specialized personas coordinating through events
- **Backpressure** — Gates that reject incomplete work (tests, lint, typecheck)
- **Memories & Tasks** — Persistent learning and runtime work tracking
- **31 Presets** — TDD, spec-driven, debugging, and more

## Documentation

Full documentation is available at **[mikeyobrien.github.io/ralph-orchestrator](https://mikeyobrien.github.io/ralph-orchestrator/)**:

- [Installation](https://mikeyobrien.github.io/ralph-orchestrator/getting-started/installation/)
- [Quick Start](https://mikeyobrien.github.io/ralph-orchestrator/getting-started/quick-start/)
- [Configuration](https://mikeyobrien.github.io/ralph-orchestrator/guide/configuration/)
- [CLI Reference](https://mikeyobrien.github.io/ralph-orchestrator/guide/cli-reference/)
- [Presets](https://mikeyobrien.github.io/ralph-orchestrator/guide/presets/)
- [Concepts: Hats & Events](https://mikeyobrien.github.io/ralph-orchestrator/concepts/hats-and-events/)
- [Architecture](https://mikeyobrien.github.io/ralph-orchestrator/advanced/architecture/)

## Contributing

Contributions are welcome! See [CONTRIBUTING.md](CONTRIBUTING.md) for guidelines and [CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md) for community standards.

## License

MIT License — See [LICENSE](LICENSE) for details.

## Acknowledgments

- **[Geoffrey Huntley](https://ghuntley.com/ralph/)** — Creator of the Ralph Wiggum technique
- **[Strands Agents SOP](https://github.com/strands-agents/agent-sop)** — Agent SOP framework
- **[ratatui](https://ratatui.rs/)** — Terminal UI framework

---

*"I'm learnding!" - Ralph Wiggum*

```yaml
cli:
  backend: "claude"
  claude:
    plugin_dirs:
      - "~/.factory/claude/plugins/official"
    mcp_configs:
      - "~/.factory/claude/mcp-policy.json"
    allowed_tools:
      - "filesystem"
    append_system_prompts:
      - "Cite skill names explicitly."
    append_system_prompt_files:
      - "~/.factory/policy/claude-system.txt"
```
