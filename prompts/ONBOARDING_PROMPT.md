# Task: Intelligent Project Onboarding & Pattern Analysis

Build a system that enables ANY user of RALPH Orchestrator to onboard their existing Claude Code projects. The system analyzes conversation history, MCP configurations, and successful workflows to generate optimized RALPH configuration - eliminating manual setup.

## The Problem

Users have been working in Claude Code locally, accumulating:
- Conversation history with proven tool usage patterns
- MCP server configurations that work for their projects
- CLAUDE.md files with project-specific instructions
- Workflows that consistently succeed (test → fix → commit cycles)

When they want to use RALPH Orchestrator, they must manually configure everything from scratch. This feature closes that gap by learning from their Claude Code history.

## Objective

Create a `ralph onboard` CLI command that:
1. **Scans** a project directory and Claude Code's data stores
2. **Analyzes** conversation history to extract successful patterns
3. **Synthesizes** optimized RALPH configuration
4. **Generates** ralph.yml and enhanced prompts based on proven workflows

---

## Philosophy: Use Their Tools to Analyze Their Tools

The onboarding system should **leverage the user's existing Claude Code ecosystem** to perform the analysis itself. Instead of just reading config files, we inherit and use:

### Inherit User's MCP Servers
When running `ralph onboard`, we load the user's `~/.claude/settings.json` MCP servers. This means if they have:
- **mcp-memory** → Use it to query episodic memory of past sessions
- **mcp-fogmap** / semantic search → Use it to find relevant conversation patterns
- **mcp-github** → Analyze their commit patterns and workflows
- **mcp-filesystem** → Efficiently scan project structure

The ClaudeAdapter already supports `inherit_user_settings=True` which loads `setting_sources: ['user', 'project', 'local']`. The onboarding process runs Claude with these settings active.

### Leverage Existing Analysis Plugins
Many users have installed MCP servers specifically for conversation analysis:

| Plugin | Purpose | How We Use It |
|--------|---------|---------------|
| `mcp-memory` / `mem0` | Episodic memory across sessions | Query for project patterns, successful workflows |
| `claude-code-history-viewer` | Browses conversation history | Extract structured conversation data |
| `fogmap` / semantic search | Search conversations by meaning | Find relevant past solutions |
| `graphiti-mcp` | Knowledge graph from conversations | Extract entity relationships |

If these are installed, the onboarding agent can use them. If not, we fall back to direct JSONL parsing.

### SDK-Ready Architecture
Anthropic will likely release proper APIs for:
- Accessing conversation history programmatically
- Querying MCP server capabilities
- Reading user preferences and settings

Our architecture should be **SDK-ready**:
```python
class HistoryAnalyzer:
    def __init__(self, project_path: Path):
        self.project_path = project_path
        self._sdk_client = None  # Future: Anthropic SDK client
        
    async def analyze(self) -> AnalysisResult:
        # Future: Use SDK when available
        if self._sdk_client:
            return await self._analyze_via_sdk()
        
        # Current: Use MCP tools if available
        if await self._has_memory_mcp():
            return await self._analyze_via_memory_mcp()
        
        # Fallback: Direct JSONL parsing
        return await self._analyze_via_jsonl()
```

### Two-Mode Operation

**Mode 1: Agent-Assisted Analysis (Recommended)**
```bash
ralph onboard ~/my-project --agent
```
Runs Claude with the user's MCP servers to intelligently analyze the project. Claude uses episodic memory, semantic search, and file tools to understand patterns.

**Mode 2: Static Analysis (Offline)**
```bash
ralph onboard ~/my-project --static
```
Parses files directly without running an agent. Faster but less intelligent. Good for CI/CD or when API access is limited.

---

## Data Sources to Analyze

### 1. Claude Code Conversation History
**Location**: `~/.claude/projects/[project-hash]/*.jsonl`

Each JSONL file contains conversation messages with:
```jsonl
{"type": "user", "content": [...], "timestamp": "..."}
{"type": "assistant", "content": [{"type": "text", "text": "..."}, {"type": "tool_use", "name": "...", "input": {...}, "id": "..."}]}
{"type": "user", "content": [{"type": "tool_result", "tool_use_id": "...", "content": "...", "is_error": false}]}
```

