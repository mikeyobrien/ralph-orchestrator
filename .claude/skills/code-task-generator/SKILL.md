---
name: code-task-generator
description: This sop generates structured code task files from rough descriptions, ideas, or PDD implementation plans. It automatically detects the input type and creates properly formatted code task files following Amazon's code task format specification. For PDD plans, it processes implementation steps one at a time to allow for learning and adaptation between steps.
type: anthropic-skill
version: "1.0"
---

# Code Task Generator

## Overview

This sop generates structured code task files from rough descriptions, ideas, or PDD implementation plans. It automatically detects the input type and creates properly formatted code task files following Amazon's code task format specification. For PDD plans, it processes implementation steps one at a time to allow for learning and adaptation between steps.

## Parameters

- **input** (required): Task description, file path, or PDD plan path. Can be a simple sentence, paragraph, detailed explanation, or path to a PDD implementation plan.
- **step_number** (optional): For PDD plans only - specific step to process. If not provided, automatically determines the next uncompleted step from the checklist.
- **output_dir** (optional, default: "tasks/"): Directory where the code task file will be created
- **task_name** (optional): For descriptions only - specific task name. If not provided, will be generated from the description.

**Constraints for parameter acquisition:**
- You MUST ask for all required parameters upfront in a single prompt rather than one at a time
- You MUST support multiple input methods for input including:
  - Direct text input
  - File path containing the description or PDD plan
  - Directory path (will look for plan.md within it)
  - URL to internal documentation
- You MUST confirm successful acquisition of all parameters before proceeding

## Steps

### 1. Detect Input Mode

Automatically determine whether input is a description or PDD plan.

**Constraints:**
- You MUST check if input is a file path that exists
- If file exists, You MUST read it and check for PDD plan structure (checklist, numbered steps)
- If file contains PDD checklist format, You MUST set mode to "pdd"
- If input is text or file without PDD structure, You MUST set mode to "description"
- You MUST inform user which mode was detected
- You MUST validate that PDD plans follow expected format with numbered steps

### 2. Analyze Input

Parse and understand the input content based on detected mode.

**Constraints:**
- For PDD mode: You MUST parse implementation plan and extract steps/checklist status
- For PDD mode: You MUST determine target step based on step_number parameter or first uncompleted step
- For description mode: You MUST identify the core functionality being requested
- You MUST extract any technical requirements, constraints, or preferences mentioned
- You MUST determine the appropriate complexity level (Low/Medium/High)
- You MUST identify the likely technology stack or domain area

### 3. Structure Requirements

Organize requirements and determine task breakdown based on mode.

**Constraints:**
- For PDD mode: You MUST extract target step's title, description, demo requirements, and constraints
- For PDD mode: You MUST preserve integration notes with previous steps
- For PDD mode: You MUST identify which specific research documents (if any) are directly relevant to each task being created
- For description mode: You MUST identify specific functional requirements from the description
- You MUST infer reasonable technical constraints and dependencies
- You MUST create measurable acceptance criteria using Given-When-Then format
- You MUST prepare task breakdown plan for approval

### 4. Plan Tasks

Present task breakdown for user approval before generation.

**Constraints:**
- You MUST analyze content to identify logical sub-tasks for implementation
- You MUST present concise one-line summary for each planned code task
- You MUST show proposed task sequence and dependencies
- You MUST ask user to approve the plan before proceeding
- You MUST allow user to request modifications to the task breakdown
- You MUST NOT proceed to generate actual code task files until user explicitly approves

### 5. Generate Tasks

Create appropriate file structure based on mode and approved plan.

