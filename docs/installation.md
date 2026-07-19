# Installation Guide

Comprehensive installation instructions for Ralph Orchestrator.

## Prerequisites

- **OS**: macOS, Linux, or Windows
- **Node.js**: 22+ (required for npm installs and the web dashboard)
- **Rust**: current stable (Edition 2024; the workspace does not declare an explicit MSRV)

## Installation Methods

### Method 1: npm (Recommended)

```bash
npm install -g @ralph-orchestrator/ralph-cli
```

### Method 2: GitHub Releases installer

```bash
curl --proto '=https' --tlsv1.2 -LsSf \
  https://github.com/mikeyobrien/ralph-orchestrator/releases/latest/download/ralph-cli-installer.sh | sh
```

### Method 3: Cargo

```bash
cargo install ralph-cli
```

### Method 4: Prebuilt Binary (cargo-dist)

Download the latest `ralph-cli-<target>.tar.xz` artifact from GitHub Releases, extract it, then place `ralph` on your PATH.

```bash
# Example (replace with the correct archive for your platform)
mkdir -p ~/bin
curl -L -o ralph.tar.xz "<release-archive-url>"
tar -xJf ralph.tar.xz
mv ralph ~/bin/
export PATH="$HOME/bin:$PATH"
```

> Homebrew is not currently published from this repository's automated release flow.

## Autoloop dependency

Ralph requires autoloop >= 0.10.0. The npm method installs autoloop
automatically as a dependency. For the GitHub Releases installer, Cargo, or a
prebuilt binary, install it separately:

```bash
npm install -g @mobrienv/autoloop
```

### No-Node install (vendored engine)

If Node.js is unavailable, install Ralph via Cargo, the GitHub Releases
installer, or a prebuilt binary, then install the pinned standalone autoloop
engine:

```bash
ralph doctor --install-engine
```

Ralph downloads the engine, verifies its SHA-256 checksum, and installs it at
`~/.ralph/engine/autoloop`. Set `RALPH_ENGINE_DIR` to install and resolve the
engine from another directory:

```bash
RALPH_ENGINE_DIR=/path/to/engine ralph doctor --install-engine
```

## Verify Installation

```bash
ralph --version
```

## Next Steps

- Install at least one supported AI backend CLI (Claude Code, Gemini CLI, Forge, Copilot CLI, etc.)
- Configure your backend API keys or auth
- Follow the quick start guide: `getting-started/quick-start.md`
