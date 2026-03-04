# Idea Honing: roo-cli Provider

Requirements clarification through Q&A.

---

## Q1: Output Format — Text-only or Stream-JSON?

Roo CLI supports two modes Ralph could integrate with:

1. **Text mode** (`roo --print "prompt"`) — Simple text output, like kiro. Easiest to implement but no real-time streaming, cost tracking, or tool-use visibility.
2. **Stream-JSON mode** (`roo --print --output-format stream-json "prompt"`) — Rich NDJSON streaming with deltas, tool_use events, cost data, and thinking events. Requires a new `RooStreamParser` (similar to how Pi has `PiStreamParser`).

**Which approach should we take?**

**A1**: Text mode only — simple, fast, proven by 6/9 providers. Add stream-json later if needed. The output format will be `OutputFormat::Text`, like kiro/gemini/codex/amp/copilot/opencode.

---

## Q2: Headless Command — What flags should the roo headless backend use?

Based on research, `roo --print "prompt"` runs headless and exits. But we need to decide on additional flags:

- `--print` — Required for non-interactive exit
- `--ephemeral` — Run without persisting state? (Prevents roo from accumulating task history across Ralph iterations)
- `--require-approval` — Should this default to false (auto-approve tools)?

The default model is `anthropic/claude-opus-4.6` but the user's working config uses `--provider bedrock --aws-profile roo-bedrock --aws-region us-east-1 --model anthropic.claude-sonnet-4-6 --max-tokens 64000`. Should model/provider be configured via ralph.yml `cli.args` or baked into the backend defaults?

