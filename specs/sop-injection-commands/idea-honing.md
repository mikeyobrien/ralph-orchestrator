# Idea Honing: SOP Injection Subcommands

This document captures the requirements clarification process for the plan and task subcommands.

---

## Q1: What does "inject the SOP" mean in terms of execution?

When you say "inject the PDD/code-task-generator SOP into the user's default backend," what behavior do you envision?

Options:
- **A) Start an interactive session** with the SOP as the initial prompt/context
- **B) Run non-interactively** where the SOP is passed and the backend responds once
- **C) Replace Ralph's event loop** - these commands bypass Ralph orchestration entirely and just use the backend directly
- **D) Other**

**Answer:** C - These commands bypass Ralph's event loop entirely and just use the backend directly.

This means `ralph plan` and `ralph task` are thin wrappers that spawn the detected backend (e.g., `claude`) with the SOP content as the prompt. No event bus, no hats, no iteration loop.

---

## Q2: How should the backend be determined?

Since these commands bypass the event loop but still need to know which CLI to invoke, how should the backend be resolved?

Options:
- **A) Auto-detect only** - Use the existing auto-detection logic (claude → kiro → gemini → codex → amp)
- **B) Config-aware** - Read `ralph.yml` if present, fall back to auto-detect
- **C) Explicit flag** - Require `--backend claude` or similar, with optional auto-detect fallback
- **D) Other**

**Answer:** Combination of B and C - Layered resolution:
1. If `--backend <name>` flag is provided, use that explicitly
2. Otherwise, read `ralph.yml` if present and use `cli.backend`
3. Fall back to auto-detect if no config or backend is "auto"

---

## Q3: How should the SOP content be passed to the backend?

The SOPs (PDD and code-task-generator) are markdown files in `.claude/skills/`. How should this content reach the backend?

Options:
- **A) Inline prompt** - Read the SOP file and pass it via `-p "content..."` (or equivalent prompt flag)
- **B) File reference** - Pass a path to the SOP file if the backend supports it
- **C) Stdin** - Pipe the SOP content to the backend's stdin
- **D) Other**

**Answer:** A - Inline prompt. Read the SOP markdown file and pass it via the backend's prompt flag (e.g., `-p` for Claude). This is the most portable approach across different backends.

---

## Q4: What execution mode should these commands use?

The SOPs are interactive workflows that require back-and-forth with the user (PDD asks clarifying questions, code-task-generator may need input). How should the backend be invoked?

Options:
- **A) Always interactive** - Spawn the backend in interactive/conversational mode
- **B) Respect config** - Use whatever `cli.default_mode` is set to in `ralph.yml`
- **C) Flag-controlled** - Default to interactive, but allow `--non-interactive` override
- **D) Other**

**Answer:** A - Always interactive. These SOPs are inherently conversational and require user input throughout the process.

---

## Q5: Where should the SOP files be sourced from?

The PDD and code-task-generator SOPs currently live in `.claude/skills/` within the ralph-orchestrator repo. For users who install ralph via cargo/homebrew, where should these files come from?

Options:
- **A) Bundled with binary** - Embed the SOP content at compile time using `include_str!`
- **B) Local `.claude/skills/`** - Look for SOPs in the user's current working directory
- **C) Shipped as data files** - Install SOPs alongside the binary (e.g., in `~/.ralph/skills/`)
- **D) Fetch from repo** - Download from GitHub if not found locally

**Answer:** A - Bundled with binary using `include_str!`. This is consistent with how ralph bundles preset collections and makes the binary self-contained. SOP updates require a new release.

---

## Q6: How should users provide input to the SOPs?

The PDD SOP needs a `rough_idea` (and optionally `project_dir`), while code-task-generator needs an input description or PDD plan path. How should users provide these?

Options:
- **A) Positional args** - `ralph plan "my rough idea"` / `ralph task path/to/plan.md`
- **B) Flags only** - `ralph plan --idea "my rough idea"` / `ralph task --input path/to/plan.md`
- **C) Let SOP ask** - Just start the session and let the SOP prompt for required inputs interactively
- **D) Combination** - Support positional args but SOP will ask if not provided

**Answer:** C and D combined - Support optional positional args for power users who want quick starts, but if not provided, the SOP will prompt interactively. This gives flexibility for both scripting and conversational workflows.

Examples:
- `ralph plan` → SOP asks for rough idea and project dir
- `ralph plan "build a REST API"` → SOP receives idea, asks for project dir
- `ralph task` → SOP asks for input
- `ralph task specs/my-feature/implementation/plan.md` → SOP uses provided path

---

## Q7: Should these be top-level commands or nested under a parent?

Options:
- **A) Top-level** - `ralph plan`, `ralph task` (alongside `ralph run`, `ralph events`, etc.)
- **B) Nested under `sop`** - `ralph sop plan`, `ralph sop task`
- **C) Nested under `new`** - `ralph new plan`, `ralph new task` (framing as "create new...")
- **D) Other**

**Answer:** A - Top-level commands. `ralph plan` and `ralph task` sit alongside existing commands like `ralph run`, `ralph events`, etc. Simple and discoverable.

---

## Q8: How should user-provided input be combined with the SOP in the prompt?

When a user runs `ralph plan "build a REST API"`, we need to send both the SOP content and the user's input to the backend. How should these be combined?

Options:
- **A) Append user input** - SOP content first, then "User's rough idea: {input}" appended
- **B) Structured template** - Use a template like: "Follow this SOP:\n{sop}\n\nUser input:\n{input}"
- **C) SOP placeholder** - The SOP contains a `{user_input}` placeholder that gets replaced
- **D) Separate messages** - Pass SOP as system prompt, user input as first user message (if backend supports)

**Answer:** Custom XML-style structure:
```
<sop>
{sop_content}
</sop>
<user-content>
{user_input}
</user-content>
```

This is clean, parseable, and clearly delineates the SOP instructions from the user's input. If no user input is provided, the `<user-content>` section is omitted.

---

## Q9: How should the command handle errors (e.g., no backend found)?

Options:
- **A) Fail fast with clear message** - Exit with error code and descriptive message
- **B) Prompt for backend** - Ask user to specify backend interactively
- **C) Suggest installation** - Detect what's missing and suggest how to install it
- **D) Combination of A and C**

**Answer:** D - Fail fast with clear message AND suggest installation. Example:
```
Error: No supported backend found.

Checked: claude, kiro, gemini, codex, amp

To install Claude CLI:
  npm install -g @anthropic-ai/claude-code

Or specify a backend explicitly:
  ralph plan --backend claude
```

---

## Requirements Complete

All core requirements have been clarified. Ready to proceed to detailed design.

