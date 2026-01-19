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

#### 3.1 JSON Config File Ghost Documentation

**Location**: Multiple docs reference `ralph.json`
- `docs/faq.md:156` - "Edit `ralph.json`"
- `docs/api/cli.md:339-346` - Creates `ralph.json` config
- `docs/troubleshooting.md:95` - "In ralph.json"
- `docs/glossary.md:29` - "Configuration settings stored in `ralph.json`"

**Issue**: The Rust v2 implementation **ONLY supports YAML** (`ralph.yml`). JSON config files are NOT supported.

**Evidence from code**:
```rust
// crates/ralph-core/src/config.rs:211
pub fn from_file(path: impl AsRef<Path>) -> Result<Self, ConfigError> {
    let content = std::fs::read_to_string(path_ref)?;
    let config: Self = serde_yaml::from_str(&content)?;  // ← YAML only!
    Ok(config)
}

// crates/ralph-cli/src/main.rs:193
#[arg(short, long, default_value = "ralph.yml", global = true)]  // ← Default is .yml
```

**Impact**: High - users creating `ralph.json` files will get parse errors or their config will be silently ignored.

**Recommendation**:
1. Search and replace all `ralph.json` references with `ralph.yml`
2. Or implement JSON support in `from_file()` with extension detection

---

#### 3.2 Outdated Python Documentation

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

## 6. Hat System Configuration

The "hat system" is Ralph's mechanism for role-based agent coordination. Each hat is a specialized persona that triggers on specific events and publishes its own events to drive workflows.

### Hat System Modes

| Mode | Configuration | Description |
|------|---------------|-------------|
| **Solo Mode** | `hats: {}` or no hats section | Ralph handles everything directly |
| **Single Hat** | One hat defined | Hat handles work, Ralph coordinates |
| **Multi-Hat** | Multiple hats defined | Hats coordinate via events, Ralph is universal fallback |

### Hat Configuration Schema

```yaml
hats:
  <hat_id>:                        # Unique identifier (used internally)
    name: "🔴 My Hat"              # Human-readable name (required)
    description: "What this hat does"  # Short purpose description (required in v2)
    triggers: ["event.start"]      # Events that activate this hat (required)
    publishes: ["event.done"]      # Events this hat can emit (optional)
    instructions: |                # Custom prompt instructions (optional)
      Your instructions here...
    backend: claude                # Override default backend (optional)
    default_publishes: "event.done"  # Fallback event if hat forgets (optional)
```

### Hat Configuration Options Table

| Field | Required | Type | Default | Description |
|-------|----------|------|---------|-------------|
| `name` | ✅ Yes | `String` | - | Human-readable display name (supports emoji) |
| `description` | ✅ Yes* | `Option<String>` | `None` | Purpose description for hat selection |
| `triggers` | ✅ Yes | `Vec<String>` | `[]` | Events that activate this hat |
| `publishes` | ❌ No | `Vec<String>` | `[]` | Events this hat can emit |
| `instructions` | ❌ No | `String` | `""` | Custom instructions prepended to prompts |
| `backend` | ❌ No | `HatBackend` | Inherits `cli.backend` | Override backend for this hat |
| `default_publishes` | ❌ No | `Option<String>` | `None` | Auto-publish if hat forgets |

*`description` is required when hats are defined (validation error if missing)

### Hat Backend Options

Hats can override the default backend using three formats:

```yaml
# Format 1: Named backend (string)
hats:
  builder:
    backend: gemini

# Format 2: Kiro with custom agent
hats:
  reviewer:
    backend:
      type: kiro
      agent: codex

# Format 3: Custom command
hats:
  specialist:
    backend:
      command: ./my-agent
      args: ["--flag", "value"]
```

### Event Loop Configuration for Hats

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `event_loop.starting_event` | `String` | `"task.start"` | Initial event Ralph publishes |
| `event_loop.completion_promise` | `String` | `"LOOP_COMPLETE"` | Output string that signals completion |

