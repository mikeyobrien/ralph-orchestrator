# Installation Guide

Comprehensive installation instructions for Ralph Orchestrator.

## System Requirements

### Minimum Requirements

- **Python**: 3.8 or higher
- **Memory**: 512 MB RAM
- **Disk**: 100 MB free space
- **OS**: Linux, macOS, or Windows

### Recommended Requirements

- **Python**: 3.10 or higher
- **Memory**: 2 GB RAM
- **Disk**: 1 GB free space
- **Git**: For checkpoint features
- **Network**: Stable internet connection

## Installation Methods

### Method 1: Git Clone (Recommended)

```bash
# Clone the repository
git clone https://github.com/mikeyobrien/ralph-orchestrator.git
cd ralph-orchestrator

# Make the orchestrator executable
chmod +x ralph_orchestrator.py
chmod +x ralph

# Install optional dependencies
pip install psutil  # For system metrics
```

### Method 2: Direct Download

```bash
# Download the latest release
wget https://github.com/mikeyobrien/ralph-orchestrator/archive/refs/tags/v1.0.0.tar.gz

# Extract the archive
tar -xzf v1.0.0.tar.gz
cd ralph-orchestrator-1.0.0

# Make executable
chmod +x ralph_orchestrator.py
```

### Method 3: uv tool (Recommended for Users)

If you just want to run Ralph without setting up a development environment:

```bash
# Install uv if you haven't already
curl -LsSf https://astral.sh/uv/install.sh | sh

# Install Ralph globally
uv tool install ralph-orchestrator

# Verify installation
ralph --help
```

### Method 4: pip Install

```bash
# Install via pip
pip install ralph-orchestrator
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

### Q Chat

Q Chat is a lightweight alternative agent.

```bash
# Install via pip
pip install q-cli

# Or clone from repository
git clone https://github.com/qchat/qchat.git
cd qchat
python setup.py install

# Verify installation
q --version
```

**Configuration:**
```bash
# Configure Q Chat
q config --set api_key="your-api-key"
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

## Dependency Installation

### Required Python Packages

Ralph Orchestrator has minimal dependencies, but some features require additional packages:

```bash
# Core functionality (no additional packages needed)
# Ralph uses only Python standard library for core features

# Optional: System metrics monitoring
pip install psutil

# Optional: Enhanced JSON handling
pip install orjson  # Faster JSON processing

# Optional: Development dependencies
pip install pytest pytest-cov black ruff
```

### Using requirements.txt

If you want to install all optional dependencies:

```bash
# Create requirements.txt
cat > requirements.txt << EOF
psutil>=5.9.0
orjson>=3.9.0
pytest>=7.0.0
pytest-cov>=4.0.0
black>=23.0.0
ruff>=0.1.0
EOF

# Install all dependencies
pip install -r requirements.txt
```

### Using uv (Recommended for Development)

```bash
# Install uv
curl -LsSf https://astral.sh/uv/install.sh | sh

# Install dependencies with uv
uv pip install psutil orjson

# Or use pyproject.toml
uv sync
```

## Verification

### Verify Installation

Run these commands to verify your installation:

```bash
# Check Python version
python --version  # Should be 3.8+

# Check Ralph Orchestrator
python ralph_orchestrator.py --version

# Check for available agents
python ralph_orchestrator.py --list-agents

# Run a test
echo "Say hello (orchestrator will iterate until completion)" > test.md
python ralph_orchestrator.py --prompt test.md --dry-run
```

### Expected Output

```
Ralph Orchestrator v1.0.0
Python 3.10.12
Available agents: claude, q, gemini
Dry run completed successfully
```

## Platform-Specific Instructions

### Linux

```bash
# Ubuntu/Debian
sudo apt update
sudo apt install python3 python3-pip git

# Fedora/RHEL
sudo dnf install python3 python3-pip git

# Arch Linux
sudo pacman -S python python-pip git
```

### macOS

```bash
# Using Homebrew
brew install python git

# Using MacPorts
sudo port install python310 git

# Verify Python installation
python3 --version
```

### Windows

```powershell
# Using PowerShell as Administrator

# Install Python from Microsoft Store
winget install Python.Python.3.11

# Or download from python.org
# https://www.python.org/downloads/windows/

# Install Git
winget install Git.Git

# Clone Ralph
git clone https://github.com/mikeyobrien/ralph-orchestrator.git
cd ralph-orchestrator

# Run Ralph
python ralph_orchestrator.py --prompt PROMPT.md
```

### Docker (Alternative)

```dockerfile
# Dockerfile
FROM python:3.11-slim

WORKDIR /app
COPY . /app

RUN pip install psutil

# Install your preferred AI agent
RUN npm install -g @anthropic-ai/claude-code

CMD ["python", "ralph_orchestrator.py"]
```

```bash
# Build and run
docker build -t ralph-orchestrator .
docker run -v $(pwd):/app ralph-orchestrator --prompt PROMPT.md
```

## Configuration Files

### Basic Configuration

Create a configuration file for default settings:

```bash
# Create .ralph.conf
cat > .ralph.conf << EOF
# Default Ralph Configuration
agent=claude
max_iterations=100
max_runtime=14400
checkpoint_interval=5
verbose=false
EOF
```

### Environment Variables

Set environment variables for common settings:

```bash
# Add to your ~/.bashrc or ~/.zshrc
export RALPH_AGENT="claude"
export RALPH_MAX_ITERATIONS="100"
export RALPH_MAX_COST="50.0"
export RALPH_VERBOSE="false"
```

## Troubleshooting Installation

### Common Issues

#### Python Version Too Old

```bash
ERROR: Python 3.8+ required, found 3.7.3
```

**Solution**: Upgrade Python
```bash
# Ubuntu/Debian
sudo apt install python3.10

# macOS
brew upgrade python

# Windows
winget upgrade Python.Python.3.11
```

#### Agent Not Found

```bash
ERROR: No AI agents detected
```

**Solution**: Install at least one agent
```bash
npm install -g @anthropic-ai/claude-code
# or
pip install q-cli
```

#### Permission Denied

```bash
Permission denied: './ralph_orchestrator.py'
```

**Solution**: Make executable
```bash
chmod +x ralph_orchestrator.py
chmod +x ralph
```

#### Module Not Found

```bash
ModuleNotFoundError: No module named 'psutil'
```

**Solution**: Install optional dependencies
```bash
pip install psutil
```

## Uninstallation

To remove Ralph Orchestrator:

```bash
# Remove the directory
rm -rf ralph-orchestrator

# Uninstall optional dependencies
pip uninstall psutil orjson

# Remove configuration files
rm ~/.ralph.conf
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