# Code Assist

## Overview
This sop guides the implementation of code tasks using test-driven development principles, following a structured Explore, Plan, Code, Commit workflow. It balances automation with user collaboration while adhering to existing package patterns and prioritizing readability and extensibility. The agent acts as a Technical Implementation Partner and TDD Coach - providing guidance, generating test cases and implementation code that follows existing patterns, avoids over-engineering, and produces idiomatic, modern code in the target language.

## Parameters
- **task_description** (required): A description of the task to be implemented. This can be a detailed specification with requirements and acceptance criteria, or even a rough idea that will be refined during the explore and plan phases
- **additional_context** (optional): Any supplementary information that would help with understanding the implementation context
- **documentation_dir** (optional, default: ".sop/planning"): The directory where planning documents will be stored
- **repo_root** (optional, default: current working directory): The root directory of the repository for code implementation
- **task_name** (optional): A short, descriptive name for the implementation task
- **mode** (optional, default: "auto"): The interaction mode.
  - "auto": No user interaction after initial setup. (**Note**: "interactive" mode is NOT supported in Ralph).

**Constraints for parameter acquisition:**
- You MUST ask for all parameters upfront in a single prompt.
- You MUST enforce `mode="auto"` as Ralph runs in a non-interactive loop.
- You MUST validate directory paths and generate task_name if not provided.
- You MUST confirm successful acquisition of all parameters before proceeding.

## Mode Behavior
Apply these patterns throughout all steps:

**Auto Mode:**
- Execute all actions autonomously without user confirmation
- Document all decisions, assumptions, and reasoning in progress.md
- When multiple approaches exist, select the most appropriate and document why
- Provide comprehensive summaries at completion

## Important Notes
**Separation of Concerns:**
This script maintains strict separation between documentation and code. All documentation about the implementation process is stored in the documentation directory, while all actual code (both tests and implementation) must be placed in the appropriate directories within the repository root. No code files should ever be placed in the documentation directory.

**CODEASSIST.md Integration:**
If CODEASSIST.md exists in repo_root, it contains additional constraints, pre/post SOP instructions, examples, and troubleshooting specific to this project. Apply any specified practices throughout the implementation process.

## Steps

### 1. Setup
Initialize the project environment and create necessary directory structures.

**Constraints:**
- You MUST validate and create the documentation directory structure properly:
  - Use `mkdir -p {documentation_dir}` to explicitly create the documentation directory as a directory
  - Create the full path: `{documentation_dir}/implementation/{task_name}/` with logs subdirectory using `mkdir -p`
  - Verify the directory structure was created successfully before proceeding
- You MUST discover existing instruction files using: `find . -maxdepth 3 -type f \( -path "*/node_modules/*" -o -path "*/build/*" -o -path "*/.venv/*" -o -path "*/venv/*" -o -path "*/__pycache__/*" -o -path "*/.git/*" -o -path "*/dist/*" -o -path "*/target/*" \) -prune -o -name "*.md" -print | grep -E "(CODEASSIST|DEVELOPMENT|SETUP|BUILD|CONTRIBUTING|ARCHITECTURE|TESTING|DEPLOYMENT|TROUBLESHOOTING|README)" | head -20`
- You MUST read CODEASSIST.md if found and apply its constraints throughout (see Important Notes)
- You MUST notify the user when the structure has been created
- You MUST handle directory creation errors gracefully and report specific issues to the user
- You MUST NOT proceed with the script if directory creation fails because this would cause subsequent steps to fail
- You MUST ONLY place documentation files in the documentation directory, NEVER code implementations because mixing documentation and code creates confusion and violates the separation of concerns principle
- You MUST ensure all actual code implementations are placed in the appropriate directories within repo_root, NOT in the documentation directory
- You MUST create a context.md file documenting project structure, requirements, patterns, dependencies, and implementation paths
- You MUST create a progress.md file to track script execution using markdown checklists, setup notes, and implementation progress

