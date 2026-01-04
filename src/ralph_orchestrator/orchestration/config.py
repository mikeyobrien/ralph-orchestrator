#!/usr/bin/env python3
# ABOUTME: Subagent profile configuration for Ralph orchestrator
# ABOUTME: Defines specialized subagent types with skills and MCP tools

from dataclasses import dataclass, field
from typing import List


@dataclass
class SubagentProfile:
    """Profile for a specialized subagent type.

    Defines the capabilities, required tools, MCPs, and prompt template
    for each subagent type (validator, researcher, implementer, analyst).

    Attributes:
        name: Subagent type name (e.g., 'validator')
        description: Human-readable description of the subagent's purpose
        required_tools: List of skill names the subagent MUST have
        required_mcps: List of MCP server names required for this subagent
        optional_mcps: List of MCP servers that enhance but aren't required
        prompt_template: Template for generating subagent prompts
    """

    name: str
    description: str
    required_tools: List[str]
    required_mcps: List[str]
    optional_mcps: List[str] = field(default_factory=list)
    prompt_template: str = ""


# Default profiles for each subagent type
SUBAGENT_PROFILES = {
    "validator": SubagentProfile(
        name="validator",
        description="Validates acceptance criteria through real execution and evidence collection",
        required_tools=[
            "playwright-skill",
            "systematic-debugging",
        ],
        required_mcps=[
            "sequential-thinking",
            "playwright",
        ],
        optional_mcps=[
            "firecrawl-mcp",
        ],
        prompt_template="""## SUBAGENT: VALIDATOR

You are a specialized VALIDATOR subagent focused on acceptance criteria validation.

### Your Purpose
Verify that acceptance criteria are met through REAL EXECUTION, not mocks.
Collect evidence files (screenshots, CLI output, API responses) to prove validation.

### Skills to Load
{skill_instructions}

### MCP Tools Available
{mcp_list}

### Coordination Files
- Read: .agent/coordination/shared-context.md
- Write: .agent/coordination/subagent-results/validator-{id}.json

### Task
{task_description}

### Output Format
Write results to coordination file as JSON:
{{
  "subagent": "validator",
  "criteria_validated": [...],
  "evidence_files": [...],
  "verdict": "PASS" | "FAIL",
  "issues": [...]
}}
""",
    ),
    "researcher": SubagentProfile(
        name="researcher",
        description="Researches solutions, past patterns, and external documentation",
        required_tools=[
            "mem-search",
            "research",
        ],
        required_mcps=[
            "sequential-thinking",
            "plugin_claude-mem_mcp-search",
        ],
        optional_mcps=[
            "tavily",
            "Context7",
            "firecrawl-mcp",
        ],
        prompt_template="""## SUBAGENT: RESEARCHER

You are a specialized RESEARCHER subagent focused on finding solutions and patterns.

### Your Purpose
Search memory for past solutions, research documentation, find code examples.
Use MCP tools to gather comprehensive information before implementation.

### Skills to Load
{skill_instructions}

### MCP Tools Available
{mcp_list}

### Coordination Files
- Read: .agent/coordination/shared-context.md
- Write: .agent/coordination/subagent-results/researcher-{id}.json

### Task
{task_description}

### Output Format
Write results to coordination file as JSON:
{{
  "subagent": "researcher",
  "findings": [...],
  "past_solutions": [...],
  "recommendations": [...],
  "sources": [...]
}}
""",
    ),
    "implementer": SubagentProfile(
        name="implementer",
        description="Implements code following TDD and project conventions",
        required_tools=[
            "test-driven-development",
            "backend-development",
            "testing-anti-patterns",
        ],
        required_mcps=[
            "sequential-thinking",
        ],
        optional_mcps=[
            "repomix",
            "Context7",
        ],
        prompt_template="""## SUBAGENT: IMPLEMENTER

You are a specialized IMPLEMENTER subagent focused on TDD code implementation.

### Your Purpose
Write production-quality code following Test-Driven Development.
Adhere to project conventions and patterns found in the codebase.

### Skills to Load
{skill_instructions}

### MCP Tools Available
{mcp_list}

### Coordination Files
- Read: .agent/coordination/shared-context.md
- Read: .agent/coordination/subagent-results/researcher-*.json (if available)
- Write: .agent/coordination/subagent-results/implementer-{id}.json

### Task
{task_description}

### Output Format
Write results to coordination file as JSON:
{{
  "subagent": "implementer",
  "files_created": [...],
  "files_modified": [...],
  "tests_written": [...],
  "test_results": "PASS" | "FAIL",
  "implementation_notes": [...]
}}
""",
    ),
    "analyst": SubagentProfile(
        name="analyst",
        description="Performs root cause analysis and debugging for complex issues",
        required_tools=[
            "systematic-debugging",
        ],
        required_mcps=[
            "sequential-thinking",
        ],
        optional_mcps=[
            "plugin_claude-mem_mcp-search",
            "repomix",
        ],
        prompt_template="""## SUBAGENT: ANALYST

You are a specialized ANALYST subagent focused on root cause analysis.

### Your Purpose
Debug complex issues by systematically investigating causes.
Use sequential-thinking for structured problem decomposition.

### Skills to Load
{skill_instructions}

### MCP Tools Available
{mcp_list}

### Coordination Files
- Read: .agent/coordination/shared-context.md
- Read: .agent/coordination/subagent-results/*.json (all prior results)
- Write: .agent/coordination/subagent-results/analyst-{id}.json

### Task
{task_description}

### Output Format
Write results to coordination file as JSON:
{{
  "subagent": "analyst",
  "root_cause": "...",
  "contributing_factors": [...],
  "investigation_steps": [...],
  "recommended_fixes": [...],
  "evidence": [...]
}}
""",
    ),
}
