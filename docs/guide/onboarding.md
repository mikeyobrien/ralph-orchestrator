# Project Onboarding

## Overview

The `ralph onboard` command analyzes existing Claude Code projects and generates optimized RALPH configuration. It learns from your conversation history, MCP configurations, and project metadata to eliminate manual setup.

## Quick Start

```bash
# Onboard the current project (uses intelligent analysis)
ralph onboard

# Onboard a specific project
ralph onboard ~/projects/my-app

# Preview what would be generated (no files written)
ralph onboard --analyze-only

# Use offline mode (no API calls)
ralph onboard --static
```

## How It Works

The onboarding process follows five steps:

```
┌─────────────────────────────────────────────────────────────┐
│                    ralph onboard                            │
├─────────────────────────────────────────────────────────────┤
│  1. Load Settings                                           │
│     • ~/.claude/settings.json (user MCP servers)            │
│     • [project]/.mcp.json (project MCP servers)             │
├─────────────────────────────────────────────────────────────┤
│  2. Scan Project                                            │
│     • Detect project type (Python, Node.js, Rust, etc.)     │
│     • Find Claude Code conversation history                 │
│     • Locate CLAUDE.md files                                │
├─────────────────────────────────────────────────────────────┤
│  3. Analyze                                                 │
│     • Agent mode: Claude analyzes using your MCPs           │
│     • Static mode: Parse JSONL history directly             │
├─────────────────────────────────────────────────────────────┤
│  4. Extract Patterns                                        │
│     • Identify successful workflows                         │
│     • Calculate tool success rates                          │
│     • Find common tool chains                               │
├─────────────────────────────────────────────────────────────┤
│  5. Generate Configuration                                  │
│     • ralph.yml (orchestrator settings)                     │
│     • RALPH_INSTRUCTIONS.md (learned patterns)              │
│     • PROMPT.md (task template)                             │
└─────────────────────────────────────────────────────────────┘
```

## Command Reference

### Basic Usage

```bash
ralph onboard [PROJECT_PATH] [OPTIONS]
```

**Arguments:**
- `PROJECT_PATH` - Path to the project to onboard (default: current directory)

### Analysis Mode Options

| Option | Description |
|--------|-------------|
| `--agent` | Use Claude agent for intelligent analysis (default) |
| `--static` | Use static analysis only (parse JSONL directly, no API calls) |
| `--use-memory` | Explicitly use mcp-memory/episodic memory for deeper analysis |

### Settings Options

| Option | Description |
|--------|-------------|
| `--inherit-settings` | Load user's `~/.claude/settings.json` (default) |
| `--no-inherit` | Don't inherit user settings (isolated analysis) |

### Output Options

| Option | Description |
|--------|-------------|
| `-o, --output-dir` | Output directory for generated files (default: project root) |
| `-a, --analyze-only` | Show analysis without generating files |
| `--merge` | Merge with existing ralph.yml instead of overwriting |
| `--dry-run` | Preview changes without writing files |
| `-v, --verbose` | Show detailed analysis output |

## Analysis Modes

### Agent-Assisted Analysis (Default)

```bash
ralph onboard ~/my-project
```

Agent-assisted analysis runs Claude with your MCP servers to intelligently analyze the project. This provides the best results because Claude can:

- Use your mcp-memory to recall patterns from past sessions
- Leverage semantic search (fogmap, etc.) to find relevant workflows
- Query your GitHub MCP for commit patterns
- Understand project structure with file-system MCPs

**Requirements:**
- Valid Anthropic API key or Claude Code installation
- Network access for API calls

### Static Analysis (Offline)

```bash
ralph onboard --static
```

Static analysis parses files directly without running an agent:

- Parses JSONL conversation files from `~/.claude/projects/`
- Extracts tool usage statistics
- Identifies tool chains and workflows
- No API calls required

**Best for:**
- CI/CD environments
- Limited API access
- Faster analysis (no API latency)
- Air-gapped environments

## Generated Files

### 1. ralph.yml

The main configuration file for RALPH Orchestrator:

```yaml
# Auto-generated by: ralph onboard
# Project: my-expo-app
# Generated: 2025-01-15

agent: claude
prompt_file: PROMPT.md
max_iterations: 75  # Based on average session length

# Resource limits (from usage patterns)
max_tokens: 500000
max_cost: 25.0
context_window: 200000

# Features
archive_prompts: true
git_checkpoint: true
verbose: false

# Learned patterns
adapters:
  claude:
    enabled: true
    timeout: 600  # Extended for long-running builds
    tool_permissions:
      allow_all: true
      # Frequently used tools (from history):
      # - Read, Write, Edit (98% success rate)
      # - Bash (npx expo start, npm test)
      # - mcp_github (commits, branches)

# Project-specific notes (from CLAUDE.md):
# - Uses Expo for mobile development
# - Test with: npm test
# - Build with: npx expo build
```

