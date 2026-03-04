# Implementation Plan: Add roo-cli as a Provider

## Checklist

- [ ] Step 1: Add roo backend definitions to `cli_backend.rs`
- [ ] Step 2: Register roo in all dispatch points
- [ ] Step 3: Add `--prompt-file` support for all prompts
- [ ] Step 4: Add auto-detection support
- [ ] Step 5: Create preset file
- [ ] Step 6: Update documentation
- [ ] Step 7: Post-Implementation Validation

---

## Step 1: Add roo backend definitions to `cli_backend.rs`

**Objective**: Create `roo()` and `roo_interactive()` methods on `CliBackend` with unit tests.

**Implementation guidance**:
- Add `roo()` method returning `CliBackend` with `command: "roo"`, `args: ["--print", "--ephemeral"]`, `prompt_mode: PromptMode::Arg`, `prompt_flag: None`, `output_format: OutputFormat::Text`, `env_vars: []`
- Add `roo_interactive()` method returning `CliBackend` with `command: "roo"`, `args: []`, `prompt_mode: PromptMode::Arg`, `prompt_flag: None`, `output_format: OutputFormat::Text`, `env_vars: []`
- Follow the exact pattern of existing backends (kiro, pi, opencode)

**Test requirements**:
- `test_roo_backend()`: Verify `roo()` produces `roo --print --ephemeral --prompt-file <path>` via `build_command()` (prompt written to temp file)
- `test_roo_interactive()`: Verify `roo_interactive()` produces `roo "test prompt"`
- `test_env_vars_default_empty`: Add `CliBackend::roo()` to the existing env_vars emptiness test

**Integration with previous work**: This is the foundation — all subsequent steps build on these definitions.

**Demo**: `cargo test -p ralph-adapters test_roo_backend` passes. The `roo()` and `roo_interactive()` constructors return correct `CliBackend` configurations.

---

## Step 2: Register roo in all dispatch points

**Objective**: Wire `roo` into all the places where backends are resolved from config/names.

**Implementation guidance**:
- In `from_config()` (line ~69): Add `"roo" => Self::roo()` match arm
- In `from_name()` (line ~235): Add `"roo" => Ok(Self::roo())` match arm
- In `for_interactive_prompt()` (line ~388): Add `"roo" => Ok(Self::roo_interactive())` match arm
- In `filter_args_for_interactive()` (line ~662): Add `"roo"` arm that filters `--print` and `--ephemeral` flags

**Test requirements**:
- `test_from_name_roo()`: `CliBackend::from_name("roo")` returns backend with command "roo"
- `test_from_config_roo()`: Config `{ backend: "roo" }` creates correct backend
- `test_from_config_roo_with_args()`: Config with `args: ["--provider", "bedrock", "--model", "anthropic.claude-sonnet-4-6"]` appends correctly
- `test_for_interactive_prompt_roo()`: Factory returns `roo_interactive()` and builds correct command
- `test_roo_interactive_mode_removes_print()`: `build_command("prompt", true)` removes `--print` from args

**Integration with previous work**: Uses `roo()` and `roo_interactive()` from Step 1.

**Demo**: `cargo test -p ralph-adapters from_name_roo` and `cargo test -p ralph-adapters from_config_roo` both pass. Running `ralph run -b roo --dry-run` (if available) shows the correct command would be spawned.

---

## Step 3: Add `--prompt-file` support for all prompts

**Objective**: All roo prompts are passed via `--prompt-file` (one code path, no size threshold).

**Implementation guidance**:
- In `build_command()`, after the existing Claude large-prompt handling block, add a block for roo:
  ```
  if self.command == "roo" {
      // Always write prompt to temp file
      // Add --prompt-file <path> to args (no positional prompt)
      // Return temp file handle to keep it alive
  }
  ```
- This is simpler than conditional logic — one path, no 7000-char threshold
- Roo natively reads from `--prompt-file` (verified working for both small and large prompts)
- Fallback to positional arg only if temp file creation fails

