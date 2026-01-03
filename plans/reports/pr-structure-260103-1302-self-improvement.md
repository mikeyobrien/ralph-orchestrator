# PR Structure: Self-Improvement System

**Date:** 2026-01-03

## Summary

Documented the PR/Issue structure for Ralph's self-improvement system.

## PR #16: Self-Improvement Runner

**Core file:** `scripts/self_improve.py` (the only file that really matters)

Supporting files in PR (less important):
- `.gitignore` additions
- `examples/ralph-self-improvement.yml`
- `examples/run_self_improvement.py`
- `prompts/ONBOARDING_PROMPT.md`
- `ralph.yml` config updates
- `tests/test_acp_client.py` minor fix

## Issue #17: Validation Gates Proposal

References PR #16 - validation would be built USING the self-improvement runner.

**Comment added:** https://github.com/mikeyobrien/ralph-orchestrator/issues/17#issuecomment-3707246054

## Key Understanding

```
PR #16 (scripts/self_improve.py)
    └── The Tool

Issue #17 (Validation Gates)
    └── Feature built WITH the tool

Future Issues/PRs
    ├── TUI feature
    ├── Onboarding feature
    └── Other features built with self_improve.py
```

## Actions Taken

1. Added comment to Issue #17 linking it to PR #16
2. Clarified that PR #16 is really about one core file
3. Documented the relationship: tool vs features built with tool
