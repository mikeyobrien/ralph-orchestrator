# Review Notes

## Consistency Check

### Observations
- The existing `AGENTS.md` is titled `# CLAUDE.md` but the file is named `AGENTS.md` — this is a naming inconsistency that should be resolved in the consolidated output.
- The legacy Node.js backend (`backend/ralph-web-server/`) coexists with the Rust-native `ralph-api` crate. The CLI has `dev:legacy-server` and `--legacy-node-api` flags indicating active migration. Documentation should clarify which is primary.
- `ralph.yml` in the repo root uses `cli.backend: pi` but the README examples show `ralph init --backend claude`. This is expected (repo-specific config vs. user-facing docs) but worth noting.
- The `AGENTS.md` references `backend/` as "Web server (@ralph-web/server) - Fastify + tRPC + SQLite" but the primary API is now `ralph-api` (Rust/Axum). The consolidated file should reflect the current state.

### No Issues Found
- Crate dependency graph is clean (no cycles, `ralph-proto` is the leaf)
- Config v1/v2 compatibility is well-documented in code comments
- Event topic naming is consistent across hat configs and code
- Persistence formats are consistent (JSONL for append-only, JSON for snapshots, Markdown for human-readable)

## Completeness Check

### Well-Documented Areas
- Event loop orchestration flow
- Hat system and pub/sub routing
- CLI commands and RPC API methods
- Task and memory data models
- Parallel loop architecture
- Hook lifecycle
- Telegram integration

### Areas Lacking Detail
1. **Backpressure gate implementation**: The config format is documented but the runtime execution path (how gates are invoked during the loop, retry behavior) is not deeply covered in the source analysis.
2. **Stream parsing internals**: `ClaudeStreamParser` and `PiStreamParser` handle vendor-specific streaming formats. The exact parsing logic and error recovery are complex but not fully documented.
3. **Config resolution precedence**: The CLI supports `-c` flag, default `ralph.yml`, and preset merging. The exact precedence and merge semantics could use more documentation.
4. **Collection builder UI**: The ReactFlow-based visual hat collection editor in the frontend is mentioned but its data flow and persistence model aren't deeply covered.
5. **MCP server tool catalog**: The `ralph-api` MCP server exposes RPC methods as MCP tools. The exact tool catalog generation and schema validation logic is complex.
6. **Nix/devenv setup**: `flake.nix` and `devenv.nix` exist for reproducible environments but their configuration isn't documented in the summary.

### Language/Tool Gaps
- No gaps: the codebase is Rust + TypeScript, both fully analyzable.

## Recommendations

1. The consolidated `AGENTS.md` should fix the `# CLAUDE.md` title and reflect the current architecture (ralph-api as primary, legacy backend as deprecated).
2. Consider adding a `config-resolution.md` to the docs site explaining config file discovery, precedence, and preset merging.
3. The `scripts/` directory contains important CI gates (`hooks-bdd-gate.sh`, `hooks-mutation-gate.sh`, `sync-embedded-files.sh`) that are referenced in CI but not prominently documented.
