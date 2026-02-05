# Hats Product Backlog

> Formerly hats. Managed by heartbeat PM/TPM cycles.
> See ~/notes/requests/2026-02-04-hats-3mo-plan.md for full 3-month plan.
> See ~/notes/research/hats-rename-plan.md for rename scope.

## In Progress

_(nothing yet -- TPM will populate on first heartbeat)_

## Ready (Prioritized)

### Rename (P0 -- blocks everything)

- [ ] **Reserve npm @hats/cli** -- Publish placeholder 0.0.1. Secures the name. **P0**
- [ ] **Reserve crates.io hats-cli** -- Publish placeholder 0.0.1. **P0**
- [ ] **Reserve crates.io hats-core, hats-proto, hats-adapters, hats-tui, hats-bench, hats-e2e, hats-telegram** -- All confirmed available. **P0**
- [ ] **Rename GitHub repo** -- mikeyobrien/hats -> mikeyobrien/hats. Auto-redirects old URLs. **P0**
- [ ] **Global codebase rename** -- hats -> hats in all crate names, Cargo.toml, binary name, config files. **P0**
- [ ] **Config fallback** -- hats reads hats.yml if hats.yml not found. HATS_* env vars read HATS_* as deprecated. **P0**
- [ ] **Publish new packages** -- npm @hats/cli, crates.io all crates, PyPI hats-cli. **P0**
- [ ] **Deprecation notices** -- Publish final version of @hats/hats-cli that prints "hats is now hats". Same for crates.io and PyPI. **P0**
- [ ] **Update homebrew formula** -- New tap or update existing. **P0**
- [ ] **Update cargo-dist config** -- npm scope @hats, binary name hats. **P0**
- [ ] **Deploy docs to hats.sh** -- mkdocs site, same content, new branding. **P1**

### Landing Page (P1)

- [ ] **hats.sh DNS propagation** -- Verify nameservers are set at registrar. Monitor until live. **P0**
- [ ] **hats.sh SSL** -- Verify cert provisioning after DNS propagates. **P1**

### Communication (P1)

- [ ] **Blog post: "Hats is now Hats"** -- Draft at ~/notes/drafts/hats-rename-blog.md. **P1**
- [ ] **X announcement thread** -- Draft at ~/notes/drafts/hats-rename-x-thread.md. **P1**
- [ ] **Migration guide** -- Draft at ~/notes/drafts/hats-migration-guide.md. **P1**
- [ ] **Geoffrey Huntley courtesy DM** -- Draft at ~/notes/drafts/huntley-dm.md. **P1**
- [ ] **PR to awesome-claude-code** -- Update link to new repo. **P1**

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

_(TPM logs completed items here with dates)_

---

## Priority Key
- **P0**: Blocking other work or shipping
- **P1**: Important, do this week
- **P2**: Next up after P1s clear
- **P3**: Future / needs validation
