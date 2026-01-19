# Preset Reference

Ralph includes 23 pre-configured hat collections for common development workflows. Presets provide ready-to-use configurations that encode proven patterns.

## Using Presets

```bash
# List all presets
ralph init --list-presets

# Initialize with a preset
ralph init --preset feature

# Override backend
ralph init --preset tdd-red-green --backend gemini

# Run directly from preset (without copying)
ralph run -c presets/research.yml -p "How does auth work?"

# Override existing config
ralph init --preset debug --force
```

## Preset Categories

### Development Workflows

| Preset | Hats | Completion | Use Case |
|--------|------|------------|----------|
| [`feature`](#feature) | builder, reviewer | `LOOP_COMPLETE` | Standard feature development with review |
| [`feature-minimal`](#feature-minimal) | builder, reviewer | `LOOP_COMPLETE` | Feature dev with auto-derived instructions |
| [`tdd-red-green`](#tdd-red-green) | test_writer, implementer, refactorer | `LOOP_COMPLETE` | Test-driven development |
| [`spec-driven`](#spec-driven) | spec_writer, spec_reviewer, implementer, verifier | `LOOP_COMPLETE` | Specification-first development |
| [`refactor`](#refactor) | planner, refactorer, verifier | `REFACTOR_COMPLETE` | Safe incremental refactoring |

### Quality & Review

| Preset | Hats | Completion | Use Case |
|--------|------|------------|----------|
| [`review`](#review) | reviewer, analyzer | `REVIEW_COMPLETE` | Code review without changes |
| [`pr-review`](#pr-review) | correctness_reviewer, security_reviewer, architecture_reviewer, synthesizer | `LOOP_COMPLETE` | Multi-perspective PR review |
| [`adversarial-review`](#adversarial-review) | builder, red_team, fixer | `LOOP_COMPLETE` | Red team security review |
| [`gap-analysis`](#gap-analysis) | analyzer, verifier, reporter | `GAP_ANALYSIS_COMPLETE` | Spec vs implementation comparison |

### Debugging & Investigation

| Preset | Hats | Completion | Use Case |
|--------|------|------------|----------|
| [`debug`](#debug) | investigator, tester, fixer, verifier | `DEBUG_COMPLETE` | Bug investigation and fixing |
| [`incident-response`](#incident-response) | observer, mitigator, investigator | `LOOP_COMPLETE` | Production incident handling |
| [`code-archaeology`](#code-archaeology) | surveyor, historian, archaeologist, modifier | `LOOP_COMPLETE` | Understanding legacy code |
| [`scientific-method`](#scientific-method) | observer, theorist, experimenter, fixer | `LOOP_COMPLETE` | Hypothesis-driven investigation |

### Documentation

| Preset | Hats | Completion | Use Case |
|--------|------|------------|----------|
| [`docs`](#docs) | writer, reviewer | `DOCS_COMPLETE` | Documentation writing |
| [`documentation-first`](#documentation-first) | documenter, reviewer, implementer, verifier | `LOOP_COMPLETE` | Write docs before code |

### Specialized Workflows

| Preset | Hats | Completion | Use Case |
|--------|------|------------|----------|
| [`api-design`](#api-design) | consumer, designer, critic, implementer | `LOOP_COMPLETE` | Consumer-driven API design |
| [`migration-safety`](#migration-safety) | planner, expander, migrator, contractor | `LOOP_COMPLETE` | Safe database/system migrations |
| [`performance-optimization`](#performance-optimization) | profiler, analyst, optimizer | `LOOP_COMPLETE` | Data-driven performance tuning |
| [`deploy`](#deploy) | builder, deployer, verifier | `LOOP_COMPLETE` | Deployment workflow |

### Learning & Collaboration

| Preset | Hats | Completion | Use Case |
|--------|------|------------|----------|
| [`research`](#research) | researcher, synthesizer | `RESEARCH_COMPLETE` | Codebase exploration, no changes |
| [`socratic-learning`](#socratic-learning) | explorer, questioner, answerer | `LOOP_COMPLETE` | Learning through questions |
| [`mob-programming`](#mob-programming) | navigator, driver, observer | `LOOP_COMPLETE` | Rotating role development |

### Baseline

| Preset | Hats | Completion | Use Case |
|--------|------|------------|----------|
| [`hatless-baseline`](#hatless-baseline) | (none) | `LOOP_COMPLETE` | Solo mode baseline testing |

---

## Preset Details

### feature

**Standard feature development with integrated code review.**

Pattern: Planner-Builder-Reviewer

```bash
ralph run --config presets/feature.yml -p "Add user authentication"
```

**Hats:**
- **Builder**: Implements one task with backpressure. One task, one commit.
- **Reviewer**: Reviews implementation for quality. Does NOT modify code.

**Event Flow:**
```
task.start → [Ralph] → build.task → [Builder] → build.done → [Ralph] →
  review.request → [Reviewer] → review.approved → [Ralph] → ...
```

**When to Use:**
- Standard feature development
- When you want code review built into the workflow
- Tasks requiring multiple implementation steps

---

### feature-minimal

**Same as feature but using auto-derived instructions.**

Pattern: Planner-Builder-Reviewer (minimal config)

```bash
ralph run --config presets/feature-minimal.yml -p "Add user authentication"
```

**Hats:**
- **Builder**: Implements code changes. One task, one commit. (instructions derived from contract)
- **Reviewer**: Reviews implementation for quality. (instructions derived from contract)

**When to Use:**
- When you want minimal configuration
- Testing auto-derived instruction behavior

---

### tdd-red-green

**Test-driven development with red-green-refactor cycle.**

Pattern: Critic-Actor Pipeline

```bash
ralph run --config presets/tdd-red-green.yml -p "Implement a binary search tree"
```

**Hats:**
- **Test Writer**: Writes FAILING tests first. Never implements.
- **Implementer**: Makes failing test pass with MINIMAL code. No extras.
- **Refactorer**: Cleans up code while keeping tests green.

**Event Flow:**
```
tdd.start → [Test Writer] → test.written → [Implementer] → test.passing →
  [Refactorer] → refactor.done → [Test Writer] → ...
            └── cycle.complete → LOOP_COMPLETE
```

**When to Use:**
- TDD-driven feature development
- When you want strict test-first discipline
- Building well-tested code from scratch

---

### spec-driven

**Specification-first development. The spec is the contract.**

Pattern: Contract-First Pipeline

```bash
ralph run --config presets/spec-driven.yml -p "Build a rate limiter"
```

**Hats:**
- **Spec Writer**: Creates precise, unambiguous specifications with examples.
- **Spec Critic**: Reviews spec for completeness. Could someone implement from this?
- **Implementer**: Implements EXACTLY what the spec says. No creative interpretation.
- **Verifier**: Verifies implementation matches spec exactly.

**Event Flow:**
```
spec.start → [Spec Writer] → spec.ready → [Spec Critic] →
  spec.approved → [Implementer] → implementation.done →
  [Verifier] → task.complete → LOOP_COMPLETE
         └── spec.rejected → [Spec Writer]
```

**When to Use:**
- Building features from requirements
- When precision matters more than speed
- APIs and contracts that must be exact

---

### refactor

**Safe incremental refactoring with verification.**

Pattern: Plan-Execute-Verify

```bash
ralph run --config presets/refactor.yml -p "Extract authentication into a separate module"
```

**Hats:**
- **Planner**: Plans refactoring steps
- **Refactorer**: Executes one refactoring step at a time
- **Verifier**: Verifies each step leaves codebase working

**Key Principle:** Every step leaves the codebase in a working state.

**When to Use:**
- Large refactoring efforts
- When you need safety guarantees
- Restructuring without changing behavior

---

### review

**Code review without making changes. Produces structured feedback.**

Pattern: Analyze-Critique

```bash
ralph run --config presets/review.yml -p "Review the auth module for security issues"
```

**Hats:**
- **Reviewer**: Reviews code with structured feedback
- **Analyzer**: Deep analysis of specific areas

**Categories produced:**
- **Critical**: Must fix before merge
- **Suggestions**: Should consider
- **Nitpicks**: Optional improvements

**When to Use:**
- Pre-merge code review
- Security audits
- Code quality assessment

---

### pr-review

**Multi-perspective code review for pull requests.**

Pattern: Multiple Critics + Synthesis

```bash
ralph run --config presets/pr-review.yml -p "Review PR #123"
```

**Hats:**
- **Correctness Reviewer**: Reviews for logical correctness and requirement alignment
- **Security Reviewer**: Reviews for security vulnerabilities
- **Architecture Reviewer**: Reviews for architectural fit and maintainability
- **Synthesizer**: Combines all feedback into final PR review

**Event Flow:**
```
task.start → [Ralph] → review.correctness → [Correctness Reviewer] → correctness.done
                    → review.security    → [Security Reviewer]    → security.done
                    → review.architecture → [Architecture Reviewer] → architecture.done
                    → synthesis.request → [Synthesizer] → review.complete → LOOP_COMPLETE
```

**When to Use:**
- Comprehensive PR review
- When multiple perspectives matter
- Critical code paths

---

### adversarial-review

**Red team / blue team security review.**

Pattern: Adversarial Critic-Actor

```bash
ralph run --config presets/adversarial-review.yml -p "Review authentication system"
```

**Hats:**
- **Blue Team (Builder)**: Implements features with security in mind
- **Red Team (Attacker)**: Penetration tester that actively tries to break the code
- **Security Fixer**: Remediates discovered vulnerabilities

**Event Flow:**
```
security.review → [Blue Team] → build.ready → [Red Team] →
  security.approved → LOOP_COMPLETE
  vulnerability.found → [Fixer] → fix.applied → [Red Team] → ...
```

**When to Use:**
- Security-critical code
- Authentication and authorization
- Payment processing

---

### gap-analysis

**Deep comparison of specs against implementation.**

Pattern: Analyze-Verify-Report

```bash
# Full gap analysis
ralph run --config presets/gap-analysis.yml

# Focus on specific spec
ralph run --config presets/gap-analysis.yml -p "Focus on cli-adapters.spec.md"
```

**Hats:**
- **Analyzer**: Reads specs and identifies acceptance criteria
- **Verifier**: Checks each criterion against implementation
- **Reporter**: Writes findings to ISSUES.md

**Output Categories:**
- **Critical Gaps**: Spec violations (implementation contradicts spec)
- **Missing Features**: Acceptance criteria not implemented
- **Undocumented Behavior**: Code without spec coverage
- **Spec Improvements**: Ambiguities, missing details

**When to Use:**
- Spec compliance audits
- Pre-release verification
- Technical debt assessment

---

### debug

**Bug investigation following scientific method.**

Pattern: Hypothesize-Test-Fix

```bash
ralph run --config presets/debug.yml -p "Tests fail intermittently in CI"
```

**Hats:**
- **Investigator**: Finds root cause through systematic investigation
- **Tester**: Designs and runs experiments to test hypotheses
- **Fixer**: Implements fix with regression test
- **Verifier**: Verifies fix solves original problem

**Scientific Method:**
1. Observe symptoms
2. Form hypothesis
3. Predict behavior
4. Test prediction
5. Narrow down or fix

**When to Use:**
- Complex bugs
- Intermittent failures
- Mysterious behavior

---

### incident-response

**Production incident handling with OODA loop.**

Pattern: Observe-Orient-Decide-Act

```bash
ralph run --config presets/incident-response.yml -p "API latency spike in production"
```

**Hats:**
- **Observer**: Rapidly assesses severity and impact
- **Mitigator**: Stops the bleeding (rollback, feature flags, scaling)
- **Investigator**: Finds root cause for postmortem

**Mitigation Priority (fastest to slowest):**
1. Rollback recent deploy
2. Feature flag disable
3. Scale up resources
4. Redirect traffic
5. Circuit breaker

**When to Use:**
- Production incidents
- When speed matters
- Outage response

---

### code-archaeology

**Understanding legacy code before changing it.**

Pattern: Archaeological Dig

```bash
ralph run --config presets/code-archaeology.yml -p "Understand the payment processing module"
```

**Hats:**
- **Surveyor**: Creates map of code territory, data flow, dependencies
- **Historian**: Researches git history to understand the "why"
- **Archaeologist**: Identifies hidden assumptions, tech debt, gotchas
- **Modifier**: Makes minimal changes informed by findings

**Artifacts Catalogued:**
- Hidden assumptions
- Implicit contracts
- Technical debt (TODOs, FIXMEs)
- Fragile areas
- Magic numbers

**When to Use:**
- Working with legacy code
- Before major refactoring
- Understanding unfamiliar codebase

---

### scientific-method

**Hypothesis-driven investigation.**

Pattern: Scientific Method

```bash
ralph run --config presets/scientific-method.yml -p "Memory leak in background worker"
```

**Hats:**
- **Observer**: Gathers factual observations systematically
- **Theorist**: Forms testable hypotheses
- **Experimenter**: Designs and runs experiments
- **Fixer**: Applies fix with regression test

**When to Use:**
- Complex debugging
- When intuition fails
- Need systematic approach

---

### docs

**Documentation writing with quality control.**

Pattern: Write-Review

```bash
ralph run --config presets/docs.yml -p "Write API documentation for auth module"
```

**Hats:**
- **Writer**: Writes documentation sections with clarity and precision
- **Reviewer**: Reviews for accuracy, clarity, and completeness

**Style Guidelines:**
- Use active voice
- Lead with "what" and "why" before "how"
- Code examples should be copy-pasteable
- Link to related sections

**When to Use:**
- Writing documentation
- API references
- User guides

---

### documentation-first

**README-driven development. Write docs before code.**

Pattern: Documentation-First

```bash
ralph run --config presets/documentation-first.yml -p "Build a CLI tool for file conversion"
```

**Hats:**
- **Documenter**: Writes documentation BEFORE any code exists
- **Reviewer**: Reviews docs - can someone implement from this?
- **Implementer**: Implements to match documentation exactly
- **Verifier**: Verifies implementation matches docs

**Doc Sections Required:**
1. Problem Statement
2. Solution Overview
3. Usage Guide (runnable examples)
4. API Reference
5. Edge Cases

**When to Use:**
- User-facing features
- APIs and CLIs
- When clarity of design matters

---

### api-design

**Consumer-driven API design.**

Pattern: Outside-In Design

```bash
ralph run --config presets/api-design.yml -p "Design REST API for user management"
```

**Hats:**
- **Consumer**: Writes usage examples as if API already exists
- **Designer**: Designs interfaces, signatures, error types
- **Critic**: Reviews for usability issues and footguns
- **Implementer**: Implements the approved design

**Design Principles:**
- Clarity > cleverness
- Consistency > convenience
- Explicit > implicit
- Predictable > powerful

**When to Use:**
- New API development
- SDK design
- Library interfaces

---

### migration-safety

**Safe database/system migrations using expand-contract pattern.**

Pattern: Expand-Contract

```bash
ralph run --config presets/migration-safety.yml -p "Migrate users table to add email verification"
```

**Hats:**
- **Planner**: Plans expand-contract migration with rollback
- **Expander**: Adds new alongside old with dual-write
- **Migrator**: Backfills data and shifts traffic gradually
- **Contractor**: Removes old system after verification

**Three Phases:**
1. **EXPAND**: Add new alongside old, dual-write
2. **MIGRATE**: Backfill data, shift traffic gradually
3. **CONTRACT**: Remove old, clean up

**When to Use:**
- Database migrations
- API version upgrades
- System replacements

---

### performance-optimization

**Data-driven performance optimization.**

Pattern: Measure-Analyze-Optimize

```bash
ralph run --config presets/performance-optimization.yml -p "Optimize database query performance"
```

**Hats:**
- **Profiler**: Measures with hard data, establishes baseline
- **Analyst**: Analyzes profiling data to find REAL bottleneck
- **Optimizer**: Implements ONE optimization at a time

**Rules:**
- No optimization without measurement
- 80/20 rule: Find the 20% causing 80% of slowness
- ONE optimization at a time to isolate effect

**When to Use:**
- Performance issues
- Optimization projects
- Bottleneck hunting

---

### deploy

**Deployment workflow with verification.**

Pattern: Build-Deploy-Verify

```bash
ralph run --config presets/deploy.yml -p "Deploy v2.0 to staging"
```

**Hats:**
- **Builder**: Builds deployment artifacts
- **Deployer**: Executes deployment, handles rollbacks
- **Verifier**: Runs smoke tests, checks health

**Events:**
- `deploy.ready`, `deploy.start`, `deploy.done`, `deploy.failed`
- `deploy.rollback`, `verify.pass`, `verify.fail`

**When to Use:**
- Deployment automation
- Release workflows
- Staged rollouts

---

### research

**Codebase exploration without making changes.**

Pattern: Research-Synthesize

```bash
ralph run --config presets/research.yml -p "How does the authentication flow work?"
```

**Hats:**
- **Researcher**: Gathers information, analyzes patterns
- **Synthesizer**: Reviews findings, creates coherent summary

**Key Behavior:** NO code changes, NO commits. Pure information gathering.

**When to Use:**
- Understanding codebase
- Architecture analysis
- Technology evaluation

---

### socratic-learning

**Learning through questions, not lectures.**

Pattern: Socratic Dialogue

```bash
ralph run --config presets/socratic-learning.yml -p "Teach me about async/await in Rust"
```

**Hats:**
- **Explorer**: Explores and forms understanding with evidence
- **Questioner**: Challenges understanding with probing questions
- **Answerer**: Researches and answers questions with evidence

**Question Types:**
- "What happens if...?"
- "Why was this approach chosen over...?"
- "How does this handle...?"
- "What are the implications of...?"

**When to Use:**
- Learning new codebase
- Training and education
- Deep understanding

---

### mob-programming

**Virtual mob programming with rotating roles.**

Pattern: Rotating Roles

```bash
ralph run --config presets/mob-programming.yml -p "Implement OAuth2 flow"
```

**Hats:**
- **Navigator**: Thinks strategically, gives clear instructions
- **Driver**: Executes navigator's instructions exactly
- **Observer**: Provides fresh-eyes feedback

**When to Use:**
- Complex implementations
- Knowledge sharing
- Multiple perspectives needed

---

### hatless-baseline

**Control preset with NO hats for baseline testing.**

```bash
ralph run --config presets/hatless-baseline.yml -p "Implement a fibonacci function"
```

**Behavior:** Ralph works directly without hat delegation. Tests core loop without hat-specific behavior.

**When to Use:**
- Baseline testing
- Debugging hat issues
- Simple tasks

---

## Creating Custom Presets

### 1. Start from Existing Preset

```bash
cp presets/feature.yml presets/my-workflow.yml
```

### 2. Modify for Your Workflow

```yaml
# presets/my-workflow.yml
event_loop:
  starting_event: "my.start"
  completion_promise: "MY_WORKFLOW_COMPLETE"

hats:
  my_hat:
    name: "My Custom Hat"
    description: "Does the thing"
    triggers: ["my.start"]
    publishes: ["my.done"]
    instructions: |
      What this hat should do...
```

### 3. Test Your Preset

```bash
ralph run -c presets/my-workflow.yml -p "Test my workflow"
```

### Design Tips

1. **Draw the event flow first** before writing YAML
2. **Keep hats focused** — one responsibility per hat
3. **Use descriptive event names** — `category.action` format
4. **Add `default_publishes`** as safety net
5. **Include clear instructions** for complex hats

## See Also

- [Hat System Guide](../guide/hat-system.md) — Concepts and configuration
- [Configuration Reference](../guide/configuration.md) — Full YAML schema
- [Event Loop Specification](../../specs/event-loop.spec.md) — Technical details
