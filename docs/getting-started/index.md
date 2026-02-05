# Getting Started

Welcome to Hats! This section will help you get up and running quickly.

## What You'll Learn

1. **[Installation](installation.md)** — Install Hats and its prerequisites
2. **[Quick Start](quick-start.md)** — Run your first Hats orchestration
3. **[Your First Task](first-task.md)** — Create and configure a real task

## Prerequisites

Before you begin, ensure you have:

- **Rust 1.75+** (if building from source)
- **At least one AI CLI tool** installed:
    - [Claude Code](https://github.com/anthropics/claude-code) (recommended)
    - [Kiro](https://kiro.dev/)
    - [Gemini CLI](https://github.com/google-gemini/gemini-cli)
    - [Codex](https://github.com/openai/codex)
    - [Amp](https://github.com/sourcegraph/amp)
    - [Copilot CLI](https://docs.github.com/copilot)
    - [OpenCode](https://opencode.ai/)

## Quick Installation

=== "npm (Recommended)"

    ```bash
    npm install -g @hats/hats-cli
    ```

=== "Homebrew (macOS)"

    ```bash
    brew install hats
    ```

=== "Cargo"

    ```bash
    cargo install hats-cli
    ```

## Verify Installation

```bash
hats --version
hats --help
```

## Next Steps

Once installed, head to the [Quick Start](quick-start.md) guide to run your first orchestration.
