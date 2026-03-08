# Research: Security Considerations for URL Imports

## Critical Insight

This is NOT a typical dependency management problem. Importing a hat = importing natural-language instructions that an AI agent follows with tool access (filesystem, git, shell). The attack surface includes prompt injection, not just code injection. OWASP ranks prompt injection as the #1 critical vulnerability for LLM applications.

## 1. Trust Model

**Recommendation: URL imports are Phase 2. Start with local file imports only.**

If/when URL imports are added:
- Default deny: `allow_remote_imports: true` must be explicit in config
- Mandatory domain allowlist
- HTTPS only — reject `http://` unconditionally
- No transitive imports: fetched files cannot themselves contain `import:`

Precedents: Deno `--allow-net`, Go module proxy, GitHub Actions SHA pinning

## 2. Caching

**Recommendation: Content-addressable local cache.**

```
~/.ralph/cache/hats/
  sha256-a1b2c3d4.yml          # content-addressable
  url-index.json               # URL → { sha256, fetched_at, etag }
```

- Cache-first by default, re-fetch only with `ralph run --refresh-imports`
- Individual files capped at 256 KB
- Total cache capped at 100 MB with LRU eviction

Precedents: npm content-addressable cache, Cargo registry cache, Deno deps cache

## 3. Network Failure Handling

| State | Lockfile exists | Lockfile absent |
|-------|----------------|-----------------|
| Network available | Fetch, verify hash matches lockfile | Fetch, create lockfile entry |
| Network unavailable | Use cache (hash must match) | **Hard error** |
| Cache miss + no network | **Hard error** | **Hard error** |

No silent degradation. Clear error messages with remediation steps.

## 4. TOCTOU / Integrity

**Recommendation: Mandatory lockfile with SHA-256 hashes.**

```yaml
# .ralph/imports.lock
version: 1
imports:
  "https://hats.ralph.dev/reviewer/v2.yml":
    sha256: "a1b2c3d4e5f6..."
    fetched_at: "2026-02-28T10:30:00Z"
```

- Trust-on-first-use (TOFU) model
- Lockfile committed to version control
- `--frozen-imports` flag for CI (fail if any import needs fetching)
- `ralph imports update` for explicit refresh with diff display

Precedents: Go go.sum, Deno deno.lock, npm lockfiles, SRI hashes

## 5. Content Validation

Validation pipeline (applied before any import is accepted):
1. **Size check** — reject > 256 KB
2. **YAML parse** — reject invalid YAML
3. **Schema validate** — deserialize into HatConfig; reject unknown fields
4. **Depth check** — reject nesting > 10 (prevents billion-laughs)
5. **Field limits** — instructions: max 50K chars, triggers: max 50 entries
6. **No anchors from remote** — reject YAML anchors/aliases (entity expansion)
7. **No YAML tags** — reject `!!` tags

## 6. Supply Chain Attack Scenarios

| Attack | Mitigation |
|--------|-----------|
| Prompt override in instructions | Human review, vendor directory |
| Subtle bias injection | `ralph imports audit` shows full content |
| Time-delayed attack (serve benign then swap) | Hash-pinned lockfile |
| Domain takeover | Domain monitoring, vendor-first approach |
| Typosquatting URLs | Domain allowlist |

## 7. Strongest Recommendation: Vendor-First Approach

```bash
ralph imports vendor  # downloads all imported hats to .ralph/vendor/hats/
```

- Actual content visible in repo, reviewed in PRs
- No network access needed at runtime
- Git history shows exactly when definitions changed
- AI agent instructions never sourced from URL at runtime

**URL imports as bootstrap convenience; production use should vendor into the repo.**

## Phase Recommendation

- **Phase 1 (this spec):** Local file imports only. No URL support. No caching/lockfile complexity.
- **Phase 2 (future):** URL imports with full security infrastructure (lockfile, cache, vendor, allowlist).

This significantly reduces scope while delivering the core value: cross-preset hat reuse.
