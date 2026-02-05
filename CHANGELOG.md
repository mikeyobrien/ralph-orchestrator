# Changelog

All notable changes to hats are documented here.

## [2.3.0] - 2025-01-28

### Added

- **Web Dashboard (Alpha)**: Full-featured web UI for monitoring and managing Hats orchestration loops
  - React + Vite + TailwindCSS frontend with Fastify + tRPC + SQLite backend
  - `hats web` command to launch both servers (backend:3000, frontend:5173)
  - Preflight checks and auto-install for fresh installs
  - Port conflict detection, labeled output, and automatic browser open
  - Node 22 pinned for backend dev with tsc+node compilation
- **Hats CLI**: Topology visualization and AI-powered diagrams (`hats hats`)
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

- Loop merge command (`hats loop merge`) and custom backend args
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
- Confession-loop preset updated to use `hats emit` command

### Changed

- Modularized codebase and fixed TUI mode

[2.3.0]: https://github.com/mikeyobrien/hats/compare/v2.2.5...v2.3.0
[2.2.5]: https://github.com/mikeyobrien/hats/compare/v2.2.4...v2.2.5
[2.2.4]: https://github.com/mikeyobrien/hats/compare/v2.2.3...v2.2.4
[2.2.3]: https://github.com/mikeyobrien/hats/compare/v2.2.2...v2.2.3
[2.2.2]: https://github.com/mikeyobrien/hats/compare/v2.2.1...v2.2.2
[2.2.1]: https://github.com/mikeyobrien/hats/compare/v2.2.0...v2.2.1
