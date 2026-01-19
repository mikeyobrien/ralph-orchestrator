---
name: release-bump
description: Use when bumping ralph-orchestrator version for a new release, after fixes are committed and ready to publish
---

# Release Bump

## Overview

Bump version and trigger release for ralph-orchestrator. All versions live in workspace `Cargo.toml` - individual crates inherit via `version.workspace = true`.

## Quick Reference

| Step | Command/Action |
|------|----------------|
| 1. Bump version | Edit `Cargo.toml`: replace all `version = "X.Y.Z"` (7 occurrences) |
| 2. Build | `cargo build` (updates Cargo.lock) |
| 3. Test | `cargo test` |
| 4. Commit | `git add Cargo.toml Cargo.lock && git commit -m "chore: bump to vX.Y.Z"` |
| 5. Push | `git push origin main` |
| 6. Tag | `git tag vX.Y.Z && git push origin vX.Y.Z` |

## Version Locations (All in Cargo.toml)

```toml
# Line ~17 - workspace version
[workspace.package]
version = "X.Y.Z"

# Lines ~113-118 - internal crate dependencies
ralph-proto = { version = "X.Y.Z", path = "crates/ralph-proto" }
ralph-core = { version = "X.Y.Z", path = "crates/ralph-core" }
ralph-adapters = { version = "X.Y.Z", path = "crates/ralph-adapters" }
ralph-tui = { version = "X.Y.Z", path = "crates/ralph-tui" }
ralph-cli = { version = "X.Y.Z", path = "crates/ralph-cli" }
ralph-bench = { version = "X.Y.Z", path = "crates/ralph-bench" }
```

**Tip:** Use Edit tool with `replace_all: true` on `version = "OLD"` → `version = "NEW"` to update all 7 at once.

## What CI Does Automatically

Once you push the tag, `.github/workflows/release.yml` triggers and:

1. Creates the GitHub Release with auto-generated notes
2. Builds binaries for macOS (arm64, x64) and Linux (arm64, x64)
3. Uploads artifacts to the GitHub Release
4. Publishes to crates.io (in dependency order)
5. Publishes to npm as `@ralph-orchestrator/ralph`

## Common Mistakes

| Mistake | Fix |
|---------|-----|
| Only updating workspace.package.version | Must update all 7 occurrences including internal deps |
| Forgetting to run tests | Always `cargo test` before commit |
| Creating release manually with `gh release create` | Just push the tag - CI creates the release with artifacts |
| Pushing tag before main | Push main first, then push the tag |
