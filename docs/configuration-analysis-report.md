# Ralph Orchestrator Configuration Analysis Report

This report analyzes the configuration systems in Ralph Orchestrator v2.0, documenting config precedence, available options across systems, and documentation gaps.

## 1. Configuration Precedence

When launching a ralph command, configuration is resolved in the following order (highest to lowest precedence):

### Precedence Order

```
1. CLI Flags (highest)        → --max-iterations 50
2. Environment Variables      → RALPH_VERBOSE=1
3. YAML Config File          → ralph.yml
4. Hardcoded Defaults (lowest) → max_iterations: 100
```

### Precedence Implementation

The precedence is implemented in `crates/ralph-cli/src/main.rs`:

```rust
// 1. Load config from file (or use defaults)
let mut config = if config_path.exists() {
    RalphConfig::from_file(&config_path)?
} else {
    RalphConfig::default()
};

// 2. Normalize v1 flat fields into v2 nested structure
config.normalize();

// 3. Apply CLI overrides (after normalization so they take final precedence)
if let Some(text) = args.prompt_text {
    config.event_loop.prompt = Some(text);
}
```

### Verbosity-Specific Precedence

Verbosity has explicit precedence handling at `main.rs:131-156`:

```rust
fn resolve(cli_verbose: bool, cli_quiet: bool) -> Self {
    // 1. CLI flags take precedence
    if cli_quiet { return Verbosity::Quiet; }
    if cli_verbose { return Verbosity::Verbose; }

    // 2. Environment variables
    if std::env::var("RALPH_QUIET").is_ok() { return Verbosity::Quiet; }
    if std::env::var("RALPH_VERBOSE").is_ok() { return Verbosity::Verbose; }

    // 3. Default
    Verbosity::Normal
}
```

### Key Observations

1. **V1/V2 Normalization**: V1 flat fields (e.g., `agent`, `max_iterations`) are normalized into V2 nested structure (e.g., `cli.backend`, `event_loop.max_iterations`) _before_ CLI overrides are applied.

2. **CLI Always Wins**: CLI flags are applied last, ensuring they override both config file and environment variables.

3. **No ENV Support in Config Loading**: Unlike the docs suggest, most environment variables are NOT read during config loading. Only `RALPH_VERBOSE` and `RALPH_QUIET` are checked by the Rust CLI.

---

## 2. Configuration Options Comparison

### Legend
- ✅ = Fully Supported
- ⚠️ = Partial/Limited Support
- ❌ = Not Supported
- 🔄 = Deprecated/Dropped

### Core Options

| Option | CLI Flag | Env Var | YAML Key | Type | Description |
|--------|----------|---------|----------|------|-------------|
| Config file | `-c, --config` | ❌ | N/A | `PathBuf` | Path to config file (default: ralph.yml) |
| Verbose | `-v, --verbose` | `RALPH_VERBOSE` | `verbose` | `bool` | Enable verbose output |
| Quiet | `-q, --quiet` | `RALPH_QUIET` | ❌ | `bool` | Suppress streaming output |
| Color mode | `--color` | ❌ | ❌ | `enum` | auto/always/never |

### Prompt Options

| Option | CLI Flag | Env Var | YAML Key | Type | Description |
|--------|----------|---------|----------|------|-------------|
| Inline prompt | `-p, --prompt` | ❌ | `event_loop.prompt` | `String` | Inline prompt text |
| Prompt file | `-P, --prompt-file` | ❌ | `event_loop.prompt_file` | `String` | Path to prompt file (default: PROMPT.md) |
| Completion promise | `--completion-promise` | ❌ | `event_loop.completion_promise` | `String` | Output that signals completion (default: LOOP_COMPLETE) |

### Execution Limits

| Option | CLI Flag | Env Var | YAML Key | Type | Description |
|--------|----------|---------|----------|------|-------------|
| Max iterations | `--max-iterations` | ❌ | `event_loop.max_iterations` | `u32` | Max loop iterations (default: 100) |
| Max runtime | ❌ | ❌ | `event_loop.max_runtime_seconds` | `u64` | Max runtime in seconds (default: 14400) |
| Max cost | ❌ | ❌ | `event_loop.max_cost_usd` | `Option<f64>` | Max cost in USD |
| Max failures | ❌ | ❌ | `event_loop.max_consecutive_failures` | `u32` | Stop after N failures (default: 5) |

### Execution Mode

| Option | CLI Flag | Env Var | YAML Key | Type | Description |
|--------|----------|---------|----------|------|-------------|
| Interactive | `-i, --interactive` | ❌ | `cli.default_mode` | `String` | Enable TUI mode |
| Autonomous | `-a, --autonomous` | ❌ | `cli.default_mode` | `String` | Force headless mode |
| Idle timeout | `--idle-timeout` | ❌ | `cli.idle_timeout_secs` | `u32` | TUI idle timeout (default: 30) |
| Experimental TUI | ❌ | ❌ | `cli.experimental_tui` | `bool` | Enable TUI support |