**Extract**:
- Tool usage frequency and success rates
- MCP server invocations (tools prefixed with `mcp_*`)
- Tool chains (sequences of tools used together)
- Common workflows and patterns

### 2. Project Instructions
**Locations**:
- `[project]/CLAUDE.md` - Project-level instructions
- `[project]/.claude/CLAUDE.md` - Hidden project instructions
- `[project]/.claude/rules/*.md` - Modular rule files
- `~/.claude/CLAUDE.md` - User-global instructions

**Extract**:
- Coding standards and conventions
- Project-specific workflows
- Framework guidelines
- Testing requirements

### 3. MCP Server Configuration
**Locations**:
- `[project]/.mcp.json` - Project-scoped MCP servers
- `~/.claude/settings.json` - User-scoped MCP servers
- Output of `claude mcp list` - Active servers

**Extract**:
- Which MCP servers are configured
- Server commands and arguments
- Environment variables needed

### 4. Full Claude Code Settings
**Location**: `~/.claude/settings.json`

This file contains the user's complete Claude Code configuration:
```json
{
  "mcpServers": {
    "github": { "command": "github-mcp-server", "args": ["stdio"] },
    "memory": { "command": "mcp-memory", "args": [] },
    "filesystem": { "command": "mcp-filesystem", "args": ["/home/user"] }
  },
  "permissions": {
    "allow": ["Read", "Write", "Edit", "Bash"],
    "deny": []
  },
  "preferences": {
    "theme": "dark",
    "autoApprove": ["mcp_filesystem/*", "mcp_github/*"]
  }
}
```

**Extract**:
- All configured MCP servers (inherit these into RALPH)
- Permission settings (tool allowlists/denylists)
- User preferences for approval modes
- Custom themes/display settings

### 5. Claude Code Extensions & Plugins
**Locations**:
- `~/.claude/extensions/` - Installed extensions
- `~/.claude/plugins/` - Custom plugins
- Per-project `.claude/` directory for local plugins

**Common Analysis Plugins**:
- **mcp-memory / mem0**: Episodic memory across conversations
- **fogmap**: Semantic search over conversation history
- **graphiti-mcp**: Knowledge graph extraction
- **claude-code-history-viewer**: Structured history browsing

**Use These For Analysis**:
If the user has analysis plugins installed, the onboarding agent should use them rather than parsing JSONL manually.

### 6. Conversation Memories
**Location**: `~/.claude/memories/` or via memory MCP

Claude Code stores learned facts and preferences:
- Project-specific knowledge
- User coding style preferences
- Common solution patterns
- Frequently used commands

**Extract**:
- Incorporate relevant memories into RALPH_INSTRUCTIONS.md
- Use memory queries to find project patterns

### 7. Project Metadata
**Files**:
- `package.json` - Node.js/JavaScript projects
- `pyproject.toml` / `setup.py` - Python projects
- `Cargo.toml` - Rust projects
- `go.mod` - Go projects
- `pubspec.yaml` - Flutter/Dart projects
- `app.json` / `expo.json` - Expo/React Native projects
- `Podfile` - iOS projects
- `build.gradle` - Android/Java projects

**Extract**:
- Project type and language
- Frameworks in use (React, FastAPI, Expo, etc.)
- Development dependencies (testing, linting, building)
- Scripts/commands for common operations

---

## Architecture

### New Module: `src/ralph_orchestrator/onboarding/`

```
onboarding/
├── __init__.py
├── scanner.py           # ProjectScanner - finds all data sources
├── settings_loader.py   # SettingsLoader - loads full Claude Code config
├── history_analyzer.py  # HistoryAnalyzer - parses JSONL OR uses MCP tools
├── pattern_extractor.py # PatternExtractor - identifies successful workflows  
├── agent_analyzer.py    # AgentAnalyzer - uses Claude + user's MCPs for analysis
├── config_generator.py  # ConfigGenerator - creates ralph.yml
├── models.py            # Data models for analysis results
└── cli.py               # CLI integration
```

### Analysis Strategy

