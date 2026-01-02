# Task: Intelligent Project Onboarding & Pattern Analysis

Build a new method/CLI option that uses Claude to automatically analyze and onboard existing projects. This system will examine patterns, tools, and workflows utilized across a project's conversation history and codebase, then generate custom instructions and prompts based on proven success patterns.

## Objective

Instead of requiring developers to manually specify `claude_all_tools` configurations for every prompt, create an intelligent onboarding system that:
1. **Analyzes the project structure** - Identifies frameworks, languages, and build systems
2. **Discovers MCP tool patterns** - Finds which MCP servers are commonly used
3. **Learns from conversation history** - Extracts successful workflows from `.agent/` directories
4. **Generates custom configurations** - Produces project-specific `ralph.yml` and instructions
5. **Creates optimized prompts** - Builds prompt templates incorporating proven patterns

## Requirements

- [ ] Create `src/ralph_orchestrator/onboarding/` module
- [ ] Implement `ProjectAnalyzer` class for codebase analysis
- [ ] Implement `PatternExtractor` class for workflow pattern recognition
- [ ] Implement `ConfigGenerator` class for generating ralph.yml configurations
- [ ] Implement `PromptTemplateGenerator` for context-aware prompt creation
- [ ] Add CLI command: `ralph onboard [project_path]`
- [ ] Add CLI command: `ralph onboard --analyze` (analysis only, no changes)
- [ ] Add CLI command: `ralph onboard --apply` (apply generated configuration)
- [ ] Support multiple project types (web app, mobile app, CLI tool, library)
- [ ] Integrate with existing Claude adapter for AI-powered analysis
- [ ] Create comprehensive test coverage (90%+)
- [ ] Document the onboarding workflow

## Technical Specifications

### 1. Project Analyzer (`src/ralph_orchestrator/onboarding/analyzer.py`)

```python
@dataclass
class ProjectProfile:
    """Comprehensive project profile from analysis."""
    project_type: str  # "web_app", "mobile_app", "cli_tool", "library", "monorepo"
    languages: List[str]  # ["python", "typescript", "swift"]
    frameworks: List[str]  # ["fastapi", "react", "expo"]
    build_systems: List[str]  # ["uv", "npm", "xcode"]
    testing_frameworks: List[str]  # ["pytest", "jest", "xctest"]
    package_managers: List[str]  # ["uv", "npm", "cocoapods"]
    mcp_servers_detected: List[str]  # From .claude.json or claude_desktop_config.json
    ci_cd: Optional[str]  # "github_actions", "gitlab_ci", etc.
    documentation_style: str  # "docstrings", "jsdoc", "markdown"
    conventions: Dict[str, Any]  # Detected coding conventions

class ProjectAnalyzer:
    """Analyzes project structure and characteristics."""
    
    async def analyze(self, project_path: Path) -> ProjectProfile:
        """Perform comprehensive project analysis."""
        ...
    
    def detect_project_type(self, path: Path) -> str:
        """Detect the type of project based on files present."""
        ...
    
    def scan_dependencies(self, path: Path) -> Dict[str, List[str]]:
        """Scan for dependencies across package managers."""
        ...
    
    def detect_mcp_servers(self, path: Path) -> List[MCPServerConfig]:
        """Detect configured MCP servers from Claude config files."""
        ...
```

### 2. Pattern Extractor (`src/ralph_orchestrator/onboarding/patterns.py`)

```python
@dataclass
class WorkflowPattern:
    """A discovered workflow pattern from conversation history."""
    name: str
    description: str
    trigger: str  # What initiates this workflow
    steps: List[str]  # The sequence of actions
    tools_used: List[str]  # MCP tools and commands used
    success_indicators: List[str]  # What indicates success
    frequency: int  # How often this pattern was used
    success_rate: float  # Historical success rate

class PatternExtractor:
    """Extracts patterns from .agent/ conversation history."""
    
    async def extract_patterns(self, agent_dir: Path) -> List[WorkflowPattern]:
        """Extract workflow patterns from agent history."""
        ...
    
    def analyze_metrics_files(self, metrics_dir: Path) -> Dict[str, Any]:
        """Analyze metrics for success patterns."""
        ...
    
    def identify_common_sequences(self, history: List[Dict]) -> List[WorkflowPattern]:
        """Find common action sequences that led to success."""
        ...
```

### 3. Config Generator (`src/ralph_orchestrator/onboarding/config_gen.py`)

```python
@dataclass
class GeneratedConfig:
    """Generated configuration for RALPH."""
    ralph_yml: Dict[str, Any]  # Full ralph.yml content
    claude_settings: Dict[str, Any]  # Claude-specific settings
    mcp_recommendations: List[MCPRecommendation]  # Recommended MCP servers
    prompt_templates: List[PromptTemplate]  # Project-specific prompt templates
    custom_instructions: str  # Project-specific instructions for CLAUDE.md

class ConfigGenerator:
    """Generates project-specific RALPH configurations."""
    
    def generate(self, profile: ProjectProfile, patterns: List[WorkflowPattern]) -> GeneratedConfig:
        """Generate full configuration from analysis."""
        ...
    
    def generate_ralph_yml(self, profile: ProjectProfile) -> Dict[str, Any]:
        """Generate optimized ralph.yml configuration."""
        ...
    
    def recommend_mcp_servers(self, profile: ProjectProfile) -> List[MCPRecommendation]:
        """Recommend MCP servers based on project type."""
        ...
    
    def generate_custom_instructions(self, profile: ProjectProfile, patterns: List[WorkflowPattern]) -> str:
        """Generate CLAUDE.md custom instructions."""
        ...
```

