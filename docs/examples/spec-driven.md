# Spec-Driven Development Example

!!! note "Documentation In Progress"
    This page is under development. Check back soon for a complete spec-driven workflow example.

## Overview

This example demonstrates using Hats with specification-first development, where requirements are formalized before implementation begins.

## Workflow

1. Create specification in `specs/` directory
2. Review and approve spec
3. Generate implementation tasks
4. Execute with Hats orchestration

## Example Spec

```markdown
# Feature: User Authentication

## Given
- User registration system exists

## When
- User provides valid credentials

## Then
- User receives authentication token
- Session is established
```

## See Also

- [TDD Workflow](tdd-workflow.md) - Test-first approach
- [Simple Task](simple-task.md) - Basic example
- [Writing Prompts](../guide/prompts.md) - Prompt best practices
