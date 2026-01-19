# Installation Guide

Comprehensive installation instructions for Ralph Orchestrator v2.0 (Rust).

## System Requirements

### Minimum Requirements

- **OS**: Linux, macOS, or Windows
- **Memory**: 512 MB RAM
- **Disk**: 50 MB free space

### Recommended Requirements

- **Memory**: 2 GB RAM
- **Disk**: 500 MB free space
- **Git**: For checkpoint features
- **Network**: Stable internet connection

## Installation Methods

### Method 1: Cargo Install (Recommended)

```bash
# Install from crates.io
cargo install ralph-orchestrator

# Verify installation
ralph --version
```

### Method 2: Build from Source

```bash
# Clone the repository
git clone https://github.com/mikeyobrien/ralph-orchestrator.git
cd ralph-orchestrator

# Build and install
cargo build --release
cargo install --path .

# Verify installation
ralph --version
```

### Method 3: Download Binary

Download pre-built binaries from the [releases page](https://github.com/mikeyobrien/ralph-orchestrator/releases).

```bash
# macOS (Apple Silicon)
curl -L https://github.com/mikeyobrien/ralph-orchestrator/releases/latest/download/ralph-aarch64-apple-darwin.tar.gz | tar xz
mv ralph /usr/local/bin/

# macOS (Intel)
curl -L https://github.com/mikeyobrien/ralph-orchestrator/releases/latest/download/ralph-x86_64-apple-darwin.tar.gz | tar xz
mv ralph /usr/local/bin/

# Linux (x86_64)
curl -L https://github.com/mikeyobrien/ralph-orchestrator/releases/latest/download/ralph-x86_64-unknown-linux-gnu.tar.gz | tar xz
mv ralph /usr/local/bin/

# Verify installation
ralph --version
```

## AI Agent Installation

Ralph requires at least one AI agent to function. Choose and install one or more:

### Claude (Anthropic)

Claude is the recommended agent for most use cases.

```bash
# Install via npm
npm install -g @anthropic-ai/claude-code

# Or download from
# https://claude.ai/code

# Verify installation
claude --version
```

**Configuration:**
```bash
# Set your API key (if required)
export ANTHROPIC_API_KEY="your-api-key-here"
```

### Kiro (AWS)

Kiro is an alternative agent with AWS integration.

```bash
# Install via npm
npm install -g @anthropic-ai/kiro-cli

# Verify installation
kiro --version
```

### Gemini (Google)

Gemini provides access to Google's AI models.

```bash
# Install via npm
npm install -g @google/gemini-cli

# Verify installation
gemini --version
```

**Configuration:**
```bash
# Set your API key
export GEMINI_API_KEY="your-api-key-here"

# Or use config file
gemini config set api_key "your-api-key"
```

## Verification

### Verify Installation

Run these commands to verify your installation:

```bash
# Check Ralph version
ralph --version

# List available commands
ralph --help

# Run a dry-run test
ralph run --dry-run -p "Say hello"
```

### Expected Output

```
ralph 2.0.0
```

## Platform-Specific Instructions

### Linux

```bash
# Ubuntu/Debian - Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env

# Install Ralph
cargo install ralph-orchestrator
```

### macOS

```bash
# Install Rust via Homebrew or rustup
brew install rustup
rustup-init

# Or use rustup directly
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Install Ralph
cargo install ralph-orchestrator
```

### Windows

```powershell
# Using PowerShell as Administrator

# Install Rust from https://rustup.rs
# Download and run rustup-init.exe

# Install Ralph
cargo install ralph-orchestrator

# Verify installation
ralph --version
```

### Docker (Alternative)

```dockerfile
# Dockerfile
FROM rust:1.75-slim AS builder

WORKDIR /app
COPY . /app

RUN cargo build --release

FROM debian:bookworm-slim
COPY --from=builder /app/target/release/ralph /usr/local/bin/

# Install your preferred AI agent
RUN npm install -g @anthropic-ai/claude-code

CMD ["ralph", "run"]
```

```bash
# Build and run
docker build -t ralph-orchestrator .
docker run -v $(pwd):/app ralph-orchestrator run -p "Your prompt"
```

## Configuration Files

### Basic Configuration

Create a configuration file for your project:

```bash
# Create ralph.yml
cat > ralph.yml << EOF
cli:
  backend: claude

limits:
  max_iterations: 100
  max_runtime: 14400
EOF
```

### Environment Variables

Set environment variables for common settings:

```bash
# Add to your ~/.bashrc or ~/.zshrc
export RALPH_BACKEND="claude"
export RALPH_MAX_ITERATIONS="100"
export RALPH_MAX_COST="50.0"
```

## Troubleshooting Installation

### Common Issues

#### Rust Not Installed

```bash
error: command 'cargo' not found
```

**Solution**: Install Rust
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env
```

#### Agent Not Found

```bash
ERROR: No AI agents detected
```

**Solution**: Install at least one agent
```bash
npm install -g @anthropic-ai/claude-code
# or
npm install -g @google/gemini-cli
```

#### Permission Denied

```bash
Permission denied: 'ralph'
```

**Solution**: Ensure cargo bin is in PATH
```bash
export PATH="$HOME/.cargo/bin:$PATH"
```

## Uninstallation

To remove Ralph Orchestrator:

```bash
# Remove via cargo
cargo uninstall ralph-orchestrator

# Remove configuration files (optional)
rm -rf ~/.config/ralph
rm ralph.yml
```

## Next Steps

After installation:

1. Read the [Quick Start Guide](quick-start.md)
2. Configure your [AI Agents](guide/agents.md)
3. Learn about [Configuration Options](guide/configuration.md)
4. Try the [Examples](examples/index.md)

## Getting Help

If you encounter issues:

- Check the [FAQ](faq.md)
- Read [Troubleshooting](troubleshooting.md)
- Open an [issue on GitHub](https://github.com/mikeyobrien/ralph-orchestrator/issues)
- Join the [discussions](https://github.com/mikeyobrien/ralph-orchestrator/discussions)

---

📚 Continue to the [User Guide](guide/overview.md) →
