# Ralph Presets

Pre-configured hat collections for common workflows.

## Quick Start

```bash
# List all available presets
ralph init --list-presets

# Use a preset directly
ralph run -c presets/research.yml -p "How does auth work?"

# Initialize project with a preset
ralph init --preset tdd-red-green

# Or copy to your project root and customize
cp presets/feature.yml ralph.yml
```

## Available Presets

### Development

| Preset | Hats | Best For |
|--------|------|----------|
| **feature.yml** | builder, reviewer | Standard feature development with code review |
| **feature-minimal.yml** | builder, reviewer | Feature dev with auto-derived instructions |
| **tdd-red-green.yml** | test_writer, implementer, refactorer | Test-driven development |
| **spec-driven.yml** | spec_writer, spec_reviewer, implementer, verifier | Specification-first development |
| **refactor.yml** | planner, refactorer, verifier | Safe incremental refactoring |

### Quality & Review

| Preset | Hats | Best For |
|--------|------|----------|
| **review.yml** | reviewer, analyzer | Code review without making changes |
| **pr-review.yml** | correctness, security, architecture reviewers + synthesizer | Multi-perspective PR review |
| **adversarial-review.yml** | builder (blue team), red_team, fixer | Security-focused red team review |
| **gap-analysis.yml** | analyzer, verifier, reporter | Spec vs implementation comparison |

### Debugging & Investigation

| Preset | Hats | Best For |
|--------|------|----------|
| **debug.yml** | investigator, tester, fixer, verifier | Bug investigation using scientific method |
| **incident-response.yml** | observer, mitigator, investigator | Production incident handling (OODA loop) |
| **code-archaeology.yml** | surveyor, historian, archaeologist, modifier | Understanding legacy code |
| **scientific-method.yml** | observer, theorist, experimenter, fixer | Hypothesis-driven investigation |

### Documentation

| Preset | Hats | Best For |
|--------|------|----------|
| **docs.yml** | writer, reviewer | Documentation writing with review cycle |
| **documentation-first.yml** | documenter, reviewer, implementer, verifier | Write docs before code |

### Specialized

| Preset | Hats | Best For |
|--------|------|----------|
| **api-design.yml** | consumer, designer, critic, implementer | Consumer-driven API design |
| **migration-safety.yml** | planner, expander, migrator, contractor | Safe database/system migrations |
| **performance-optimization.yml** | profiler, analyst, optimizer | Data-driven performance tuning |
| **deploy.yml** | builder, deployer, verifier | Deployment workflow |

### Learning & Collaboration

| Preset | Hats | Best For |
|--------|------|----------|
| **research.yml** | researcher, synthesizer | Codebase exploration, no code changes |
| **socratic-learning.yml** | explorer, questioner, answerer | Learning through Socratic dialogue |
| **mob-programming.yml** | navigator, driver, observer | Virtual mob programming |

### Minimal Backend Presets

Located in `minimal/` - simple configurations for specific backends:

| Preset | Description |
|--------|-------------|
| **minimal/claude.yml** | Claude Code CLI defaults |
| **minimal/kiro.yml** | Kiro CLI defaults |
| **minimal/gemini.yml** | Gemini CLI defaults |
| **minimal/codex.yml** | Codex CLI defaults |
| **minimal/amp.yml** | Amp CLI defaults |

### Baseline

| Preset | Hats | Best For |
|--------|------|----------|
| **hatless-baseline.yml** | (none) | Solo mode baseline testing |

## Choosing a Preset

| If you need to... | Use |
|-------------------|-----|
| Build a feature with review | `feature.yml` |
| Practice TDD | `tdd-red-green.yml` |
| Build from requirements | `spec-driven.yml` |
| Understand code without changing it | `research.yml` |
| Write or update documentation | `docs.yml` |
| Restructure code safely | `refactor.yml` |
| Find and fix a bug | `debug.yml` |
| Review someone's code | `review.yml` or `pr-review.yml` |
| Compare specs against implementation | `gap-analysis.yml` |
| Handle a production incident | `incident-response.yml` |
| Work with legacy code | `code-archaeology.yml` |
| Design a new API | `api-design.yml` |
| Migrate a database/system | `migration-safety.yml` |
| Optimize performance | `performance-optimization.yml` |
| Learn about a codebase | `socratic-learning.yml` |
| Quick backend test | `minimal/<backend>.yml` |

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
      What this hat should do and how.
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

## Creating New Presets

1. Copy the closest existing preset
2. Draw your hat flow diagram first
3. Modify hats for your workflow
4. Adjust triggers to create your event flow
5. Set appropriate safeguards
6. Choose a meaningful completion promise

**Tip:** Use `category.action` event naming (e.g., `build.done`, `review.approved`).

## Full Documentation

See the [Hat System Guide](../docs/guide/hat-system.md) and [Preset Reference](../docs/reference/presets.md) for complete documentation.
