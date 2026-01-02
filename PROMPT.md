# RALPH Self-Improvement: Intelligent Project Onboarding

This branch is configured to implement the **Intelligent Project Onboarding & Pattern Analysis** feature.

## Quick Start

```bash
# Run RALPH to implement this feature
ralph run -c examples/ralph-self-improvement.yml -P prompts/ONBOARDING_PROMPT.md -v
```

## What This Does

RALPH will work on implementing:
- Project analysis and pattern recognition
- MCP server recommendations based on project type
- Configuration generation (`ralph.yml`, `CLAUDE.md`)
- CLI commands: `ralph onboard --analyze`, `ralph onboard --apply`
- Comprehensive tests and documentation

## Feature Specification

See `prompts/ONBOARDING_PROMPT.md` for full details.

## Success Criteria

- [ ] `ralph onboard` correctly identifies project type
- [ ] Pattern extraction works from `.agent/` history
- [ ] Generated configurations work without modification
- [ ] 90%+ test coverage on new code
- [ ] Documentation complete with examples

---

**Status:** 🚧 Ready to Start

**Completion Marker:** When complete, add `- [x] TASK_COMPLETE` here.