**Constraints:**
- For PDD mode: You MUST create a folder named `step{NN}` where NN is zero-padded (e.g., step01, step02, step10)
- For PDD mode: You MUST create multiple code task files within the step folder, named sequentially: `task-01-{title}.code-task.md`, `task-02-{title}.code-task.md`, etc.
- For PDD mode: You MUST break down the step into logical implementation phases focusing on functional components, NOT separate testing tasks
- For PDD mode: You MUST include "Reference Documentation" section with path to design/detailed-design.md as required reading
- For PDD mode: You MUST include specific research documents in "Additional References" only if they are directly relevant to the task (e.g., specific technology research for that component)
- For PDD mode: You MUST add a note instructing agents to read the detailed design before implementation
- For description mode: You MUST create single task or multiple tasks as planned
- You MUST add YAML frontmatter with `status: pending`, `created: <current date in YYYY-MM-DD format>`, `started: null`, `completed: null`
- You MUST generate task names using kebab-case format
- You MUST create files with `.code-task.md` extension
- You MUST follow the exact format specified in the Code Task Format section below
- You MUST include comprehensive acceptance criteria that cover the main functionality
- You MUST include unit test requirements as part of the acceptance criteria for each implementation task
- You MUST NOT create separate tasks for "add unit tests" or "write tests" because testing should be integrated into each functional implementation task
- You MUST provide realistic complexity assessment and required skills
- You MUST save files to the specified output directory

### 6. Report Results

Inform user about generated tasks and next steps.

**Constraints:**
- You MUST list all generated code task files with their paths
- For PDD mode: You MUST provide the step demo requirements for context
- For description mode: You MUST provide a brief summary of what was created
- You MUST suggest running code-assist on each task in appropriate sequence
- For PDD mode: You MUST NOT create any additional log files or summary documents
- For description mode: You MUST offer to create additional related tasks if the scope seems large

### 7. Offer Ralph Integration

After generating code tasks, offer to create a PROMPT.md file for Ralph.

**Constraints:**
- You MUST ask the user: "Would you like me to create a PROMPT.md for Ralph to implement these tasks?"
- If the user agrees, You MUST create a minimal PROMPT.md file containing:
  - A clear objective statement (what to implement)
  - Reference to the generated code task files
  - Suggested execution order
  - Brief acceptance criteria
- The PROMPT.md should be concise - Ralph will read the task files for details
- If the user declines, You SHOULD acknowledge and conclude the session

## Code Task Format Specification

Each code task file MUST follow this exact structure:

```markdown
---
status: pending
created: YYYY-MM-DD
started: null
completed: null
---
# Task: [Task Name]

## Description
[A clear description of what needs to be implemented and why]

## Background
[Relevant context and background information needed to understand the task]

## Reference Documentation
**Required:**
- Design: [path to detailed design document]

**Additional References (if relevant to this task):**
- [Specific research document or section]

**Note:** You MUST read the detailed design document before beginning implementation. Read additional references as needed for context.

## Technical Requirements
1. [First requirement]
2. [Second requirement]
3. [Third requirement]

## Dependencies
- [First dependency with details]
- [Second dependency with details]

## Implementation Approach
1. [First implementation step or approach]
2. [Second implementation step or approach]

## Acceptance Criteria

1. **[Criterion Name]**
   - Given [precondition]
   - When [action]
   - Then [expected result]

2. **[Another Criterion]**
   - Given [precondition]
   - When [action]
   - Then [expected result]

## Metadata
- **Complexity**: [Low/Medium/High]
- **Labels**: [Comma-separated list of labels]
- **Required Skills**: [Skills needed for implementation]
```

### Code Task Format Example