**Test requirements**:
- `test_roo_uses_prompt_file()`: Any prompt results in `--prompt-file` arg and temp file
- `test_roo_prompt_file_content()`: Verify temp file contains the full prompt text
- `test_roo_prompt_file_fallback()`: Verify positional arg is used if temp file creation fails

**Integration with previous work**: Modifies `build_command()` which uses `roo()` backend from Step 1.

**Demo**: `cargo test -p ralph-adapters test_roo_uses_prompt_file` passes. All prompts use `--prompt-file`.

---

## Step 4: Add auto-detection support

**Objective**: Enable `roo` in auto-detection when `cli.backend: auto` is configured.

**Implementation guidance**:
- In `auto_detect.rs`, add `"roo"` at the end of `DEFAULT_PRIORITY` array
- `detection_command("roo")` should return `"roo"` (default behavior, no special mapping needed)
- In `NoBackendError::fmt()`, add `"  • Roo CLI:      https://github.com/RooVetGit/Roo-Code"` to the install suggestions

**Test requirements**:
- `test_detection_command_roo()`: `detection_command("roo")` returns `"roo"`
- `test_default_priority_includes_roo()`: `DEFAULT_PRIORITY` contains `"roo"`
- `test_default_priority_roo_after_pi()`: "roo" comes after "pi" (last position)
- Update `test_no_backend_error_display()`: Verify error message includes "Roo CLI"

**Integration with previous work**: Independent of Steps 1-3. Uses only `auto_detect.rs`.

**Demo**: `cargo test -p ralph-adapters detection_command_roo` passes. The error message for missing backends includes roo.

---

## Step 5: Create preset file

**Objective**: Create `presets/minimal/roo.yml` as a ready-to-use configuration.

**Implementation guidance**:
- Copy `presets/minimal/kiro.yml` as the starting point
- Change `cli.backend` from `"kiro"` to `"roo"`
- Keep all other settings identical (event_loop, core guardrails, hats)
- Update the comment header to reference Roo Code CLI

**Test requirements**:
- Verify YAML is valid: `python3 -c "import yaml; yaml.safe_load(open('presets/minimal/roo.yml'))"`
- Verify `cli.backend` is `"roo"` in the file

**Integration with previous work**: Uses the backend name registered in Steps 1-2.

**Demo**: `cat presets/minimal/roo.yml` shows a valid configuration. `ralph run --preset roo -- --provider bedrock ...` would use roo as the backend.

---

## Step 6: Update documentation

**Objective**: Update code documentation to reflect roo as a supported backend.

**Implementation guidance**:
- In `crates/ralph-adapters/src/lib.rs`, update the crate-level doc comment to include `- Roo (Roo Code)` in the list of supported backends
- Ensure all new public methods have doc comments explaining their purpose

**Test requirements**:
- `cargo doc -p ralph-adapters --no-deps` completes without warnings

**Integration with previous work**: References all work from Steps 1-5.

**Demo**: `cargo doc` builds cleanly. The ralph-adapters documentation lists Roo as a supported backend.

---

## Step 7: Post-Implementation Validation

**Objective**: Verify the complete implementation works end-to-end.

**Validation checklist**:
- [ ] `cargo test -p ralph-adapters` — All tests pass (existing + new roo tests)
- [ ] `cargo test` — Full project test suite passes (no regressions)
- [ ] `cargo build` — Clean build with no warnings
- [ ] `cargo doc -p ralph-adapters --no-deps` — Documentation builds cleanly
- [ ] YAML validation: `presets/minimal/roo.yml` is valid
- [ ] Manual E2E (if roo is configured): `ralph run -b roo -- --provider bedrock --aws-profile roo-bedrock --aws-region us-east-1 --model anthropic.claude-sonnet-4-6 --max-tokens 64000 -p "Create hello.txt with Hello World"` completes at least one iteration

**Demo**: Full end-to-end demonstration:
1. Show `cargo test -p ralph-adapters` with all roo tests passing
2. Show `presets/minimal/roo.yml` contents
3. Show `ralph run -b roo` starting and executing an iteration with roo backend
