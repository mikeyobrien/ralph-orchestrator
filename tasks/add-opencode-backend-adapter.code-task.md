---
status: completed
created: 2026-01-17
started: 2026-01-17
completed: 2026-01-17
---
# Task: Add OpenCode CLI Backend Adapter

## Description
Add OpenCode as a supported CLI backend adapter in Ralph, enabling orchestration of OpenCode sessions alongside existing backends (Claude, Kiro, Gemini, Codex, Amp, Copilot). This follows the established adapter pattern and mirrors the recent GitHub Copilot CLI addition.

## Background
Ralph's adapter architecture uses a factory pattern where each backend is a `CliBackend` struct configuration rather than a trait implementation. Adding a new backend requires factory methods for three modes (headless, interactive, TUI), updates to match statements in existing functions, and registration in the auto-detection system.

OpenCode is an open-source AI coding agent (https://opencode.ai) built in Go with a Bubble Tea TUI. It supports multiple LLM providers and has CLI flags similar to other coding agents:
- `-p` flag for passing prompts
- `--dangerously-skip-permissions` for autonomous/headless operation
- Plain text output (no JSON streaming)

## Reference Documentation
**Required:**
- Existing adapter implementations in `crates/ralph-adapters/src/cli_backend.rs`
- Recent Copilot adapter addition (commit `6c551840`) as the primary template

**Additional References:**
- OpenCode CLI documentation: https://opencode.ai/docs/cli/
- Auto-detection logic in `crates/ralph-adapters/src/auto_detect.rs`

## Technical Requirements
1. Add `pub fn opencode() -> Self` factory method with `--dangerously-skip-permissions` flag for autonomous mode
2. Add `pub fn opencode_interactive() -> Self` factory method without auto-approve flag
3. Add `pub fn opencode_tui() -> Self` factory method with positional prompt argument (no `-p` flag)
4. Update `from_config()` match statement to handle `"opencode"` backend name
5. Update `from_name()` match statement to handle `"opencode"` backend name
6. Update `for_interactive_prompt()` match statement to return `opencode_interactive()`
7. Add `"opencode"` to `DEFAULT_PRIORITY` array in `auto_detect.rs`
8. Add unit tests for all three factory methods following existing test patterns

## Dependencies
- `crates/ralph-adapters/src/cli_backend.rs` — Main file for factory methods and match statements
- `crates/ralph-adapters/src/auto_detect.rs` — Auto-detection priority list
- Existing Copilot adapter implementation as template

## Implementation Approach
1. Read the existing Copilot adapter implementation to understand the exact pattern
2. Add the three factory methods (`opencode`, `opencode_interactive`, `opencode_tui`) in `cli_backend.rs`
3. Update all relevant match statements in `cli_backend.rs`:
   - `from_config()`
   - `from_name()`
   - `for_interactive_prompt()`
4. Add `"opencode"` to `DEFAULT_PRIORITY` in `auto_detect.rs`
5. Add unit tests mirroring the Copilot test structure
6. Run `cargo test` to verify all tests pass
7. Run `cargo clippy` to ensure no linting issues

## Acceptance Criteria

1. **Factory Method - Headless Mode**
   - Given the `CliBackend::opencode()` factory method is called
   - When the backend configuration is inspected
   - Then command is `"opencode"`, args include `"--dangerously-skip-permissions"`, prompt_mode is `Arg`, prompt_flag is `Some("-p")`, output_format is `Text`

2. **Factory Method - Interactive Mode**
   - Given the `CliBackend::opencode_interactive()` factory method is called
   - When the backend configuration is inspected
   - Then command is `"opencode"`, args is empty (no auto-approve flag), prompt_mode is `Arg`, prompt_flag is `Some("-p")`

3. **Factory Method - TUI Mode**
   - Given the `CliBackend::opencode_tui()` factory method is called
   - When the backend configuration is inspected
   - Then command is `"opencode"`, args is empty, prompt_flag is `None` (positional argument)

4. **Configuration Loading**
   - Given a config file with `backend: opencode`
   - When `CliBackend::from_config()` is called
   - Then a properly configured OpenCode backend is returned

5. **Backend Name Resolution**
   - Given the string `"opencode"`
   - When `CliBackend::from_name()` is called
   - Then a properly configured OpenCode backend is returned

6. **Interactive Mode Conversion**
   - Given an OpenCode backend in headless mode
   - When `for_interactive_prompt()` is called
   - Then an interactive-mode OpenCode backend without `--dangerously-skip-permissions` is returned

7. **Auto-Detection Registration**
   - Given OpenCode is installed on the system
   - When `detect_backend()` is called with auto-detection enabled
   - Then OpenCode is included in the detection candidates

8. **Unit Test Coverage**
   - Given the OpenCode adapter implementation
   - When running `cargo test`
   - Then all new tests pass and existing tests remain green

9. **Code Quality**
   - Given the implementation is complete
   - When running `cargo clippy`
   - Then no new warnings are introduced

## Metadata
- **Complexity**: Low
- **Labels**: Adapter, Backend, OpenCode, CLI Integration
- **Required Skills**: Rust, pattern matching, unit testing