```
┌─────────────────────────────────────────────────────────────┐
│                    ralph onboard                            │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│  ┌─────────────┐     ┌──────────────────────────────────┐  │
│  │ --agent     │────▶│  AgentAnalyzer                   │  │
│  │ (default)   │     │  • Runs Claude with user's MCPs  │  │
│  └─────────────┘     │  • Uses mcp-memory if available  │  │
│                      │  • Semantic search for patterns  │  │
│                      │  • Intelligent extraction        │  │
│                      └──────────────────────────────────┘  │
│                                                             │
│  ┌─────────────┐     ┌──────────────────────────────────┐  │
│  │ --static    │────▶│  HistoryAnalyzer (Direct)        │  │
│  │             │     │  • Parses JSONL files directly   │  │
│  └─────────────┘     │  • No API calls needed           │  │
│                      │  • Faster but less intelligent   │  │
│                      └──────────────────────────────────┘  │
│                                                             │
│                            ▼                                │
│                  ┌──────────────────┐                      │
│                  │ ConfigGenerator  │                      │
│                  │ • ralph.yml      │                      │
│                  │ • INSTRUCTIONS   │                      │
│                  │ • PROMPT.md      │                      │
│                  └──────────────────┘                      │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

### Core Classes

#### SettingsLoader
```python
class SettingsLoader:
    """Loads and merges Claude Code settings from all sources."""
    
    def __init__(self, project_path: Path):
        self.project_path = project_path
        self.user_home = Path.home()
        
    def load_user_settings(self) -> Dict[str, Any]:
        """Load ~/.claude/settings.json"""
        
    def load_project_mcp(self) -> Dict[str, Any]:
        """Load [project]/.mcp.json"""
        
    def get_mcp_servers(self) -> Dict[str, MCPServerConfig]:
        """Get merged MCP server configurations (project overrides user)"""
        
    def get_permissions(self) -> PermissionConfig:
        """Get tool permission settings"""
        
    def get_analysis_plugins(self) -> List[str]:
        """Detect installed analysis plugins (memory, fogmap, etc.)"""
        
    def has_memory_plugin(self) -> bool:
        """Check if mcp-memory or similar is configured"""
```

#### ProjectScanner
```python
class ProjectScanner:
    """Discovers all analyzable data sources for a project."""

    def __init__(self, project_path: Path, settings: SettingsLoader):
        self.project_path = project_path
        self.settings = settings

    def find_claude_history(self) -> List[Path]:
        """Find conversation JSONL files in ~/.claude/projects/"""
        
    def find_claude_md_files(self) -> List[Path]:
        """Find CLAUDE.md and .claude/rules/*.md files"""
        
    def find_mcp_config(self) -> Dict[str, Any]:
        """Delegate to SettingsLoader for merged MCP config"""
        
    def detect_project_type(self) -> ProjectType:
        """Determine project type from manifest files"""
        
    def get_project_hash(self) -> str:
        """Get the hash Claude Code uses for this project in ~/.claude/projects/"""
```

#### HistoryAnalyzer
```python
class HistoryAnalyzer:
    """Parses Claude Code conversation history."""

    def __init__(self, jsonl_files: List[Path]):
        self.files = jsonl_files

    def extract_tool_usage(self) -> Dict[str, ToolUsageStats]:
        """Extract tool usage frequency and success rates"""

    def extract_mcp_usage(self) -> Dict[str, MCPServerStats]:
        """Extract MCP server usage patterns"""

    def extract_tool_chains(self) -> List[ToolChain]:
        """Identify sequences of tools commonly used together"""

    def extract_conversations(self) -> List[Conversation]:
        """Parse full conversations for deeper analysis"""
```

#### PatternExtractor
```python
class PatternExtractor:
    """Identifies successful workflow patterns."""

    def __init__(self, history: HistoryAnalyzer):
        self.history = history

    def identify_workflows(self) -> List[Workflow]:
        """Identify common workflow patterns (test→fix→commit, etc.)"""

    def identify_successful_tools(self) -> List[str]:
        """Tools with high success rates to prioritize"""

    def identify_project_patterns(self) -> ProjectPatterns:
        """Project-specific patterns (Expo commands, iOS sim, etc.)"""

    def generate_system_prompt_additions(self) -> str:
        """Generate system prompt text from patterns"""
