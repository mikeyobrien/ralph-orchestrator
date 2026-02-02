---
name: code-task-generator
description: Generates structured .code-task.md files from descriptions or PDD implementation plans. Auto-detects input type, creates properly formatted tasks with Given-When-Then acceptance criteria.
type: anthropic-skill
version: "1.1"
---

# Code Task Generator

## Overview

Generate structured code task files from rough descriptions or PDD implementation plans. Auto-detects input type and creates properly formatted `.code-task.md` files. For PDD plans, processes one step at a time to allow learning between steps.

## Important Notes

These rules apply across ALL steps:

- **User approval required:** Present the task breakdown plan and get explicit approval before generating any files.
- **Tests are integrated:** Include unit test requirements in each task's acceptance criteria. Never create separate "add tests" tasks.
- **PDD mode references:** Always include the design document path as required reading. Only include research docs if directly relevant to the specific task.

## Parameters

- **input** (required): Task description, file path, or PDD plan path
- **step_number** (optional, PDD only): Specific step to process. Auto-determines next uncompleted step if omitted.
- **output_dir** (optional, default: `specs/{task_name}/tasks/`): Output directory for code task files
- **task_name** (optional, description mode only): Override auto-generated task name

**Constraints:**
- You MUST ask for all required parameters upfront in a single prompt
- You MUST support input as: direct text, file path, directory path (looks for plan.md), or URL

## Steps

### 1. Detect Input Mode

Check if input is a file with PDD plan structure (checklist + numbered steps). Set mode to "pdd" or "description" and inform the user.

### 2. Analyze Input

- **PDD mode:** Parse the plan, extract steps and checklist status, determine target step (from step_number or first uncompleted)
- **Description mode:** Identify core functionality, technical requirements, complexity level (Low/Medium/High), and technology domain

### 3. Structure Requirements

- **PDD mode:** Extract the target step's title, description, demo requirements, constraints, and integration notes with previous steps. Identify relevant research documents.
- **Description mode:** Identify functional requirements, infer technical constraints and dependencies.

For both modes: create measurable acceptance criteria in Given-When-Then format and prepare a task breakdown plan.

### 4. Plan Tasks

Present the proposed breakdown to the user:
- One-line summary per task
- Proposed sequence and dependencies
- You MUST NOT generate files until the user explicitly approves

### 5. Generate Tasks

Create files following the Code Task Format below.

**PDD mode specifics:**
- Create `step{NN}/` folder (zero-padded: step01, step02, step10)
- Name files sequentially: `task-01-{title}.code-task.md`, `task-02-{title}.code-task.md`
- Break down by functional components, not testing phases

**All tasks:**
- You MUST use the exact Code Task Format structure below
- You MUST include YAML frontmatter with `status: pending`, `created: YYYY-MM-DD`, `started: null`, `completed: null`
- You MUST use kebab-case names with `.code-task.md` extension
- You MUST include acceptance criteria covering main functionality and unit tests

### 6. Report Results

List generated files with paths. For PDD mode, include the step's demo requirements. Suggest running code-assist on tasks in sequence, or using Ralph for autonomous implementation.

### 7. Offer Ralph Integration

Ask: "Would you like me to set up Ralph to implement these tasks autonomously?"

If yes, create a concise PROMPT.md with objective, spec directory reference, execution order, and acceptance criteria. Suggest the appropriate command:
- Full pipeline: `ralph run --config presets/pdd-to-code-assist.yml`
- Simpler flow: `ralph run --config presets/spec-driven.yml`

## Code Task Format Specification

Each code task file MUST follow this structure:

```markdown
---
status: pending
created: YYYY-MM-DD
started: null
completed: null
---
# Task: [Task Name]

## Description
[What needs to be implemented and why]

## Background
[Context needed to understand the task]

## Reference Documentation
**Required:**
- Design: specs/{task_name}/design.md

**Additional References (if relevant to this task):**
- [Specific research document or section]

**Note:** Read the design document before beginning implementation.

## Technical Requirements
1. [First requirement]
2. [Second requirement]

## Dependencies
- [Dependency with details]

## Implementation Approach
1. [Implementation step or approach]

## Acceptance Criteria

1. **[Criterion Name]**
   - Given [precondition]
   - When [action]
   - Then [expected result]

## Metadata
- **Complexity**: [Low/Medium/High]
- **Labels**: [Comma-separated labels]
- **Required Skills**: [Skills needed]
```

## Examples

**Description mode input:** `"I need a function that validates email addresses and returns detailed error messages"`

**Description mode output:** `specs/email-validator/tasks/email-validator.code-task.md` — task with acceptance criteria for valid/invalid email handling, error messages, and unit tests.

**PDD mode input:** `"specs/data-pipeline/plan.md"`

**PDD mode output:** `specs/data-pipeline/tasks/step02/` containing `task-01-create-data-models.code-task.md`, `task-02-implement-validation.code-task.md`, `task-03-add-serialization.code-task.md` — each with design.md reference, acceptance criteria, and demo requirements.

## Troubleshooting

**Vague description:** Ask clarifying questions, suggest common patterns, create a basic task and offer to refine.

**Complex description:** Suggest breaking into smaller tasks, focus on core functionality first, offer to create related tasks.

**Missing technical details:** Make reasonable assumptions, include multiple approaches, note areas needing user decisions.

**Plan file not found:** Check if path is a directory (look for plan.md within), suggest common PDD plan locations.

**Invalid plan format:** Identify missing sections, suggest running PDD to generate a proper plan, extract what's available.

**All steps complete:** Inform user, ask if they want a specific step anyway, suggest reviewing for new steps.
