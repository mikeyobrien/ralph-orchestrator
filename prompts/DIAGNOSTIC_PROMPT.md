# Diagnostic Prompt: Project Type Detection & Validation Gate Configuration

**SESSION 0 PROMPT - Run BEFORE any implementation work**

This prompt analyzes a target project to determine the appropriate functional validation strategy and generates configuration for agnostic validation gates.

## Objective

Analyze the current project directory to:
1. Detect project type (web, iOS, CLI, API, library, hybrid)
2. Identify available validation methods (MCP tools, test frameworks, simulators)
3. Generate `validation_config.json` with appropriate validation gates
4. Recommend MCP servers for functional testing

## Context

**Source Files to Analyze:**
- `package.json`, `pyproject.toml`, `Cargo.toml`, etc. (project manifests)
- `*.xcodeproj`, `*.xcworkspace` (iOS/macOS projects)
- `playwright.config.*`, `puppeteer.*` (browser automation config)
- `Dockerfile`, `docker-compose.yml` (containerized apps)
- `README.md`, `CLAUDE.md` (project documentation)

**Available MCP Servers:**
- `puppeteer` - Browser automation (web)
- `playwright` - Cross-browser testing (web)
- `xc-mcp` - iOS Simulator control (iOS/macOS)
- `docker` - Container orchestration (any)
- Custom MCP servers defined in `.mcp.json`

## Analysis Protocol

### Phase 1: Project Detection

Scan the project root and identify:

```
1. MANIFEST DETECTION
   - package.json → Node.js/JavaScript project
   - pyproject.toml/setup.py → Python project
   - Cargo.toml → Rust project
   - go.mod → Go project
   - *.xcodeproj/*.xcworkspace → iOS/macOS project
   - pubspec.yaml → Flutter/Dart project
   - Gemfile → Ruby project

2. FRAMEWORK DETECTION
   - next.config.* → Next.js (web, SSR)
   - nuxt.config.* → Nuxt.js (web, SSR)
   - vite.config.* → Vite (web, SPA)
   - app.json + metro.config.js → React Native (mobile)
   - fastapi/flask/django → Python web API
   - gin/echo/fiber → Go web API
   - actix/axum → Rust web API

3. OUTPUT TYPE DETECTION
   - dist/, build/, .next/ → Web build output
   - *.app, *.ipa → iOS app bundle
   - *.exe, *.dmg → Desktop app
   - bin/, target/release/ → CLI binary
   - No UI files → Library/SDK
```

### Phase 2: Validation Method Selection

Based on project type, select validation methods:

| Project Type | Primary Validation | Secondary Validation | MCP Server |
|-------------|-------------------|---------------------|------------|
| Web SPA | Browser automation | Network requests | puppeteer/playwright |
| Web SSR | Browser + API calls | Lighthouse | playwright |
| iOS App | Simulator control | XCTest | xc-mcp |
| macOS App | Simulator/App launch | XCTest | xc-mcp |
| React Native | iOS Sim + Android Emu | Jest | xc-mcp + android |
| CLI Tool | Shell execution | Exit codes + stdout | bash |
| API Server | HTTP requests | Response validation | curl/httpie |
| Library | Import + function calls | Unit tests | native test runner |
| Hybrid | Multiple methods | Cross-platform | multiple |

### Phase 3: Configuration Generation

Generate `validation_config.json`:

```json
{
  "project_type": "<detected_type>",
  "validation_gates": [
    {
      "id": "build",
      "type": "compilation",
      "command": "<build_command>",
      "success_criteria": {
        "exit_code": 0,
        "no_errors": true
      }
    },
    {
      "id": "functional",
      "type": "<web|ios|cli|api>",
      "mcp_server": "<server_name>",
      "tools": ["<tool_list>"],
      "validation_steps": [
        {
          "action": "<action_type>",
          "target": "<target>",
          "expected": "<expected_result>"
        }
      ]
    },
    {
      "id": "unit_tests",
      "type": "test_runner",
      "command": "<test_command>",
      "success_criteria": {
        "pass_rate": 0.95
      }
    }
  ],
  "mcp_servers": {
    "<server_name>": {
      "enabled": true,
      "tools": ["<allowed_tools>"]
    }
  },
  "security": {
    "allowed_commands": ["<command_list>"],
    "restricted_paths": ["<path_list>"]
  }
}
```

## Requirements

### 1. Scan Project Structure

```bash
# List project root
ls -la

# Find manifest files
find . -maxdepth 2 -name "package.json" -o -name "pyproject.toml" -o -name "*.xcodeproj" -o -name "Cargo.toml" 2>/dev/null

# Check for iOS project
find . -name "*.xcodeproj" -o -name "*.xcworkspace" 2>/dev/null

# Check for web frameworks
ls -la *.config.* next.config.* vite.config.* 2>/dev/null

# Check for existing MCP config
cat .mcp.json 2>/dev/null || echo "No MCP config found"
```

### 2. Read Key Files

Read and analyze:
- Project manifest (package.json, pyproject.toml, etc.)
- README.md for project description
- CLAUDE.md for AI instructions
- Existing test configurations

### 3. Determine Validation Strategy

Based on analysis, document:
1. **Project Type**: The detected type(s)
2. **Build Command**: How to compile/build the project
3. **Run Command**: How to start the application
4. **Test Command**: How to run existing tests
5. **Validation Method**: Which MCP server/tools to use
6. **Success Criteria**: How to determine if validation passed

### 4. Generate Configuration File

Create `validation_config.json` in project root with:
- All detected validation gates
- MCP server configuration
- Security constraints
- Success criteria per gate

## Output Format

Save output to: `validation_config.json`

Also create a summary in: `.agent/diagnostic_report.md`

