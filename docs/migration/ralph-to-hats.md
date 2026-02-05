# Migration Guide: ralph-orchestrator to hats

> ralph-orchestrator has been renamed to **hats**. This guide covers
> everything you need to update.

## TL;DR

| Old | New |
|-----|-----|
| `ralph` CLI | `hats` CLI |
| `ralph.yml` | `hats.yml` |
| `.ralph/` directory | `.hats/` directory |
| `RALPH_*` env vars | `HATS_*` env vars |
| `@ralph-orchestrator/ralph-cli` (npm) | `@hats/hats-cli` (npm) |
| `ralph-cli` (crates.io) | `hats-cli` (crates.io) |
| `ralph-orchestrator` (PyPI) | `hats-cli` (PyPI) |
| `mikeyobrien/ralph-orchestrator` (GitHub) | `mikeyobrien/hats` (GitHub) |

## What didn't change

- All behavior, features, and configuration options are identical.
- Config file format is the same -- only the filename changed.
- Presets, hats, event topics, and event loop semantics are unchanged.
- Your `.ralph/specs/` and `.ralph/tasks/` content works as-is under `.hats/`.

## Step-by-step migration

### 1. Install the new package

```bash
# npm (recommended)
npm install -g @hats/hats-cli

# or cargo
cargo install hats-cli

# or homebrew
brew install hats
```

Uninstall the old package when ready:

```bash
npm uninstall -g @ralph-orchestrator/ralph-cli
# or
cargo uninstall ralph-cli
```

### 2. Rename your config file

```bash
mv ralph.yml hats.yml
```

No content changes needed. The format is identical.

**Backward compatibility**: hats will read `ralph.yml` if `hats.yml` is
not found, and print a deprecation warning suggesting you rename it.
This fallback will be removed in a future major version.

### 3. Rename your state directory

```bash
mv .ralph .hats
```

Or just let hats create a fresh `.hats/` directory on next run. Your
specs and tasks are portable:

```bash
mkdir -p .hats
cp -r .ralph/specs .hats/specs
cp -r .ralph/tasks .hats/tasks
```

Events from previous runs (`.ralph/events-*.jsonl`) are archived and
don't need to be migrated.

### 4. Update environment variables

| Old | New |
|-----|-----|
| `RALPH_VERBOSE` | `HATS_VERBOSE` |
| `RALPH_BACKEND` | `HATS_BACKEND` |
| `RALPH_TELEGRAM_BOT_TOKEN` | `HATS_TELEGRAM_BOT_TOKEN` |
| `RALPH_*` | `HATS_*` |

**Backward compatibility**: `RALPH_*` variables are read as a deprecated
fallback when the corresponding `HATS_*` variable is not set. A warning
is printed to stderr. This fallback will be removed in a future major
version.

### 5. Update CI/CD and scripts

Find and replace in your workflows:

```bash
# Before
ralph run -p "task" --max-iterations 10
ralph plan "feature"
ralph doctor

# After
hats run -p "task" --max-iterations 10
hats plan "feature"
hats doctor
```

### 6. Update GitHub Actions

If you reference the repo in workflows:

```yaml
# Before
uses: mikeyobrien/ralph-orchestrator@main

# After
uses: mikeyobrien/hats@main
```

GitHub auto-redirects the old URL, but update for clarity.

### 7. Update .gitignore

```gitignore
# Before
.ralph/

# After
.hats/
!.hats/specs/
!.hats/specs/**
!.hats/tasks/
!.hats/tasks/**
```

## Presets

All preset names are unchanged. The preset files themselves have been
updated internally:

```bash
hats run --preset spec-driven -p "task"
hats run --preset feature -p "task"
```

## Timeline

| Phase | Status |
|-------|--------|
| Codebase rename | Done |
| Config fallback (ralph.yml, RALPH_*) | Done |
| New packages published (npm, crates.io, PyPI) | Pending |
| Old packages deprecated with redirect | Pending |
| GitHub repo renamed | Pending |
| Fallback removal | Future major version |

## FAQ

**Q: Will my old `ralph` command keep working?**
A: The final version of the `ralph` packages will print a message
directing you to install `hats`. The command itself won't be maintained.

**Q: Do I need to re-run `hats init`?**
A: No. Just rename `ralph.yml` to `hats.yml`. The content is identical.

**Q: What about the Telegram bot?**
A: Same bot, new env var name. `HATS_TELEGRAM_BOT_TOKEN` replaces
`RALPH_TELEGRAM_BOT_TOKEN`. The old name works as a fallback for now.

**Q: Will old GitHub URLs break?**
A: No. GitHub auto-redirects renamed repos. But update your bookmarks.
