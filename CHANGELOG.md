# Changelog

All notable changes to ralph-orchestrator are documented here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [2.8.0] - 2026-03-10

### Added

- `ralph mcp serve` for exposing Ralph as a workspace-scoped MCP server over stdio.
- User-scoped default config discovery and support for per-user Ralph defaults.
- TUI update availability notices in the header.
- Human guidance can now trigger a clean restart request flow.

### Changed

- Consolidated the core preset set around the maintained workflows and refreshed preset docs, examples, and evaluation tooling.
- Refined PDD and code-task guidance to reduce Ralph-specific noise and improve handoff quality.

### Fixed

- Hardened multi-hat preset event contracts, late-event recovery, active hat display, and downstream debug/review handoffs.
- Preserved runtime limits from core config when using hats.
- Fixed headless loop runner backend selection.
- Made restart resumption use the required single-command shell flow and added contract coverage for it.

## [2.7.0] - 2026-03-06

### Added

- Per-project orchestrator lifecycle hooks v1.
- `kiro-acp` backend with ACP executor support.
- Subprocess TUI over JSON-RPC stdin/stdout.
- Improved TUI tool rendering for ACP-backed flows.

### Changed

- Simplified internal code paths by removing redundant clones and deduplicating `now_ts`.
- Replaced deprecated `Duration` method usage with `from_secs`.
- `ralph plan` PDD SOP now syncs from the canonical `strands-agents/agent-sop` upstream source, with a small Ralph-specific loop handoff addendum.
- Added embedded asset sync, check, and upstream refresh helpers for SOP maintenance.
- Unified and modernized preset documentation.
- Added `llms.txt` map generation and CI validation.
- Hardened web `tsx` preflight behavior and added funding metadata.

### Fixed

- Avoid self-lock contention in subprocess TUI mode.
- Accumulate Pi text deltas into flowing paragraphs in the TUI.
- Clean up zombie worktree loops more reliably.
- Fix ACP orphaned processes, garbled TUI output, and missing tool details.
- Resolve clippy issues and missing struct fields.

## [2.6.0] - 2026-02-25

### Added

- Rust RPC v1 control plane and web client migration to the new RPC contract.
- Shell completions support for `ralph` CLI.
- `fresh-eyes` preset with enforced review passes.

### Fixed

- Hat display no longer gets stuck on the previous iteration's hat.
- UTF-8 safe truncation to prevent panics on multi-byte characters.
- Hat-level backend shorthand `args` is honored for custom hats (including OpenCode).
- Deprecated `project.*` config keys now fail fast with a clear migration hint to `core.*`.

## [2.5.1] - 2026-02-14

### Changed

- Version bump for 2.5.1 release.

## [2.3.0] - 2025-01-28

### Added

- **Web Dashboard (Alpha)**: Full-featured web UI for monitoring and managing Ralph orchestration loops
  - React + Vite + TailwindCSS frontend with Fastify + tRPC + SQLite backend
  - `ralph web` command to launch both servers (backend:3000, frontend:5173)
  - Preflight checks and auto-install for fresh installs
  - Port conflict detection, labeled output, and automatic browser open
  - Node 22 pinned for backend dev with tsc+node compilation
- **Hats CLI**: Topology visualization and AI-powered diagrams (`ralph hats`)
- **Event Publishing Guide**: Skip topology display when a hat is already active
- **Parallel config gate**: `features.parallel` config option to control worktree spawning
- **Per-hat backend args**: `args` support in hat-level backend configurations
- **New presets**: Additional presets and improved workflow patterns
- **Documentation**: Reorganized docs with governance files and enhanced README

### Fixed

- Honor hat-level backend configuration and args overrides
- Backend dev workflow uses tsc+node instead of ts-node

## [2.2.5] - 2025-01-17

### Added

- Loop merge command (`ralph loop merge`) and custom backend args
- Config override support for core fields via CLI
- Mock adapter for cost-free E2E testing
- CI: Run mock E2E tests on every PR/push

### Fixed

- CI workaround for claude-code-action fork PR bug
- CI write permissions for handling fork PRs

## [2.2.4] - 2025-01-14

### Fixed

- TUI hang under npx process group
- Clarify cost display as estimate for subscription users

## [2.2.3] - 2025-01-12

### Added

- Multi-loop concurrency via git worktrees
- OBJECTIVE section in prompts to prevent goal drift
- Claude Code GitHub workflow

### Fixed

- UTF-8 truncation panics in event output

### Changed

- Updated preset configurations

## [2.2.2] - 2025-01-10

### Fixed

- Signal handler registration moved after TUI initialization
- Docs: markdown attribute on divs for badge rendering

## [2.2.1] - 2025-01-08

### Added

- CLI ergonomics: backend flag, builtin presets, URL configs
- Comprehensive MkDocs documentation site for v2

### Fixed

- TUI: require stdin to be terminal for TUI enablement
- MkDocs strict build failures
- Confession-loop preset updated to use `ralph emit` command

### Changed

- Modularized codebase and fixed TUI mode

[Unreleased]: https://github.com/mikeyobrien/ralph-orchestrator/compare/v2.8.0...HEAD
[2.8.0]: https://github.com/mikeyobrien/ralph-orchestrator/compare/v2.7.0...v2.8.0
[2.7.0]: https://github.com/mikeyobrien/ralph-orchestrator/compare/v2.6.0...v2.7.0
[2.6.0]: https://github.com/mikeyobrien/ralph-orchestrator/compare/v2.5.1...v2.6.0
[2.5.1]: https://github.com/mikeyobrien/ralph-orchestrator/compare/v2.5.0...v2.5.1
[2.3.0]: https://github.com/mikeyobrien/ralph-orchestrator/compare/v2.2.5...v2.3.0
[2.2.5]: https://github.com/mikeyobrien/ralph-orchestrator/compare/v2.2.4...v2.2.5
[2.2.4]: https://github.com/mikeyobrien/ralph-orchestrator/compare/v2.2.3...v2.2.4
[2.2.3]: https://github.com/mikeyobrien/ralph-orchestrator/compare/v2.2.2...v2.2.3
[2.2.2]: https://github.com/mikeyobrien/ralph-orchestrator/compare/v2.2.1...v2.2.2
[2.2.1]: https://github.com/mikeyobrien/ralph-orchestrator/compare/v2.2.0...v2.2.1