```markdown
# Diagnostic Report

## Project Analysis

**Type**: {detected_type}
**Framework**: {framework_name}
**Language**: {primary_language}

## Validation Strategy

### Build Gate
- Command: `{build_command}`
- Expected: Exit code 0, no errors

### Functional Validation Gate
- Type: {validation_type}
- MCP Server: {mcp_server}
- Tools: {tool_list}

### Test Gate
- Command: `{test_command}`
- Pass Rate: 95%

## MCP Server Configuration

| Server | Enabled | Tools |
|--------|---------|-------|
| {name} | {yes/no} | {tools} |

## Security Configuration

### Allowed Commands
{command_list}

### Restricted Paths
{path_list}

## Recommendations

{recommendations_for_validation}
```

## Success Criteria

1. `validation_config.json` created with valid JSON
2. All validation gates have actionable commands
3. MCP server selection matches project type
4. Security constraints are appropriate
5. Diagnostic report provides clear summary

## Example Outputs

### Web Project (Next.js)

```json
{
  "project_type": "web_ssr",
  "validation_gates": [
    {
      "id": "build",
      "type": "compilation",
      "command": "npm run build",
      "success_criteria": { "exit_code": 0 }
    },
    {
      "id": "functional",
      "type": "web",
      "mcp_server": "playwright",
      "tools": [
        "mcp__playwright__browser_navigate",
        "mcp__playwright__browser_snapshot",
        "mcp__playwright__browser_click",
        "mcp__playwright__browser_type"
      ],
      "validation_steps": [
        { "action": "navigate", "target": "http://localhost:3000", "expected": "page_loads" },
        { "action": "snapshot", "expected": "has_content" }
      ]
    },
    {
      "id": "unit_tests",
      "type": "test_runner",
      "command": "npm test",
      "success_criteria": { "pass_rate": 0.95 }
    }
  ],
  "mcp_servers": {
    "playwright": {
      "enabled": true,
      "tools": ["browser_navigate", "browser_snapshot", "browser_click", "browser_type", "browser_fill_form"]
    }
  },
  "security": {
    "allowed_commands": ["npm", "npx", "node", "git"],
    "restricted_paths": ["/etc", "/usr/bin", "~/.ssh"]
  }
}
```

### iOS Project (SwiftUI)

```json
{
  "project_type": "ios_app",
  "validation_gates": [
    {
      "id": "build",
      "type": "compilation",
      "command": "xcodebuild -scheme MyApp -destination 'platform=iOS Simulator,name=iPhone 15 Pro' build",
      "success_criteria": { "exit_code": 0 }
    },
    {
      "id": "functional",
      "type": "ios",
      "mcp_server": "xc-mcp",
      "tools": [
        "mcp__xc-mcp__simctl-boot",
        "mcp__xc-mcp__simctl-install",
        "mcp__xc-mcp__simctl-launch",
        "mcp__xc-mcp__screenshot",
        "mcp__xc-mcp__idb-ui-tap"
      ],
      "validation_steps": [
        { "action": "boot_simulator", "target": "iPhone 15 Pro", "expected": "booted" },
        { "action": "install_app", "target": "build/MyApp.app", "expected": "installed" },
        { "action": "launch_app", "target": "com.example.MyApp", "expected": "running" },
        { "action": "screenshot", "expected": "has_ui_elements" }
      ]
    },
    {
      "id": "unit_tests",
      "type": "test_runner",
      "command": "xcodebuild test -scheme MyApp -destination 'platform=iOS Simulator,name=iPhone 15 Pro'",
      "success_criteria": { "pass_rate": 0.95 }
    }
  ],
  "mcp_servers": {
    "xc-mcp": {
      "enabled": true,
      "tools": ["simctl-boot", "simctl-install", "simctl-launch", "screenshot", "idb-ui-tap", "idb-ui-input"]
    }
  },
  "security": {
    "allowed_commands": ["xcodebuild", "xcrun", "simctl", "git"],
    "restricted_paths": ["/etc", "/usr/bin", "~/.ssh"]
  }
}
```

### CLI Tool (Python)

```json
{
  "project_type": "cli",
  "validation_gates": [
    {
      "id": "build",
      "type": "compilation",
      "command": "pip install -e .",
      "success_criteria": { "exit_code": 0 }
    },
    {
      "id": "functional",
      "type": "cli",
      "mcp_server": null,
      "tools": ["bash"],
      "validation_steps": [
        { "action": "execute", "target": "mycli --help", "expected": { "exit_code": 0, "contains": "Usage" } },
        { "action": "execute", "target": "mycli process test.txt", "expected": { "exit_code": 0, "output_file": "output.txt" } }
      ]
    },
    {
      "id": "unit_tests",
      "type": "test_runner",
      "command": "pytest tests/ -v",
      "success_criteria": { "pass_rate": 0.95 }
    }
  ],
  "mcp_servers": {},
  "security": {
    "allowed_commands": ["python", "pip", "pytest", "git", "mycli"],
    "restricted_paths": ["/etc", "/usr/bin", "~/.ssh"]
  }
}
```

## Usage

This prompt is invoked by ralph-orchestrator as Session 0:

```python
# In orchestrator initialization
if not Path("validation_config.json").exists():
    ralph.execute_prompt("prompts/DIAGNOSTIC_PROMPT.md")

# Load validation config
with open("validation_config.json") as f:
    validation_config = json.load(f)

# Configure MCP servers based on detected type
ralph.configure_mcp_servers(validation_config["mcp_servers"])
```

---

## Current Status

**Status**: Ready for execution
**Priority**: Run FIRST before any implementation work

### Next Action
Execute this prompt to generate `validation_config.json` for the target project.