### Backend Options

| Option | CLI Flag | Env Var | YAML Key | Type | Description |
|--------|----------|---------|----------|------|-------------|
| Backend | ❌ (via init) | ❌ | `cli.backend` | `String` | claude/kiro/gemini/codex/amp/auto/custom |
| Custom command | ❌ | ❌ | `cli.command` | `Option<String>` | Command for custom backend |
| Custom args | ❌ | ❌ | `cli.args` | `Vec<String>` | Args for custom backend |
| Prompt mode | ❌ | ❌ | `cli.prompt_mode` | `String` | arg/stdin (default: arg) |
| Prompt flag | ❌ | ❌ | `cli.prompt_flag` | `Option<String>` | Custom prompt flag |

### V1 Compatibility (YAML Only, Normalized)

| V1 Field | V2 Equivalent | Type | Notes |
|----------|---------------|------|-------|
| `agent` | `cli.backend` | `String` | Flat format |
| `prompt_file` | `event_loop.prompt_file` | `String` | Flat format |
| `completion_promise` | `event_loop.completion_promise` | `String` | Flat format |
| `max_iterations` | `event_loop.max_iterations` | `u32` | Flat format |
| `max_runtime` | `event_loop.max_runtime_seconds` | `u64` | Flat format |
| `max_cost` | `event_loop.max_cost_usd` | `f64` | Flat format |

### Dropped/Deferred Fields (Accepted with Warning)

| Field | Status | Reason |
|-------|--------|--------|
| `max_tokens` | 🔄 Dropped | Controlled by CLI tool |
| `retry_delay` | 🔄 Dropped | Handled differently in v2 |
| `archive_prompts` | 🔄 Deferred | Feature not yet available |
| `enable_metrics` | 🔄 Deferred | Feature not yet available |
| `adapters.*.tool_permissions` | 🔄 Dropped | CLI manages permissions |

### Special Options (CLI Only)

| Option | CLI Flag | Description |
|--------|----------|-------------|
| Dry run | `--dry-run` | Show what would execute |
| Record session | `--record-session <FILE>` | Record to JSONL for replay |
| TUI (deprecated) | `--tui` | Deprecated, use -i |

### Init Subcommand Options

| Option | CLI Flag | Description |
|--------|----------|-------------|
| Backend | `--backend` | Generate minimal config |
| Preset | `--preset` | Use embedded preset |
| List presets | `--list-presets` | Show available presets |
| Force | `--force` | Overwrite existing config |

### Events Subcommand Options

| Option | CLI Flag | Description |
|--------|----------|-------------|
| Last N | `--last` | Show last N events |
| Topic filter | `--topic` | Filter by event topic |
| Iteration filter | `--iteration` | Filter by iteration |
| Output format | `--format` | table/json |
| Custom file | `--file` | Path to events file |
| Clear | `--clear` | Clear event history |

### Plan/Task Subcommand Options

| Option | CLI Flag | Description |
|--------|----------|-------------|
| Idea/Input | `<IDEA>` / `<INPUT>` | Positional argument |
| Backend override | `-b, --backend` | Override config backend |

---

## 3. Documentation Gaps and Inaccuracies

### Critical Issues

#### 3.1 Outdated Python Documentation

**Location**: `docs/api/config.md`, `docs/api/cli.md`

**Issue**: These files document the Python v1.x implementation, NOT the current Rust v2.0 implementation.

**Evidence**:
- References `python ralph_orchestrator.py` commands
- Documents Python-style configuration with `argparse`
- Lists options that don't exist in v2 (`--no-git`, `--no-archive`, `--agent-args`)

**Impact**: Users following this documentation will get incorrect information.

**Recommendation**: Either delete these files or completely rewrite for Rust v2.

---

#### 3.2 Environment Variables Discrepancy

**Location**: `docs/guide/configuration.md:15-22`, `docs/glossary.md:233-245`, various deployment docs

**Issue**: Documentation claims many environment variables are supported, but the Rust implementation only reads:
- `RALPH_VERBOSE`
- `RALPH_QUIET`

**Documented but NOT Implemented**:
- `RALPH_AGENT`
- `RALPH_MAX_ITERATIONS`
- `RALPH_MAX_RUNTIME`
- `RALPH_MAX_COST`
- `RALPH_DRY_RUN`
- `RALPH_CHECKPOINT_INTERVAL`
- etc.

**Evidence**: Grep of codebase shows only `RALPH_VERBOSE` and `RALPH_QUIET` are read in `main.rs:148-151`.

