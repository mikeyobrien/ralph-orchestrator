# Ralph Orchestrator Agent Skills

This directory is the canonical public skill package for external agent
harnesses that operate Ralph.

It ships three skills:

- `ralph-hats` for creating, inspecting, validating, and improving hat
  collections
- `ralph-loop` for running, monitoring, resuming, merging, and debugging Ralph
  loops
- `ralph-docs` for introspecting and improving Ralph itself via the
  published `llms.txt` doc map — answering "how does Ralph do X?"
  questions and scoping code changes to the ralph-orchestrator repo

These are public agent skills. They are not part of Ralph's internal
`ralph tools skill` registry.

## Install with Claude Code

Add this repository as a marketplace source:

```text
/plugin marketplace add mikeyobrien/ralph-orchestrator
```

Then install the `ralph-orchestrator` plugin from the marketplace browser.

## Install with Vercel `npx skills`

List the skills in this repository:

```bash
npx skills add mikeyobrien/ralph-orchestrator --list
```

Install all skills for Claude Code:

```bash
npx skills add mikeyobrien/ralph-orchestrator \
  --skill ralph-hats \
  --skill ralph-loop \
  --skill ralph-docs \
  -a claude-code \
  -y
```

Install one skill for Codex-style agents:

```bash
npx skills add mikeyobrien/ralph-orchestrator \
  --skill ralph-loop \
  -a codex \
  -y
```

During local development you can also install from the checked-out repo:

```bash
npx skills add . --list
```
