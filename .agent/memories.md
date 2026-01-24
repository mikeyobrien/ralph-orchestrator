# Memories

## Patterns

## Decisions

## Fixes

## Context

### mem-1769281121-c2cc
> integration test: MockLoopManager E2E tests created in tests/integration.rs under mock_loop_tests module, gated by #[cfg(feature="test-mode")]. Tests cover: full lifecycle (start/verify/stop), config-not-found error, stop-nonexistent error, and multiple concurrent loops. Uses direct MockLoopManager injection via create_mock_loop_test_server helper.
<!-- tags:  | created: 2026-01-24 -->
