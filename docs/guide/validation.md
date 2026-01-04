# Validation Feature Guide

Ralph Orchestrator includes an opt-in validation feature that enables collaborative validation strategies between the AI and users. This guide covers how to enable, configure, and use the validation system.

## Overview

The validation feature allows Ralph Orchestrator to:

1. **Propose validation strategies** - AI analyzes the project and recommends how to validate it
2. **Request user confirmation** - Users approve, modify, or decline before validation proceeds
3. **Execute real validation** - No mocks; actual browser screenshots, simulator runs, CLI outputs
4. **Capture evidence** - Screenshots and outputs saved as proof of successful validation

## Key Principles

| Principle | Description |
|-----------|-------------|
| **Opt-in** | Disabled by default; must be explicitly enabled |
| **Claude-only** | Currently only works with Claude adapter |
| **Collaborative** | AI proposes, user confirms |
| **Real execution** | No mocks; actual screenshots and outputs |
| **Flexible** | AI recommends tools based on project context |

## Enabling Validation

### Command Line Flags

```bash
# Enable validation (opt-in)
ralph run -P PROMPT.md --enable-validation

# Enable validation without interactive confirmation (for CI/CD)
ralph run -P PROMPT.md --enable-validation --no-validation-interactive
```

### Configuration File

```yaml
# ralph.yml
enable_validation: true
validation_interactive: true  # Set to false for CI/CD pipelines
```

## CLI Options

| Flag | Default | Description |
|------|---------|-------------|
| `--enable-validation` | `False` | Enable the validation feature |
| `--no-validation-interactive` | Interactive enabled | Disable interactive confirmation prompts |

**Example:**

```bash
# Interactive mode (default) - user must approve validation strategy
ralph run -P PROMPT.md --enable-validation

# Non-interactive mode - auto-approve for CI/CD
ralph run -P PROMPT.md --enable-validation --no-validation-interactive
```

## How Validation Works

### Proposal Flow

When validation is enabled, Ralph Orchestrator follows this flow:

```
1. Enable validation flag detected
2. AI analyzes project (type, build commands, dependencies)
3. AI discovers available tools (MCP servers, browser automation, etc.)
4. AI drafts validation proposal
5. User reviews and confirms (or modifies/declines)
6. If approved: validation proceeds
   If declined: orchestration continues without validation
```

### Validation Proposal Example

When you run with `--enable-validation`, you'll see a proposal like this:

```
🔍 Analyzing project for validation strategy...

📋 Validation Proposal:

Based on analyzing your project, here's what I found:

**Project Analysis:**
- Type: Next.js web application
- Build: `npm run build`
- Run: `npm run dev` (serves at localhost:3000)
- Tests: Jest + Playwright tests exist

**My Validation Proposal:**

Since this is a web app, I recommend validating it the way a user would -
by actually loading it in a browser and interacting with it.

My approach:
1. Create sandbox directory for isolation
2. Build the project to catch compilation errors
3. Start the dev server
4. Use Playwright to:
   - Navigate to the main page
   - Verify it renders correctly
   - Take screenshots as proof
   - Test key user interactions
5. Save screenshots to validation-evidence/web/

**Questions for you:**
- Which pages or features are most critical to validate?
- Any specific user flows I should test?

Does this make sense? [Approve/Modify/Skip]: _
```

## Supported Validation Types

### Web Applications

**What gets validated:**
- Page loads and renders correctly
- Interactive elements work (buttons, forms)
- Navigation between pages
- Responsive design

**Tools used:**
- Playwright or Puppeteer (MCP servers)
- Browser screenshots

**Evidence captured:**
- `validation-evidence/web/*.png` - Browser screenshots
- `validation-evidence/web/validation-log.txt` - Test results

### iOS Applications

**What gets validated:**
- App builds successfully with xcodebuild
- App launches in iOS Simulator
- UI elements render correctly
- Navigation works between screens

**Tools used:**
- xc-mcp (Xcode and iOS Simulator control)
- Simulator screenshots