**Impact**: High - users expecting environment-based configuration will have it silently ignored.

**Recommendation**: Either implement environment variable reading or update all documentation to remove these references.

---

#### 3.3 Missing YAML Schema Documentation

**Location**: None exists

**Issue**: There is no comprehensive YAML schema reference. Users must read Rust source code to understand:
- All available fields
- Default values
- Validation rules
- Field relationships (e.g., mutual exclusivity)

**Evidence**: `crates/ralph-core/src/config.rs` contains all this information in code comments and struct definitions but it's not exposed as user documentation.

**Recommendation**: Generate a complete YAML schema reference from the config.rs source.

---

### Moderate Issues

#### 3.4 Inconsistent CLI Flag Documentation

**Location**: `docs/guide/configuration.md` vs `README.md` vs actual `--help`

**Issues**:
- `docs/guide/configuration.md` uses Python-style `--` flags
- README is mostly accurate but incomplete
- `--help` output is the source of truth

**Recommendation**: Make README the canonical CLI reference, link to `ralph --help` for full details.

---

#### 3.5 Preset README vs Actual Presets

**Location**: `presets/README.md`

**Issues**:
- Lists `checkpoint_interval` in example but this isn't a valid YAML key
- Example uses `ralph start` command which doesn't exist (should be `ralph run`)
- Lists 7 presets, actual count is 23

**Evidence**: `ralph init --list-presets` shows 23 presets.

**Recommendation**: Update README with all 23 presets and correct examples.

---

#### 3.6 Hat Configuration Examples Incorrect

**Location**: `docs/guide/configuration.md:433-472`

**Issue**: Uses list format for hats (`hats: -name:`) but actual config uses map format (`hats: my_hat: name:`).

**Evidence**: All preset files use map format, config.rs expects `HashMap<String, HatConfig>`.

**Recommendation**: Fix all hat examples to use correct map format.

---

### Minor Issues

#### 3.7 Missing adapter_settings Documentation

**Location**: None

**Issue**: `adapters` config section allows per-backend settings (`timeout`, `enabled`) but is undocumented.

**Location in code**: `config.rs:156-206`

---

#### 3.8 TUI Configuration Undocumented

**Location**: None

**Issue**: `tui.prefix_key` configuration is undocumented.

**Default**: `ctrl-a`

**Location in code**: `config.rs:679-737`

---

#### 3.9 Core Configuration Undocumented

**Location**: Partially in README

**Issue**: `core.scratchpad`, `core.specs_dir`, `core.guardrails` are only mentioned in passing.

**Location in code**: `config.rs:559-603`

---

## 4. Summary of Recommendations

### High Priority

1. **Delete or Archive Python Docs**: `docs/api/config.md` and `docs/api/cli.md` are dangerously outdated
2. **Environment Variables**: Either implement claimed env vars or remove them from all docs
3. **Create YAML Schema Reference**: Generate from config.rs with all fields, types, defaults

### Medium Priority

4. **Fix presets/README.md**: Update command, preset count, and examples
5. **Fix Hat Config Examples**: Use map format, not list format
6. **Consolidate CLI Reference**: README should be canonical, link to --help

### Low Priority

7. **Document adapters section**: Per-backend timeout and enabled settings
8. **Document TUI config**: prefix_key customization
9. **Document core config**: scratchpad, specs_dir, guardrails

---

## 5. Configuration Flow Diagram

```
┌─────────────────────────────────────────────────────────────────┐
│                     Configuration Resolution                     │
└─────────────────────────────────────────────────────────────────┘

                          ┌──────────────┐
                          │  CLI Flags   │  ← Highest precedence
                          │  (--max-*)   │
                          └──────┬───────┘
                                 │
                                 ▼
                       ┌─────────────────┐
                       │   Env Vars      │  ← Only RALPH_VERBOSE/QUIET
                       │  (Limited!)     │
                       └────────┬────────┘
                                │
                                ▼
            ┌───────────────────────────────────────┐
            │           ralph.yml                    │
            │  ┌─────────────────────────────────┐  │
            │  │  V1 Flat Fields    ──normalize──│──┼──┐
            │  │  (agent, max_*)                 │  │  │
            │  └─────────────────────────────────┘  │  │
            │                                       │  │
            │  ┌─────────────────────────────────┐  │  │
            │  │  V2 Nested Fields               │◄─┼──┘
            │  │  (cli.backend, event_loop.*)    │  │
            │  └─────────────────────────────────┘  │
            └───────────────────────────────────────┘
                                │
                                ▼
                       ┌─────────────────┐
                       │    Defaults     │  ← Lowest precedence
                       │  (from code)    │
                       └─────────────────┘
```

---

*Report generated: 2026-01-17*
*Ralph Orchestrator Version: v2.0 (Rust)*
