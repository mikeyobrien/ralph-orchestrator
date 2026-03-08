# Research: Embedded Presets & Import Interactions

## Current Architecture

### Two Directories, Kept in Sync
| Directory | Purpose |
|-----------|---------|
| `presets/` (repo root) | Canonical source, human-editable |
| `crates/ralph-cli/presets/` | Mirror for `include_str!()` embedding |

Sync via `scripts/sync-embedded-files.sh` before publishing.

### 16 Embedded Presets
bugfix, code-assist, debug, deploy, docs, feature, fresh-eyes, gap-analysis, hatless-baseline, merge-loop, pdd-to-code-assist, pr-review, refactor, research, review, spec-driven

### 11 Minimal Presets (NOT embedded)
In `presets/minimal/`: claude, kiro, codex, gemini, amp, builder, code-assist, opencode, smoke, test, preset-evaluator

## Config Source Resolution

```rust
pub enum ConfigSource {
    File(PathBuf),           // Local file path
    Builtin(String),         // Legacy "builtin:name" → REJECTED
    Remote(String),          // HTTP/HTTPS URL
    Override { key, value }  // core.field=value
}

pub enum HatsSource {
    File(PathBuf),           // Local file path
    Builtin(String),         // builtin:feature → embedded preset
    Remote(String),          // HTTP/HTTPS URL
}
```

## Key Constraint: Embedded Presets Can't Import Files

An embedded preset compiled into the binary has no filesystem context. If `builtin:feature` contained `import: ./shared-hats/builder.yml`, there's no directory to resolve the relative path against.

**Options:**
1. **Disallow imports in embedded presets** — simplest, most predictable
2. **Resolve relative to CWD** — surprising behavior
3. **Resolve relative to a well-known directory** (e.g., `~/.ralph/hats/`) — adds a new concept

**Recommendation:** Disallow `import:` in embedded presets. Embedded presets are curated, self-contained definitions. If users need imports, they use file-based presets.

## Impact on Import Design

| Source | Can contain `import:`? | Path resolution base |
|--------|----------------------|---------------------|
| File-based preset | Yes | Directory containing the preset file |
| Builtin (embedded) | No | N/A — reject with clear error |
| Remote URL preset | No (security) | N/A — reject transitive imports |
| Hat file (via -H) | Yes | Directory containing the hat file |

## Recommended Pattern

```bash
# Self-contained embedded preset (no imports)
ralph run -H builtin:feature -p "Add OAuth"

# File-based preset with imports
ralph run -H ./my-workflow.yml -p "Add OAuth"
# my-workflow.yml can contain: import: ./shared-hats/builder.yml

# Hat file with imports
ralph run -c ralph.yml -H ./hats/custom.yml -p "Add OAuth"
```
