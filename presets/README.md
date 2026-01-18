# Ralph Presets

Pre-configured hat collections for common workflows.

## Quick Start

```bash
# Use a preset directly
ralph run -c presets/research.yml -p "How does auth work?"

# Or copy to your project root and customize
cp presets/feature.yml ralph.yml
```

## Available Presets

### Core Workflows

| Preset | Description | Best For |
|--------|-------------|----------|
| **research.yml** | Deep exploration without code changes | Codebase analysis, architecture review |
| **docs.yml** | Writer/editor/reviewer cycle | Documentation, READMEs, guides |
| **refactor.yml** | Test-verify-change-verify cycle | Safe incremental refactoring |
| **debug.yml** | Scientific method investigation | Bug hunting, root cause analysis |
| **review.yml** | Structured code review | Reviewing changes without modifying |
| **feature.yml** | Planner/builder/reviewer cycle | Feature development with review |
| **feature-minimal.yml** | Auto-derived instructions version | Same as feature, less config |
| **gap-analysis.yml** | Spec vs implementation comparison | Finding missing/broken features |

### Advanced Patterns

| Preset | Pattern | Description |
|--------|---------|-------------|
| **tdd-red-green.yml** | Critic-Actor | Failing test → implementation → refactor |
| **spec-driven.yml** | Contract-First | Spec is the contract, implementation follows |
| **documentation-first.yml** | Documentation-First | README-driven development |
| **adversarial-review.yml** | Red Team/Blue Team | One builds, another tries to break |
| **pr-review.yml** | Multi-Perspective | Specialized reviewers examine aspects |
| **mob-programming.yml** | Rotating Roles | Multiple perspectives on same code |

### Specialized Workflows

| Preset | Description |
|--------|-------------|
| **deploy.yml** | Planner → Builder → Deployer → Verifier cycle |
| **incident-response.yml** | OODA loop for production issues |
| **migration-safety.yml** | Expand-contract pattern for migrations |
| **performance-optimization.yml** | Measure → optimize → verify cycle |
| **code-archaeology.yml** | Understand legacy code before changing |
| **scientific-method.yml** | Hypothesis-driven debugging |
| **socratic-learning.yml** | Teaching through questions |
| **api-design.yml** | Consumer-driven API design |

### Minimal Backend Presets

Located in `minimal/` - simple configurations for specific backends:

| Preset | Description |
|--------|-------------|
| **minimal/claude.yml** | Claude Code CLI defaults |
| **minimal/kiro.yml** | Kiro CLI defaults |
| **minimal/gemini.yml** | Gemini CLI defaults |
| **minimal/codex.yml** | Codex CLI defaults |
| **minimal/amp.yml** | Amp CLI defaults |
| **minimal/builder.yml** | Single builder hat, no planning |
| **minimal/code-assist.yml** | TDD-based implementation |
| **minimal/smoke.yml** | Quick smoke test with Haiku |
| **minimal/test.yml** | Minimal config for testing |
| **minimal/preset-evaluator.yml** | Meta preset for evaluating presets |

### Testing & Baseline

| Preset | Description |
|--------|-------------|
| **hatless-baseline.yml** | No hats - tests core Ralph loop |

---

## Preset Details

### research.yml
**Completion Promise:** `RESEARCH_COMPLETE`

For exploration tasks without code changes. Great for:
- "How does X work in this codebase?"
- "What are the dependencies between modules?"
- "Analyze the performance characteristics of..."

**Hat Flow:**
```
task.start → [researcher] → research.finding → [synthesizer] → research.followup → ...
```

---

### docs.yml
**Completion Promise:** `DOCS_COMPLETE`

For writing documentation with quality control. The writer/editor/reviewer cycle ensures accuracy and clarity.

**Hat Flow:**
```
task.start → [planner] → write.section → [writer] → write.done → [reviewer] → ...
```

---

### refactor.yml
**Completion Promise:** `REFACTOR_COMPLETE`

For safe code refactoring. Each step is atomic and verified.

**Key Principle:** Every step leaves the codebase in a working state.

**Hat Flow:**
```
task.start → [planner] → refactor.task → [refactorer] → refactor.done → [verifier] → ...
```

---

### debug.yml
**Completion Promise:** `DEBUG_COMPLETE`

For systematic bug investigation using scientific method: hypothesize, test, narrow down.

**Hat Flow:**
```
task.start → [investigator] → hypothesis.test → [tester] → hypothesis.confirmed → [fixer] → ...
```

---

### review.yml
**Completion Promise:** `REVIEW_COMPLETE`

For code review without making changes. Produces structured feedback by severity:
- **Critical** — Must fix before merge
- **Suggestions** — Should consider
- **Nitpicks** — Optional improvements

**Hat Flow:**
```
task.start → [reviewer] → review.section → [analyzer] → analysis.complete → ...
```

---

### feature.yml
**Completion Promise:** `LOOP_COMPLETE`

Enhanced default workflow with integrated code review. Every implementation goes through review.

**Hat Flow:**
```
task.start → [planner] → build.task → [builder] → build.done → [reviewer] → review.approved → ...
```

---

### gap-analysis.yml
**Completion Promise:** `GAP_ANALYSIS_COMPLETE`

Deep comparison of specs against implementation. Outputs to `ISSUES.md` with categories:
- **Critical Gaps** — Spec violations
- **Missing Features** — Acceptance criteria not implemented
- **Undocumented Behavior** — Code without spec coverage
- **Spec Improvements** — Ambiguities, missing details

**Self-contained:** Uses inline `prompt:` config—no separate PROMPT.md needed.

**Usage:**
```bash
# Full gap analysis
ralph run -c presets/gap-analysis.yml

# Focus on specific spec
ralph run -c presets/gap-analysis.yml -p "Focus on cli-adapters.spec.md"
```

---

## Customizing Presets

### Adding a Hat

```yaml
hats:
  my_custom_hat:
    name: "My Custom Hat"
    description: "What this hat does"
    triggers:
      - custom.trigger
    publishes:
      - custom.done
    instructions: |
      What this hat does and how.
```

### Modifying Triggers

Change the workflow by adjusting which events trigger which hats:

```yaml
hats:
  planner:
    triggers:
      - task.start
      - build.done
      - my.custom.event  # Added custom event
```

### Adjusting Safeguards

```yaml
event_loop:
  max_iterations: 50        # Fewer iterations for smaller tasks
  max_runtime_seconds: 1800 # 30 minute timeout
```

---

## Choosing a Preset

| If you need to... | Use |
|-------------------|-----|
| Understand code without changing it | `research.yml` |
| Write or update documentation | `docs.yml` |
| Restructure code safely | `refactor.yml` |
| Find and fix a bug | `debug.yml` |
| Review someone's code | `review.yml` |
| Build a new feature | `feature.yml` |
| Compare specs against implementation | `gap-analysis.yml` |
| Test-driven development | `tdd-red-green.yml` |
| Spec-first development | `spec-driven.yml` |
| Quick backend test | `minimal/<backend>.yml` |

---

## Creating New Presets

1. Copy the closest existing preset
2. Modify hats for your workflow
3. Adjust triggers to create your event flow
4. Set appropriate safeguards
5. Choose a meaningful completion promise

**Tip:** Draw your hat flow diagram first, then implement it.

See [YAML Schema Reference](../docs/reference/yaml-schema.md) for configuration details.
