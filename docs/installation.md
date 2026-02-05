# Installation Guide

Comprehensive installation instructions for Hats.

## Prerequisites

- **OS**: macOS, Linux, or Windows
- **Node.js**: 18+ (required for npm installs)
- **Rust**: 1.70+ (required for cargo installs)
- **Homebrew**: required for the Homebrew method

## Installation Methods

### Method 1: npm (Recommended)

```bash
npm install -g @hats/hats-cli
```

### Method 2: Homebrew (macOS/Linux)

```bash
brew install hats
```

### Method 3: Cargo

```bash
cargo install hats-cli
```

### Method 4: Prebuilt Binary (cargo-dist)

Download the latest release artifact for your OS/arch from GitHub Releases (built with cargo-dist), then place it on your PATH.

```bash
# Example (replace with the correct archive for your platform)
mkdir -p ~/bin
curl -L -o hats.tar.gz "<release-archive-url>"
tar -xzf hats.tar.gz
mv hats ~/bin/
export PATH="$HOME/bin:$PATH"
```

## Verify Installation

```bash
hats --version
```

## Next Steps

- Install at least one supported AI backend CLI (Claude Code, Gemini CLI, Copilot CLI, etc.)
- Configure your backend API keys or auth
- Follow the quick start guide: `getting-started/quick-start.md`