```markdown
---
status: pending
created: 2026-01-15
started: null
completed: null
---
# Task: Create Email Validator Function

## Description
Create a function that validates email addresses and returns detailed error messages for invalid formats. This will be used across the application to ensure data quality and provide user-friendly feedback.

## Background
The application currently accepts any string as an email address, leading to data quality issues and failed communications. We need a robust validation function that can identify common email format errors and provide specific feedback to users.

## Reference Documentation
**Required:**
- Design: planning/design/detailed-design.md

**Additional References (if relevant to this task):**
- planning/research/validation-libraries.md (for email validation approach)

**Note:** You MUST read the detailed design document before beginning implementation. Read additional references as needed for context.

## Technical Requirements
1. Create a function that accepts an email string and returns validation results
2. Implement comprehensive email format validation using regex or email parsing library
3. Return detailed error messages for specific validation failures
4. Support common email formats including international domains
5. Include performance optimization for high-volume validation

## Dependencies
- Email validation library or regex patterns
- Error handling framework for structured error responses
- Unit testing framework for comprehensive test coverage

## Implementation Approach
1. Research and select appropriate email validation approach (regex vs library)
2. Implement core validation logic with specific error categorization
3. Add comprehensive error messaging for different failure types
4. Optimize for performance if needed for high-volume scenarios

## Acceptance Criteria

1. **Valid Email Acceptance**
   - Given a properly formatted email address
   - When the validation function is called
   - Then the function returns success with no errors

2. **Invalid Format Detection**
   - Given an email with invalid format (missing @, invalid characters, etc.)
   - When the validation function is called
   - Then the function returns failure with specific error message

3. **Detailed Error Messages**
   - Given various types of invalid emails
   - When validation fails
   - Then specific error messages are returned (e.g., "Missing @ symbol", "Invalid domain format")

4. **Performance Requirements**
   - Given 1000 email validations
   - When executed in sequence
   - Then all validations complete within 1 second

5. **Unit Test Coverage**
   - Given the email validator implementation
   - When running the test suite
   - Then all validation scenarios have corresponding unit tests with >90% coverage

## Metadata
- **Complexity**: Low
- **Labels**: Validation, Email, Data Quality, Utility Function
- **Required Skills**: Regular expressions, email standards, unit testing
```

## Examples

### Example Input (Description Mode)
```
input: "I need a function that validates email addresses and returns detailed error messages"
output_dir: "tasks/"
```

### Example Output (Description Mode)
```
Detected mode: description

Generated code task: tasks/email-validator.code-task.md

Created task for email validation functionality with comprehensive acceptance criteria and implementation guidance.

Next steps: Run code-assist on the generated task to implement the solution.
```

### Example Input (PDD Mode)
```
input: "planning/implementation/plan.md"
```

### Example Output (PDD Mode)
```
Detected mode: pdd

Generated code tasks for step 2: planning/implementation/step02/

Created tasks:
- task-01-create-data-models.code-task.md
- task-02-implement-validation.code-task.md  
- task-03-add-serialization.code-task.md

Next steps: Run code-assist on each task in sequence

Step demo: Working data models with validation that can create, validate, and serialize/deserialize data objects
```

## Troubleshooting

### Vague Description (Description Mode)
If the task description is too vague or unclear:
- You SHOULD ask clarifying questions about specific requirements
- You SHOULD suggest common patterns or approaches for the domain
- You SHOULD create a basic task and offer to refine it based on feedback

### Complex Description (Description Mode)
If the description suggests a very large or complex task:
- You SHOULD suggest breaking it into multiple smaller tasks
- You SHOULD focus on the core functionality for the initial task
- You SHOULD offer to create additional related tasks

### Missing Technical Details (Description Mode)
If technical implementation details are unclear:
- You SHOULD make reasonable assumptions based on common practices
- You SHOULD include multiple implementation approaches in the task
- You SHOULD note areas where the user should make technical decisions

### Plan File Not Found (PDD Mode)
If the specified plan file doesn't exist:
- You SHOULD check if the path is a directory and look for plan.md within it
- You SHOULD suggest common locations where PDD plans might be stored
- You SHOULD validate the file path format and suggest corrections

### Invalid Plan Format (PDD Mode)
If the plan doesn't follow expected PDD format:
- You SHOULD identify what sections are missing or malformed
- You SHOULD suggest running the PDD script to generate a proper plan
- You SHOULD attempt to extract what information is available

### No Uncompleted Steps (PDD Mode)
If all steps in the checklist are marked complete:
- You SHOULD inform the user that all steps appear to be complete
- You SHOULD ask if they want to generate a task for a specific step anyway
- You SHOULD suggest reviewing the implementation plan for potential new steps
