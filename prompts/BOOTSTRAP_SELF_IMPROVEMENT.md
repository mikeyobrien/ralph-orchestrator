# Task: RALPH Self-Improvement Bootstrap

This meta-prompt enables RALPH to enhance itself by implementing the Intelligent Project Onboarding and Real-Time TUI features. Run this on your fork of ralph-orchestrator to have RALPH build its own improvements.

## Overview

This prompt orchestrates RALPH to:
1. Work on your fork of the ralph-orchestrator repository
2. Implement one of two major features (Onboarding or TUI)
3. Create a pull request when success criteria are met
4. Iterate until the feature is production-ready

## Prerequisites

Before running this prompt, ensure:

1. **Fork Setup**
   ```bash
   # If you haven't forked yet
   gh repo fork mikeyobrien/ralph-orchestrator --clone
   cd ralph-orchestrator
   git remote add upstream https://github.com/mikeyobrien/ralph-orchestrator.git
   ```

2. **Sync with upstream**
   ```bash
   git fetch upstream
   git checkout main
   git merge upstream/main
   ```

3. **Create feature branch**
   ```bash
   # For Onboarding feature
   git checkout -b feature/intelligent-onboarding

   # For TUI feature
   git checkout -b feature/realtime-tui
   ```

4. **MCP Configuration**
   Ensure your Claude configuration includes these MCP servers:
   - `filesystem` - For reading/writing code files
   - `github` - For creating PRs and managing the repo
   - `memory` - For persisting context across sessions
   - `sequential-thinking` - For complex problem decomposition

## Configuration

### ralph.yml for self-improvement

```yaml
# Ralph Orchestrator Self-Improvement Configuration
agent: claude
prompt_file: prompts/ONBOARDING_PROMPT.md  # or TUI_PROMPT.md
max_iterations: 100
max_runtime: 14400  # 4 hours
checkpoint_interval: 3  # Frequent checkpoints for safety
retry_delay: 2
max_tokens: 2000000
max_cost: 100.0
context_window: 200000
context_threshold: 0.75  # Summarize earlier due to complexity

# Features
archive_prompts: true
git_checkpoint: true
enable_metrics: true
verbose: true

# Telemetry for debugging
iteration_telemetry: true
output_preview_length: 1000

# Claude adapter settings
adapters:
  claude:
    enabled: true
    timeout: 600  # Longer timeout for complex operations
    max_retries: 5
    tool_permissions:
      allow_all: true
```

### Required MCP Servers

Ensure these are configured in your Claude settings:

```json
{
  "mcpServers": {
    "filesystem": {
      "command": "npx",
      "args": ["-y", "@anthropic/mcp-filesystem", "/path/to/ralph-orchestrator"]
    },
    "github": {
      "command": "npx",
      "args": ["-y", "@anthropic/mcp-github"],
      "env": {
        "GITHUB_TOKEN": "your-github-token"
      }
    },
    "memory": {
      "command": "npx",
      "args": ["-y", "@anthropic/mcp-memory"]
    },
    "sequential-thinking": {
      "command": "npx",
      "args": ["-y", "@anthropic/mcp-sequential-thinking"]
    }
  }
}
```

## Execution

### Option 1: Implement Onboarding Feature

```bash
cd /path/to/your-fork/ralph-orchestrator
git checkout -b feature/intelligent-onboarding

# Run RALPH to implement the onboarding feature
ralph run -P prompts/ONBOARDING_PROMPT.md -v
```

### Option 2: Implement TUI Feature

```bash
cd /path/to/your-fork/ralph-orchestrator
git checkout -b feature/realtime-tui

# Run RALPH to implement the TUI feature  
ralph run -P prompts/TUI_PROMPT.md -v
```

### Option 3: Use This Bootstrap (Recommended)

This bootstrap prompt handles branch creation, implementation, and PR submission:

```bash
ralph run -P prompts/BOOTSTRAP_SELF_IMPROVEMENT.md -v
```

## Feature Selection

Which feature are you implementing? (Select one by uncommenting)

