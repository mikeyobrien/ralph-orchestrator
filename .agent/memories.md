# Memories

## Patterns

### mem-1769391452-a0b9
> Added task breakdown and state management guidance to Ralph's core prompt. Also added context file listing (.agent/*.md) to help agents discover research notes.
<!-- tags: prompt, ux, guidance | created: 2026-01-26 -->

### mem-1769102069-37b5
> Coverage reporting configured with cargo-tarpaulin. Run 'cargo tarpaulin --out Html --output-dir coverage --skip-clean' to generate reports. Badge shows 65% coverage in README.
<!-- tags: testing, coverage, ci | created: 2026-01-22 -->

### mem-1769098040-8c4a
> Added three prompt enhancements to core_prompt() in hatless_ralph.rs: (1) Task breakdown guidance explaining when/how to create tasks, (2) State management guidance distinguishing memories (persistent) from context files (session-specific), (3) Auto-listing of .agent/*.md files for context discovery. Implementation uses regular string literals with \n escapes to avoid raw string syntax issues with backticks.
<!-- tags: prompts, ux, guidance | created: 2026-01-22 -->

### mem-1769047449-ae29
> E2E Tier 7 scenarios (IncrementalFeatureScenario, ChainedLoopScenario) test memory+tasks working together across multiple loops. Located in crates/ralph-e2e/src/scenarios/incremental.rs
<!-- tags: e2e, testing, memories, tasks | created: 2026-01-22 -->

## Decisions

### mem-1769058662-9978
> Created comprehensive MkDocs documentation site for v2 Rust implementation. Uses Material theme with deep purple/amber color scheme, Inter font. Includes: Getting Started, Concepts (Tenets, Hats, Events, Memories, Backpressure), User Guide (Config, Presets, CLI, Backends), Advanced (Architecture, Testing, Diagnostics), API Reference for all 5 main crates, Examples, Contributing guide, Reference section.
<!-- tags: docs, mkdocs, architecture | created: 2026-01-22 -->

### mem-1769053131-adaf
> Ralph should never close a task unless it's actually been completed. Tasks must have verified completion evidence before closure.
<!-- tags: tasks, workflow, policy | created: 2026-01-22 -->

## Fixes

### mem-1769390410-9433
> Improved task list CLI: Added --days and --limit flags to limit output, and added colors/sorting for better readability.
<!-- tags: cli, ux, task | created: 2026-01-26 -->

### mem-1769047926-2118
> Memory CLI output improvements: Use relative dates (today/yesterday/N days ago), longer content previews (50 chars), cyan colored tags, boxed detail views with visual separators. Follow clig.dev CLI UX guidelines: human-first output with JSON fallback, colors disabled for non-TTY.
<!-- tags: cli, ux, memory | created: 2026-01-22 -->

## Context

### mem-1769391457-53da
> Added end-to-end integration tests for event isolation in crates/ralph-cli/tests/integration_events_isolation.rs to verify fix for issue #82.
<!-- tags: testing, events, integration | created: 2026-01-26 -->

### mem-1769098088-0181
> confession: objective=Add task breakdown guidance, state management guidance, and context file listing to prompt generation, met=Yes, evidence=crates/ralph-core/src/hatless_ralph.rs:244-318, cargo build pass, 347 tests pass
<!-- tags: confession | created: 2026-01-22 -->

### mem-1769087132-aa84
> confession: verify=grep for '<event topic' in presets/, confidence=90 for opencode fix, 40 for preset format fix
<!-- tags: confession | created: 2026-01-22 -->

### mem-1769087128-9b65
> confession: objective=Fix issue 89 (update presets to use ralph emit), met=Partial, evidence=confession-loop.yml:56,57,108 still uses deprecated <event topic> XML format
<!-- tags: confession | created: 2026-01-22 -->

### mem-1769055756-489a
> confession: objective=validate build.done event, met=Yes, evidence=cargo build pass, 344 tests pass, clippy clean (only deprecated lint)
<!-- tags: confession | created: 2026-01-22 -->

### mem-1769055680-0faf
> Build validation complete: cargo build passes, all 344 tests pass (135 adapters, 110 core, etc.), smoke tests pass (12 smoke_runner + 9 kiro), clippy clean with only deprecated lint warning
<!-- tags: build, validation, release | created: 2026-01-22 -->

### mem-1769046701-9e40
> Ralph uses Rust workspace with crates in crates/ directory. Examples go in crates/ralph-cli/examples/
<!-- tags: structure | created: 2026-01-22 -->
