# Research: Backend Compatibility for Interactive SOP Sessions

## Summary

Our `plan` and `task` commands need **interactive mode with an initial prompt** - a specific mode that differs from both pure headless (-p) and pure interactive (no prompt) modes.

## Backend Analysis

### Claude CLI
- **Headless**: `claude -p "prompt"` (exits after response)
- **Interactive**: `claude` (no args, REPL mode)
- **Interactive + Initial Prompt**: `claude "initial prompt"` (positional arg, no `-p` flag)

Ralph already has `claude_tui()` which handles this correctly:
```rust
// No -p flag - prompt is positional
prompt_flag: None,
```

**Status**: ✅ Works - use `claude_tui()` pattern

Sources:
- [Claude CLI Reference](https://code.claude.com/docs/en/cli-reference)
- [Claude Code Best Practices](https://www.anthropic.com/engineering/claude-code-best-practices)

---

### Kiro CLI
- **Headless**: `kiro-cli chat --no-interactive "prompt"`
- **Interactive**: `kiro-cli` or `kiro-cli chat`
- **Interactive + Initial Prompt**: `kiro-cli chat "prompt"` (no `--no-interactive` flag)

Ralph's `filter_args_for_interactive()` already removes `--no-interactive` for interactive mode.

**Status**: ✅ Works - remove `--no-interactive`, keep prompt as positional

Sources:
- [Kiro CLI Docs](https://kiro.dev/cli/)
- [Kiro CLI Commands](https://kiro.dev/docs/cli/reference/cli-commands/)

---

### Gemini CLI
- **Headless**: `gemini -p "prompt"` (non-interactive)
- **Interactive**: `gemini` (REPL mode)
- **Interactive + Initial Prompt**: `gemini -i "prompt"` or `gemini --prompt-interactive "prompt"`

⚠️ **IMPORTANT DISCOVERY**: Gemini has a dedicated `-i`/`--prompt-interactive` flag that:
> "starts an interactive session with the provided prompt as the initial input. The prompt is processed within the interactive session, not before it."

This is different from `-p` which runs headless!

**Current Ralph Implementation**:
```rust
// Uses -p which is HEADLESS
prompt_flag: Some("-p".to_string()),
```

**Required Change**: Need a `gemini_interactive()` variant that uses `-i` instead of `-p`.

**Status**: ⚠️ Needs new method - use `-i` flag for interactive+prompt

Sources:
- [Gemini CLI Headless Mode](https://google-gemini.github.io/gemini-cli/docs/cli/headless.html)
- [Gemini CLI Parameters](https://medium.com/google-cloud/gemini-cli-tutorial-series-part-2-gemini-cli-command-line-parameters-e64e21b157be)
- [Gemini CLI Cheatsheet](https://www.philschmid.de/gemini-cli-cheatsheet)

---

### Codex CLI (OpenAI)
- **Headless**: `codex exec --full-auto "prompt"`
- **Interactive**: `codex` (launches TUI)
- **Interactive + Initial Prompt**: `codex "prompt"` (no `exec` subcommand, no `--full-auto`)

Ralph's `filter_args_for_interactive()` removes `--full-auto`, but also needs to remove `exec` subcommand.

**Current Ralph Implementation**:
```rust
args: vec!["exec".to_string(), "--full-auto".to_string()],
```

**Required Change**: For interactive mode, should be just `codex "prompt"` (no `exec`).

**Status**: ⚠️ Needs adjustment - remove `exec` subcommand for interactive

Sources:
- [Codex CLI Reference](https://developers.openai.com/codex/cli/reference/)
- [Codex CLI Features](https://developers.openai.com/codex/cli/features/)

---

### AMP / Amazon Q CLI
- **Note**: Amazon Q CLI has been rebranded/merged with Kiro CLI
- **Headless**: `amp -x "prompt" --dangerously-allow-all`
- **Interactive**: `amp` (REPL mode)
- **Interactive + Initial Prompt**: `amp -x "prompt"` (remove `--dangerously-allow-all`)

Ralph's `filter_args_for_interactive()` already removes `--dangerously-allow-all`.

**Status**: ✅ Works - existing filter handles it

Sources:
- [Amazon Q CLI Reference](https://docs.aws.amazon.com/amazonq/latest/qdeveloper-ug/command-line-reference.html)

---

## Implementation Requirements

### New Backend Method Needed

Create an "interactive with initial prompt" variant for backends that need different flags:

```rust
impl CliBackend {
    /// Creates backend configured for interactive mode with initial prompt.
    /// This is different from headless mode (-p) and pure interactive mode (no prompt).
    pub fn for_interactive_prompt(backend_name: &str) -> Result<Self, CustomBackendError> {
        match backend_name {
            "claude" => Ok(Self::claude_tui()),  // Already exists
            "kiro" => Ok(Self::kiro_interactive()),
            "gemini" => Ok(Self::gemini_interactive()), // NEW - uses -i
            "codex" => Ok(Self::codex_interactive()),   // NEW - no exec
            "amp" => Ok(Self::amp_interactive()),
            _ => Err(CustomBackendError),
        }
    }

    /// Gemini in interactive mode with initial prompt
    pub fn gemini_interactive() -> Self {
        Self {
            command: "gemini".to_string(),
            args: vec!["--yolo".to_string()],
            prompt_mode: PromptMode::Arg,
            prompt_flag: Some("-i".to_string()),  // NOT -p!
            output_format: OutputFormat::Text,
        }
    }

    /// Codex in interactive TUI mode with initial prompt
    pub fn codex_interactive() -> Self {
        Self {
            command: "codex".to_string(),
            args: vec![],  // No "exec" subcommand
            prompt_mode: PromptMode::Arg,
            prompt_flag: None,
            output_format: OutputFormat::Text,
        }
    }
}
```

### Compatibility Matrix

| Backend | Headless Flag | Interactive+Prompt Flag | Needs New Method |
|---------|---------------|------------------------|------------------|
| Claude  | `-p`          | (positional)           | No (`claude_tui`) |
| Kiro    | `--no-interactive` | (remove flag)     | No (filter works) |
| Gemini  | `-p`          | `-i`                   | **Yes** |
| Codex   | `exec --full-auto` | (no exec)         | **Yes** |
| AMP     | `--dangerously-allow-all` | (remove flag) | No (filter works) |

## Conclusion

Our approach is viable across all backends, but requires:

1. **New `gemini_interactive()` method** using `-i` instead of `-p`
2. **New `codex_interactive()` method** without the `exec` subcommand
3. A factory method `for_interactive_prompt()` to select the right variant

The XML prompt format (`<sop>...</sop><user-content>...</user-content>`) will work with all backends since they all pass prompts as text.