**Evidence captured:**
- `validation-evidence/ios/*.png` - Simulator screenshots
- `validation-evidence/ios/validation-log.txt` - Build/run logs

### CLI Tools

**What gets validated:**
- Commands execute successfully
- Help output is correct
- Exit codes are appropriate
- Output format matches expectations

**Tools used:**
- Standard shell execution
- Output capture

**Evidence captured:**
- `validation-evidence/cli/cli-output.txt` - Terminal output
- `validation-evidence/cli/` - Source files for reference

## Sandbox Isolation

All validation runs in an isolated sandbox to protect your main codebase:

```bash
# Default sandbox location
SANDBOX_DIR="/tmp/ralph-validation-$(date +%s)"

# Or using Docker via MCP_DOCKER for complete isolation
```

**What happens in the sandbox:**
1. Project copied/cloned to sandbox
2. Build commands executed in isolation
3. Validation tests run
4. Evidence captured
5. Sandbox cleaned up after completion

## Evidence Files

After successful validation, evidence is saved to the `validation-evidence/` directory:

```
validation-evidence/
├── ios/
│   ├── 01-home-screen.png
│   ├── 02-detail-screen.png
│   └── validation-log.txt
├── web/
│   ├── 01-initial-load.png
│   ├── 02-after-interaction.png
│   ├── 03-mobile-viewport.png
│   └── validation-log.txt
└── cli/
    ├── cli-output.txt
    └── source-files/
```

These files serve as proof that validation succeeded and can be committed to your repository.

## CI/CD Integration

For CI/CD pipelines, use non-interactive mode:

```bash
# GitHub Actions example
- name: Run Ralph with Validation
  run: |
    ralph run -P PROMPT.md \
      --enable-validation \
      --no-validation-interactive
```

In non-interactive mode:
- Validation proposals are auto-approved
- No user input required
- Exit codes indicate success/failure

## Requirements

### Claude Adapter Required

The validation feature currently only works with the Claude adapter:

```bash
# This works
ralph run -P PROMPT.md -a claude --enable-validation

# This fails with ValueError
ralph run -P PROMPT.md -a gemini --enable-validation
# Error: Validation feature is only available with Claude adapter
```

### Optional MCP Servers

For full functionality, these MCP servers enhance validation:

| MCP Server | Purpose |
|------------|---------|
| `playwright` | Browser automation for web apps |
| `puppeteer` | Alternative browser automation |
| `xc-mcp` | Xcode and iOS Simulator control |
| `MCP_DOCKER` | Container isolation |

The AI will propose validation strategies based on which tools are available.

## Graceful Degradation

The validation system handles edge cases gracefully:

| Scenario | Behavior |
|----------|----------|
| User declines validation | Orchestration proceeds normally without validation |
| Proposal generation fails | Warning logged, continues without validation |
| MCP servers unavailable | AI suggests alternative validation methods |
| Interactive timeout | Treated as decline (in interactive mode) |

## Troubleshooting

### Common Issues

**"Validation feature is only available with Claude adapter"**

Solution: Use the Claude adapter with `-a claude`

**Validation proposal not appearing**

Check that:
- `--enable-validation` flag is set
- Using Claude adapter
- Project has recognizable structure

**Evidence files not being saved**

Ensure:
- `validation-evidence/` directory is writable
- Validation was approved (not declined)
- Sandbox cleanup didn't delete them prematurely

## Best Practices

1. **Start with interactive mode** - Review proposals before automating
2. **Commit evidence files** - They serve as proof and documentation
3. **Use CI/CD non-interactive mode carefully** - Ensure proposals match expectations
4. **Keep sandbox cleanup enabled** - Prevents disk space issues
5. **Review validation logs** - They contain detailed test results

## See Also

- [Configuration Guide](configuration.md) - Full configuration options
- [Prompts Guide](prompts.md) - How prompts work with validation
- [Troubleshooting](../troubleshooting.md) - Common issues and solutions