### Available Presets (23 total)

| Category | Presets |
|----------|---------|
| **Development** | `feature.yml`, `feature-minimal.yml`, `tdd-red-green.yml`, `spec-driven.yml`, `refactor.yml` |
| **Quality** | `review.yml`, `pr-review.yml`, `adversarial-review.yml`, `gap-analysis.yml` |
| **Debugging** | `debug.yml`, `incident-response.yml`, `code-archaeology.yml`, `scientific-method.yml` |
| **Documentation** | `docs.yml`, `documentation-first.yml` |
| **Specialized** | `api-design.yml`, `migration-safety.yml`, `performance-optimization.yml`, `deploy.yml` |
| **Learning** | `research.yml`, `socratic-learning.yml`, `mob-programming.yml` |
| **Baseline** | `hatless-baseline.yml` |

### Hat Validation Rules

The only hard validation rule remaining after "Hatless Ralph" redesign:

| Rule | Error Message |
|------|---------------|
| **Unique Triggers** | `Ambiguous routing for trigger 'X'. Both 'hat1' and 'hat2' trigger on 'X'.` |
| **Reserved Triggers** | `Reserved trigger 'task.start' used by hat 'X' - use 'work.start' instead.` |
| **Missing Description** | `Hat 'X' is missing required 'description' field` |

**Note**: Many former validation rules (entry point, orphan events, reachability) were removed because Ralph acts as a universal fallback.

### Hat System Documentation Gaps

#### 6.1 No Dedicated Hat System Guide

**Location**: None exists

**Issue**: There is no user-facing documentation that explains:
- What the hat system is and why to use it
- How to choose between solo mode and multi-hat mode
- How to design custom hat workflows
- Event naming conventions and best practices

**Available Resources**:
- `specs/hat-collections.spec.md` - Technical specification (not user-friendly)
- `presets/README.md` - Lists 7 of 23 presets, outdated examples
- GitHub README - Brief overview only

**Recommendation**: Create `docs/guide/hat-system.md` with:
1. Conceptual overview (what are hats, when to use them)
2. Quick start (using a preset)
3. Custom hat creation tutorial
4. Event flow design patterns
5. Troubleshooting common issues

---

#### 6.2 Preset Documentation Incomplete

**Location**: `presets/README.md`

**Issues**:
1. Documents 7 presets, but 23 exist
2. Uses `ralph start` command (should be `ralph run`)
3. Missing presets not documented at all:
   - `adversarial-review.yml`
   - `api-design.yml`
   - `code-archaeology.yml`
   - `deploy.yml`
   - `documentation-first.yml`
   - `feature-minimal.yml`
   - `hatless-baseline.yml`
   - `incident-response.yml`
   - `migration-safety.yml`
   - `mob-programming.yml`
   - `performance-optimization.yml`
   - `pr-review.yml`
   - `scientific-method.yml`
   - `socratic-learning.yml`
   - `spec-driven.yml`
   - `tdd-red-green.yml`

**Recommendation**: Auto-generate preset documentation from YAML frontmatter or add all presets manually.

---

#### 6.3 Hat Backend Configuration Undocumented

**Location**: Mentioned in `docs/guide/configuration.md` but with incorrect format

**Issue**: The three backend formats (Named, KiroAgent, Custom) are only documented in code comments at `config.rs:773-784`.

**Example from docs** (WRONG):
```yaml
hats:
  - name: builder
    backend: gemini
```

**Actual format** (CORRECT):
```yaml
hats:
  builder:
    name: "🟢 Builder"
    backend: gemini
```

---

#### 6.4 Event Naming Conventions Undocumented

**Location**: None

**Issue**: No documentation on:
- Reserved event names (`task.start`, `task.resume`)
- Naming conventions (e.g., `category.action`)
- Glob pattern support for triggers
- Self-routing behavior

**Evidence**: `specs/hat-collections.spec.md` has technical details but they're not exposed to users.

---

#### 6.5 `default_publishes` Feature Undocumented

