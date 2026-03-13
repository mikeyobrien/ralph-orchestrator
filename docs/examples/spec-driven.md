# Spec-Driven Development Example

!!! note "Example Only"
    Spec-driven development is now documented as an example pattern, not shipped as a built-in preset.

## Overview

This example demonstrates a specification-first workflow, where requirements are formalized before implementation begins.

If you want a supported builtin today, start with `builtin:code-assist` for implementation work or `builtin:pdd-to-code-assist` for the longer idea-to-code flow.

If you specifically want an example-only automated design workflow, see [Automated PDD Design](pdd-design.md) and its example preset at `docs/examples/presets/auto-pdd.yml`.

## Workflow

1. Create specification/design artifacts in `.agents/scratchpad/implementation/{spec_name}/`
2. Review and approve spec
3. Generate implementation tasks
4. Execute with Ralph orchestration

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
