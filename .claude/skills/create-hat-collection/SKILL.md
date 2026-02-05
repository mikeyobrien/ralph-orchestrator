---
name: create-hat-collection
description: Generates new Hats hat collection presets through guided conversation. Asks clarifying questions, validates against schema constraints, and outputs production-ready YAML files.
---

# Create Hat Collection

## Overview

This skill generates Hats hat collection presets through a guided, conversational workflow. It asks clarifying questions about your workflow, validates the configuration against schema constraints, and produces a production-ready YAML preset file.

**Output:** A complete `.yml` preset file in the `presets/` directory.

## When to Use

- Creating a new multi-agent workflow from scratch
- Transforming a workflow idea into a structured preset
- Need guidance on hat design patterns and event routing

**Not for:** Modifying existing presets (use `/creating-hat-collections` reference instead)

## Workflow

### Phase 1: Understand the Workflow

Ask clarifying questions to understand:

1. **Purpose:** What problem does this workflow solve?
2. **Pattern:** Which architecture pattern fits best?
   - **Pipeline:** A→B→C linear flow (analyze→summarize)
   - **Critic-Actor:** One proposes, another critiques (code review)
   - **Supervisor-Worker:** Coordinator delegates to specialists
   - **Scientific:** Observe→Hypothesize→Test→Fix (debugging)
3. **Roles:** What distinct agent personas are needed?
4. **Handoffs:** When should each role hand off to the next?
5. **Completion:** What signals the workflow is done?

### Phase 2: Design Event Flow

Map the workflow as an event chain:

```
task.start → [Role A] → event.a → [Role B] → event.b → [Role C] → LOOP_COMPLETE
                                                    ↓
                                         event.rejected → [Role A]
```

**Constraints to validate:**
- Each trigger maps to exactly ONE hat (no ambiguous routing)
- `task.start` and `task.resume` are RESERVED (never use as triggers)
- Every hat must publish at least one event
- Chain must eventually reach LOOP_COMPLETE

### Phase 3: Generate Preset

Create the YAML file with these sections:

```yaml
# <Preset Name>
# Pattern: <Architecture Pattern>
# <One-line description>
#
# Usage:
#   hats run --config presets/<name>.yml --prompt "<example prompt>"

event_loop:
  starting_event: "<first.event>"  # Hats publishes this

hats:
  hat_key:
    name: "<Emoji> Display Name"
    description: "<Short description of the hat's purpose>"
    triggers: ["event.triggers.this"]
    publishes: ["event.this.publishes", "alternate.event"]
    default_publishes: "event.this.publishes"
    instructions: |
      ## <HAT NAME> MODE

      <Clear role definition - what this hat does>

      ### Process
      1. <Step one>
      2. <Step two>
      3. Publish appropriate event

      ### Event Format
      ```
      <event topic="event.name">
      key: value
      </event>
      ```

      ### DON'T
      - <Common mistake to avoid>
      - <Another mistake>
```

## Schema Reference

### Required Top-Level Fields

| Field | Description |
|-------|-------------|
| `event_loop.starting_event` | First event Hats publishes |

### Hat Definition Fields

| Field | Required | Description |
|-------|----------|-------------|
| `name` | Yes | Display name with optional emoji (e.g., "🔍 Analyzer") |
| `description` | Yes | Short description of the hat's purpose (one sentence) |
| `triggers` | Yes | Events this hat responds to (list) |
| `publishes` | Yes | Events this hat can emit (list) |
| `default_publishes` | Recommended | Fallback event if hat forgets to publish |
| `instructions` | Yes | Role-specific prompt (use `\|` for multiline) |

### Fields That DON'T Exist

Never use these—they're not in the schema:
- `emoji` (put emoji in `name` instead)
- `system_prompt` (use `instructions`)
- `subscriptions` / `publications` (use `triggers` / `publishes`)

## Event Naming Conventions

```
<phase>.ready / <phase>.done      # Phase transitions
<thing>.approved / <thing>.rejected  # Review gates
<noun>.found / <noun>.missing     # Discovery events
<action>.request / <action>.complete # Request-response
```

**Examples:** `analysis.complete`, `review.approved`, `build.blocked`, `spec.rejected`

## Validation Checklist

Before finalizing the preset, verify:

- [ ] Each trigger maps to exactly ONE hat
- [ ] No hat uses `task.start` or `task.resume` as triggers
- [ ] Every hat has `name`, `description`, `triggers`, `publishes`, and `instructions`
- [ ] `default_publishes` is set for hats with multiple publish options
- [ ] Event chain can reach LOOP_COMPLETE
- [ ] No orphan events that never get handled (warning only)
- [ ] Instructions include role definition, process steps, event format, and DON'Ts

## Output Location

Save generated presets to: `presets/<name>.yml`

For minimal/embedded presets: `presets/minimal/<name>.yml`

## Testing the Preset

After generation, test with:

```bash
# Dry run to validate config parsing
cargo run --bin hats -- run -c presets/<name>.yml -p "test prompt" --dry-run

# Run smoke tests
cargo test -p hats-core smoke_runner
```

Use `/evaluate-presets` for comprehensive validation.

## Example Conversation

**User:** I want a preset for doing code reviews with a security focus.

**Assistant:** I'll help create a security-focused code review preset. Let me ask a few questions:

1. **Review depth:** Should this be a quick scan or thorough analysis?
2. **Roles needed:** I'm thinking:
   - Security Analyst (finds vulnerabilities)
   - Code Reviewer (checks logic/style)
   - Summarizer (creates final report)

   Does this cover your needs, or should we add/remove roles?

3. **Iteration:** If issues are found, should the code be fixed, or just reported?

**User:** Thorough analysis, those roles work, just report issues don't fix.

**Assistant:** Here's the event flow I'm designing:

```
task.start → [Security Analyst] → security.complete
                                        ↓
          → [Code Reviewer] → review.complete
                                        ↓
          → [Summarizer] → LOOP_COMPLETE
```

Let me generate the preset...

[Generates YAML with three hats following the schema]

## Common Patterns Reference

### Pipeline (Sequential)
```
A → B → C → done
```
Use for: analysis workflows, document processing

### Critic-Actor (Review Loop)
```
Actor → Critic → approved/rejected
                    ↓
         rejected → Actor (retry)
```
Use for: code review, quality gates

### Supervisor-Worker
```
Supervisor → worker.task → Worker → work.done → Supervisor
```
Use for: complex task decomposition

### Scientific Method
```
Observe → Hypothesize → Test → confirmed/rejected
                                    ↓
                         rejected → Observe
```
Use for: debugging, investigation
