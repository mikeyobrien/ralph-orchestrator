---
name: code-assist
description: Guides implementation of code tasks using test-driven development in an Explore, Plan, Code, Commit workflow. Acts as a Technical Implementation Partner and TDD Coach — following existing patterns, avoiding over-engineering, and producing idiomatic, modern code.
type: anthropic-skill
version: "1.1"
---

# Code Assist

## Overview

Guides implementation of code tasks using test-driven development in an Explore, Plan, Code, Commit workflow. Balances automation with user collaboration while adhering to existing package patterns and prioritizing readability and extensibility.

## Parameters

- **task_description** (required): Task specification, rough idea, file path to `.code-task.md`, or URL
- **additional_context** (optional): Supplementary information for implementation context
- **documentation_dir** (optional, default: ".sop/planning"): Directory for planning documents
- **repo_root** (optional, default: current working directory): Repository root for code implementation
- **task_name** (optional): Short descriptive name (auto-generated if not provided)
- **mode** (optional, default: "auto"): "interactive" (confirmation at each step) or "auto" (autonomous execution)

**Constraints:**
- You MUST ask for all parameters upfront in a single prompt to avoid repeated interruptions
- You MUST validate inputs: normalize mode to "interactive"/"auto", verify paths, generate task_name if needed

## Mode Behavior

Apply these patterns throughout all steps:

**Interactive Mode:** Present actions for confirmation. Explain pros/cons when multiple approaches exist. Ask clarifying questions. Pause at key decision points. Provide educational context.

**Auto Mode:** Execute autonomously. Document all decisions and reasoning in progress.md. Select the most appropriate approach and document why. Provide comprehensive summaries at completion.

## Important Notes

**Separation of Concerns:**
Documentation goes in `{documentation_dir}`. Code (tests and implementation) goes in `repo_root`. Never mix them. Documentation should guide implementation with high-level concepts — not provide it. When including code snippets in documentation, keep them brief and clearly label them as examples or references.

**CODEASSIST.md Integration:**
If CODEASSIST.md exists in repo_root, read it and apply its constraints throughout.

## Steps

### 1. Setup

Initialize the project environment and create necessary directory structures.

**Constraints:**
- You MUST create `{documentation_dir}/implementation/{task_name}/` (with logs subdirectory) and verify it exists before proceeding
- You MUST discover instruction files (CODEASSIST.md, README.md, CONTRIBUTING.md, ARCHITECTURE.md, etc.) and summarize relevant information in context.md
- You MUST create context.md (project structure, requirements, patterns, dependencies) and progress.md (execution tracking with markdown checklists)
- If task_description points to a `.code-task.md` with YAML frontmatter, update `status: in_progress` and `started: <date>` (if not already set)

