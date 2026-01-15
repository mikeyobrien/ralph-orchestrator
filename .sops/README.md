# Standard Operating Procedures (SOPs)

This directory contains agent SOPs (Standard Operating Procedures) for structured AI-assisted development workflows.

## Provenance

These SOPs are sourced from the [strands-agents/agent-sop](https://github.com/strands-agents/agent-sop) repository, an open-source collection of workflow definitions for AI coding agents.

## Available SOPs

| SOP | Description |
|-----|-------------|
| [code-assist.sop.md](./code-assist.sop.md) | TDD-based code implementation following Explore → Plan → Code → Commit workflow |
| [code-task-generator.sop.md](./code-task-generator.sop.md) | Generates structured code task files from descriptions or PDD plans |

## Usage

These SOPs can be referenced by AI coding agents (Claude Code, Cursor, Kiro, etc.) to follow structured, repeatable workflows for code implementation tasks.

### code-assist

Use for implementing code tasks with test-driven development:
- Explores codebase patterns and requirements
- Plans test strategy and implementation approach
- Implements using RED → GREEN → REFACTOR cycle
- Commits with conventional commit messages

### code-task-generator

Use for breaking down requirements into structured tasks:
- Accepts rough descriptions or PDD implementation plans
- Generates `.code-task.md` files with acceptance criteria
- Supports both single tasks and multi-step PDD workflows

## Acknowledgements

These SOPs are adapted from the excellent [strands-agents/agent-sop](https://github.com/strands-agents/agent-sop) repository. Thank you to the Strands Agents team for open-sourcing these workflow definitions.

## License

See the [original repository](https://github.com/strands-agents/agent-sop) for license information.