```

#### AgentAnalyzer
```python
class AgentAnalyzer:
    """Uses Claude with user's MCP servers for intelligent analysis."""
    
    def __init__(self, project_path: Path, settings: SettingsLoader):
        self.project_path = project_path
        self.settings = settings
        self.adapter = ClaudeAdapter(inherit_user_settings=True)
        
    async def analyze(self) -> AnalysisResult:
        """Run Claude to analyze the project using available tools."""
        
        # Build analysis prompt
        prompt = self._build_analysis_prompt()
        
        # Run Claude with user's MCP servers inherited
        result = await self.adapter.aexecute(
            prompt,
            inherit_user_settings=True,  # Load user's ~/.claude/settings.json
            enable_all_tools=True,
        )
        
        return self._parse_analysis_result(result)
    
    def _build_analysis_prompt(self) -> str:
        """Build prompt for Claude to analyze the project."""
        has_memory = self.settings.has_memory_plugin()
        
        prompt = f"""Analyze this project to generate RALPH Orchestrator configuration.
        
Project path: {self.project_path}

{"Use your mcp-memory/episodic memory to recall patterns from past work on this project." if has_memory else ""}

Please analyze:
1. Project structure and type (framework, language)
2. Common tools and commands used
3. Successful workflow patterns
4. MCP servers that are most useful

Return a structured analysis with:
- project_type: string
- frameworks: list
- common_tools: list with success rates
- workflows: list of step sequences
- recommended_config: dict for ralph.yml
"""
        return prompt

#### ConfigGenerator
```python
class ConfigGenerator:
    """Generates RALPH configuration from analysis."""

    def __init__(self, scanner: ProjectScanner, extractor: PatternExtractor, 
                 settings: SettingsLoader):
        self.scanner = scanner
        self.extractor = extractor
        self.settings = settings

    def generate_ralph_yml(self) -> str:
        """Generate optimized ralph.yml content"""
        
    def generate_prompt_md(self) -> str:
        """Generate initial PROMPT.md with context"""

    def generate_instructions(self) -> str:
        """Generate RALPH_INSTRUCTIONS.md from learned patterns"""
        
    def _include_mcp_servers(self) -> Dict[str, Any]:
        """Include user's MCP servers in generated config"""
        # Inherit servers that were used successfully
        return self.settings.get_mcp_servers()
```

---

## CLI Commands

### Primary Command
```bash
ralph onboard [PROJECT_PATH]
```

**Analysis Mode Options**:
- `--agent` (default) - Use Claude + user's MCP servers for intelligent analysis
- `--static` - Parse files directly without running an agent (offline mode)
- `--use-memory` - Explicitly use mcp-memory/episodic memory for analysis

**Output Options**:
- `--analyze-only` / `-a` - Show analysis without generating files
- `--output-dir` / `-o` - Output directory for generated files (default: project root)
- `--merge` - Merge with existing ralph.yml instead of overwriting
- `--dry-run` - Preview changes without writing files
- `--verbose` / `-v` - Show detailed analysis output
- `--format` - Output format: `yaml` (default), `json`, `text`

**Settings Options**:
- `--inherit-settings` (default) - Load user's ~/.claude/settings.json
- `--no-inherit` - Don't inherit user settings
- `--mcp-server NAME` - Explicitly use specific MCP server for analysis

### Examples
```bash
# Onboard with intelligent agent analysis (uses your MCPs)
ralph onboard ~/projects/my-expo-app

# Analyze only, show what would be generated
ralph onboard ~/projects/my-api --analyze-only

# Use static analysis (no API calls, offline mode)
ralph onboard . --static

# Explicitly use episodic memory for deeper analysis
ralph onboard . --use-memory

# Merge with existing config
ralph onboard . --merge

# Preview what would be generated
ralph onboard . --dry-run --verbose

# Don't inherit user settings (isolated analysis)
ralph onboard . --no-inherit
```

---

## Output Artifacts

### 1. ralph.yml
Generated configuration optimized for the project:

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
      # - mcp_filesystem (project operations)

