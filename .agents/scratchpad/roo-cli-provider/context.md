# Context: Add roo-cli as a Provider

## Project Structure
- **ralph-adapters crate**: `crates/ralph-adapters/src/`
- **Key files**: `cli_backend.rs` (backend definitions), `auto_detect.rs` (auto-detection), `lib.rs` (crate docs)
- **Presets**: `presets/minimal/` (per-backend YAML configs)

## Requirements
1. Add `roo()` and `roo_interactive()` backend constructors
2. Register roo in `from_config`, `from_name`, `for_interactive_prompt`, `filter_args_for_interactive`
3. Always use `--prompt-file` for all roo prompts in `build_command()`
4. Add "roo" to `DEFAULT_PRIORITY` (last position) and `NoBackendError` display
5. Create `presets/minimal/roo.yml`
6. Update `lib.rs` doc comment

## Patterns (from existing code)
- Backend constructors return `Self { command, args, prompt_mode, prompt_flag, output_format, env_vars }`
- `from_config()` / `from_name()` / `for_interactive_prompt()` use match arms
- `filter_args_for_interactive()` removes headless-only flags per backend
- `build_command()` handles special prompt passing (Claude temp file for >7000 chars)
- Tests follow `test_<backend>_backend()`, `test_from_name_<backend>()`, etc.

## Dependencies
- `tempfile::NamedTempFile` (already imported in cli_backend.rs)
- `std::io::Write` (already imported)

## Implementation Path
Roo follows kiro/pi pattern: Text output, Arg prompt mode, no prompt flag. Special: always uses `--prompt-file` for prompts.
