# Documentation Stack Research

**Date**: 2026-01-23
**Context**: Research modern documentation stacks compatible with GitHub Pages for potential migration from MkDocs Material.

## Executive Summary

**Recommendation: Migrate to Astro Starlight**

MkDocs is unmaintained since August 2024 and is a supply chain risk. Material for MkDocs entered maintenance mode in November 2025 with the team shifting to Zensical. Starlight offers the best balance of performance, GitHub Pages compatibility, modern DX, and future-proofing.

## Critical Finding: MkDocs Maintenance Status

⚠️ **MkDocs is unmaintained since August 2024**

- No releases in over a year
- Accumulating unresolved issues and PRs
- Material for MkDocs team has cut ties to MkDocs as a dependency
- Material for MkDocs v9.7.0 is the final version, now in maintenance mode
- Team created Zensical as replacement (not yet mature for adoption)

**Source**: [Material for MkDocs Blog - Zensical](https://squidfunk.github.io/mkdocs-material/blog/2025/11/05/zensical/)

## Current Setup Analysis

**Location**: `mkdocs.yml` with Material theme
**Docs Count**: ~75 markdown files in `docs/`
**Features Used**:
- Material theme with dark/light mode
- Deep purple/amber color scheme
- Inter font, JetBrains Mono for code
- Navigation tabs, sections, instant loading
- Search with highlighting and suggestions
- Code copy, annotations, syntax highlighting
- Mermaid diagrams
- Versioning via mike
- Content tabs, admonitions

## Framework Comparison

### 1. Astro Starlight ⭐ **Recommended**

**Overview**: Full-featured docs framework built on Astro.

| Aspect | Details |
|--------|---------|
| Build Speed | Excellent - Astro's partial hydration |
| GitHub Pages | Native support, official deploy guide |
| Search | Built-in Pagefind (fast, local) |
| Dark Mode | Built-in, follows system preference |
| i18n | Built-in internationalization |
| Versioning | Supported via Astro recipes |
| Framework | Any (React, Vue, Svelte, Solid) |
| Maturity | Production-ready since 2023 |

**Pros**:
- Framework-agnostic (write components in React, Vue, Svelte, etc.)
- Excellent accessibility by default
- One-line Tailwind integration
- Strong community momentum
- Stellar performance via Astro
- Markdown-first with MDX support

**Cons**:
- Newer than Docusaurus (started 2023 vs 2017)
- Fewer plugins than Docusaurus ecosystem

**GitHub Pages Deploy**: Official action available, minimal config.

**Migration Effort**: Medium - content portable, config needs rewrite.

**Sources**:
- [Starlight Official Site](https://starlight.astro.build/)
- [GitHub - withastro/starlight](https://github.com/withastro/starlight)
- [Astro GitHub Pages Guide](https://docs.astro.build/en/guides/deploy/github/)

---

### 2. Docusaurus 3.x

**Overview**: Meta's battle-tested docs framework built on React.

| Aspect | Details |
|--------|---------|
| Build Speed | Moderate - React SSG overhead |
| GitHub Pages | Native support |
| Search | Algolia DocSearch v4 with AI |
| Dark Mode | Built-in |
| i18n | Built-in (CrowdIn integration) |
| Versioning | First-class support |
| Framework | React only |
| Maturity | Very mature (since 2017) |

**Pros**:
- Most mature option, huge ecosystem
- First-class versioning support
- MDX for interactive components
- Used by React Native, Redux, Supabase
- AI-powered search with DocSearch v4

**Cons**:
- React-only (can't use Vue/Svelte components)
- Tightly coupled to Infima CSS (hard to customize)
- Slower builds for large sites
- No Tailwind integration without hacks

**Migration Effort**: Medium - good docs migration tooling.

**Sources**:
- [Docusaurus Official](https://docusaurus.io/)
- [Docusaurus 3.9 AI Search - InfoQ](https://www.infoq.com/news/2025/10/docusaurus-3-9-ai-search/)
- [GitHub - facebook/docusaurus](https://github.com/facebook/docusaurus)

---

### 3. VitePress

**Overview**: Vite & Vue powered SSG optimized for docs.

| Aspect | Details |
|--------|---------|
| Build Speed | Excellent - Vite's HMR |
| GitHub Pages | Supported |
| Search | Built-in local search |
| Dark Mode | Built-in |
| i18n | Supported |
| Versioning | Manual (via folder structure) |
| Framework | Vue only |
| Maturity | Mature (Vue team maintained) |

**Pros**:
- Lightning-fast dev server (Vite HMR)
- Clean, modern default theme
- Vue components in markdown
- Powers Vue.js official docs
- Minimal and focused

**Cons**:
- Vue-only for customization
- Less feature-rich out of box than Starlight/Docusaurus
- Manual versioning setup

**Migration Effort**: Low-Medium - very markdown-focused.

**Sources**:
- [VitePress Official](https://vitepress.dev/)
- [GitHub - vuejs/vitepress](https://github.com/vuejs/vitepress)

---

### 4. Nextra

**Overview**: Next.js based docs framework.

| Aspect | Details |
|--------|---------|
| Build Speed | Good (Next.js optimized) |
| GitHub Pages | Supported (static export) |
| Search | Built-in Pagefind |
| Dark Mode | Built-in |
| i18n | Built-in |
| Versioning | Supported |
| Framework | React (Next.js) |
| Maturity | Mature, Nextra 4 released |

**Pros**:
- Full Next.js power (SSR, ISR, RSC)
- MDX 3 support
- Used by Node.js, React, Tailwind docs
- Hybrid rendering flexibility

**Cons**:
- Next.js dependency (heavier than needed for docs)
- Less documentation-focused than Starlight/Docusaurus
- Breaking changes in Nextra 4

**Migration Effort**: Medium.

**Sources**:
- [Nextra Official](https://nextra.site/)
- [Nextra 4 Migration Guide - The Guild](https://the-guild.dev/blog/nextra-4)

---

### 5. Zensical (Future Option)

**Overview**: Material for MkDocs team's new SSG, direct replacement.

| Aspect | Details |
|--------|---------|
| Build Speed | Faster than MkDocs |
| GitHub Pages | Will support |
| mkdocs.yml | Native compatibility |
| Maturity | **Too new (Nov 2025)** |

**Note**: While Zensical can read `mkdocs.yml` natively and promises a smooth migration path, it was only announced in November 2025. Wait 6-12 months for it to mature before considering.

**Source**: [Zensical Announcement](https://squidfunk.github.io/mkdocs-material/blog/2025/11/05/zensical/)

---

## Feature Comparison Matrix

| Feature | MkDocs Material | Starlight | Docusaurus | VitePress | Nextra |
|---------|-----------------|-----------|------------|-----------|--------|
| **Maintained** | ⚠️ Maintenance mode | ✅ Active | ✅ Active | ✅ Active | ✅ Active |
| **Build Speed** | Slow | Fast | Moderate | Fastest | Fast |
| **GitHub Pages** | ✅ | ✅ | ✅ | ✅ | ✅ |
| **Search** | Plugin | Built-in | Algolia AI | Built-in | Pagefind |
| **Dark Mode** | ✅ | ✅ | ✅ | ✅ | ✅ |
| **i18n** | Plugin | ✅ Built-in | ✅ Built-in | ✅ | ✅ |
| **Versioning** | mike | Recipes | ✅ First-class | Manual | ✅ |
| **Mermaid** | ✅ | ✅ | ✅ | ✅ | ✅ |
| **Code Blocks** | ✅ | ✅ | ✅ | ✅ | ✅ |
| **Tailwind** | ❌ Hard | ✅ One-line | ❌ Hard | ✅ | ✅ |
| **Custom Components** | Python | Any framework | React only | Vue only | React |

## Migration Considerations

### Content Portability
All frameworks use Markdown/MDX. The ~75 docs files will port with minimal changes:
- Frontmatter syntax varies slightly
- Admonitions have different syntax per framework
- Code block annotations need adjustment

### Features to Recreate
1. **Navigation structure** - All frameworks support equivalent nav config
2. **Dark/light mode** - All support out of box
3. **Search** - All have built-in options
4. **Mermaid diagrams** - All support with plugin/config
5. **Versioning** - Docusaurus easiest, others manual

### Migration Steps (Starlight)
1. Create new Starlight project alongside existing docs
2. Port markdown content with frontmatter adjustments
3. Configure navigation in `astro.config.mjs`
4. Add Mermaid support via `@astrojs/starlight-mermaid`
5. Set up GitHub Pages deployment workflow
6. Verify all pages render correctly
7. Switch DNS/repo settings

## Recommendation

### Short-term (Now)
**Migrate to Astro Starlight**

Reasons:
1. MkDocs is unmaintained - supply chain risk
2. Material for MkDocs in maintenance mode
3. Starlight has best modern DX
4. Framework-agnostic for future flexibility
5. Excellent GitHub Pages support
6. Built-in accessibility
7. Strong community momentum

### Long-term (6-12 months)
**Monitor Zensical**

If Zensical matures and delivers on its promise of native `mkdocs.yml` compatibility, it could be the smoothest migration path with minimal content changes. However, it's too new to recommend now.

### Alternative: Stay on MkDocs Material

If migration bandwidth is limited, Material for MkDocs will receive security fixes for 12 months (until Nov 2026). This buys time but doesn't solve the underlying issue.

## Action Items

1. [ ] Create proof-of-concept Starlight site with 5-10 docs
2. [ ] Test GitHub Pages deployment workflow
3. [ ] Evaluate migration effort for full docs
4. [ ] Plan migration timeline
5. [ ] Set calendar reminder to evaluate Zensical in 6 months

---

## Sources

- [Starlight Official](https://starlight.astro.build/)
- [Docusaurus Official](https://docusaurus.io/)
- [VitePress Official](https://vitepress.dev/)
- [Nextra Official](https://nextra.site/)
- [Material for MkDocs - Zensical](https://squidfunk.github.io/mkdocs-material/blog/2025/11/05/zensical/)
- [Starlight vs Docusaurus - LogRocket](https://blog.logrocket.com/starlight-vs-docusaurus-building-documentation/)
- [Top Static Site Generators 2025 - CloudCannon](https://cloudcannon.com/blog/the-top-five-static-site-generators-for-2025-and-when-to-use-them/)