# Project-specific notes (from CLAUDE.md):
# - Uses Expo for mobile development
# - iOS Simulator at /Applications/Xcode.app/...
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
- Use NativeWind for styling
```

### 3. Enhanced PROMPT.md Template
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

---

## Success Criteria

### Functional Requirements
- [ ] `ralph onboard` CLI command implemented and working
- [ ] Correctly parses Claude Code JSONL conversation files
- [ ] Extracts tool usage statistics (frequency, success rate)
- [ ] Identifies MCP servers from both project and user config
- [ ] Detects project type from manifest files (package.json, etc.)
- [ ] Generates valid ralph.yml with appropriate settings
- [ ] Generates RALPH_INSTRUCTIONS.md from learned patterns
- [ ] `--analyze-only` mode shows analysis without writing files
- [ ] `--merge` mode combines with existing configuration
- [ ] `--dry-run` mode previews changes

### MCP & Settings Integration
- [ ] Loads `~/.claude/settings.json` for user MCP servers
- [ ] Loads `[project]/.mcp.json` for project-specific MCPs
- [ ] `--agent` mode runs Claude with inherited user settings
- [ ] Detects and uses mcp-memory for episodic analysis when available
- [ ] Falls back to static JSONL parsing when MCPs unavailable
- [ ] Generated ralph.yml includes user's proven MCP servers
- [ ] `--static` mode works completely offline

### SDK Readiness
- [ ] Architecture supports future Anthropic SDK integration
- [ ] HistoryAnalyzer has pluggable backends (JSONL, MCP, SDK)
- [ ] Settings loading is abstracted for SDK replacement
- [ ] Analysis results are in a stable, versioned format

### Quality Requirements
- [ ] Works for projects with no Claude Code history (graceful fallback)
- [ ] Works for Python, JavaScript/TypeScript, Rust, Go projects
- [ ] Handles missing files gracefully (no crashes)
- [ ] Respects user's existing CLAUDE.md content
- [ ] Generated config reduces manual setup by 80%+
- [ ] Unit tests achieve >90% coverage
- [ ] Integration tests verify end-to-end flow

### Documentation
- [ ] CLI help text is clear and complete
- [ ] Usage examples in docs/guide/
- [ ] API documentation for programmatic use
- [ ] Troubleshooting section for common issues
- [ ] List of supported analysis plugins documented

---

## Technical Specifications

### Language
Python 3.10+

### Dependencies
- `pyyaml` - YAML parsing/generation
- `pathlib` - Path handling
- `json` - JSONL parsing
- `dataclasses` - Data models
- `typing` - Type hints
- No new external dependencies required

### File Locations
- Module: `src/ralph_orchestrator/onboarding/`
- CLI integration: `src/ralph_orchestrator/__main__.py`
- Tests: `tests/test_onboarding.py`
- Docs: `docs/guide/onboarding.md`

### Integration Points
- Extend `__main__.py` with `onboard` subcommand
- Use existing `RalphConsole` for output formatting
- Use existing `RalphConfig` for config handling
- Access file system via standard Python (not MCP for portability)

---

## Implementation Phases

### Phase 1: Settings & Infrastructure (Priority: HIGH)
- [ ] Implement `SettingsLoader` class
- [ ] Load `~/.claude/settings.json` (user settings)
- [ ] Load `[project]/.mcp.json` (project MCP config)
- [ ] Merge settings with proper precedence
- [ ] Detect installed analysis plugins (memory, fogmap, etc.)

### Phase 2: Core Scanning (Priority: HIGH)
- [ ] Implement `ProjectScanner` class
- [ ] Detect project type from manifest files
- [ ] Find Claude Code history directory mapping (hash lookup)
- [ ] Parse and merge MCP configurations

### Phase 3: Agent-Assisted Analysis (Priority: HIGH)
- [ ] Implement `AgentAnalyzer` class
- [ ] Run Claude with `inherit_user_settings=True`
- [ ] Build analysis prompts that leverage user's MCPs
- [ ] Use mcp-memory for episodic analysis when available
- [ ] Parse structured analysis results from Claude

### Phase 4: Static Analysis Fallback (Priority: MEDIUM)
- [ ] Implement `HistoryAnalyzer` class (JSONL parsing)
- [ ] Parse JSONL conversation format directly
- [ ] Extract tool usage statistics
- [ ] Identify tool chains and sequences
- [ ] Works offline without API calls

### Phase 5: Pattern Extraction (Priority: MEDIUM)
- [ ] Implement `PatternExtractor` class
- [ ] Identify common workflow patterns
- [ ] Extract successful tool combinations
- [ ] Generate system prompt additions
- [ ] Works with both agent and static analysis results

### Phase 6: Config Generation (Priority: HIGH)
- [ ] Implement `ConfigGenerator` class
- [ ] Generate ralph.yml with inherited MCP servers
- [ ] Generate RALPH_INSTRUCTIONS.md from patterns
- [ ] Generate PROMPT.md template
- [ ] Include user's proven tool permissions

### Phase 7: CLI Integration (Priority: HIGH)
- [ ] Add `onboard` subcommand to CLI
- [ ] Implement `--agent` and `--static` modes
- [ ] Implement `--use-memory` flag
- [ ] Implement `--inherit-settings` / `--no-inherit`
- [ ] Add progress output and formatting
- [ ] Handle errors gracefully

### Phase 8: Testing & Documentation (Priority: MEDIUM)
- [ ] Unit tests for all modules
- [ ] Integration tests for both analysis modes
- [ ] Mock MCP servers for testing
- [ ] Usage documentation with examples
- [ ] Document supported analysis plugins

---

## Example Scenarios

### Scenario 1: Expo Mobile App
```bash
$ ralph onboard ~/projects/my-expo-app

