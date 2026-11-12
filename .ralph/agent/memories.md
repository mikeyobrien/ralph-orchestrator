# Memories

## Patterns

### mem-1794518972-fc67
> ralph-core auto_commit_changes (git_ops.rs) stages git add --update plus untracked files filtered by is_run_artifact; when adding new Ralph-written runtime paths, extend that filter (and is_durable_ralph_artifact for files meant to be committed), because user repos' .gitignore does not cover Ralph's layout
<!-- tags: git, auto-commit | created: 2026-11-12 -->

### mem-1794518372-080b
> When a ralph-cli integration test spawns the ralph binary, use tests/common::ralph_command() (strips inherited RALPH_* vars), because a child inheriting the live loop's RALPH_EVENTS_FILE/RALPH_WORKSPACE_ROOT writes into that loop; integration_env_isolation_guard.rs fails on direct Command::new(CARGO_BIN_EXE_ralph). For in-process ralph-core tests that persist state, set config.core.workspace_root to a tempdir since CoreConfig::default() reads RALPH_WORKSPACE_ROOT. scripts/check-test-event-isolation.sh is the end-to-end check.
<!-- tags: testing, events, cli | created: 2026-11-12 -->

## Decisions

## Fixes

## Context
