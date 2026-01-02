# Task: RALPH Self-Improvement - Build Onboarding & TUI Features

**YOU ARE RALPH ORCHESTRATOR BUILDING YOURSELF.**

This is a meta-prompt that instructs RALPH to implement two major features on its own codebase. You will analyze the existing code, implement new modules, write tests, and update documentation.

## Mission

Implement these two features:

### 1. Intelligent Project Onboarding (`prompts/ONBOARDING_PROMPT.md`)
Enable ANY Claude Code user to automatically onboard their existing projects to RALPH. The system analyzes:
- `~/.claude/projects/[hash]/*.jsonl` - Conversation history 
- `CLAUDE.md` files - Project instructions
- `.mcp.json` - MCP server configurations
- Project manifests (package.json, pyproject.toml, etc.)

And generates optimized `ralph.yml` and system prompts based on proven workflows.

### 2. Real-Time TUI (`prompts/TUI_PROMPT.md`)
Build a beautiful terminal interface using Textual for live orchestration monitoring:
- Real-time progress display
- Streaming agent output
- Live metrics visualization
- Keyboard navigation and controls

---

## Execution Protocol

### Pre-Flight Checks
Before starting, verify:
1. [ ] Read `prompts/ONBOARDING_PROMPT.md` completely
2. [ ] Read `prompts/TUI_PROMPT.md` completely
3. [ ] Understand existing code structure in `src/ralph_orchestrator/`
4. [ ] Review `src/ralph_orchestrator/__main__.py` for CLI patterns
5. [ ] Check `pyproject.toml` for dependency management

### Phase 1: Onboarding Feature (HIGH PRIORITY)

**Objective**: Create `ralph onboard <project_path>` CLI command

**Implementation Order**:
1. Create `src/ralph_orchestrator/onboarding/__init__.py`
2. Implement `scanner.py` - Find Claude history, CLAUDE.md, MCP configs
3. Implement `history_analyzer.py` - Parse JSONL, extract tool patterns
4. Implement `pattern_extractor.py` - Identify workflows
5. Implement `config_generator.py` - Generate ralph.yml
6. Add CLI subcommand in `__main__.py`
7. Write tests in `tests/test_onboarding.py`
8. Create `docs/guide/onboarding.md`

**Key Technical Challenges**:
- Parsing Claude Code's JSONL format (multiple message types)
- Mapping project paths to `~/.claude/projects/[hash]/` directories
- Handling projects with no conversation history gracefully
- Generating valid YAML configuration

**Success Markers**:
```bash
# These should work when Phase 1 is complete:
ralph onboard ~/projects/my-app --analyze-only
ralph onboard ~/projects/my-app
ralph onboard . --merge
```

### Phase 2: TUI Feature (MEDIUM PRIORITY)

**Objective**: Create `ralph tui` and `ralph watch` CLI commands

**Implementation Order**:
1. Add `textual` dependency to `pyproject.toml`
2. Create `src/ralph_orchestrator/tui/__init__.py`
3. Implement main app in `tui/app.py`
4. Create widgets: `progress.py`, `output.py`, `tasks.py`, `metrics.py`
5. Add styling in `tui/ralph.tcss`
6. Implement `connection.py` for orchestrator communication
7. Add CLI subcommands in `__main__.py`
8. Write tests in `tests/test_tui.py`
9. Create `docs/guide/tui.md`

**Key Technical Challenges**:
- Integrating Textual's async event loop with orchestrator
- Real-time data streaming without blocking
- Responsive layout for different terminal sizes
- Smooth animations without flickering

**Success Markers**:
```bash
# These should work when Phase 2 is complete:
ralph tui -P PROMPT.md
ralph watch
```

---

## Code Quality Standards

### Python Style
- Type hints for all functions
- Docstrings for all public classes/methods
- Follow existing code patterns in the codebase
- Use dataclasses for data models
- Use async/await for I/O operations

### Testing
- Unit tests for all new modules
- Integration tests for CLI commands
- Mock external dependencies (file system, Claude history)
- Target 85%+ coverage

### Documentation
- Update relevant docs in `docs/`
- Add CLI help text
- Include usage examples

---

## Tool Usage Guidelines

### File Operations
- Use `Read` to examine existing files before modifying
- Use `Edit` for targeted changes to existing files
- Use `Write` for new files
- Use `Grep` to find patterns across the codebase

### Testing
- Run `pytest tests/test_onboarding.py -v` after implementing onboarding
- Run `pytest tests/test_tui.py -v` after implementing TUI
- Run `ruff check src/` to verify code quality

### Git
- Commit after completing each major component
- Use descriptive commit messages
- Check git status before committing

---

## Completion Checklist

### Onboarding Feature
- [ ] `src/ralph_orchestrator/onboarding/` module exists
- [ ] `scanner.py` finds all data sources
- [ ] `history_analyzer.py` parses JSONL files
- [ ] `pattern_extractor.py` identifies workflows
- [ ] `config_generator.py` generates ralph.yml
- [ ] CLI `ralph onboard` command works
- [ ] Tests pass with 85%+ coverage
- [ ] Documentation is complete

### TUI Feature
- [ ] `src/ralph_orchestrator/tui/` module exists
- [ ] Main TUI application launches
- [ ] Progress panel shows real-time updates
- [ ] Output viewer streams agent output
- [ ] Metrics panel displays live stats
- [ ] Task sidebar shows queue
- [ ] Keyboard shortcuts work
- [ ] CLI `ralph tui` command works
- [ ] Tests pass with 85%+ coverage
- [ ] Documentation is complete

---

## When Complete

When BOTH features are fully implemented and tested:

1. Update this file to mark completion:
   ```markdown
   - [x] TASK_COMPLETE
   ```

2. Update the individual feature prompts:
   - `prompts/ONBOARDING_PROMPT.md` → Status: ✅ COMPLETE
   - `prompts/TUI_PROMPT.md` → Status: ✅ COMPLETE

3. Run final verification:
   ```bash
   pytest tests/ -v
   ruff check src/
   ralph onboard --help
   ralph tui --help
   ```

---

## Current Status

**Status**: 🚧 IN PROGRESS
**Phase**: Starting Phase 1 (Onboarding)
**Priority**: Onboarding > TUI

### Next Action
Begin implementing `src/ralph_orchestrator/onboarding/scanner.py` to find and inventory all Claude Code data sources for a project.

---

- [ ] TASK_COMPLETE