```
# FEATURE_TARGET: onboarding
# FEATURE_TARGET: tui
```

## Implementation Instructions

### Phase 1: Setup (Iteration 1-3)

1. **Verify environment**
   - [ ] Confirm we're on the correct feature branch
   - [ ] Verify all MCP servers are accessible
   - [ ] Check that tests pass in current state

2. **Read the feature prompt**
   - For Onboarding: Read `prompts/ONBOARDING_PROMPT.md`
   - For TUI: Read `prompts/TUI_PROMPT.md`

3. **Create module structure**
   - Create necessary directories
   - Add `__init__.py` files
   - Set up basic imports

### Phase 2: Core Implementation (Iteration 4-50)

1. **Follow the Implementation Steps** in the feature prompt
2. **Write tests as you go** - TDD approach
3. **Commit after each major step** with descriptive messages
4. **Update progress** in the feature prompt file

### Phase 3: Integration (Iteration 51-70)

1. **Add CLI commands** to `__main__.py`
2. **Update documentation** in `docs/`
3. **Run full test suite** and fix any failures
4. **Verify backwards compatibility**

### Phase 4: Polish (Iteration 71-90)

1. **Run linting** with `ruff check src/`
2. **Fix any type errors** if using mypy
3. **Optimize performance** where needed
4. **Add examples** to `examples/`

### Phase 5: PR Submission (Iteration 91-100)

1. **Verify all success criteria** are met
2. **Push to fork**
   ```bash
   git push origin feature/<feature-name>
   ```

3. **Create Pull Request** using GitHub MCP:
   ```python
   # Using mcp_github_create_pull_request
   {
     "owner": "mikeyobrien",
     "repo": "ralph-orchestrator",
     "title": "feat: Intelligent Project Onboarding & Pattern Analysis",
     "body": "## Summary\n\nImplements intelligent project onboarding...",
     "head": "<your-username>:feature/intelligent-onboarding",
     "base": "main"
   }
   ```

4. **Mark task complete** in the feature prompt

## Success Criteria Validation

Before creating a PR, verify:

### For Onboarding Feature
- [ ] `ralph onboard --analyze` produces valid output
- [ ] `ralph onboard --apply` creates proper configuration files
- [ ] All new code has test coverage ≥90%
- [ ] Documentation is complete with examples
- [ ] All existing tests still pass
- [ ] No new linting errors

### For TUI Feature
- [ ] `ralph tui` launches the interface
- [ ] Real-time output streaming works
- [ ] All keyboard shortcuts function correctly
- [ ] WebSocket connection mode works
- [ ] Test coverage ≥85%
- [ ] Documentation includes screenshots

## Rollback Strategy

If issues occur:

```bash
# Reset to last good checkpoint
git log --oneline -10  # Find good commit
git reset --hard <good-commit>

# Or reset to main
git fetch upstream
git reset --hard upstream/main
```

## Monitoring Progress

Watch progress with the web UI:
```bash
# In another terminal
uv run python -m ralph_orchestrator.web
# Open http://localhost:8080
```

Or check metrics:
```bash
ls -la .agent/metrics/
cat .agent/metrics/metrics_*.json | jq '.summary'
```

## Communication Protocol

### Updating the prompt file

After completing each major step, update the relevant prompt file:
- Check off completed items in requirements
- Update the Progress section
- Add notes about any deviations

### Signaling completion

When all success criteria are met:
1. Add `- [x] TASK_COMPLETE` to the feature prompt
2. This will stop the orchestration loop
3. The final iteration should create the PR

## Notes

- This is a meta-prompt: RALPH improving RALPH
- The implementation should respect all existing conventions
- Focus on incremental, tested changes
- Each commit should leave the codebase in a working state
- Prefer small, focused iterations over large changes

## Progress Tracking

### Current Feature: [TO BE SELECTED]

### Status: NOT STARTED

### Session Notes:
(Add notes during implementation)

---

**Completion Marker:** When the PR is successfully created, add `- [x] TASK_COMPLETE` here.