**Location**: Only in code at `config.rs:824-826`

**Issue**: The `default_publishes` field allows a hat to auto-emit an event if it forgets to write one. This is useful for ensuring workflow continuity but is completely undocumented.

**Example**:
```yaml
hats:
  refactorer:
    triggers: ["refactor.task"]
    publishes: ["refactor.done", "cycle.complete"]
    default_publishes: "cycle.complete"  # Auto-emit if hat forgets
```

---

## 7. Updated Recommendations Summary

### High Priority (Updated)

1. **Fix JSON Config Ghost Docs**: Replace all `ralph.json` references with `ralph.yml` (or implement JSON support)
2. **Delete or Archive Python Docs**: `docs/api/config.md` and `docs/api/cli.md` are dangerously outdated
3. **Environment Variables**: Either implement claimed env vars or remove them from all docs
4. **Create YAML Schema Reference**: Generate from config.rs with all fields, types, defaults
5. **Create Hat System Guide**: New `docs/guide/hat-system.md` with conceptual overview and tutorials

### Medium Priority (Updated)

6. **Fix presets/README.md**: Update command (`ralph run`), add all 23 presets
7. **Fix Hat Config Examples**: Use map format, not list format
8. **Document Hat Backend Formats**: Three formats (Named, KiroAgent, Custom)
9. **Consolidate CLI Reference**: README should be canonical, link to --help

### Low Priority

10. **Document adapters section**: Per-backend timeout and enabled settings
11. **Document TUI config**: prefix_key customization
12. **Document core config**: scratchpad, specs_dir, guardrails
13. **Document event naming conventions**: Reserved names, patterns, self-routing
14. **Document default_publishes**: Fallback event mechanism

---

## 8. Post-Update Verification (2026-01-18)

### Completed Fixes ✅

| Item | Status | Notes |
|------|--------|-------|
| `ralph.json` references in faq.md | ✅ Fixed | Now uses `ralph.yml` |
| `ralph.json` references in troubleshooting.md | ✅ Fixed | Now uses `ralph.yml` |
| `ralph.json` references in glossary.md | ✅ Fixed | Now uses `ralph.yml` |
| presets/README.md command | ✅ Fixed | Uses `ralph run` not `ralph start` |
| presets/README.md preset count | ✅ Fixed | Lists all 23 presets |
| docs/guide/configuration.md hat examples | ✅ Fixed | Uses map format |
| docs/guide/configuration.md CLI flags | ✅ Fixed | Documents `--backend`, `--max-runtime`, `--max-cost` |

### Remaining Issues ⚠️

| Item | Location | Issue |
|------|----------|-------|
| List-format hat examples | `docs/migration/v2-hatless-ralph.md` | Still uses `hats: - name:` format |
| Python CLI commands | `docs/installation.md` | Still references `python ralph_orchestrator.py` |
| Python CLI commands | `docs/guide/cost-management.md` | Still references Python CLI |
| Python CLI commands | `docs/guide/agents.md` | Still references Python CLI |
| Python CLI commands | `docs/examples/*.md` | Still references Python CLI |
| Incorrect short flag | `docs/guide/configuration.md:168` | Shows `-n 50` but `--max-iterations` has no short flag |
| Outdated Python API docs | `docs/api/config.md`, `docs/api/cli.md` | Still exist, completely outdated |

### Summary

The core configuration documentation (`docs/guide/configuration.md`) and presets documentation (`presets/README.md`) have been updated and are now accurate for Ralph v2.0.

However, several secondary documentation files still contain Python v1 references that need cleanup:
- Installation guide
- Cost management guide
- Agents guide
- Example files
- Migration guide (list-format hats)
- API reference docs (should be deleted or archived)

**Recommendation**: Create follow-up tasks to clean up remaining Python references in secondary docs.

---

*Report generated: 2026-01-17*
*Report updated: 2026-01-18*
*Ralph Orchestrator Version: v2.0.7 (Rust)*
