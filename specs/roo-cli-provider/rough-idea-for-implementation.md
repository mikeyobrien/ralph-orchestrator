# Rough Idea: Add roo-cli as a Provider to Ralph Orchestrator

## What
Add `roo` (Roo Code CLI) as a new backend provider in ralph-orchestrator, enabling `ralph run -b roo` and `ralph plan -b roo`.

## Context
- Roo Code CLI (`roo`, v0.1.15+) is an AI coding assistant that supports multiple LLM providers (Anthropic, OpenAI, AWS Bedrock, OpenRouter)
- Roo CLI source is at `/Users/skven/workplace/Roo-Code/apps/cli/`
- Install: `cd /Users/skven/workplace/Roo-Code/src && pnpm run bundle && cd /Users/skven/workplace/Roo-Code && pnpm --filter @roo-code/cli build && npm install -g ./apps/cli`
- Binary: `roo` (installed globally via npm)

## Key Decisions (already researched and validated)
1. **Text mode only** — Use `roo --print --ephemeral "prompt"` (plain text output, like kiro). Stream-JSON deferred to later.
2. **Always use `--prompt-file`** — Roo natively supports `--prompt-file <path>`. All prompts use this (one code path, no size threshold).
3. **`--ephemeral` included** — Fixed in roo v0.1.15+ to work with Bedrock. Clean disk state per iteration.
4. **Tool auto-approval is default** — No `--trust-all-tools` flag needed (unlike kiro).
5. **Interactive mode**: `roo "prompt"` for `ralph plan`.
6. **Auto-detect last** in priority: `["claude", "kiro", ..., "pi", "roo"]`.
7. **Roo always exits code 0** — Even on API errors. Ralph relies on LOOP_COMPLETE/event presence.
8. **Event tags work** — Roo emits `<event topic="build.done">` and `LOOP_COMPLETE` when Ralph-style system instructions are provided.

## Working Test Commands
```bash
# Headless (text mode)
roo --provider bedrock --aws-profile roo-bedrock --aws-region us-east-1 --model anthropic.claude-sonnet-4-6 --max-tokens 64000 --print "Say hello"

# With prompt file
echo "Your prompt" > /tmp/prompt.txt
roo --provider bedrock --aws-profile roo-bedrock --aws-region us-east-1 --model anthropic.claude-sonnet-4-6 --max-tokens 64000 --print --prompt-file /tmp/prompt.txt

# Stream JSON (for future reference)
roo --provider bedrock --aws-profile roo-bedrock --aws-region us-east-1 --model anthropic.claude-sonnet-4-6 --max-tokens 64000 --print --output-format stream-json "Say hello"
```

## Files to Modify
1. `crates/ralph-adapters/src/cli_backend.rs` — Add `roo()`, `roo_interactive()`, registration in `from_config`, `from_name`, `for_interactive_prompt`
2. `crates/ralph-adapters/src/auto_detect.rs` — Add "roo" to `DEFAULT_PRIORITY`, update `NoBackendError`
3. `crates/ralph-adapters/src/lib.rs` — Update doc comment
4. `presets/minimal/roo.yml` — New preset (mirror kiro.yml, change backend to "roo")

## Design & Research (already completed)
Full PDD artifacts at `.agents/planning/2026-03-03-roo-cli-provider/`:
- `design/detailed-design.md` — Complete design document
- `implementation/plan.md` — 7-step implementation plan with checklist
- `research/assumption-validation.md` — All 9 assumptions verified with live tests
- `research/roo-cli-interface.md` — Complete CLI flag documentation
