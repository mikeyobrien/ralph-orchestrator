# Ralph Hat Collections

This directory contains the canonical built-in hat collections Ralph still ships and supports.

Built-ins are embedded into the CLI from these files and exposed through `ralph init --list-presets`.

## Supported Builtins

| Collection | Source | Best for |
|---|---|---|
| `autoresearch` | `presets/autoresearch.yml` | Autonomous experiment loop for any measurable improvement |
| `code-assist` | `presets/code-assist.yml` | Default implementation workflow |
| `debug` | `presets/debug.yml` | Investigation and fix verification |
| `research` | `presets/research.yml` | Read-only exploration and synthesis |
| `review` | `presets/review.yml` | Adversarial code review |
| `pdd-to-code-assist` | `presets/pdd-to-code-assist.yml` | Advanced end-to-end idea-to-code workflow |

## Internal Presets

These remain loadable for Ralph internals or testing, but are intentionally hidden from normal builtin listings:

- `hatless-baseline`
- `merge-loop`

## Product Positioning

- `code-assist` is the recommended default for implementation work.
- `pdd-to-code-assist` is intentionally kept as an advanced, fun example. It is slower, more expensive, and less predictable than `code-assist`.
- Other historical presets are now treated as documentation examples instead of supported builtins.

## Quick Start

```bash
ralph init --backend claude
ralph init --list-presets

ralph run -c ralph.yml -H builtin:autoresearch -p "Improve test coverage in src/core/"
ralph run -c ralph.yml -H builtin:code-assist -p "Add OAuth login"
ralph run -c ralph.yml -H builtin:debug -p "Investigate intermittent timeout"
ralph run -c ralph.yml -H builtin:research -p "Map auth architecture"
ralph run -c ralph.yml -H builtin:review -p "Review changes in src/api/"
ralph run -c ralph.yml -H builtin:pdd-to-code-assist -p "Build a new import pipeline"
```

## Examples Instead of Builtins

Example workflow patterns now live in the docs rather than as shipped preset files. See:

- `docs/examples/`
- `presets/COLLECTION.md`

## Importing Autoloop Presets

Ralph can load hat collections authored for [`@mobrienv/autoloop`](https://github.com/mobrienv/autoloop) without translating them by hand. Autoloop presets are multi-file directories (`autoloops.toml` + `topology.toml` + `roles/*.md` + optional `harness.md`); Ralph's preset loader detects and imports them directly.

**Three ways to target an autoloop preset with `-H`:**

```bash
# 1. Explicit directory path
ralph run -c ralph.yml -H ./path/to/autocode -p "..."

# 2. `autoloop:<name>` — resolves via lookup order below
ralph run -c ralph.yml -H autoloop:autocode -p "..."

# 3. Autoloop dir in the usual `./presets/<name>/` slot (auto-detected)
ralph run -c ralph.yml -H ./presets/autocode -p "..."
```

**Lookup order for `autoloop:<name>`:**

1. `./presets/<name>/` (project-local)
2. `$XDG_CONFIG_HOME/ralph/autoloop-presets/<name>/`
3. `$HOME/.config/ralph/autoloop-presets/<name>/`
4. `$AUTOLOOP_PRESETS_DIR/<name>/` (explicit override)

First directory whose layout matches (`autoloops.toml` + `topology.toml` present) wins.

**Mapping:**

| Autoloop | Ralph |
|---|---|
| `topology.toml` `[[role]].id` | `hats.<id>` |
| `role.prompt_file` contents | `hat.instructions` |
| `role.emits` | `hat.publishes` |
| `[handoff]` entries (inverted) | `hat.triggers` |
| `topology.completion` / `event_loop.completion_event` | `event_loop.completion_promise` |
| `event_loop.max_iterations` | `event_loop.max_iterations` |
| `event_loop.required_events` | `event_loop.required_events` |
| `handoff["loop.start"]` route | `event_loop.starting_event = "loop.start"` |
| `harness.md` (whole file) | prepended to every hat's `instructions` |

Ralph fields autoloop doesn't populate (`cli.backend`, `core.specs_dir`, `core.guardrails`, etc.) still come from your `ralph.yml`, same as with a builtin hat collection.

The importer is extensible — see `crates/ralph-core/src/preset_source.rs` for the `PresetSource` trait; new preset shapes plug in by implementing `detect` + `load`.

## Source Of Truth

- Canonical builtins: `presets/*.yml`
- Builtin index: `presets/index.json`
- Embedded CLI mirror: `crates/ralph-cli/presets/*.yml`
- Sync script: `./scripts/sync-embedded-files.sh`
