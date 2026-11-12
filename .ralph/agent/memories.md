# Memories

## Patterns

### mem-1794520647-8bf2
> Claude stream context window: ClaudeSessionMetrics reads result-line modelUsage.<init model>.contextWindow (lenient deserializer, since that line also carries cost); loop_runner::reported_or_configured_context_window prefers a reported window over resolve_context_window_for_backend on both PTY and headless paths. When another backend starts reporting a window, set SessionResult.context_window > 0 and the loop picks it up.
<!-- tags: metrics, cli | created: 2026-11-12 -->

### mem-1794519940-12fe
> When a ralph-cli integration test needs a headless Claude iteration without an API call, use backend: claude with command: pointing at a script that cats a stream-json fixture, and run 'ralph run --autonomous' (not --no-tui, which conflicts with --autonomous); see tests/integration_headless_metrics.rs. Claude usage aggregation for both executors lives in claude_stream.rs ClaudeSessionMetrics.
<!-- tags: testing, metrics, cli | created: 2026-11-12 -->

### mem-1794518972-fc67
> ralph-core auto_commit_changes (git_ops.rs) stages git add --update plus untracked files filtered by is_run_artifact; when adding new Ralph-written runtime paths, extend that filter (and is_durable_ralph_artifact for files meant to be committed), because user repos' .gitignore does not cover Ralph's layout
<!-- tags: git, auto-commit | created: 2026-11-12 -->

### mem-1794518372-080b
> When a ralph-cli integration test spawns the ralph binary, use tests/common::ralph_command() (strips inherited RALPH_* vars), because a child inheriting the live loop's RALPH_EVENTS_FILE/RALPH_WORKSPACE_ROOT writes into that loop; integration_env_isolation_guard.rs fails on direct Command::new(CARGO_BIN_EXE_ralph). For in-process ralph-core tests that persist state, set config.core.workspace_root to a tempdir since CoreConfig::default() reads RALPH_WORKSPACE_ROOT. scripts/check-test-event-isolation.sh is the end-to-end check.
<!-- tags: testing, events, cli | created: 2026-11-12 -->

## Decisions

## Fixes

## Context
