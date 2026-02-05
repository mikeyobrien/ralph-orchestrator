# Hats Product Backlog

> Formerly ralph-orchestrator. Managed by heartbeat PM/TPM cycles.
> See ~/notes/requests/2026-02-04-hats-3mo-plan.md for full 3-month plan.
> See ~/notes/research/hats-rename-plan.md for rename scope.

## In Progress

### Bug Fixes (P0)

- [x] **#157 Infinite loop when agent fails to emit events** -- consecutive_fallbacks counter oscillated between 0 and 1 instead of reaching MAX_FALLBACK_ATTEMPTS. Fixed by resetting only on real JSONL events. _(2026-02-05)_
- [ ] **default_publishes dead code** -- `record_event_count()` and `check_default_publishes()` defined in EventLoop but never called by loop runner. The `default_publishes` config field has no effect. **P1**

### Dark Factory / BDD (P0)

- [x] **BDD preset** -- presets/bdd.yml with spec_writer, implementer, verifier hats. Embedded in binary. _(2026-02-05)_
- [x] **Proof artifact module** -- hats-core/src/proof.rs: ProofArtifact struct, write/read/list. 10 tests. _(2026-02-05)_
- [x] **Proof config** -- features.proof.enabled in HatsConfig. BDD preset enables by default. _(2026-02-05)_
- [x] **Wire proof into completion** -- loop_completion.rs generates proofs on BDD loop finish. 13 integration tests. _(2026-02-05)_
- [ ] **Gherkin parser** -- Parse .feature files natively (currently agent-driven via prompt). **P1**
- [ ] **ProofData population** -- Wire test result counts from loop events into ProofData (currently None). **P1**
- [ ] **Proof CLI command** -- `hats proofs list`, `hats proofs show <id>`. **P2**
- [ ] **bdd-preset.feature acceptance tests** -- Map 14 Gherkin scenarios to runnable tests. **P2**

## Ready (Prioritized)

### Rename (P0 -- blocks everything)

- [ ] **Reserve npm @hats/cli** -- Publish placeholder 0.0.1. Secures the name. **P0** (GATED: publish)
- [ ] **Reserve crates.io hats-cli** -- Publish placeholder 0.0.1. **P0** (GATED: publish)
- [ ] **Reserve crates.io hats-core, hats-proto, hats-adapters, hats-tui, hats-bench, hats-e2e, hats-telegram** -- All confirmed available. **P0** (GATED: publish)
- [ ] **Rename GitHub repo** -- mikeyobrien/ralph-orchestrator -> mikeyobrien/hats. Auto-redirects old URLs. **P0** (GATED: external)
- [ ] **Publish new packages** -- npm @hats/cli, crates.io all crates, PyPI hats-cli. **P0** (GATED: publish)
- [ ] **Deprecation notices** -- Publish final version of @ralph-orchestrator/ralph-cli that prints "ralph is now hats". Same for crates.io and PyPI. **P0** (GATED: publish)
- [ ] **Update homebrew formula** -- New tap or update existing. **P0** (GATED: external)

### Landing Page (P1)

- [ ] **hats.sh DNS propagation** -- Verify nameservers are set at registrar. Monitor until live. **P0**
- [ ] **hats.sh SSL** -- Verify cert provisioning after DNS propagates. **P1**

### Communication (P1)

- [x] **Blog post: "Ralph is now Hats"** -- Draft at ~/notes/drafts/hats-rename-blog.md. **P1** _(2026-02-05)_
- [x] **X announcement thread** -- Draft at ~/notes/drafts/hats-rename-x-thread.md. **P1** _(2026-02-05)_
- [x] **Migration guide** -- In-repo at docs/migration/ralph-to-hats.md. **P1** _(2026-02-05)_
- [x] **Geoffrey Huntley courtesy DM** -- Draft at ~/notes/drafts/huntley-dm.md. **P1** _(2026-02-05)_
- [ ] **PR to awesome-claude-code** -- Update link to new repo. **P1** (GATED: external)

### Integration (P1)

- [ ] **rho run command** -- Spawns hats loops from rho. Spec needed. **P1**
- [ ] **Cross-link landing pages** -- runrho.dev mentions hats, hats.sh mentions rho. **P1**
- [ ] **Shared auth design** -- One account for both hats.sh and runrho.dev. Spec needed. **P2**

### Hats Cloud (P2 -- Month 2)

- [ ] **API spec** -- POST/GET/DELETE /v1/loops. OpenAPI-style. **P2**
- [ ] **Architecture doc** -- Workers + Hetzner + R2 + D1. **P2**
- [ ] **Pricing page copy** -- Free/Pro/Team tiers. **P2**

## Icebox

- [ ] **Preset marketplace** -- Premium presets with revenue share. Needs user base. **P3**
- [ ] **Team dashboard** -- Shared cost tracking across developers. **P3**
- [ ] **CI/CD templates** -- GitHub Actions workflow for hats cloud. **P3**
- [ ] **Bundle pricing** -- hats + rho Pro at $25/mo. Needs both cloud products live. **P3**

## Done

- [x] **#157 Infinite loop fix** -- consecutive_fallbacks counter reset bug in loop_runner.rs. _(2026-02-05)_
- [x] **Migration guide** -- In-repo at docs/migration/ralph-to-hats.md. Covers CLI, config, env vars, state dir, CI/CD. _(2026-02-05)_
- [x] **Blog post draft** -- ~/notes/drafts/hats-rename-blog.md. _(2026-02-05)_
- [x] **X announcement thread** -- ~/notes/drafts/hats-rename-x-thread.md. 5-tweet thread. _(2026-02-05)_
- [x] **Geoffrey Huntley courtesy DM** -- ~/notes/drafts/huntley-dm.md. _(2026-02-05)_
- [x] **Fix flaky benchmark test** -- bench_get_for_topic_baseline threshold 10K->50K ns for ARM debug builds. _(2026-02-05)_
- [x] **Clean .ralph/ from git** -- Added .ralph/ to .gitignore, removed tracked runtime artifacts. _(2026-02-05)_
- [x] **Global codebase rename** -- ralph -> hats in all crate names, Cargo.toml, binary name, config files. 585 files changed. 725 tests pass. _(2026-02-05)_
- [x] **Config fallback** -- hats reads ralph.yml if hats.yml not found. HATS_* env vars read RALPH_* as deprecated. hats_env_var() helper. _(2026-02-05)_
- [x] **Update cargo-dist config** -- npm scope @hats, binary name hats. _(2026-02-05, done as part of global rename)_
- [x] **BDD preset + proof infrastructure** -- presets/bdd.yml, proof.rs, ProofConfig, wired into loop completion. 23 new tests. _(2026-02-05)_
- [x] **hats.sh landing page** -- Built and deployed on Cloudflare Pages. _(2026-02-04)_

---

## Priority Key
- **P0**: Blocking other work or shipping
- **P1**: Important, do this week
- **P2**: Next up after P1s clear
- **P3**: Future / needs validation

## GitHub Issues Triage (2026-02-05)

| Issue | Status | Rename Impact |
|-------|--------|--------------|
| #157 Infinite loop with v2.4.4 | **Fixed** (local) | None -- loop_runner.rs fix |
| #148 RFC: CLI/TUI guidance | Open | Examples reference `ralph` CLI -- fix in docs |
| #135 docs site: llms.txt | Open | Docs move to hats.sh |
| #128 broken ralph completions zsh | Open | Moot after rename -- binary is now `hats` |
| #127 MCP tools auth | Open | No impact |
| #120 Windows support | Open | No impact |
| #106 grepai feature req | Open | No impact |
| #27 MCP support | Open | No impact |