**A2**: Use `--ephemeral` as default. Headless command: `roo --print --ephemeral "prompt"`.
- Tool auto-approval is already the default in roo (`--require-approval` defaults to false), so no additional flag needed (unlike kiro's `--trust-all-tools`)
- Model/provider configured via `cli.args` in ralph.yml — not baked into the backend
- Extra args from ralph.yml are always appended (existing pattern in `from_config()`)

---

## Q3: Interactive Mode — What should `roo_interactive()` look like?

For `ralph plan` and PTY interactive mode, we need a variant that removes headless flags and lets the user interact with roo's TUI. Following the pattern of other backends:

- Kiro interactive: removes `--no-interactive`
- Claude interactive: removes `-p` flag
- For roo: remove `--print` and `--ephemeral`

Proposed interactive command: `roo "prompt"` (just positional arg, no flags)

**A3**: Both modes needed.
- **Headless** (`ralph run`): `roo --print --ephemeral "prompt"`
- **Interactive** (`ralph plan`): `roo "prompt"` (no `--print`, no `--ephemeral`)

---

## Q4: Auto-Detection Priority — Where should "roo" be in the detection order?

The current `DEFAULT_PRIORITY` is: `["claude", "kiro", "kiro-acp", "gemini", "codex", "amp", "copilot", "opencode", "pi"]`

Where should "roo" go? The detection command would check `roo --version`.

**A4**: At the end of the priority list. New order: `["claude", "kiro", "kiro-acp", "gemini", "codex", "amp", "copilot", "opencode", "pi", "roo"]`. Only auto-detected as fallback if nothing else is available. Detection command: `roo --version`.

---

## Q5: Preset Configuration — What should the minimal roo preset look like?

Should the `presets/minimal/roo.yml` just mirror the kiro preset pattern (same hats, guardrails) but with `cli.backend: "roo"`? Or should it include roo-specific defaults like `--provider` and `--model` flags?

**A5**: Mirror kiro preset pattern — same hats (builder), same guardrails, just `cli.backend: "roo"` with empty `cli.args: []`. No model/provider baked in. User configures model/provider via their `ralph.yml` `cli.args` or CLI `-- ...` flags.

---

## Q6: Scope Confirmation — Is this everything?

Based on our discussion, the full scope is:
1. **Backend definitions**: `roo()` headless and `roo_interactive()` in `cli_backend.rs`
2. **Registration points**: `from_config()`, `from_name()`, `for_interactive_prompt()`, `from_hat_backend()`
3. **Auto-detection**: Add "roo" at end of `DEFAULT_PRIORITY`, `detection_command("roo") → "roo"`
4. **Error display**: Add roo install URL in `NoBackendError`
5. **Preset file**: `presets/minimal/roo.yml`
6. **Tests**: Unit tests for all new backend functions
7. **Documentation**: Update `lib.rs` doc comment to include Roo

Anything else?

---

## Q7: Large Prompts — How should roo handle prompts >7000 chars?

Claude has a special case: when prompts exceed 7000 characters, Ralph writes the prompt to a temp file and tells Claude to read it. This is because long CLI arguments can fail on some systems.

Roo has `--prompt-file <path>` which reads a prompt from a file — verified working.

**Historical note**: Initially `--ephemeral` broke Bedrock auth, but this was fixed in roo v0.1.15+. `--ephemeral` is now included in the default backend.

**A7**: Always use `--prompt-file` for all prompts. One code path, no size threshold — write prompt to temp file, pass `--prompt-file <path>`. Roo handles it natively. Simpler than conditional logic, verified working for both small and large prompts.

---

## Q8: PTY Mode — Should roo support PTY mode (`pty_mode: true`)?

Ralph's PTY mode runs the CLI tool in a pseudo-terminal, preserving rich TUI features (colors, spinners). This is used for:
- `ralph run` with TUI display (ratatui-based)
- Interactive sessions

Kiro uses `pty_mode: false` by default. Claude uses PTY for its TUI. Should roo default to `pty_mode: false` or `true`?

**A8**: `pty_mode: false` (pipe mode). Roo with `--print` outputs clean text, PTY adds unnecessary ANSI complexity. 6/9 providers use pipe mode. User can enable PTY via config if needed.

---

## Q9: Error Handling — How should roo failures be detected?

With text mode, Ralph detects failures via:
1. **Exit code** — non-zero means failure
2. **LOOP_COMPLETE token** — agent emits this when done
3. **Event tags** — `<event topic="build.done">` etc.

Should we add any roo-specific error detection? For example, roo outputs `[task complete]` when done and `Error:` lines when failing. Should Ralph parse these?

**A9**: No roo-specific error handling. Use standard Ralph mechanisms (exit code + event tags + LOOP_COMPLETE). Keep it simple.

---

## Q10: Roo Modes — Should the backend support `--mode` flag?

Roo has a `--mode` flag (`code`, `architect`, `ask`, `debug`, etc.). Ralph's hat system conceptually overlaps with roo modes — for example, the "Builder" hat could map to `--mode code`, while a "Planner" hat could use `--mode architect`.

Should we support passing `--mode` through the backend, or leave it for users to configure via `cli.args`?

**A10**: Leave it for users to configure via `cli.args`. Roo defaults to `--mode code` which is the right default for Ralph's builder hat. Users can override in their ralph.yml if needed.

---

## Requirements Clarification Complete

All key decisions have been made. Summary of confirmed requirements:

1. **Output format**: Text mode only (like kiro)
2. **Headless command**: `roo --print --ephemeral "prompt"` (both flags included)
3. **Prompt passing**: Always use `--prompt-file` for all prompts (one code path, simpler)
4. **Interactive mode**: `roo "prompt"` (for `ralph plan`, no `--print`/`--ephemeral`)
5. **Auto-detection**: "roo" at end of priority list
6. **Preset**: Mirror kiro pattern, no model/provider baked in
7. **PTY mode**: `pty_mode: false` (pipe mode)
8. **Error handling**: Standard Ralph mechanisms, no roo-specific parsing. Note: roo always exits 0, even on errors.
9. **Roo modes/provider/model**: Configured via `cli.args`, not baked into backend
10. **Event tags**: Work correctly when Ralph provides event protocol as system instructions
