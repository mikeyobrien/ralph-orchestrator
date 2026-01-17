---
status: completed
created: 2026-01-17
started: 2026-01-17
completed: 2026-01-17
---
# Task: Add GitHub Copilot CLI Backend Support

## Description
Add support for GitHub Copilot CLI (`copilot`) as a new backend in ralph-orchestrator. This enables users to use Copilot as their AI agent alongside existing backends (Claude, Kiro, Gemini, Codex, Amp).

## Background
GitHub Copilot CLI is a standalone AI-powered terminal tool (separate from `gh copilot` extension). Key characteristics:

- **Command:** `copilot` (standalone binary)
- **Programmatic mode:** `copilot -p "prompt"` for headless execution
- **Automation flag:** `--allow-all-tools` grants blanket tool permissions (similar to Claude's `--dangerously-skip-permissions`)
- **Authentication:** Via `GH_TOKEN` or `GITHUB_TOKEN` environment variables
- **Output format:** Text only (no JSON streaming - unlike Claude)
- **Interactive mode:** Runs without `--allow-all-tools`, requires user approval for tools

The implementation follows the established backend pattern used by Claude, Kiro, Gemini, Codex, and Amp.

## Technical Requirements

1. Add `CliBackend::copilot()` factory method for autonomous/headless mode
2. Add `CliBackend::copilot_tui()` factory method for TUI mode (no `-p` flag)
3. Add `CliBackend::copilot_interactive()` factory method for interactive mode
4. Register "copilot" in all relevant match statements:
   - `from_config()` - config file parsing
   - `from_name()` - named backend lookup
   - `for_interactive_prompt()` - interactive mode factory
5. Update `filter_args_for_interactive()` to remove `--allow-all-tools` in interactive mode
6. Add "copilot" to `DEFAULT_PRIORITY` in auto-detection
7. Update `NoBackendError` display with Copilot install link
8. Add comprehensive unit tests following existing patterns

## Dependencies

- Existing `CliBackend` struct and factory pattern in `crates/ralph-adapters/src/cli_backend.rs`
- Auto-detection module in `crates/ralph-adapters/src/auto_detect.rs`
- No external crate dependencies required

## Implementation Approach

### 1. Factory Methods (`cli_backend.rs`)

```rust
/// Creates the Copilot backend for autonomous mode.
///
/// Uses copilot CLI with --allow-all-tools for automated tool approval.
/// Output is plain text (no JSON streaming available).
pub fn copilot() -> Self {
    Self {
        command: "copilot".to_string(),
        args: vec!["--allow-all-tools".to_string()],
        prompt_mode: PromptMode::Arg,
        prompt_flag: Some("-p".to_string()),
        output_format: OutputFormat::Text,
    }
}

/// Creates the Copilot TUI backend for interactive mode.
///
/// Runs Copilot in full interactive mode (no -p flag), allowing
/// Copilot's native TUI to render. The prompt is passed as a
/// positional argument.
pub fn copilot_tui() -> Self {
    Self {
        command: "copilot".to_string(),
        args: vec![],  // No --allow-all-tools in TUI mode
        prompt_mode: PromptMode::Arg,
        prompt_flag: None,  // Positional argument
        output_format: OutputFormat::Text,
    }
}

/// Copilot in interactive mode (removes --allow-all-tools).
pub fn copilot_interactive() -> Self {
    Self {
        command: "copilot".to_string(),
        args: vec![],
        prompt_mode: PromptMode::Arg,
        prompt_flag: Some("-p".to_string()),
        output_format: OutputFormat::Text,
    }
}
```

### 2. Registration Updates

Add "copilot" case to:
- `from_config()` match statement
- `from_name()` match statement
- `for_interactive_prompt()` match statement
- `filter_args_for_interactive()` to filter `--allow-all-tools`

### 3. Auto-Detection (`auto_detect.rs`)

```rust
pub const DEFAULT_PRIORITY: &[&str] = &["claude", "kiro", "gemini", "codex", "amp", "copilot"];
```

Add to `NoBackendError::fmt()`:
```rust
writeln!(f, "  • Copilot CLI: https://docs.github.com/copilot/using-github-copilot/using-copilot-cli")?;
```

## Acceptance Criteria

1. **Autonomous Mode Configuration**
   - Given a config file with `backend: "copilot"`
   - When Ralph initializes the backend
   - Then it creates a CliBackend with command="copilot", args=["--allow-all-tools"], prompt_flag="-p"

2. **TUI Mode Support**
   - Given `copilot_tui()` is called
   - When building the command
   - Then prompt is positional (no -p flag) and no --allow-all-tools

3. **Interactive Mode Flag Filtering**
   - Given a Copilot backend with `--allow-all-tools`
   - When `build_command()` is called with `interactive=true`
   - Then `--allow-all-tools` is removed from args

4. **Config Parsing**
   - Given various config inputs ("copilot", from_name, from_hat_backend)
   - When creating backends
   - Then correct Copilot backend is returned

5. **Auto-Detection**
   - Given `copilot` command is available in PATH
   - When auto-detection runs
   - Then "copilot" is detected and can be selected

6. **Error Message**
   - Given no backends are available
   - When `NoBackendError` is displayed
   - Then Copilot CLI install link is included

7. **Build and Tests Pass**
   - Given all changes are complete
   - When running `cargo build` and `cargo test`
   - Then both complete successfully with no errors

## Metadata
- **Complexity**: Medium
- **Labels**: Backend, CLI Integration, Copilot, GitHub
- **Required Skills**: Rust, CLI integration patterns, unit testing