### 2. RALPH_INSTRUCTIONS.md

Learned patterns and workflows:

```markdown
# RALPH Instructions for my-expo-app

## Project Context
This is an Expo/React Native mobile application.

## Proven Workflows

### Development Cycle
1. Make code changes
2. Run `npx expo start` to test
3. Check iOS Simulator for visual feedback
4. Run `npm test` to verify
5. Commit with descriptive message

### Common Tools
- **File Operations**: Read, Write, Edit (prefer Edit for small changes)
- **Terminal**: Bash for npm/npx commands
- **MCP Servers**:
  - mcp_github: For version control operations
  - mcp_ios-simulator: For mobile testing

### Project-Specific Commands
- Start dev server: `npx expo start`
- Run tests: `npm test`
- Build iOS: `npx expo run:ios`
- Build Android: `npx expo run:android`

### Learned Preferences
- Use TypeScript strict mode
- Follow React hooks patterns
- Prefer functional components
```

### 3. PROMPT.md

A template for your tasks:

```markdown
# Task: [Your task description]

## Project Context
<!-- Auto-detected by onboarding -->
- **Type**: Expo/React Native Mobile App
- **Language**: TypeScript
- **Key Frameworks**: Expo, React Navigation, NativeWind

## Available Tools
Based on your project's history, these tools are most effective:
- File editing: Edit, Write, Read
- Commands: `npx expo start`, `npm test`
- MCP: mcp_github, mcp_ios-simulator

## Requirements
- [ ] Requirement 1
- [ ] Requirement 2

## Success Criteria
- [ ] TASK_COMPLETE when all requirements met
```

## Data Sources

The onboarding command analyzes several data sources:

### 1. Claude Code Conversation History

**Location:** `~/.claude/projects/[project-hash]/*.jsonl`

Extracts:
- Tool usage frequency and success rates
- MCP server invocations
- Tool chains (sequences used together)
- Common workflows

### 2. Project Instructions

**Locations:**
- `[project]/CLAUDE.md` - Project-level instructions
- `[project]/.claude/CLAUDE.md` - Hidden project instructions
- `[project]/.claude/rules/*.md` - Modular rule files
- `~/.claude/CLAUDE.md` - User-global instructions

### 3. MCP Server Configuration

**Locations:**
- `[project]/.mcp.json` - Project-scoped MCP servers
- `~/.claude/settings.json` - User-scoped MCP servers

### 4. Project Metadata

Detects project type from:
- `package.json` - Node.js/JavaScript/TypeScript
- `pyproject.toml` / `setup.py` - Python
- `Cargo.toml` - Rust
- `go.mod` - Go
- `pubspec.yaml` - Flutter/Dart
- `app.json` / `expo.json` - Expo/React Native
- `Podfile` - iOS
- `build.gradle` - Android/Java

## Supported Project Types

| Type | Detection File | Auto-Detected Patterns |
|------|----------------|----------------------|
| Python | `pyproject.toml`, `setup.py`, `requirements.txt` | pytest, uvicorn, pip commands |
| Node.js | `package.json` | npm/yarn commands, jest tests |
| TypeScript | `package.json` with TypeScript | tsc, ts-node patterns |
| Rust | `Cargo.toml` | cargo build/test/run |
| Go | `go.mod` | go build/test/run |
| Expo | `app.json`, `expo.json` | expo start/build, simulator |
| Flutter | `pubspec.yaml` | flutter run/test/build |
| iOS | `Podfile`, `*.xcodeproj` | xcodebuild, simulator |
| Android | `build.gradle` | gradle tasks |

## Supported Analysis Plugins

When using agent-assisted analysis, RALPH can leverage these MCP servers if installed:

| Plugin | Purpose | How It's Used |
|--------|---------|---------------|
| `mcp-memory` / `mem0` | Episodic memory across sessions | Query for project patterns and successful workflows |
| `fogmap` | Semantic search over conversations | Find relevant past solutions by meaning |
| `graphiti-mcp` | Knowledge graph extraction | Extract entity relationships from history |
| `claude-code-history-viewer` | Structured history browsing | Navigate and filter conversation data |
| `mcp-github` | GitHub integration | Analyze commit patterns and workflows |
| `mcp-filesystem` | File system access | Efficiently scan project structure |

