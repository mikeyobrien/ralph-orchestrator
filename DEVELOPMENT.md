# Development Guide

This guide documents the spec-driven development workflow for Ralph Orchestrator. All changes flow through specs—they are the source of truth.

## Quick Start

### Prerequisites

- [Nix](https://nixos.org/download.html) package manager
- [devenv](https://devenv.sh/getting-started/) 
- [direnv](https://direnv.net/docs/installation.html) (optional but recommended)

### Setup

**Option A: With direnv (Recommended)**

```bash
# Clone the repo
cd ralph-orchestrator

# Allow direnv to activate the dev environment
direnv allow

# You'll automatically have all tools (rustfmt, clippy, just, etc.)
```

**Option B: With nix develop**

```bash
cd ralph-orchestrator

# Enter the development shell
nix develop

# All tools are available in this shell
```

**Option C: Without Nix (Not Recommended)**

If you can't use Nix, you'll need to manually install:
- Rust toolchain with `rustup component add rustfmt clippy`
- [just](https://github.com/casey/just) command runner

### Development Workflow

```bash
# Install git hooks (one-time setup)
./scripts/setup-hooks.sh

# Run all checks (fmt, lint, test)
just check

# Format code
just fmt

# Run lints
just lint

# Run tests
just test
```

**Important:** Pre-commit hooks will block commits that fail formatting or linting. Run `just check` before committing.

## Core Principle

> **Specs are contracts, not documentation.** Implementation follows specs; specs don't follow implementation.

```
Spec → Review → Dogfood → Implement → Verify → Done
```

---

## Workflow Summary

| Change Type | Input | Process | Output |
|-------------|-------|---------|--------|
| **New Feature** | Idea/requirement | Create spec → Ralph implements | Working feature |
| **Modified Feature** | Spec update | Gap analysis → Ralph addresses | Updated implementation |
| **Bug Fix** | Bug report | ISSUES.md → Ralph fixes → Spec update | Fixed behavior + regression guard |

---

## New Feature Workflow

When adding a new capability to Ralph.

### Step 1: Create the Spec

Create a new spec file in `specs/`:

```bash
# Naming convention: <feature-name>.spec.md
touch specs/my-feature.spec.md
```

**Required spec structure:**

```markdown
---
status: draft
gap_analysis: null
related:
  - other-spec.spec.md
---

# Feature Name

## Overview
[1-2 paragraph description of what this feature does and why]

## Design
[How it works, key decisions, configuration options]

## Acceptance Criteria

### Criterion Name
- **Given** [precondition]
- **When** [action]
- **Then** [expected outcome]

[Repeat for each testable behavior]
```

**Guidelines:**
- No code examples in specs (implementation details)
- Focus on observable behavior, not internal mechanics
- Each acceptance criterion should be independently testable
- Reference `related:` specs when features interact

### Step 2: Dogfood the Spec

Before implementation, validate the spec itself:

```bash
# Read the spec as if you're implementing it for the first time
# Ask: "Can I build this from ONLY this spec and the codebase?"
```

**Checklist:**
- [ ] All acceptance criteria are testable
- [ ] No ambiguous requirements
- [ ] YAGNI check: is every feature actually needed?
- [ ] KISS check: is this the simplest solution?

Update `status: review` when ready.

### Step 3: Run Ralph to Implement

```bash
# Option A: Use the built-in spec implementation prompt
ralph start --prompt prompts/implement-spec-delta.md

# Option B: Create a focused PROMPT.md
cat > /tmp/ralph-impl/PROMPT.md << 'EOF'
Implement the spec at ./specs/my-feature.spec.md

## Rules
1. Read the spec completely before writing code
2. Implement ONLY what the spec requires
3. Add tests for each acceptance criterion
4. Run backpressure: cargo check && cargo test && cargo clippy

## Completion
Output LOOP_COMPLETE when all acceptance criteria pass.
EOF

cd /tmp/ralph-impl && ralph start
```

### Step 4: Verify Implementation

```bash
# Run all tests
cargo test

# Dogfood the implementation manually
# Try happy paths, error cases, edge cases
```

### Step 5: Update Spec Status

```yaml
---
status: implemented
gap_analysis: 2026-01-14
---
```

---

## Modified Feature Workflow

When updating existing functionality.

### Step 1: Update the Spec

Modify the spec to reflect the new desired behavior:

```bash
# Edit the spec
vim specs/existing-feature.spec.md

# Add/modify acceptance criteria
# Update design section if architecture changes
```

### Step 2: Run Gap Analysis

Gap analysis identifies differences between specs and implementation.

```bash
# Option A: Full automated gap analysis
ralph start --prompt prompts/spec-sync.md

# Option B: Manual gap analysis using Ralph
cat > /tmp/ralph-gap/PROMPT.md << 'EOF'
Perform gap analysis between specs and implementation.

## Process
1. Read all specs in ./specs/ with status != draft
2. For each acceptance criterion, verify implementation exists
3. Document gaps in GAPS.md

## Gap Categories
- **Breaking**: Spec says X, code does Y
- **Missing**: Spec describes feature, no implementation
- **Incomplete**: Feature exists but doesn't match spec
- **Untested**: Behavior exists but no test

## Output
Create/update GAPS.md with findings, then LOOP_COMPLETE.
EOF

cd /tmp/ralph-gap && ralph start
```

### Step 3: Review GAPS.md

After gap analysis, review the output:

```markdown
# GAPS.md structure
## Summary
| Priority | Issue | Spec | Status |
|----------|-------|------|--------|
| P0 | Critical bug | spec.md | NEW |
| P1 | Missing feature | spec.md | TODO |
| P2 | Minor issue | spec.md | BACKLOG |

## Details
[Detailed description of each gap]
```

**Priority levels:**
- **P0**: Breaking changes or critical bugs—fix immediately
- **P1**: Missing required features—fix before release
- **P2**: Minor gaps—address when convenient
- **P3**: Nice-to-have—future enhancement

### Step 4: Run Ralph to Address Gaps

```bash
cat > /tmp/ralph-fix/PROMPT.md << 'EOF'
Address gaps identified in GAPS.md

## Priority Order
1. Fix all P0 (breaking) issues first
2. Then P1 (missing) features
3. Backpressure after EACH fix: cargo check && cargo test

## Process
- Read the gap description
- Find the relevant spec section
- Implement the fix
- Add/update tests
- Mark gap as resolved

## Completion
When all P0 and P1 gaps are resolved, LOOP_COMPLETE.
EOF

cd /tmp/ralph-fix && ralph start
```

### Step 5: Update Gap Analysis Date

```yaml
---
status: implemented
gap_analysis: 2026-01-14  # Today's date
---
```

---

## Bug Fix Workflow

When fixing reported issues.

### Step 1: Document in ISSUES.md

Add the bug to `ISSUES.md`:

```markdown
## Active Issues

### [BUG-001] Brief description
- **Reported**: 2026-01-14
- **Severity**: P0/P1/P2
- **Symptoms**: What users observe
- **Expected**: What should happen
- **Spec reference**: Which spec defines correct behavior (if any)
- **Status**: NEW → IN_PROGRESS → FIXED → VERIFIED
```

### Step 2: Run Ralph to Fix

```bash
cat > /tmp/ralph-bugfix/PROMPT.md << 'EOF'
Fix the bug described in ISSUES.md: [BUG-001]

## Process
1. Read the issue description
2. Reproduce the bug (write a failing test)
3. Find the root cause
4. Fix the code
5. Verify the test passes
6. Run full test suite

## Important
- The failing test MUST exist before fixing
- This prevents regressions

## Completion
When bug is fixed AND test passes, LOOP_COMPLETE.
EOF

cd /tmp/ralph-bugfix && ralph start
```

### Step 3: Update Specs for Regression Prevention

**Critical**: Ensure the spec captures the correct behavior.

```bash
# Check if spec exists for this behavior
grep -r "relevant keyword" specs/

# If spec exists but doesn't cover the bug case:
# Add acceptance criterion to the spec

# If no spec exists:
# Consider if this warrants a spec or just a test
```

**Add to spec:**

```markdown
### Edge Case: [Bug description]
- **Given** [conditions that triggered the bug]
- **When** [action that exposed the bug]
- **Then** [correct behavior, not the bug]
```

### Step 4: Update ISSUES.md

```markdown
### [BUG-001] Brief description
- **Status**: VERIFIED
- **Resolution**: Fixed in commit abc123
- **Regression test**: Added to spec-name.spec.md
```

---

## Quick Reference

### Commands

```bash
# New feature implementation
ralph start --prompt prompts/implement-spec-delta.md

# Full gap analysis
ralph start --prompt prompts/spec-sync.md

# Check spec status
grep -r "^status:" specs/*.spec.md

# Find specs missing gap analysis
grep -l "gap_analysis: null" specs/*.spec.md
```

### Spec Status Lifecycle

```
draft → review → approved → implemented → deprecated
  │        │         │            │
  │        │         │            └─ Periodically run gap analysis
  │        │         └─ Ralph implements
  │        └─ Dogfood and refine
  └─ Initial creation
```

### File Locations

| File | Purpose |
|------|---------|
| `specs/*.spec.md` | Feature specifications |
| `ISSUES.md` | Bug tracking and gap analysis results |
| `GAPS.md` | Output from gap analysis runs |
| `prompts/spec-sync.md` | Ralph prompt for full gap analysis |
| `prompts/implement-spec-delta.md` | Ralph prompt for spec implementation |
| `CLAUDE.md` | Agent instructions (dogfooding process) |

### Backpressure Commands

Always run after code changes:

```bash
cargo check           # Type checking
cargo test            # Run tests
cargo clippy -- -D warnings  # Lint
```

---

## Anti-Patterns

### ❌ Implementation Without Spec
```
Bad:  "I'll just add this feature real quick"
Good: "Let me create a spec first"
```

### ❌ Spec After Implementation
```
Bad:  "I built it, now I'll document it"
Good: "Spec defines behavior, implementation follows"
```

### ❌ Skipping Gap Analysis
```
Bad:  "I updated the spec, it's probably fine"
Good: "Run gap analysis to verify implementation matches"
```

### ❌ Bug Fix Without Regression Test
```
Bad:  "Fixed the bug, moving on"
Good: "Fixed the bug, added test, updated spec"
```

### ❌ Over-Engineering
```
Bad:  "While I'm here, let me also refactor this..."
Good: "Fix only what the spec/issue requires"
```

---

## Workflows with Ralph Loop

### Running Ralph in Isolated Directory

**Important**: Always run Ralph loops in a temp directory to avoid polluting the workspace.

```bash
# Create isolated workspace
WORK_DIR=$(mktemp -d)
cp -r . "$WORK_DIR"
cd "$WORK_DIR"

# Run Ralph
ralph start

# Review changes, cherry-pick what you want
```

### Parallel Workflows

For large gap analyses, run multiple Ralph instances:

```bash
# Terminal 1: Fix P0 issues
WORK1=$(mktemp -d) && cp -r . "$WORK1" && cd "$WORK1"
ralph start --prompt "Fix P0 gaps from GAPS.md"

# Terminal 2: Fix P1 issues (independent)
WORK2=$(mktemp -d) && cp -r . "$WORK2" && cd "$WORK2"
ralph start --prompt "Fix P1 gaps from GAPS.md"
```

---

## Behavioral Verification

For critical behaviors, use the behavioral verification catalog:

```bash
# Verify specific behaviors
ralph /verify-behaviors --category planner

# Verify single behavior
ralph /verify-behaviors --id PL-007

# Update behavior catalog after spec changes
ralph /update-behaviors
```

See `specs/behavioral-verification.spec.md` for the full catalog.

---

## Appendix: Spec Template

```markdown
---
status: draft
gap_analysis: null
related: []
---

# Feature Name

## Overview

[Brief description of the feature and its purpose]

## Design

### Configuration

[Any configuration options]

### Behavior

[How the feature works]

## Acceptance Criteria

### Happy Path
- **Given** [precondition]
- **When** [action]
- **Then** [expected outcome]

### Error Handling
- **Given** [error condition]
- **When** [action]
- **Then** [graceful handling]

### Edge Cases
- **Given** [edge case]
- **When** [action]
- **Then** [correct behavior]
```