> 💬 See [Mode Behavior](#mode-behavior) for mode-specific interaction guidance

### 2. Explore Phase

#### 2.1 Analyze Requirements and Context

Analyze the task description and existing documentation to identify core functionality, edge cases, and constraints.

**Constraints:**
- You MUST produce a clear list of functional requirements and acceptance criteria, even from rough descriptions
- You MUST determine appropriate file paths, language, and alignment with existing project structure
- In interactive mode, discuss requirements with the user, clarify ambiguities, and validate your understanding

> 💬 See [Mode Behavior](#mode-behavior) for mode-specific interaction guidance

#### 2.2 Research Existing Patterns

Search for similar implementations and identify interfaces, libraries, and components the implementation will interact with.

**Constraints:**
- You MUST search the repository for relevant code, patterns, and conventions
- You MUST create a dependency map and update context.md with implementation paths

> 💬 See [Mode Behavior](#mode-behavior) for mode-specific interaction guidance

### 3. Plan Phase

#### 3.1 Design Test Strategy

Create test scenarios covering normal operation, edge cases, and error conditions.

**Constraints:**
- You MUST cover all acceptance criteria with at least one test scenario
- You MUST define explicit input/output pairs and save scenarios to plan.md
- You MUST design tests that will initially fail (no mock implementations during design)

> 💬 See [Mode Behavior](#mode-behavior) for mode-specific interaction guidance

#### 3.2 Implementation Planning & Tracking

Outline the high-level structure and create an implementation plan.

**Constraints:**
- You MUST save the plan to plan.md with key implementation tasks
- You MUST maintain an implementation checklist in progress.md using markdown checkbox format
- In interactive mode, present multiple approaches with pros/cons and discuss trade-offs

> 💬 See [Mode Behavior](#mode-behavior) for mode-specific interaction guidance

### 4. Code Phase

**Phase-wide constraints:**
- You MUST place all code (tests and implementation) in repo_root, never in documentation_dir
- You MUST verify tests/builds pass before advancing to the next sub-step
- You MUST follow the existing codebase's conventions (naming, patterns, error handling, testing style)
- Pipe build output to `{documentation_dir}/implementation/{task_name}/logs/` and search for success/failure indicators

#### 4.1 Implement Test Cases

Write test cases following strict TDD principles.

**Constraints:**
- You MUST implement tests for ALL requirements before writing ANY implementation code
- You MUST execute tests to verify they fail as expected, documenting failure reasons
- You MUST follow the testing framework conventions used in the existing codebase

> 💬 See [Mode Behavior](#mode-behavior) for mode-specific interaction guidance

#### 4.2 Develop Implementation Code

Write implementation code to pass the tests, focusing on simplicity and correctness first.

**Constraints:**
- You MUST follow the TDD cycle: RED → GREEN → REFACTOR
- You MUST implement only what is needed to make current tests pass (YAGNI, KISS, SOLID)
- You MUST execute tests after each implementation step to verify they pass

> 💬 See [Mode Behavior](#mode-behavior) for mode-specific interaction guidance

#### 4.3 Refactor and Optimize

Review the implementation for simplification, improvement, and convention alignment.

**Constraints:**
- You MUST verify all tests and builds pass before starting — fix any failures first
- You MUST align implementation with surrounding codebase conventions (naming, structure, error handling, imports, logging)
- You MUST maintain test-passing status throughout refactoring

> 💬 See [Mode Behavior](#mode-behavior) for mode-specific interaction guidance

#### 4.4 Validate Implementation

Verify the implementation is complete and correct.

**Constraints:**
- You MUST run the full test suite and build, verifying all pass
- You MUST verify all implementation plan items are completed
- You MUST NOT claim completion while any tests are failing

> 💬 See [Mode Behavior](#mode-behavior) for mode-specific interaction guidance

### 5. Commit Phase

Draft a conventional commit message and perform the git commit.

**Constraints:**
- You MUST NOT commit until builds AND tests are verified passing
- You MUST follow the Conventional Commits specification
- You MUST NOT push to remote repositories
- If task_description was a `.code-task.md`, update frontmatter to `status: completed` and `completed: <date>` before committing
- Document the commit hash in progress.md

> 💬 See [Mode Behavior](#mode-behavior) for mode-specific interaction guidance

## Desired Outcome

* Complete, well-tested implementation meeting all requirements
* Comprehensive test suite validating the implementation
* Clean code following existing patterns, prioritizing readability, avoiding over-engineering
* Implementation artifacts in `{documentation_dir}/implementation/{task_name}/` (context.md, plan.md, progress.md, logs/)
* Properly committed changes with conventional commit messages

## Examples

### Feature Implementation

**Input:**
```
task_description: "Create a utility function that validates email addresses"
mode: "interactive"
```

**Expected Process:**
1. Check for CODEASSIST.md and discover instruction files
2. Detect project type from existing files (pom.xml, package.json, etc.)
3. Set up directory structure in .sop/planning/implementation/email-validator/
4. Explore requirements and create context documentation
5. Plan test scenarios for valid/invalid email formats
6. Implement tests first (TDD approach)
7. Implement the validation function
8. Commit with conventional commit message

## Troubleshooting

**Directory/Permission Issues:** Create directories if possible, inform user of permission issues, suggest alternatives.

**Build Failures:** Check CODEASSIST.md for guidance, verify correct directory, try clean builds, check dependencies.

**Multi-Package Coordination:** Verify dependency order, build in order, create separate commits per package.

**Task File Frontmatter Issues:** Skip frontmatter updates if missing/malformed. Don't fail the task due to frontmatter issues.

**Implementation Challenges:** Document in progress.md, propose alternatives. In interactive mode ask for guidance; in auto mode select the most promising approach and document the decision.

## Artifacts

• `{documentation_dir}/implementation/{task_name}/context.md` — Workspace structure, requirements, patterns, dependencies, implementation paths
• `{documentation_dir}/implementation/{task_name}/plan.md` — Test scenarios, implementation strategy
• `{documentation_dir}/implementation/{task_name}/progress.md` — Execution tracking, TDD cycles, commit status, challenges
• `{documentation_dir}/implementation/{task_name}/logs/` — Build outputs
