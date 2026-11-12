# Memories

## Patterns

### mem-1794518372-080b
> When a ralph-cli integration test spawns the ralph binary, use tests/common::ralph_command() (strips inherited RALPH_* vars), because a child inheriting the live loop's RALPH_EVENTS_FILE/RALPH_WORKSPACE_ROOT writes into that loop; integration_env_isolation_guard.rs fails on direct Command::new(CARGO_BIN_EXE_ralph). For in-process ralph-core tests that persist state, set config.core.workspace_root to a tempdir since CoreConfig::default() reads RALPH_WORKSPACE_ROOT. scripts/check-test-event-isolation.sh is the end-to-end check.
<!-- tags: testing, events, cli | created: 2026-11-12 -->

## Decisions

## Fixes

## Context