🔍 Scanning project: ~/projects/my-expo-app
   ✓ Found package.json (Expo project detected)
   ✓ Found .claude/CLAUDE.md
   ✓ Found 47 conversation files in Claude history

📊 Analyzing conversation history...
   ✓ Parsed 1,247 messages
   ✓ Found 892 tool uses
   ✓ Identified 15 MCP server invocations

🎯 Extracted patterns:
   • Top tools: Edit (312), Bash (245), Read (198)
   • Workflows: test→fix→commit (found 23 times)
   • MCP servers: mcp_github, mcp_ios-simulator

📝 Generating configuration...
   ✓ Created ralph.yml
   ✓ Created RALPH_INSTRUCTIONS.md
   ✓ Created PROMPT.md template

✅ Onboarding complete! Run 'ralph run' to start.
```

### Scenario 2: Python API with No History
```bash
$ ralph onboard ~/projects/fastapi-backend

🔍 Scanning project: ~/projects/fastapi-backend
   ✓ Found pyproject.toml (Python project detected)
   ✓ Found requirements.txt
   ⚠ No Claude conversation history found

📊 Generating config from project metadata...
   • Detected: FastAPI, pytest, black
   • Common commands: pytest, uvicorn

📝 Generating configuration...
   ✓ Created ralph.yml (with sensible defaults)
   ✓ Created PROMPT.md template

✅ Onboarding complete! Customize PROMPT.md and run 'ralph run'.
```

---

## Notes

### Graceful Degradation
- Works with full MCP ecosystem → best results
- Works with just JSONL history → good results  
- Works with just project metadata → basic results
- Works with nothing → sensible defaults

### Privacy & Security
- All analysis happens locally (even agent-assisted uses local Claude)
- Conversation content is analyzed but not stored externally
- MCP servers run in user's environment with their permissions
- Generated configs contain patterns, not sensitive content

### MCP Plugin Ecosystem
- **mcp-memory / mem0**: Anthropic's episodic memory - query past learnings
- **fogmap**: Semantic search over conversations by meaning
- **graphiti-mcp**: Knowledge graph extraction from conversations
- **claude-code-history-viewer**: Structured history browsing
- More plugins are expected as the ecosystem grows

### SDK Future-Proofing
- When Anthropic releases official APIs for Claude Code data access:
  - SettingsLoader can be updated to use SDK
  - HistoryAnalyzer can switch to SDK-based retrieval
  - Architecture is designed for easy SDK integration
- Current JSONL parsing is a fallback that will remain useful

### Configuration Inheritance
- User's proven MCP servers are included in generated ralph.yml
- Tool permissions from settings.json are respected
- CLAUDE.md rules are incorporated into RALPH_INSTRUCTIONS.md

### Caching & Performance
- Cache analysis results in `.ralph-cache/` for large projects
- Incremental analysis when history has new conversations
- Static mode is fast; agent mode is thorough

---

**Status**: 🚧 IN PROGRESS
**Priority**: HIGH
**Estimated Effort**: 4-6 development iterations
