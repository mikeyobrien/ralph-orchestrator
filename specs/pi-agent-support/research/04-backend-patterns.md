# Research: Existing Backend Patterns in Ralph

## Summary

All non-Claude backends use `OutputFormat::Text` with raw output passthrough. Pi would be the second backend to get structured NDJSON streaming, giving it feature parity with Claude for TUI display, cost tracking, and tool call visibility.

## Backend Comparison

| Backend | Command | Headless Flag | Permission Flag | Output Format | Interactive Mode |
|---------|---------|--------------|-----------------|---------------|-----------------|
| Claude | `claude` | `-p` | `--dangerously-skip-permissions` | StreamJson | positional arg (no `-p`) |
| Kiro | `kiro-cli` | `--no-interactive` | `--trust-all-tools` | Text | remove `--no-interactive` |
| Gemini | `gemini` | `-p` | `--yolo` | Text | `-i` instead of `-p` |
| Codex | `codex` | `exec` subcommand | `--yolo` | Text | no `exec` |
| Amp | `amp` | `-x` flag | `--dangerously-allow-all` | Text | remove `--dangerously-allow-all` |
| Copilot | `copilot` | `-p` | `--allow-all-tools` | Text | remove `--allow-all-tools` |
| OpenCode | `opencode` | `run` subcommand | (none) | Text | `--prompt` flag |
| **Pi** | `pi` | `-p` | (none needed) | **PiStreamJson** | positional arg (no `-p`) |

## Pattern: Adding a New Backend

From studying the codebase, adding a backend requires:

1. **`cli_backend.rs`**: Add `pi()` and `pi_interactive()` constructor methods
2. **`cli_backend.rs`**: Add `"pi"` to `from_name()`, `from_config()`, `for_interactive_prompt()`
3. **`auto_detect.rs`**: Add `"pi"` to `DEFAULT_PRIORITY` list
4. **`auto_detect.rs`**: No `detection_command()` mapping needed (binary name matches)
5. **Tests**: Add unit tests for the new backend constructors

## Pi-Specific Considerations

### No Permission Flag Needed

Pi doesn't have a `--dangerously-skip-permissions` equivalent. It auto-approves all tool calls when run in print mode (`-p`). No additional flags needed.

### Output Format

Pi supports 3 output modes:
- `--mode text` (default): Final response text only, no streaming
- `--mode json`: NDJSON event stream (every delta, tool call, etc.)
- `--mode rpc`: Bidirectional JSON protocol

For Ralph integration: `--mode json` is the right choice for NDJSON streaming.

### Session Management

Pi manages sessions by default. For Ralph (where each iteration is independent):
- Use `--no-session` to disable session persistence
- Pi handles its own context/compaction within a single invocation

### Tool Restrictions

Pi supports `--tools <list>` to restrict available tools. Ralph could use this to disable tools when running pi under specific hats, but the default toolset (read, bash, edit, write) matches what Ralph expects.

### Model/Provider Selection

Pi supports `--provider <name>` and `--model <id>` flags. Ralph's hat system could use these to run different pi configurations per hat:

```yaml
hats:
  planner:
    backend:
      type: pi
      args: ["--provider", "anthropic", "--model", "claude-sonnet-4"]
  builder:
    backend:
      type: pi
      args: ["--provider", "openai-codex", "--model", "gpt-5.2-codex"]
```

### Thinking Level

Pi supports `--thinking <level>` (off, minimal, low, medium, high, xhigh). Could be exposed as a hat-level configuration.

## Pi's Unique Advantages Over Claude CLI

1. **Multi-provider**: Pi can use any provider (Anthropic, OpenAI, Google, etc.) while Claude CLI is Anthropic-only
2. **Extensions**: Pi's extension system adds custom tools, commands, and behaviors
3. **Skills**: Pi's skill system provides domain-specific instructions
4. **Custom tools**: Pi supports user-defined tools beyond the built-in set

These could be relevant for advanced Ralph configurations where different hats use different providers or capabilities.