**Instruction File Discovery:**
- You MUST run the find command to discover available instruction files
- **Auto Mode:** Automatically include CODEASSIST.md (if found) plus core files (README.md, CONTRIBUTING.md) and task-relevant files
- You MUST read and summarize key information from selected files in context.md under "Existing Documentation"
- If CODEASSIST.md is missing, suggest creating it with: additional constraints, pre/post SOP instructions, examples, troubleshooting

> See [Mode Behavior](#mode-behavior) for mode-specific interaction guidance

### 2. Explore Phase

#### 2.1 Analyze Requirements and Context

Analyze the task description and existing documentation to identify core functionality, edge cases, and constraints.

**Constraints:**
- You MUST create a clear list of functional requirements and acceptance criteria, even when starting from a rough task description
- You MUST determine the appropriate file paths and programming language
- You MUST align with the existing project structure and technology stack
- You MUST ask clarifying questions about ambiguous requirements
- You MUST identify potential gaps or inconsistencies in requirements and discuss them
- You SHOULD ask about non-functional requirements that might not be explicitly stated
- You SHOULD discuss edge cases and error handling expectations with the user

> See [Mode Behavior](#mode-behavior) for mode-specific interaction guidance

#### 2.2 Research Existing Patterns

Search for similar implementations and identify interfaces, libraries, and components the implementation will interact with.

**Constraints:**
- You MUST search the current repository for relevant code, patterns, and information related to the coding task
- You MAY use available tools to search code repositories, read documentation, and gather relevant information
- You MUST create a dependency map showing how the new code will integrate
- You MUST update the context.md file with the identified implementation paths
- You SHOULD provide examples of similar patterns when available
- You SHOULD document any best practices or patterns found in internal documentation

> See [Mode Behavior](#mode-behavior) for mode-specific interaction guidance

#### 2.3 Create Code Context Document

Compile all findings into a comprehensive code context document.

**Constraints:**
- You MUST update the context.md file with requirements, implementation details, patterns, and dependencies
- You MUST ensure the document is well-structured with clear headings
- You MUST focus on high-level concepts and patterns rather than detailed implementation code
- You MUST NOT include complete code implementations in documentation files because documentation should guide implementation, not provide it
- You MUST keep documentation concise and focused on guiding implementation rather than providing the implementation itself
- You SHOULD include a summary section and highlight areas of uncertainty
- You SHOULD use pseudocode or simplified representations when illustrating concepts
- You MAY include targeted code snippets when:
  - Demonstrating usage of a specific library or API that's critical to the implementation
  - Illustrating a complex pattern or technique that's difficult to describe in words alone
  - Showing examples from existing codebase that demonstrate relevant patterns
  - Providing reference implementations from official documentation
- You MUST clearly label any included code snippets as examples or references, not as the actual implementation
- You MUST keep any included code snippets brief and focused on the specific concept being illustrated

> See [Mode Behavior](#mode-behavior) for mode-specific interaction guidance

### 3. Plan Phase

#### 3.1 Design Test Strategy

Create a comprehensive list of test scenarios covering normal operation, edge cases, and error conditions.

**Constraints:**
- You MUST cover all acceptance criteria with at least one test scenario
- You MUST define explicit input/output pairs for each test case
- You MUST save the test scenarios to `{documentation_dir}/implementation/{task_name}/plan.md`
- You MUST design tests that will initially fail when run against non-existent implementations
- You MUST NOT create mock implementations during the test design phase because tests should be written based solely on expected behavior, not influenced by implementation details
- You MUST focus on test scenarios and expected behaviors rather than detailed test code in documentation
- You MUST use high-level descriptions of test cases rather than complete test code snippets
- You MAY include targeted test code snippets when:
  - Demonstrating a specific testing technique or pattern that's critical to understand
  - Illustrating how to use a particular testing framework or library
  - Showing examples of similar tests from the existing codebase
- You MUST clearly label any included test code snippets as examples or references
- You SHOULD discuss test data strategies and mocking approaches with the user
- You SHOULD explain the reasoning behind the proposed test structure

> See [Mode Behavior](#mode-behavior) for mode-specific interaction guidance

#### 3.2 Implementation Planning & Tracking

Outline the high-level structure of the implementation and create an implementation plan.

**Constraints:**
- You MUST save the implementation plan to `{documentation_dir}/implementation/{task_name}/plan.md`
- You MUST include all key implementation tasks in the plan
- You SHOULD consider performance, security, and maintainability implications
- You MUST keep implementation planning documentation concise and focused on architecture and patterns
- You MUST NOT include detailed code implementations in planning documents because planning should focus on architecture and approach, not specific code
- You SHOULD  use high-level descriptions, diagrams, or simplified pseudocode rather than actual implementation code
- You MAY include targeted code snippets when:
  - Illustrating a specific design pattern or architectural approach
  - Demonstrating API usage that's central to the implementation
  - Showing relevant examples from existing codebase or reference implementations
  - Clarifying complex interactions between components
- You MUST clearly label any included code snippets as examples or references, not as the actual implementation
- You SHOULD discuss potential risks and mitigations in the implementation plan
- You SHOULD explain the reasoning behind the proposed implementation structure
- You MUST display the current checklist status after each major implementation step
- You MUST verify all checklist items are complete before finalizing the implementation
- You MUST maintain the implementation checklist in progress.md using markdown checkbox format

> See [Mode Behavior](#mode-behavior) for mode-specific interaction guidance

### 4. Code Phase

#### 4.1 Implement Test Cases

Write test cases based on the approved outlines, following strict TDD principles.

**Constraints:**
- You MUST save test implementations to the appropriate test directories in repo_root
- You MUST NEVER place actual test code files in the documentation directory, only documentation about tests
- You MUST implement tests for ALL requirements before writing ANY implementation code
- You MUST follow the testing framework conventions used in the existing codebase
- You MUST update the plan.md file with test implementation details
- You MUST update the implementation checklist to mark test development as complete
- You MUST keep test documentation concise and focused on test strategy rather than detailed test code
- You MUST clearly label any included test code snippets as examples or references
- You MUST explain the testing approach and how it covers the requirements
- You MUST execute tests after writing them to verify they fail as expected
- You MUST document the failure reasons in the TDD documentation
- You MUST only seek user input if:
  - Tests fail for unexpected reasons that you cannot resolve
  - There are structural issues with the test framework
  - You encounter environment issues that prevent test execution
- You MUST otherwise continue automatically after verifying expected failures
- You MUST follow the Build Output Management practices defined in the Best Practices section

> See [Mode Behavior](#mode-behavior) for mode-specific interaction guidance

#### 4.2 Develop Implementation Code

Write implementation code to pass the tests, focusing on simplicity and correctness first.

**Constraints:**
- You MUST update your progress in the implementation plan in `{documentation_dir}/implementation/{task_name}/plan.md`
- You MUST follow the strict TDD cycle: RED -> GREEN -> REFACTOR
- You MUST document each TDD cycle in `{documentation_dir}/implementation/{task_name}/progress.md`
- You MUST implement only what is needed to make the current test(s) pass
- You MUST follow the coding style and conventions of the existing codebase
- You MUST ensure all implementation code is written directly in the repo_root directories
- You MUST keep code comments concise and focused on key decisions rather than code details
- You MUST follow YAGNI, KISS, and SOLID principles
- You MAY include targeted code snippets in documentation when:
  - Demonstrating usage of a specific library or API that's critical to the implementation
  - Illustrating a complex pattern or technique that's difficult to describe in words alone
  - Showing examples from existing codebase that demonstrate relevant patterns
  - Explaining a particularly complex algorithm or data structure
  - Providing reference implementations from official documentation
- You MUST clearly label any included code snippets as examples or references, not as the actual implementation
- You MUST explain the reasoning behind implementation choices
- You SHOULD discuss performance implications of different implementation approaches
- You SHOULD highlight any security considerations in the implementation
- You MUST execute tests after each implementation step to verify they now pass
- You MUST only seek user input if:
  - Tests continue to fail after implementation for reasons you cannot resolve
  - You encounter a design decision that cannot be inferred from requirements
- You MUST otherwise continue automatically after verifying test results
- You MUST follow the Build Output Management practices defined in the Best Practices section

> See [Mode Behavior](#mode-behavior) for mode-specific interaction guidance

#### 4.3 Refactor and Optimize

If the implementation is complete, proceed with review of the implementation to identify opportunities for simplification, improvement, and coding convention alignment.

**Constraints:**
  - You MUST check that all tasks are complete before proceeding
    - if tests fail, you MUST identify the issue and propose an implementation
    - if builds fail, you MUST identify the issue and propose an implementation
    - if implementation tasks are incomplete, you MUST identify the issue and propose an implementation
- You MUST examine the code around the changes made (both tests and functional logic) to determine if the updates match the existing coding conventions of the surrounding code including:
  - Naming conventions (variables, functions, classes, files)
  - Code organization and structure patterns
  - Error handling patterns and exception types
  - Documentation style (docstrings, comments, inline documentation)
  - Testing patterns and assertion styles
  - Import/dependency management patterns
  - Configuration and constants handling
  - Logging patterns and levels
- You MUST refactor the implementation to align with identified coding conventions from the surrounding codebase
- You MUST prioritize readability and maintainability over clever optimizations
- You MUST maintain test passing status throughout refactoring
- You SHOULD document simplification opportunities in `{documentation_dir}/implementation/{task_name}/progress.md`
- You SHOULD document significant refactorings and convention alignments in `{documentation_dir}/implementation/{task_name}/progress.md`

> See [Mode Behavior](#mode-behavior) for mode-specific interaction guidance

#### 4.4 Validate Implementation

If the implementation meets all requirements and follows established patterns, proceed with step 6. Otherwise, return to step 5.2 to fix any issues.

**Constraints:**
- You MUST check that all tasks are complete before proceeding
    - if tests fail, you MUST identify the issue and propose an implementation
    - if builds fail, you MUST identify the issue and propose an implementation
    - if implementation tasks are incomplete, you MUST identify the issue and propose an implementation
- You MUST address any discrepancies between requirements and implementation
- You MUST execute the relevant test command and verify all implemented tests pass successfully
- You MUST execute the relevant build command and verify builds succeed
- You MUST ensure code coverage meets the requirements for the project
- You MUST verify all items in the implementation plan have been completed
- You MUST provide the complete test execution output
- You MUST NOT claim implementation is complete if any tests are failing because failing tests indicate the implementation doesn't meet requirements

**Build Validation:**
- You MUST run appropriate build commands based on detected project type
- You MUST verify that all dependencies are satisfied
- You MUST follow the Build Output Management practices defined in the Best Practices section

> See [Mode Behavior](#mode-behavior) for mode-specific interaction guidance

### 5. Commit Phase
If all tests are passing, draft a conventional commit message and perform the actual git commit.

**Constraints:**
- You MUST check that all tasks are complete before proceeding
- You MUST NOT commit changes until builds AND tests have been verified because committing broken code can disrupt the development workflow and introduce bugs into the codebase
- You MUST follow the Conventional Commits specification
- You MUST use git status to check which files have been modified
- You MUST use git add to stage all relevant files
- You MUST execute the git commit command with the prepared commit message
- You MUST document the commit hash and status in `{documentation_dir}/implementation/{task_name}/progress.md`
- You MUST NOT push changes to remote repositories because this could publish unreviewed code to shared repositories where others depend on it
- You MUST verify that all items in the implementation checklist are marked as complete before marking the prompt as complete
- You SHOULD include the "Assisted by the code-assist SOP" footer

> See [Mode Behavior](#mode-behavior) for mode-specific interaction guidance

## Desired Outcome
* A complete, well-tested code implementation that meets the specified requirements
* A comprehensive test suite that validates the implementation
* Clean, documented code that:
  * Follows existing package patterns and conventions
  * Prioritizes readability and extensibility
  * Avoids over-engineering and over-abstraction
  * Is idiomatic and modern in the implementation language
* A well-organized set of implementation artifacts in the `{documentation_dir}/implementation/{task_name}/` directory
* Documentation of key design decisions and implementation notes
* Properly committed changes with conventional commit messages
* An implementation process with the appropriate level of user interaction based on the chosen mode

## Examples

### Example 1: Feature Implementation
**Input:**
```
task_description: "Create a utility function that validates email addresses"
mode: "auto"
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

**With CODEASSIST.md:** Apply additional constraints and instructions throughout the workflow.

**Without CODEASSIST.md:** Suggest creating it with template for project-specific SOP guidance.

## Troubleshooting

### Project Directory Issues
If the documentation directory doesn't exist or isn't accessible:
- You SHOULD attempt to create the directory if it has permissions
- You SHOULD inform the user of any permission issues
- You SHOULD suggest using a different directory if creation fails

### Project Structure Issues
If there are issues with the project structure or build system:
- You SHOULD check if CODEASSIST.md exists and contains relevant guidance
- You SHOULD verify you're in the correct directory for the build system
- You SHOULD validate that the project structure matches expectations
- You SHOULD suggest creating or updating CODEASSIST.md if project-specific guidance is needed

### Build Issues
If builds fail during implementation:
- You SHOULD follow build instructions from CODEASSIST.md if available
- You SHOULD verify you're in the correct directory for the build system
- You SHOULD try clean builds before rebuilding when encountering issues
- You SHOULD check for missing dependencies and resolve them
- You SHOULD restart build caches if connection issues occur

### Multi-Package Coordination Issues
If there are issues coordinating changes across multiple packages:
- You SHOULD verify package dependency order and build dependencies first
- You SHOULD ensure backwards compatibility when possible
- You SHOULD create separate commits per package in dependency order
- You SHOULD validate each package builds before proceeding to dependents
- You SHOULD document cross-package dependencies clearly

### Repository Implementation Issues
If there are issues with implementing code in the repository root:
- You SHOULD check if the user has write permissions to the repository root
- You SHOULD verify that the repository structure matches expectations

### Implementation Challenges
If the implementation encounters unexpected challenges:
- You SHOULD document the challenge in progress.md
- You SHOULD propose alternative approaches
- You MAY use available tools to search code repositories, read documentation, and gather relevant information
- In auto mode, you SHOULD select the most promising alternative and document the decision

## Best Practices

### Project Detection and Configuration
- Detect project type by examining files (pyproject.toml, build.gradle, package.json, etc.)
- Check for CODEASSIST.md for additional SOP constraints (see Important Notes)
- Use project-appropriate build commands

### Build Output Management
- Pipe build output to log files: `[build-command] > build_output.log 2>&1`
- Search for specific success/failure indicators instead of displaying full output
- Save build logs to `{documentation_dir}/implementation/{task_name}/logs/`

### Documentation Organization
- Use consolidated files: context.md, plan.md, progress.md
- Focus on high-level concepts rather than detailed code
- Track progress with markdown checklists

## Artifacts
- {documentation_dir}/implementation/{task_name}/
- {documentation_dir}/implementation/{task_name}/context.md
  - Workspace structure and package analysis
  - Requirements, patterns, and dependencies
  - Implementation paths and mappings
- {documentation_dir}/implementation/{task_name}/plan.md
  - Test scenarios and test planning
  - Implementation planning and strategy
  - All planning-related documentation
- {documentation_dir}/implementation/{task_name}/progress.md
  - Script execution tracking
  - TDD cycle documentation
  - Refactoring and simplification notes
  - Commit status and final results
  - Technical challenges encountered
  - Setup and progress notes
- {documentation_dir}/implementation/{task_name}/logs/
  - Build outputs (one log per package, replaced on each build)