### 4. CLI Integration (`src/ralph_orchestrator/__main__.py`)

```bash
# Analyze project and show recommendations
ralph onboard /path/to/project --analyze

# Generate configuration files
ralph onboard /path/to/project --generate

# Apply configuration (with confirmation)
ralph onboard /path/to/project --apply

# Full onboard with Claude assistance
ralph onboard /path/to/project --interactive
```

### 5. Project Type Detection Logic

```python
PROJECT_SIGNATURES = {
    "web_app": {
        "files": ["package.json", "index.html", "vite.config.*"],
        "frameworks": ["react", "vue", "angular", "svelte"],
    },
    "mobile_app": {
        "files": ["app.json", "*.xcodeproj", "android/build.gradle"],
        "frameworks": ["expo", "react-native", "flutter"],
    },
    "python_backend": {
        "files": ["pyproject.toml", "requirements.txt", "main.py"],
        "frameworks": ["fastapi", "django", "flask"],
    },
    "cli_tool": {
        "files": ["setup.py", "pyproject.toml", "__main__.py"],
        "patterns": ["argparse", "click", "typer"],
    },
}
```

### 6. MCP Server Recommendations by Project Type

```python
MCP_RECOMMENDATIONS = {
    "web_app": [
        {"name": "chrome-devtools", "reason": "Browser debugging and screenshots"},
        {"name": "filesystem", "reason": "File operations for build artifacts"},
        {"name": "fetch", "reason": "API testing during development"},
    ],
    "mobile_app": {
        "ios": [
            {"name": "ios-simulator", "reason": "iOS simulator control"},
            {"name": "filesystem", "reason": "Xcode project management"},
        ],
        "expo": [
            {"name": "fetch", "reason": "Expo development server interaction"},
            {"name": "filesystem", "reason": "Configuration management"},
        ],
    },
    "python_backend": [
        {"name": "filesystem", "reason": "Code and config management"},
        {"name": "fetch", "reason": "API endpoint testing"},
        {"name": "memory", "reason": "Context persistence across sessions"},
    ],
}
```

## Implementation Steps

### Step 1: Core Analysis Infrastructure
- [ ] Create `src/ralph_orchestrator/onboarding/__init__.py`
- [ ] Implement `ProjectAnalyzer` with file detection
- [ ] Add dependency scanning for major package managers
- [ ] Detect MCP server configurations

### Step 2: Pattern Extraction
- [ ] Parse `.agent/metrics/` JSON files
- [ ] Extract tool usage patterns
- [ ] Calculate success rates per workflow
- [ ] Identify common action sequences

### Step 3: Configuration Generation
- [ ] Generate optimized `ralph.yml`
- [ ] Create project-specific `CLAUDE.md` content
- [ ] Build prompt templates for common workflows
- [ ] Generate MCP server recommendations

### Step 4: CLI Commands
- [ ] Add `ralph onboard` subcommand
- [ ] Implement `--analyze`, `--generate`, `--apply` flags
- [ ] Add `--interactive` mode with Claude guidance
- [ ] Create diff preview before applying changes

### Step 5: Testing & Documentation
- [ ] Unit tests for all analyzers (90%+ coverage)
- [ ] Integration tests with sample projects
- [ ] Documentation in `docs/guide/onboarding.md`
- [ ] Example outputs for each project type

## Success Criteria

- [ ] `ralph onboard` correctly identifies project type for web/mobile/CLI/library projects
- [ ] Pattern extraction identifies 80%+ of common workflows from conversation history
- [ ] Generated `ralph.yml` configurations work without modification
- [ ] MCP server recommendations are relevant to project type
- [ ] Custom instructions improve Claude's understanding of project context
- [ ] CLI provides clear, actionable output
- [ ] All existing tests pass
- [ ] New tests achieve 90%+ coverage
- [ ] Documentation is comprehensive and includes examples

## Example Output

### Analysis Output
```
$ ralph onboard ./my-mobile-app --analyze

📊 Project Analysis: my-mobile-app
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

📁 Project Type: Mobile App (Expo/React Native)

🔧 Detected Stack:
   • Languages: TypeScript, JavaScript
   • Frameworks: Expo SDK 51, React Native 0.74
   • Build System: expo-cli, eas-cli
   • Package Manager: npm
   • Testing: Jest, React Native Testing Library

🔌 MCP Servers Detected:
   ✓ filesystem (configured)
   ✓ fetch (configured)
   
💡 Recommended MCP Servers:
   • ios-simulator - Control iOS simulator for testing
   • memory - Persist context across development sessions

📈 Workflow Patterns Found:
   1. "Build & Test" (used 45 times, 92% success)
      - expo prebuild → expo run:ios → jest
   2. "Hot Reload Debug" (used 128 times, 88% success)
      - Code change → Simulator refresh → Console check
   3. "Release Build" (used 12 times, 100% success)
      - eas build → TestFlight upload

🎯 Generated Files Preview:
   • ralph.yml (optimized for mobile development)
   • CLAUDE.md (project-specific instructions)
   • prompts/templates/ (5 workflow templates)

Run 'ralph onboard ./my-mobile-app --apply' to apply these configurations.
```

## Notes

- This feature is designed to be run once at project setup, but can be re-run to update patterns
- The AI analysis uses Claude to provide intelligent recommendations
- All generated configurations should be treated as starting points for customization
- Pattern extraction respects privacy - only analyzes structure, not content of conversations

## Progress

### Status: NOT STARTED

### Next Steps:
1. Create the onboarding module structure
2. Implement ProjectAnalyzer
3. Add file detection and dependency scanning

---

**Completion Marker:** When all success criteria are met, add `- [x] TASK_COMPLETE` here.
