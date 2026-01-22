# AI-Native Development Agent Specs: Best Practices

> Research compiled January 2026, focusing on developments from November 2025 - January 2026

## Table of Contents

- [Spec Generator Tool](#spec-generator-tool)
- [Emerging Standards](#emerging-standards)
- [Spec Structure](#spec-structure)
- [Correctness & Validation](#correctness--validation)
- [Token Efficiency](#token-efficiency)
- [Anti-Patterns](#anti-patterns)
- [Sources](#sources)

---

## Spec Generator Tool

Generate token-efficient `.spec.md` files following these best practices.

**Location:** `~/.claude/skills/spec-generator/` (global skill)

**Usage:**
```
/spec-generator "<description>" [--output <dir>]
```

**Output:** `<name>.spec.md` files with:
- Goal (verb + noun, ~50 tokens)
- Context (pointers only, ~100 tokens)
- Requirements (numbered, testable)
- Acceptance Criteria (Given-When-Then)

---

## Emerging Standards

### AGENTS.md (Multi-Vendor Standard)

Launched jointly by Google, OpenAI, Factory, Sourcegraph, and Cursor in late 2025. A vendor-neutral markdown file replacing fragmented tool-specific formats.

**Adoption (January 2026):** Claude Code, OpenAI Codex CLI, GitHub Copilot, Cursor, VS Code Insiders, Amp, Goose, Letta.

### Agent Skills (December 2025)

Released by Anthropic as an open standard on December 18, 2025. Now adopted by OpenAI ChatGPT, Codex CLI, and others.

**Core format:**
```
skill-name/
├── SKILL.md          # Required: YAML frontmatter + instructions
├── reference.md      # Optional: additional documentation
└── scripts/          # Optional: executable tools
```

**SKILL.md structure:**
```yaml
---
name: skill-name
description: Brief description for agent discovery
---

# Instructions
[Skill instructions here]
```

### Spec-Driven Development (SDD)

Tools like GitHub Spec Kit, AWS Kiro, and Tessl use structured spec files as the source of truth for agent behavior:

```
project/
├── requirements.md   # User journeys, problem statements, success criteria
├── design.md         # Architecture, tech stack, constraints
└── tasks.md          # Atomic, independently testable work items
```

---

## Spec Structure

### Progressive Disclosure (3-Level Architecture)

1. **Level 1 - Metadata**: Name + description loaded at startup (~50-100 tokens)
2. **Level 2 - Core instructions**: Full spec content loaded when relevant
3. **Level 3 - Bundled resources**: Additional files accessed only when needed

This pattern minimizes baseline context consumption while enabling deep capability.

### CLAUDE.md / AGENTS.md Best Practices

| Guideline | Rationale |
|-----------|-----------|
| **< 300 lines** (ideally < 60) | Frontier models follow ~150-200 instructions reliably; each added instruction degrades all instructions |
| **Universally applicable only** | Task-specific content gets ignored; system prompt warns content "may or may not be relevant" |
| **Pointers over copies** | Use `file:line` references instead of embedded snippets that become outdated |
| **No style guidelines** | Use linters/formatters—LLMs are expensive and slow compared to deterministic tools |
| **Commands, not prose** | Concrete build/test/lint commands beat abstract descriptions |

### Spec File Content Structure

**What to include:**
- Build/test/lint commands with descriptions
- Directory structure and key file locations
- Workflow patterns (branching, commit conventions)
- Project-specific warnings or edge cases

**What to exclude:**
- Database schema design instructions
- Task-specific configurations
- Code style/formatting rules (use linters)
- Exhaustive API documentation (use progressive disclosure)

---

## Correctness & Validation

### Verification Approaches (2025-2026)

| Approach | Description | Use Case |
|----------|-------------|----------|
| **Execution-based evaluation** | Run tool calls and assess outcomes | Production validation |
| **AST correctness** | Check syntactic validity of generated code | Fast feedback loops |
| **Formal verification** | Use proof checkers that reject invalid proofs | Safety-critical systems |
| **Temporal logic monitoring** | Monitor execution traces against behavioral specs | Agent debugging |

### Key Research Findings

**VeriGuard (October 2025):** Dual-stage framework providing formal safety guarantees through behavioral policy synthesis and verification.

**Temporal Logic for Agents (August 2025):** Monitors tool calls and state transitions using hardware verification techniques.

**Critical insight:** "LLMs can translate syntax, but they do not preserve semantics. Treating surface similarity as success risks brittle systems that fail when correctness truly matters."

### Practical Validation Workflow

1. **Specify** - Define success criteria upfront in spec files
2. **Generate** - Agent produces implementation
3. **Validate** - Run deterministic tests (not LLM judgment)
4. **Iterate** - Feedback loop with spec refinement

---

## Token Efficiency

### Context Rot (Chroma Research, 2025)

**Key finding:** LLM performance degrades non-uniformly as input length increases—even on simple tasks.

| Observation | Implication |
|-------------|-------------|
| Models break earlier than advertised (200k → ~130k reliable) | Budget 65% of claimed context |
| Shuffled text outperforms structured text | Coherent flow can disrupt attention |
| Position accuracy drops with length | Place critical info early |
| Performance gap: 300 tokens vs 113k tokens is substantial | Prefer focused context |

### Token Optimization Strategies

**1. Minimal high-signal context**
> "Find the smallest set of high-signal tokens that maximize the likelihood of your desired outcome." — Anthropic

**2. Just-in-time retrieval**
- Maintain lightweight identifiers (paths, queries) not full content
- Enable agents to dynamically load data via tools
- Use metadata (folder names, timestamps) as behavioral signals

**3. Context compaction for long tasks**
- Summarize conversations nearing limits
- Preserve: architectural decisions, unresolved issues, implementation details
- Discard: redundant tool outputs, exploration dead-ends

**4. Sub-agent architectures**
- Delegate focused tasks to specialized sub-agents with clean context
- Return condensed summaries (1,000-2,000 tokens) not full exploration

**5. Caching**
- Claude's 90% cached input discount for repetitive workflows
- Cache system prompts, common context, frequently-used instructions

### Data Serialization

| Format | Token Efficiency |
|--------|------------------|
| CSV | 40-50% more efficient than JSON for tabular data |
| Markdown tables | Good balance of readability and efficiency |
| JSON | Verbose; use only when structure required |

### Batch Processing

**BatchPrompt technique:** Process multiple items in single prompt rather than individually.
- Batch Permutation and Ensembling (BPE): Multiple permutations with majority voting
- Self-reflection-guided Early Stopping (SEAS): Stop voting early for confident responses

---

## Anti-Patterns

### Spec Authoring

| Anti-Pattern | Problem | Fix |
|--------------|---------|-----|
| Auto-generated specs (`/init`) | Generic, not tuned to project | Craft deliberately |
| "Hotfix" accumulation | Dilutes effectiveness as instruction count grows | Prune aggressively |
| Exhaustive examples | Token bloat, diminishing returns | Curate diverse canonical examples |
| Implementation details in specs | Constrains agent flexibility | Focus on "what" and "why" |

### Context Management

| Anti-Pattern | Problem | Fix |
|--------------|---------|-----|
| Full documents when summaries suffice | 10x+ unnecessary API costs | Pre-process, use RAG |
| Keeping full conversation history | Context rot degrades performance | Clear frequently, checkpoint |
| Loading all files upfront | Wastes tokens on irrelevant content | Progressive disclosure |
| Ignoring token costs | Jevons paradox—cheaper tokens → more usage | Budget and monitor |

### Validation

| Anti-Pattern | Problem | Fix |
|--------------|---------|-----|
| LLM-as-judge for correctness | Non-deterministic, expensive | Deterministic tests |
| Surface similarity checks | Misses semantic errors | Execution-based validation |
| Skipping human review | Agents miss edge cases | Maintain approval gates |

---

## Emerging Patterns (January 2026)

### Skill-Based Architecture

Skills are becoming "potentially a bigger deal than MCP" for encoding institutional knowledge:

- Version-controlled, shareable modules
- Progressive disclosure built-in
- Cross-platform portability (Claude, OpenAI, Cursor)

### Spec-Driven Development Adoption

> "Spec-driven development is not a choice but a necessity as we move from vibe coding a cool app to building real-world brownfield projects."

**Best for:** Greenfield projects, regulated environments, team coordination

**Limitations:** Large existing codebases, rapidly evolving requirements

### RAG Evolution

Classical RAG fading as default for document queries. Better long-context handling and improved small models enable direct context inclusion where previously retrieval was required.

---

## Sources

### Standards & Specifications
- [Anthropic - Agent Skills](https://www.anthropic.com/engineering/equipping-agents-for-the-real-world-with-agent-skills)
- [Anthropic - Claude Code Best Practices](https://www.anthropic.com/engineering/claude-code-best-practices)
- [Anthropic - Effective Context Engineering](https://www.anthropic.com/engineering/effective-context-engineering-for-ai-agents)
- [GitHub - Spec-Driven Development Toolkit](https://github.blog/ai-and-ml/generative-ai/spec-driven-development-with-ai-get-started-with-a-new-open-source-toolkit/)

### Research
- [Chroma Research - Context Rot](https://research.trychroma.com/context-rot)
- [VeriGuard - Verified Code Generation](https://liner.com/review/veriguard-enhancing-llm-agent-safety-via-verified-code-generation)
- [Temporal Logic for Agent Correctness](https://arxiv.org/abs/2509.20364)
- [Simon Willison - 2025: The Year in LLMs](https://simonwillison.net/2025/Dec/31/the-year-in-llms/)

### Industry Analysis
- [Thoughtworks - Spec-Driven Development](https://www.thoughtworks.com/en-us/insights/blog/agile-engineering-practices/spec-driven-development-unpacking-2025-new-engineering-practices)
- [RedMonk - 10 Things Developers Want from Agentic IDEs](https://redmonk.com/kholterhoff/2025/12/22/10-things-developers-want-from-their-agentic-ides-in-2025/)
- [The New Stack - Agent Skills Standards](https://thenewstack.io/agent-skills-anthropics-next-bid-to-define-ai-standards/)
- [HumanLayer - Writing a Good CLAUDE.md](https://www.humanlayer.dev/blog/writing-a-good-claude-md)

### Token Efficiency
- [Portkey - Optimize Token Efficiency](https://portkey.ai/blog/optimize-token-efficiency-in-prompts/)
- [K2View - MCP Strategies for Token-Efficient Context](https://www.k2view.com/blog/mcp-strategies-for-grounded-prompts-and-token-efficient-llm-context/)
