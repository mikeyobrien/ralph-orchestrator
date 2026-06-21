---
name: ralph-docs
description: Introspect, explain, and improve Ralph Orchestrator using its published llms.txt doc map. Use this skill whenever the user asks questions about Ralph's behavior, wants to understand how a Ralph internal works (event loop, hats, memories, tasks, backends, presets), debug an unfamiliar failure mode, or propose a code change to the ralph-orchestrator repo. The skill teaches the agent to discover authoritative answers from the live docs via llms.txt before guessing, and to scope improvements through the published architecture rather than the local checkout alone.
---

# Ralph Docs

Introspect Ralph Orchestrator the same way the framework expects a smart agent to
— consult the published documentation map at
<https://mikeyobrien.github.io/ralph-orchestrator/llms.txt>, fetch only the
sections relevant to the question, and answer from authoritative sources.

Use this skill to behave like an internal Ralph contributor rather than a
guess-first assistant.

## Use This Skill For

- Answering "how does Ralph do X?" questions about the event loop, hats,
  memories, tasks, presets, backends, CLI, TUI, diagnostics, waves, or API.
- Explaining an observed behavior ("why did my loop terminate with
  max_iterations?", "why didn't my hat fire?") from first principles in the
  docs, not pattern-matching.
- Proposing and scoping an improvement to the `ralph-orchestrator` codebase —
  locating the right crate, the relevant concept doc, and the existing test
  surface before writing code.
- Triaging a ralph bug report: map symptoms to likely subsystem, pull in the
  concept + reference docs for that subsystem, identify the probable file path
  in the repo.
- Onboarding: answer a new user's setup/quick-start questions from the official
  Getting Started pages instead of the agent's stale training data.

## Core Principle: llms.txt Is The Router

Ralph's llms.txt is a curated map, not a full-text dump. The workflow is:

1. **Discover** — fetch `llms.txt` to see the top-level sections
   (Getting Started / Concepts / User Guide / Advanced / API / Examples /
   Contributing / Reference).
2. **Narrow** — pick the 1–3 pages that actually answer the question. The map
   links directly to `.md` versions — those are agent-optimized and should be
   preferred over scraping HTML.
3. **Fetch** — pull only those pages. Do not fetch more than three pages
   speculatively; the budget should be spent on answering, not browsing.
4. **Cross-check** — for code-level claims, confirm against the repo (crate
   paths are listed in AGENTS.md / CLAUDE.md inside the ralph-orchestrator
   checkout).
5. **Answer** — cite the page you relied on. Include the URL so the user can
   verify.

## Workflow

1. Identify the question's subsystem using the taxonomy in
   `references/llms-txt-map.md` (hats, event loop, memories, tasks, backends,
   presets, CLI, TUI, diagnostics, waves, API).
2. If `~/.cache/ralph-docs/llms.txt` exists and is <7 days old, use it. Else
   refetch it:

   ```bash
   mkdir -p ~/.cache/ralph-docs
   curl -sSfL https://mikeyobrien.github.io/ralph-orchestrator/llms.txt \
     -o ~/.cache/ralph-docs/llms.txt
   ```

3. Pick the 1–3 linked `.md` pages most relevant to the subsystem. The map
   entries are documented in `references/llms-txt-map.md`; use it to shortcut
   the grep.
4. Fetch just those pages via `curl -sSfL <url> -o ~/.cache/ralph-docs/<stem>.md`
   and read them. Agents with `web_fetch` or an equivalent tool should use that
   instead.
5. Answer the user's question grounded in what you just read. Quote the
   relevant sentence when the user asks "does Ralph do X?" so they can audit.
6. If the answer requires a code change, switch to the ralph-orchestrator
   checkout and follow `references/contributing.md` for the propose-a-change
   workflow.

## Scope Boundaries

- This skill answers and explains. For creating/modifying user hats, defer to
  **ralph-hats**. For operating a live loop (running, resuming, merging,
  debugging), defer to **ralph-loop**. For code changes to
  ralph-orchestrator itself, this skill scopes the change; the actual editing
  uses the agent's native code-editing tools.
- Do not invent features not present in the published docs. If llms.txt does
  not surface an answer, say so and suggest checking the source tree at
  <https://github.com/mikeyobrien/ralph-orchestrator>.
- Do not rely on the agent's pretraining for version-sensitive claims (e.g.
  CLI flags, preset names). Ralph's CLI evolves; always verify against
  `guide/cli-reference.md` or `reference/changelog.md`.

## Guardrails

- Prefer `.md` URLs from llms.txt over scraping the rendered HTML.
- Cache fetched pages under `~/.cache/ralph-docs/` with a 7-day staleness
  threshold. Refetch llms.txt before any other doc to detect renames/moves.
- When cited docs contradict the local checkout (docs newer than the user's
  installed ralph version), note the mismatch and suggest `ralph --version` so
  the user can decide which to trust.
- For "how to improve Ralph" requests, always read
  `concepts/tenets/index.md` first. Ralph's six tenets are load-bearing;
  changes that fight them usually belong somewhere else.

## Output Expectations

- Answer first, then link. Don't make the user wait for a verbose tour.
- Include at least one source URL for non-trivial claims.
- When proposing a code change, name the crate + file (see
  `references/contributing.md` for the crate map), the concept doc that
  justifies the change, and the test file that should cover it.

## Read These References When Needed

- For the llms.txt section map + which pages answer which question:
  `references/llms-txt-map.md`
- For FAQ recipes (common introspection patterns):
  `references/common-questions.md`
- For how to propose a code change, including crate layout and PR conventions:
  `references/contributing.md`
