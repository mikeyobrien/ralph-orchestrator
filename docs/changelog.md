# Changelog

All notable changes to Ralph Orchestrator will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- **Completion Marker Detection**: Task can now signal completion via `- [x] TASK_COMPLETE` checkbox marker in prompt file
  - Orchestrator checks for marker before each iteration
  - Immediately exits loop when marker is found
  - Supports both `- [x] TASK_COMPLETE` and `[x] TASK_COMPLETE` formats
- **Loop Detection**: Automatic detection of repetitive agent outputs using rapidfuzz
  - Compares current output against last 5 outputs
  - Uses 90% similarity threshold to detect loops
  - Prevents infinite loops from runaway agents
- New dependency: `rapidfuzz>=3.0.0,<4.0.0` for fast fuzzy string matching
- Documentation static site with MkDocs
- Comprehensive API reference documentation
- Additional example scenarios
- Performance monitoring tools

### Changed

- Improved error handling in agent execution
- Enhanced checkpoint creation logic
- `SafetyGuard.reset()` now also clears loop detection history

### Fixed

- Race condition in state file updates
- Memory leak in long-running sessions

## [1.2.0] - 2025-12

### Added

- **ACP (Agent Client Protocol) Support**: Full integration with ACP-compliant agents
  - JSON-RPC 2.0 message protocol implementation
  - Permission handling with four modes: `auto_approve`, `deny_all`, `allowlist`, `interactive`
  - File operations (`fs/read_text_file`, `fs/write_text_file`) with security validation
  - Terminal operations (`terminal/create`, `terminal/output`, `terminal/wait_for_exit`, `terminal/kill`, `terminal/release`)
  - Session management and streaming updates
  - Agent scratchpad mechanism for context persistence across iterations
- New CLI options: `--acp-agent`, `--acp-permission-mode`
- ACP configuration support in `ralph.yml` under `adapters.acp`
- Environment variable overrides: `RALPH_ACP_AGENT`, `RALPH_ACP_PERMISSION_MODE`, `RALPH_ACP_TIMEOUT`
- 305+ new ACP-specific tests

### Changed

- Expanded test suite to 920+ tests
- Updated documentation for ACP support

## [1.1.0] - 2025-12

### Added

- Async-first architecture for non-blocking operations
- Thread-safe async logging with rotation and security masking
- Rich terminal output with syntax highlighting
- Inline prompt support (`-p "your task"`)
- Claude Agent SDK integration with MCP server support
- Async git checkpointing (non-blocking)
- Security validation system with path traversal protection
- Sensitive data masking in logs (API keys, tokens, passwords)
- Thread-safe configuration with RLock
- VerboseLogger with session metrics and re-entrancy protection
- Iteration statistics tracking with memory-efficient storage

### Changed

- Expanded test suite to 620+ tests
- Improved error handling with ClaudeErrorFormatter
- Enhanced signal handling with subprocess-first cleanup

### Fixed

- Division by zero in countdown progress bar
- Process reference leak in QChatAdapter
- Blocking file I/O in async functions
- Exception chaining in error handlers

## [1.0.3] - 2025-09-07

### Added

- Production deployment guide
- Docker support with Dockerfile and docker-compose.yml
- Kubernetes deployment manifests
- Health check endpoint for monitoring

### Changed

- Improved resource limit handling
- Enhanced logging with structured JSON output
- Updated dependencies to latest versions

### Fixed

- Git checkpoint creation on Windows
- Agent timeout handling in edge cases

## [1.0.2] - 2025-09-07

### Added

- Q Chat integration improvements
- Real-time metrics collection
- Interactive CLI mode
- Bash and ZSH completion scripts

### Changed

- Refactored agent manager for better extensibility
- Improved context window management
- Enhanced progress reporting

### Fixed

- Unicode handling in prompt files
- State persistence across interruptions

## [1.0.1] - 2025-09-07

### Added

- Gemini CLI integration
- Advanced context management strategies
- Cost tracking and estimation
- HTML report generation

### Changed

- Optimized iteration performance
- Improved error recovery mechanisms
- Enhanced Git operations

### Fixed

- Agent detection on macOS
- Prompt archiving with special characters
- Checkpoint interval calculation

## [1.0.0] - 2025-09-07

### Added

- Initial release with core functionality
- Claude CLI integration
- Q Chat integration
- Git-based checkpointing
- Prompt archiving
- State persistence
- Comprehensive test suite
- CLI wrapper script
- Configuration management
- Metrics collection

