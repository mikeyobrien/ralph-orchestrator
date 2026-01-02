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

### 4. Project Metadata
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
├── scanner.py          # ProjectScanner - finds all data sources
├── history_analyzer.py # HistoryAnalyzer - parses JSONL conversations
├── pattern_extractor.py # PatternExtractor - identifies successful workflows
├── config_generator.py  # ConfigGenerator - creates ralph.yml
├── models.py           # Data models for analysis results
└── cli.py              # CLI integration
```

### Core Classes

#### ProjectScanner
```python
class ProjectScanner:
    """Discovers all analyzable data sources for a project."""
    
    def __init__(self, project_path: Path):
        self.project_path = project_path
        
    def find_claude_history(self) -> List[Path]:
        """Find conversation JSONL files in ~/.claude/projects/"""
        
    def find_claude_md_files(self) -> List[Path]:
        """Find CLAUDE.md and .claude/rules/*.md files"""
        
    def find_mcp_config(self) -> Dict[str, Any]:
        """Parse .mcp.json and settings.json for MCP servers"""
        
    def detect_project_type(self) -> ProjectType:
        """Determine project type from manifest files"""
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

#### ConfigGenerator
```python
class ConfigGenerator:
    """Generates RALPH configuration from analysis."""
    
    def __init__(self, scanner: ProjectScanner, extractor: PatternExtractor):
        self.scanner = scanner
        self.extractor = extractor
        
    def generate_ralph_yml(self) -> str:
        """Generate optimized ralph.yml content"""
        
    def generate_prompt_md(self) -> str:
        """Generate initial PROMPT.md with context"""
        
    def generate_instructions(self) -> str:
        """Generate RALPH_INSTRUCTIONS.md from learned patterns"""
```

---

## CLI Commands

### Primary Command
```bash
ralph onboard [PROJECT_PATH]
```

**Options**:
- `--analyze-only` / `-a` - Show analysis without generating files
- `--output-dir` / `-o` - Output directory for generated files (default: project root)
- `--merge` - Merge with existing ralph.yml instead of overwriting
- `--dry-run` - Preview changes without writing files
- `--verbose` / `-v` - Show detailed analysis output
- `--format` - Output format: `yaml` (default), `json`, `text`

### Examples
```bash
# Onboard a mobile project
ralph onboard ~/projects/my-expo-app

# Analyze only (no file generation)
ralph onboard ~/projects/my-api --analyze-only

# Merge with existing config
ralph onboard . --merge

# Preview what would be generated
ralph onboard . --dry-run --verbose
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

### Phase 1: Core Scanning (Priority: HIGH)
- [ ] Implement `ProjectScanner` class
- [ ] Detect project type from manifest files
- [ ] Find Claude Code history directory mapping
- [ ] Parse MCP configurations

### Phase 2: History Analysis (Priority: HIGH)
- [ ] Implement `HistoryAnalyzer` class
- [ ] Parse JSONL conversation format
- [ ] Extract tool usage statistics
- [ ] Identify tool chains and sequences

### Phase 3: Pattern Extraction (Priority: MEDIUM)
- [ ] Implement `PatternExtractor` class
- [ ] Identify common workflow patterns
- [ ] Extract successful tool combinations
- [ ] Generate system prompt additions

### Phase 4: Config Generation (Priority: HIGH)
- [ ] Implement `ConfigGenerator` class
- [ ] Generate ralph.yml from analysis
- [ ] Generate RALPH_INSTRUCTIONS.md
- [ ] Generate PROMPT.md template

### Phase 5: CLI Integration (Priority: HIGH)
- [ ] Add `onboard` subcommand to CLI
- [ ] Implement all CLI options
- [ ] Add progress output and formatting
- [ ] Handle errors gracefully

### Phase 6: Testing & Documentation (Priority: MEDIUM)
- [ ] Unit tests for all modules
- [ ] Integration tests for CLI
- [ ] Usage documentation
- [ ] Example outputs

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

- The feature should work even without conversation history (fallback to project metadata)
- Privacy: conversation content should be analyzed locally, never sent externally
- The generated configs are starting points - users can customize further
- Consider caching analysis results for large projects
- Handle edge cases: corrupted JSONL, missing permissions, etc.

---

**Status**: 🚧 IN PROGRESS
**Priority**: HIGH
**Estimated Effort**: 3-5 development iterations
