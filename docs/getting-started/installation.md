# Installation

This guide covers all installation methods for Ralph Orchestrator.

## Prerequisites

### AI CLI Tools

Ralph needs at least one AI CLI tool to function. Install one of the following:

=== "Claude Code (Recommended)"

    ```bash
    # Via npm
    npm install -g @anthropic-ai/claude-code

    # Or visit https://claude.ai/code for setup instructions
    ```

=== "Kiro"

    ```bash
    # Visit https://kiro.dev/ for installation
    ```

=== "Gemini CLI"

    ```bash
    npm install -g @google/gemini-cli
    ```

=== "Codex"

    ```bash
    # Visit https://github.com/openai/codex
    ```

=== "Amp"

    ```bash
    # Visit https://github.com/sourcegraph/amp
    ```

=== "Copilot CLI"

    ```bash
    npm install -g @github/copilot
    ```

=== "OpenCode"

    ```bash
    curl -fsSL https://opencode.ai/install | bash
    ```

## Installing Ralph

### Via npm (Recommended)

The easiest way to install Ralph:

```bash
# Install globally
npm install -g @ralph-orchestrator/ralph-cli

# Or run directly with npx
npx @ralph-orchestrator/ralph-cli --version
```

### Via Homebrew (macOS)

```bash
brew install ralph-orchestrator
```

### Via Cargo

If you have Rust installed:

```bash
cargo install ralph-cli
```

### From Source

For the latest development version:

```bash
# Clone the repository
git clone https://github.com/mikeyobrien/ralph-orchestrator.git
cd ralph-orchestrator

# Build release binary
cargo build --release

# Add to PATH
export PATH="$PATH:$(pwd)/target/release"

# Or create symlink
sudo ln -s $(pwd)/target/release/ralph /usr/local/bin/ralph
```

## Verify Installation

```bash
# Check version
ralph --version

# Show help
ralph --help

# List available presets
ralph init --list-presets
```

## Migrating from v1 (Legacy)

If you have the legacy Ralph v1 installed, uninstall it first:

```bash
# If installed via pip
pip uninstall ralph-orchestrator

# If installed via pipx
pipx uninstall ralph-orchestrator

# If installed via uv
uv tool uninstall ralph-orchestrator

# Verify removal
which ralph  # Should return nothing or point to new Rust version
```

The v1 release is no longer maintained. See [Migration from v1](../reference/migration-v1.md) for details.

## Troubleshooting

### Command Not Found

If `ralph` is not found after installation:

```bash
# For npm global installs, ensure npm bin is in PATH
export PATH="$PATH:$(npm config get prefix)/bin"

# For cargo installs
export PATH="$PATH:$HOME/.cargo/bin"
```

### No AI Agents Detected

Ralph auto-detects available AI CLI tools. If none are found:

1. Install one of the supported AI CLI tools (see Prerequisites)
2. Ensure the tool is in your PATH
3. Try running the AI CLI directly to verify it works

### Permission Denied

If you get permission errors:

```bash
# For npm
sudo npm install -g @ralph-orchestrator/ralph-cli

# For symlinks
sudo ln -s $(pwd)/target/release/ralph /usr/local/bin/ralph
```

## Next Steps

Now that Ralph is installed, proceed to the [Quick Start](quick-start.md) guide.
