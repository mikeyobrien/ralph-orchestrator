---
name: pdd
description: Transforms a rough idea into a detailed design document with implementation plan. Follows Prompt-Driven Development — iterative requirements clarification, research, design, and planning.
type: anthropic-skill
version: "1.1"
---

# Prompt-Driven Development

## Overview

Transform a rough idea into a detailed design with an implementation plan. The process is iterative: clarify requirements, research, design, plan — moving between phases as needed.

## Important Notes

These rules apply across ALL steps:

- **User-driven flow:** Never proceed to the next step without explicit user confirmation. At each transition, ask the user what they want to do next.
- **Iterative:** The user can move between requirements clarification and research at any time. Always offer this option at phase transitions.
- **Record as you go:** Append questions, answers, and findings to project files in real time — don't batch-write at the end.
- **Mermaid diagrams:** Include diagrams for architectures, data flows, and component relationships in research and design documents.
- **Sources:** Cite references and links in research documents when based on external materials.

## Parameters

- **rough_idea** (required): The initial concept or idea to develop
- **project_dir** (optional, default: `specs/{task_name}/`): Base directory for all artifacts. `{task_name}` is derived as kebab-case from the idea (e.g., "build a rate limiter" → `rate-limiter`). Aligns with Ralph's spec-driven pipeline.

**Constraints:**
- You MUST ask for all required parameters upfront in a single prompt
- You MUST support multiple input methods: direct text, file path, URL
- You MUST derive `task_name` from the rough idea as kebab-case
- You MUST NOT overwrite an existing project directory — ask for a new path if it already has contents

## Steps

### 1. Create Project Structure

Create the directory and initial files:
- `{project_dir}/rough-idea.md` — the provided rough idea
- `{project_dir}/requirements.md` — Q&A record (initially empty)
- `{project_dir}/research/` — directory for research notes

Inform the user the structure feeds into Ralph's spec-driven presets.

### 2. Initial Process Planning

Ask the user their preferred starting point:
- Requirements clarification (default)
- Preliminary research on specific topics
- Provide additional context first

### 3. Requirements Clarification

Guide the user through questions to refine the idea into a thorough specification.

**Constraints:**
- You MUST ask ONE question at a time — do not list multiple questions
- You MUST NOT pre-populate answers or batch-write Q&A to requirements.md
- You MUST follow this cycle for each question:
  1. Formulate and append question to requirements.md
  2. Present to user, wait for complete response
  3. Append answer to requirements.md
  4. Proceed to next question
- You MUST ask the user if requirements clarification is complete before moving on

Cover edge cases, user experience, technical constraints, and success criteria. Suggest options when the user is unsure.

### 4. Research

Conduct research on technologies, libraries, or existing code to inform the design.

**Constraints:**
- You MUST propose a research plan to the user and incorporate their suggestions
- You MUST document findings in `{project_dir}/research/` as separate topic files
- You MUST periodically check in with the user to share findings and confirm direction
- You MUST summarize key findings before moving on

### 5. Iteration Checkpoint

Summarize the current state of requirements and research, then ask the user:
- Proceed to design?
- Return to requirements clarification?
- Conduct additional research?

### 6. Create Detailed Design

Create `{project_dir}/design.md` as a standalone document with these sections:
- Overview
- Detailed Requirements (consolidated from requirements.md)
- Architecture Overview
- Components and Interfaces
- Data Models
- Error Handling
- Acceptance Criteria (Given-When-Then format for machine verification)
- Testing Strategy
- Appendices (Technology Choices, Research Findings, Alternative Approaches)

**Constraints:**
- You MUST write the design as standalone — understandable without reading other files
- You MUST consolidate all requirements from requirements.md
- You MUST include an appendix summarizing research (technology choices, alternatives, limitations)
- You MUST review the design with the user and iterate on feedback

### 7. Develop Implementation Plan

Create `{project_dir}/plan.md` — a numbered series of incremental implementation steps.

**Guiding principle:** Each step builds on previous steps, results in working demoable functionality, and follows TDD practices. No orphaned code — every step ends with integration. Core end-to-end functionality should be available as early as possible.

**Constraints:**
- You MUST include a checklist at the top of plan.md tracking each step
- You MUST format as "Step N:" with: objective, implementation guidance, test requirements, integration notes, and demo description
- You MUST ensure the plan covers all aspects of the design without duplicating design details

### 8. Summarize and Present Results

Create `{project_dir}/summary.md` listing all artifacts, a brief overview, and suggested next steps. Present this summary in the conversation.

### 9. Offer Ralph Integration

Ask: "Would you like me to set up Ralph to implement this autonomously?"

If yes, create a concise PROMPT.md (under 100 lines) with:
- Objective statement
- Key requirements
- Acceptance criteria (Given-When-Then)
- Reference to `specs/{task_name}/`

Suggest the appropriate command:
- Full pipeline: `ralph run --config presets/pdd-to-code-assist.yml`
- Simpler flow: `ralph run --config presets/spec-driven.yml`

## Example

**Input:** "I want to build a template management feature for our internal tool — create, edit, share templates, generate documents with custom fields."

**Output:** A `specs/template-management/` directory containing rough-idea.md, requirements.md, research/, design.md, plan.md, and summary.md — plus optionally a PROMPT.md for autonomous implementation.

## Troubleshooting

**Requirements stall:** Suggest switching to a different aspect, provide examples, or pivot to research to unblock decisions.

**Research limitations:** Document what's missing, suggest alternatives with available information, ask user for additional context. Don't block progress.

**Design complexity:** Break into smaller components, focus on core functionality first, suggest phased implementation, return to requirements to re-prioritize.
