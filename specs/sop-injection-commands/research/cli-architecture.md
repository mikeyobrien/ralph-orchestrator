# Research: Ralph CLI Architecture

## CLI Location and Structure

**File:** `crates/ralph-cli/src/main.rs`

### Key Structures:

- **Cli struct** (lines 172-193): Top-level parser using `clap`
- **Commands enum** (lines 195-214): Defines all subcommands:
  ```rust
  enum Commands {
      Run(RunArgs),
      Resume(ResumeArgs),
      Events(EventsArgs),
      Init(InitArgs),
      Clean(CleanArgs),
      Emit(EmitArgs),
  }
  ```

## Default Backend Resolution

**Files:**
- `crates/ralph-adapters/src/cli_backend.rs` - Backend definitions
- `crates/ralph-adapters/src/auto_detect.rs` - Auto-detection logic
- `crates/ralph-core/src/config.rs` - Config types

### Resolution Flow:
1. Load config (default: `ralph.yml`)
2. Check if `cli.backend == "auto"`
3. If auto: detect via `command --version` checks
4. Priority: ["claude", "kiro", "gemini", "codex", "amp"]

### CliBackend Factory:
```rust
pub fn from_config(config: &CliConfig) -> Result<Self, CustomBackendError> {
    match config.backend.as_str() {
        "claude" => Ok(Self::claude()),
        "kiro" => Ok(Self::kiro()),
        // ...
    }
}
```

## Skills Locations

### PDD (Prompt-Driven Development)
**File:** `.claude/skills/pdd/SKILL.md`

Transforms rough ideas into detailed designs with implementation plans.

### Code-Task-Generator
**File:** `.claude/skills/code-task-generator/SKILL.md`

Generates `.code-task.md` files from descriptions or PDD plans.

## Content Injection Pattern

Content is injected via:
1. `InstructionBuilder::build_custom_hat()` for custom hats
2. Prompt flag (`-p`, `--prompt`) passed to backend command
3. Event context from EventBus

## Existing Subcommand Pattern

```rust
async fn run_command(
    config_path: PathBuf,
    verbose: bool,
    color_mode: ColorMode,
    args: RunArgs,
) -> Result<()> {
    // 1. Load config
    // 2. Normalize v1 → v2
    // 3. Apply CLI overrides
    // 4. Validate
    // 5. Auto-detect backend if needed
    // 6. Execute
}
```
