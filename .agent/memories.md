# Memories

## Patterns

### mem-1769047449-ae29
> E2E Tier 7 scenarios (IncrementalFeatureScenario, ChainedLoopScenario) test memory+tasks working together across multiple loops. Located in crates/ralph-e2e/src/scenarios/incremental.rs
<!-- tags: e2e, testing, memories, tasks | created: 2026-01-22 -->

## Decisions

### mem-1769053131-adaf
> Ralph should never close a task unless it's actually been completed. Tasks must have verified completion evidence before closure.
<!-- tags: tasks, workflow, policy | created: 2026-01-22 -->

## Fixes

### mem-1769047926-2118
> Memory CLI output improvements: Use relative dates (today/yesterday/N days ago), longer content previews (50 chars), cyan colored tags, boxed detail views with visual separators. Follow clig.dev CLI UX guidelines: human-first output with JSON fallback, colors disabled for non-TTY.
<!-- tags: cli, ux, memory | created: 2026-01-22 -->

## Context

### mem-1769055756-489a
> confession: objective=validate build.done event, met=Yes, evidence=cargo build pass, 344 tests pass, clippy clean (only deprecated lint)
<!-- tags: confession | created: 2026-01-22 -->

### mem-1769055680-0faf
> Build validation complete: cargo build passes, all 344 tests pass (135 adapters, 110 core, etc.), smoke tests pass (12 smoke_runner + 9 kiro), clippy clean with only deprecated lint warning
<!-- tags: build, validation, release | created: 2026-01-22 -->

### mem-1769046701-9e40
> Ralph uses Rust workspace with crates in crates/ directory. Examples go in crates/ralph-cli/examples/
<!-- tags: structure | created: 2026-01-22 -->
