# Ralph Presets

Pre-configured hat collections for common workflows.

## Quick Start

```bash
# Use a preset directly
ralph start --config presets/research.yml --prompt "How does auth work?"

# Or copy to your project root and customize
cp presets/feature.yml ralph.yml
```

## Available Presets

| Preset | Hats | Best For |
|--------|------|----------|
| **research.yml** | researcher, synthesizer | Codebase exploration, architecture analysis, no code changes |
| **docs.yml** | planner, writer, reviewer | Documentation writing with edit/review cycles |
| **refactor.yml** | planner, refactorer, verifier | Safe incremental refactoring with verification |
| **debug.yml** | investigator, tester, fixer, verifier | Bug investigation using scientific method |
| **review.yml** | reviewer, analyzer | Code review without making changes |
| **feature.yml** | planner, builder, reviewer | Feature development with integrated review |
| **fresh-eyes.yml** | builder, fresh_eyes_auditor, fresh_eyes_gatekeeper | Implementation with enforced repeated fresh-eyes bug-catching passes |
| **gap-analysis.yml** | analyzer, verifier, reporter | Deep spec-to-implementation comparison, outputs to ISSUES.md |

## Preset Details

### research.yml
**Completion Promise:** `RESEARCH_COMPLETE`

For exploration tasks where you need to understand something without changing code. Great for:
- "How does X work in this codebase?"
- "What are the dependencies between modules?"
- "Analyze the performance characteristics of..."

**Hat Flow:**
```
task.start → [researcher] → research.finding → [synthesizer] → research.followup → [researcher] → ...
```

---

### docs.yml
**Completion Promise:** `DOCS_COMPLETE`

For writing documentation with quality control. The writer/editor/reviewer cycle ensures accuracy and clarity.

**Hat Flow:**
```
task.start → [planner] → write.section → [writer] → write.done → [reviewer] → review.done → [planner] → ...
```

---

### refactor.yml
**Completion Promise:** `REFACTOR_COMPLETE`

For safe code refactoring. Each step is atomic and verified. Checkpoint interval is set to 3 for frequent git snapshots.

**Key Principle:** Every step leaves the codebase in a working state.

**Hat Flow:**
```
task.start → [planner] → refactor.task → [refactorer] → refactor.done → [verifier] → verify.passed → [planner] → ...
```

---

### debug.yml
**Completion Promise:** `DEBUG_COMPLETE`

For systematic bug investigation. Uses scientific method: hypothesize, test, narrow down.

**Hat Flow:**
```
task.start → [investigator] → hypothesis.test → [tester] → hypothesis.rejected → [investigator] → ...
                                                         → hypothesis.confirmed → fix.propose → [fixer] → ...
```

---

### review.yml
**Completion Promise:** `REVIEW_COMPLETE`

For code review without making changes. Produces structured feedback categorized by severity.

**Categories:**
- **Critical** — Must fix before merge
- **Suggestions** — Should consider
- **Nitpicks** — Optional improvements

**Hat Flow:**
```
task.start → [reviewer] → review.section → [analyzer] → analysis.complete → [reviewer] → ...
```

---

### feature.yml
**Completion Promise:** `LOOP_COMPLETE`

Enhanced default workflow with integrated code review. Every implementation goes through review before being marked complete.

**Hat Flow:**
```
task.start → [planner] → build.task → [builder] → build.done → [planner] → review.request → [reviewer] → review.approved → [planner] → ...
```

---

### gap-analysis.yml
**Completion Promise:** `GAP_ANALYSIS_COMPLETE`

Deep comparison of specs against implementation. Systematically verifies each acceptance criterion and documents discrepancies in ISSUES.md.

**Self-contained preset:** Uses inline `prompt:` config—no separate PROMPT.md needed.

**Output:** Writes structured findings to `ISSUES.md` with categories:
- **Critical Gaps** — Spec violations (implementation contradicts spec)
- **Missing Features** — Acceptance criteria not implemented
- **Undocumented Behavior** — Code without spec coverage
- **Spec Improvements** — Ambiguities, missing details

**Hat Flow:**
```
task.start → [analyzer] → analyze.spec → [verifier] → verify.complete → [analyzer] → report.request → [reporter] → report.complete → [analyzer] → ...
```

**Usage:**
```bash
# Full gap analysis of all specs
ralph run --config presets/gap-analysis.yml

# Focus on specific spec
ralph run --config presets/gap-analysis.yml -p "Focus on cli-adapters.spec.md"
```

---

## Customizing Presets

### Adding a Hat

```yaml
hats:
  # ... existing hats ...

  my_custom_hat:
    name: "My Custom Hat"
    triggers: ["custom.trigger"]
    publishes: ["custom.done"]
    instructions: |
      What this hat does and how.
```

### Modifying Triggers

To change the workflow, adjust which events trigger which hats:

```yaml
hats:
  planner:
    triggers: ["task.start", "build.done", "my.custom.event"]  # Added custom event
```

### Adjusting Safeguards

```yaml
event_loop:
  max_iterations: 50        # Fewer iterations for smaller tasks
  max_runtime_seconds: 1800 # 30 minute timeout
  checkpoint_interval: 2    # More frequent git checkpoints
```

## Choosing a Preset

| If you need to... | Use |
|-------------------|-----|
| Understand code without changing it | `research.yml` |
| Write or update documentation | `docs.yml` |
| Restructure code safely | `refactor.yml` |
| Find and fix a bug | `debug.yml` |
| Review someone's code | `review.yml` |
| Build a new feature | `feature.yml` |
| Force repeated skeptical post-implementation checks | `fresh-eyes.yml` |
| Compare specs against implementation | `gap-analysis.yml` |

## Creating New Presets

1. Copy the closest existing preset
2. Modify hats for your workflow
3. Adjust triggers to create your event flow
4. Set appropriate safeguards
5. Choose a meaningful completion promise

**Tip:** Draw your hat flow diagram first, then implement it.