### Features

- Auto-detection of available AI agents
- Configurable iteration and runtime limits
- Error recovery with exponential backoff
- Verbose and dry-run modes
- JSON configuration file support
- Environment variable configuration

### Documentation

- Complete README with examples
- Installation instructions
- Usage guide
- API documentation
- Contributing guidelines

## [0.9.0] - 2025-09-06 (Beta)

### Added

- Beta release for testing
- Basic orchestration loop
- Claude integration
- Simple checkpointing

### Known Issues

- Limited error handling
- No metrics collection
- Single agent support only

## [0.5.0] - 2025-09-05 (Alpha)

### Added

- Initial alpha release
- Proof of concept implementation
- Basic Ralph loop
- Manual testing only

---

## Version History Summary

### Major Versions

- **1.0.0** - First stable release with full feature set
- **0.9.0** - Beta release for community testing
- **0.5.0** - Alpha proof of concept

### Versioning Policy

We use Semantic Versioning (SemVer):

- **MAJOR** version for incompatible API changes
- **MINOR** version for backwards-compatible functionality additions
- **PATCH** version for backwards-compatible bug fixes

### Deprecation Policy

Features marked for deprecation will:

1. Be documented in the changelog
2. Show deprecation warnings for 2 minor versions
3. Be removed in the next major version

### Support Policy

- **Current version**: Full support with bug fixes and features
- **Previous minor version**: Bug fixes only
- **Older versions**: Community support only

## Upgrade Guide

### From 0.x to 1.0

1. **Configuration Changes**
   - Old: `max_iter` → New: `max_iterations`
   - Old: `agent_name` → New: `agent`

2. **API Changes**
   - `RalphOrchestrator.execute()` → `RalphOrchestrator.run()`
   - Return format changed from tuple to dictionary

3. **File Structure**
   - State files moved from `.ralph/` to `.agent/metrics/`
   - Checkpoint format updated

### Migration Script

```bash
#!/bin/bash
# Migrate from 0.x to 1.0

# Backup old data
cp -r .ralph .ralph.backup

# Create new structure
mkdir -p .agent/metrics .agent/prompts .agent/checkpoints

# Migrate state files
mv .ralph/*.json .agent/metrics/ 2>/dev/null

# Update configuration
if [ -f "ralph.conf" ]; then
    python -c "
import json
with open('ralph.conf') as f:
    old_config = json.load(f)
# Update keys
old_config['max_iterations'] = old_config.pop('max_iter', 100)
old_config['agent'] = old_config.pop('agent_name', 'auto')
# Save new config
with open('ralph.json', 'w') as f:
    json.dump(old_config, f, indent=2)
"
fi

echo "Migration complete!"
```

## Release Process

### 1. Pre-release Checklist

- [ ] All tests passing
- [ ] Documentation updated
- [ ] Changelog updated
- [ ] Version bumped in setup.py
- [ ] README examples tested

### 2. Release Steps

```bash
# 1. Update version
vim setup.py  # Update version number

# 2. Commit changes
git add -A
git commit -m "Release version X.Y.Z"

# 3. Tag release
git tag -a vX.Y.Z -m "Version X.Y.Z"

# 4. Push to GitHub
git push origin main --tags

# 5. Create GitHub release
gh release create vX.Y.Z --title "Version X.Y.Z" --notes-file RELEASE_NOTES.md

# 6. Publish to PyPI (if applicable)
python setup.py sdist bdist_wheel
twine upload dist/*
```

### 3. Post-release

- [ ] Announce on social media
- [ ] Update documentation site
- [ ] Close related issues
- [ ] Plan next release

## Contributors

Thanks to all contributors who have helped improve Ralph Orchestrator:

- Geoffrey Huntley (@ghuntley) - Original Ralph Wiggum technique
- Community contributors via GitHub

## How to Contribute

See [CONTRIBUTING.md](contributing.md) for details on:

- Reporting bugs
- Suggesting features
- Submitting pull requests
- Development setup

## Links

- [GitHub Repository](https://github.com/mikeyobrien/ralph-orchestrator)
- [Issue Tracker](https://github.com/mikeyobrien/ralph-orchestrator/issues)
- [Discussions](https://github.com/mikeyobrien/ralph-orchestrator/discussions)
- [Documentation](https://mikeyobrien.github.io/ralph-orchestrator/)