The onboarding command automatically detects which plugins are available and uses them when beneficial.

## Examples

### Example 1: Expo Mobile App

```bash
$ ralph onboard ~/projects/my-expo-app

🔍 Scanning project: ~/projects/my-expo-app
   ✓ Found package.json (Expo project detected)
   ✓ Found .claude/CLAUDE.md
   ✓ Found 47 conversation file(s) in Claude history

📊 Analyzing conversation history...
   ✓ Parsed 892 unique tools
   ✓ Found 15 MCP server(s) used in history
   ✓ Identified 23 tool chains

🎯 Extracting patterns...
   ✓ Found 5 workflow patterns
   ✓ Top tools: Edit, Bash, Read, Write, Grep

📝 Generating configuration files...
────────────────────────────────────────
✓ Onboarding complete!
Generated files in: /Users/nick/projects/my-expo-app
  - ralph.yml (configuration)
  - RALPH_INSTRUCTIONS.md (learned patterns)
  - PROMPT.md (task template)

Next steps:
  1. Edit PROMPT.md with your task description
  2. Run 'ralph run' to start the orchestrator
```

### Example 2: Python API with No History

```bash
$ ralph onboard ~/projects/fastapi-backend --static

🔍 Scanning project: ~/projects/fastapi-backend
   ✓ Found pyproject.toml (Python project detected)
   ✓ Found requirements.txt
   ⚠ No Claude Code conversation history found

📊 Using static analysis (parsing JSONL files directly)...
   ⚠ No history files to analyze, using defaults

🎯 Extracting patterns...
   ✓ Found 0 workflow patterns (using project defaults)

📝 Generating configuration files...
────────────────────────────────────────
✓ Onboarding complete!
Generated files in: /Users/nick/projects/fastapi-backend
```

### Example 3: Preview Mode

```bash
$ ralph onboard --analyze-only

🔍 Scanning project: .
   ✓ Found package.json (Node.js project detected)
   ✓ Found 12 conversation file(s) in Claude history

📊 Using agent-assisted analysis...
   ✓ Agent analysis completed successfully

🎯 Extracting patterns...
   ✓ Found 3 workflow patterns
   ✓ Top tools: Edit, Read, Bash

────────────────────────────────────────
Analysis Results (--analyze-only)

ralph.yml preview:
────────────────────────────────────────
# Auto-generated by: ralph onboard
# Project: .
agent: claude
prompt_file: PROMPT.md
max_iterations: 50
...
────────────────────────────────────────

RALPH_INSTRUCTIONS.md preview:
────────────────────────────────────────
# RALPH Instructions for .
...
```

## Graceful Degradation

The onboarding command works at various levels of data availability:

| Available Data | Quality | Notes |
|----------------|---------|-------|
| Full MCP ecosystem + history | Best | Agent uses memory, semantic search |
| Just JSONL history | Good | Parses tool usage, workflows |
| Just project metadata | Basic | Detects type, infers commands |
| Nothing | Defaults | Sensible defaults for project type |

## Privacy & Security

- All analysis happens locally
- Conversation content is analyzed but not stored externally
- MCP servers run in your environment with your permissions
- Generated configs contain patterns, not sensitive content
- No data is sent to external services (except Claude API for agent mode)

## Troubleshooting

### No conversation history found

Claude Code stores history in `~/.claude/projects/`. If no history is found:

1. Verify you've used Claude Code in this project
2. Check the project path matches what Claude Code sees
3. Use `--static` mode as a fallback

### Agent analysis fails

If agent-assisted analysis fails:

1. RALPH automatically falls back to static analysis
2. Check your Anthropic API key is valid
3. Verify network connectivity
4. Use `--static` flag to skip agent mode entirely

### MCP servers not detected

Ensure your MCP configuration is in one of these locations:
- `~/.claude/settings.json` (user-level)
- `[project]/.mcp.json` (project-level)

### Generated config seems incomplete

Try:
1. Run without `--static` to use agent analysis
2. Ensure you have conversation history for this project
3. Check that CLAUDE.md files exist and contain project context

## Next Steps

After onboarding:

1. **Review generated files** - Check ralph.yml and RALPH_INSTRUCTIONS.md for accuracy
2. **Edit PROMPT.md** - Add your specific task description
3. **Run the orchestrator** - Execute `ralph run` to start
4. **Iterate on configuration** - Adjust settings based on results

See also:
- [Configuration](configuration.md) - Full ralph.yml reference
- [Prompts](prompts.md) - Writing effective prompts
- [Cost Management](cost-management.md) - Budget controls
