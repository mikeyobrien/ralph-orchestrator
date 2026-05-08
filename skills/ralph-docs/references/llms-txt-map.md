# Ralph llms.txt Section Map

The authoritative doc index is at
<https://mikeyobrien.github.io/ralph-orchestrator/llms.txt>.

This file is a **routing shortcut** — given a user question or bug report, it
tells you which 1–3 pages to fetch. Always refetch llms.txt itself before any
other page; URLs do get renamed.

## Top-Level Sections

1. **Getting Started** — setup, install, first task. Use for onboarding.
2. **Concepts** — mental model. Use for "why does Ralph work this way?"
3. **User Guide** — practical CLI usage. Use for "how do I…?"
4. **Advanced** — architecture, subsystems. Use for deep questions.
5. **API Reference** — crate-level rustdoc. Use for code changes.
6. **Examples** — working config patterns. Use as templates.
7. **Contributing** — dev setup, PR conventions. Use before shipping changes.
8. **Reference** — changelog, FAQ, glossary, troubleshooting. Use for
   version-specific claims and error triage.

## Question → Page Map

Use this before speculatively fetching. The canonical URL pattern is
`https://mikeyobrien.github.io/ralph-orchestrator/<path>/index.md`.

### Event loop behavior

- "Why did my loop terminate?" → `reference/troubleshooting/index.md`
- "What is the starting event?" → `concepts/hats-and-events/index.md`
- "How does fresh-context-each-iteration work?" →
  `concepts/tenets/index.md`, `concepts/ralph-wiggum-technique/index.md`
- "How do completion events work?" → `advanced/event-system/index.md`

### Hats (user-authored workflows)

- "How do I write a hat?" → `advanced/custom-hats/index.md`
- "What's a trigger vs a publish?" → `concepts/hats-and-events/index.md`
- "Which builtin preset should I use?" → `guide/presets/index.md`
- "Why is my hat not firing?" → `reference/troubleshooting/index.md` +
  `concepts/hats-and-events/index.md`

### Memories + Tasks

- "How do memories persist?" → `advanced/memory-system/index.md`
- "Where are tasks stored?" → `advanced/task-system/index.md`,
  `concepts/memories-and-tasks/index.md`
- "How do I reset them?" — check CLI: `guide/cli-reference/index.md` (`ralph clean`)

### Backends

- "What backends are supported?" → `guide/backends/index.md`
- "How does backend selection work?" → `guide/backends/index.md` +
  `reference/faq/index.md` (auto-detect order)
- "Why does my backend time out?" → `reference/troubleshooting/index.md`
- kiro-acp, claude, gemini, codex, pi, roo, copilot, opencode, amp —
  all covered in `guide/backends/index.md`

### Presets

- "List the presets" → `guide/presets/index.md` (+ `ralph hats list-presets`
  for discoverable ones on the user's system)
- "Authoring YAML presets" → `guide/presets/index.md`
- "Authoring TOML presets" → link out to
  <https://mikeyobrien.github.io/autoloop/guides/creating-presets>
- Preset resolver paths → `guide/presets/index.md` (since PR #316)

### CLI + TUI

- Flag meanings → `guide/cli-reference/index.md`
- Autonomous vs interactive → `guide/cli-reference/index.md`
- RPC mode / subprocess TUI → `api/ralph-cli/index.md`
- Web dashboard → `advanced/diagnostics/index.md`

### Parallel loops + waves

- Worktrees, merge queue → `advanced/parallel-loops/index.md`
- Wave dispatch → `advanced/agent-waves/index.md`

### Code contribution

- Crate layout → AGENTS.md in the repo root (not on the doc site)
- Dev setup → `contributing/setup/index.md`
- Style → `contributing/style/index.md`
- Tests → `contributing/testing/index.md`
- PRs → `contributing/pull-requests/index.md`

## Freshness

llms.txt is regenerated on doc deploys. Cache for at most 7 days. Any doc
page linked from it is the canonical source for its topic — prefer it over
README snippets in the repo, which may lag.

## When llms.txt Doesn't Cover It

- **Crate-internal details** not surfaced in API docs: read the source at
  <https://github.com/mikeyobrien/ralph-orchestrator/tree/main/crates>.
- **Recent changes**: consult `reference/changelog/index.md` and the GitHub
  releases/commit log.
- **Experimental features**: may live in `specs/` in the repo, not on the
  doc site.
