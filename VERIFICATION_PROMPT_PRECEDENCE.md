# Prompt Precedence Verification

**Date:** 2026-01-13  
**Task:** Verify prompt precedence rules match spec  
**Status:** ✅ VERIFIED

## Spec Requirements (specs/event-loop.spec.md)

The spec defines the following precedence (highest to lowest):

1. CLI `-p "text"` (inline prompt text)
2. CLI `-P path` (prompt file path)
3. Config `event_loop.prompt` (inline prompt text)
4. Config `event_loop.prompt_file` (prompt file path)
5. Default `PROMPT.md`

**Additional rules:**
- `-p` and `-P` are mutually exclusive (error if both specified)
- `prompt` and `prompt_file` are mutually exclusive (error if both specified)
- CLI flags override config (more specific wins)

## Implementation Verification

### 1. CLI Argument Parsing (crates/ralph-cli/src/main.rs)

**Lines 138-144:** CLI arguments defined with mutual exclusivity
```rust
#[arg(short = 'p', long = "prompt-text", conflicts_with = "prompt_file")]
prompt_text: Option<String>,

#[arg(short = 'P', long = "prompt-file", conflicts_with = "prompt_text")]
prompt_file: Option<PathBuf>,
```
✅ Mutual exclusivity enforced by clap

**Lines 290-296:** CLI overrides applied after config normalization
```rust
if let Some(text) = args.prompt_text {
    config.event_loop.prompt = Some(text);
    config.event_loop.prompt_file = String::new(); // Clear file path
} else if let Some(path) = args.prompt_file {
    config.event_loop.prompt_file = path.to_string_lossy().to_string();
    config.event_loop.prompt = None; // Clear inline
}
```
✅ CLI flags take precedence over config
✅ Clears the other field to maintain mutual exclusivity

### 2. Config Validation (crates/ralph-core/src/config.rs)

**Lines 309-316:** Config mutual exclusivity check
```rust
if self.event_loop.prompt.is_some() && 
   !self.event_loop.prompt_file.is_empty() && 
   self.event_loop.prompt_file != default_prompt_file() {
    return Err(ConfigError::MutuallyExclusive {
        field1: "event_loop.prompt".to_string(),
        field2: "event_loop.prompt_file".to_string(),
    });
}
```
✅ Errors if both config fields are explicitly set
✅ Allows inline prompt with default `PROMPT.md` value

### 3. Prompt Resolution (crates/ralph-cli/src/main.rs)

**Lines 757-796:** `resolve_prompt_content()` function
```rust
// 1. Check for inline prompt first (CLI -p or config prompt)
if let Some(ref inline_text) = event_loop_config.prompt {
    return Ok(inline_text.clone());
}

// 2. Check for prompt file (CLI -P or config prompt_file or default)
let prompt_file = &event_loop_config.prompt_file;
if !prompt_file.is_empty() {
    // Read file...
}

// 3. No valid prompt source found
anyhow::bail!("No prompt specified...")
```
✅ Correct precedence: inline first, then file, then error
✅ Note: CLI overrides already applied to config at this point

## Test Coverage

**Lines 1259-1292:** Tests verify mutual exclusivity and precedence
- `test_prompt_and_prompt_file_mutually_exclusive` ✅
- `test_prompt_with_default_prompt_file_allowed` ✅
- `test_prompt_file_with_no_inline_allowed` ✅

All tests passing.

## Conclusion

The implementation **MATCHES** the spec requirements:

1. ✅ CLI `-p` takes highest precedence
2. ✅ CLI `-P` takes second precedence
3. ✅ Config `prompt` takes third precedence
4. ✅ Config `prompt_file` takes fourth precedence
5. ✅ Default `PROMPT.md` is fallback
6. ✅ Mutual exclusivity enforced at both CLI and config levels
7. ✅ CLI flags override config values
8. ✅ Helpful error messages when no prompt found

**No gaps found between spec and implementation.**
