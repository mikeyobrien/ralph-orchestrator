# Roadmap: Code Story

## Overview

Code Story transforms code repositories into audio narratives through a 4-agent Claude SDK pipeline. Development proceeds from foundation (database, auth, agent framework) through individual agent implementation, frontend development, and culminates in API/self-hosting capabilities. Each phase targets 2-3 atomic tasks per plan to maintain quality and enable Playwright MCP validation at each gate.

## Milestones

- 📋 **v1.0 Core Platform** - Phases 1-8 (planned)
- 📋 **v1.1 Full Experience** - Phases 9-10 (planned)
- 📋 **v2.0 Open Source & API** - Phases 11-13 (planned)

## Phases

- [ ] **Phase 1: Foundation** - Database schema, project setup, agent framework core
- [ ] **Phase 2: Intent Agent** - Onboarding conversation and story plan generation
- [ ] **Phase 3: Repo Analyzer Agent** - GitHub fetching and code structure analysis
- [ ] **Phase 4: Story Architect Agent** - Narrative script generation with styles
- [ ] **Phase 5: Voice Director Agent** - ElevenLabs synthesis and audio assembly
- [ ] **Phase 6: FastAPI Backend** - REST API, authentication, job queue
- [ ] **Phase 7: React Frontend** - Web app with onboarding flow and player
- [ ] **Phase 8: Expo Mobile** - iOS/Android app with audio playback
- [ ] **Phase 9: Full Experience** - All narrative styles, full customization
- [ ] **Phase 10: API & Docs** - Public API, documentation, API keys
- [ ] **Phase 11: Admin Dashboard** - User management, analytics, cost tracking
- [ ] **Phase 12: Self-Hosting** - Docker/Kubernetes deployment guides
- [ ] **Phase 13: Enterprise** - Team features, SSO, priority support

## Phase Details

### Phase 1: Foundation
**Goal**: Project setup, database schema, and agent framework scaffolding
**Depends on**: Nothing (first phase)
**Plans**: 5 plans

Plans:
- [ ] 01-01: Python project setup with uv, dependencies, directory structure
- [ ] 01-02: PostgreSQL database schema design and migrations
- [ ] 01-03: Agent framework core (Skill, Agent, Orchestrator classes)
- [ ] 01-04: Environment configuration and secrets management
- [ ] 01-05: Base skill library structure and utility functions

**Validation Gate**: Playwright MCP verifies project initializes, migrations run, agent framework instantiates

---

### Phase 2: Intent Agent
**Goal**: Implement the Intent Agent for onboarding conversations
**Depends on**: Phase 1
**Plans**: 4 plans

Plans:
- [ ] 02-01: Intent Agent system prompt and Opus 4.5 configuration
- [ ] 02-02: Intent analysis skill (categorize user goals)
- [ ] 02-03: Story plan generation skill (chapters, timing, focus areas)
- [ ] 02-04: Follow-up question skill and conversation flow

**Validation Gate**: Playwright MCP simulates user providing intent, verifies structured plan output

---

### Phase 3: Repo Analyzer Agent
**Goal**: Implement the Repository Analyzer Agent for code analysis
**Depends on**: Phase 1
**Plans**: 5 plans

Plans:
- [ ] 03-01: Repo Analyzer system prompt and configuration
- [ ] 03-02: GitHub API integration skill (fetch tree, file contents)
- [ ] 03-03: Python AST analysis skill (classes, functions, imports)
- [ ] 03-04: Architectural pattern recognition skill
- [ ] 03-05: Key component identification and dependency mapping

**Validation Gate**: Playwright MCP provides GitHub URL, verifies structured analysis JSON output

---

### Phase 4: Story Architect Agent
**Goal**: Implement the Story Architect Agent for narrative generation
**Depends on**: Phases 2-3
**Plans**: 5 plans

Plans:
- [ ] 04-01: Story Architect system prompt with narrative style framework
- [ ] 04-02: Chapter script generation skill
- [ ] 04-03: Five narrative style prompts (fiction, documentary, tutorial, podcast, technical)
- [ ] 04-04: Pacing calculation and voice direction markers
- [ ] 04-05: Chapter transitions and script assembly

**Validation Gate**: Playwright MCP provides analysis + intent, verifies complete narrative script output

---

### Phase 5: Voice Director Agent
**Goal**: Implement the Voice Director Agent for audio synthesis
**Depends on**: Phase 4
**Plans**: 4 plans

Plans:
- [ ] 05-01: Voice Director system prompt and configuration
- [ ] 05-02: ElevenLabs API integration and synthesis skill
- [ ] 05-03: Script preparation and chunking for API limits
- [ ] 05-04: Audio chapter assembly and metadata generation

**Validation Gate**: Playwright MCP provides script, verifies audio file generation (MP3 output)

---

### Phase 6: FastAPI Backend
**Goal**: Complete REST API with authentication and job queue
**Depends on**: Phases 1-5
**Plans**: 6 plans

Plans:
- [ ] 06-01: FastAPI project structure with routers and middleware
- [ ] 06-02: JWT authentication (register, login, protected routes)
- [ ] 06-03: Story endpoints (create, get, list, delete)
- [ ] 06-04: Redis + Celery job queue for async story generation
- [ ] 06-05: WebSocket endpoint for generation progress
- [ ] 06-06: S3/cloud storage integration for audio files

**Validation Gate**: Playwright MCP simulates full user flow - register, login, submit URL, poll status, download audio

---

### Phase 7: React Frontend
**Goal**: Web application with onboarding flow and audio player
**Depends on**: Phase 6
**Plans**: 6 plans

Plans:
- [ ] 07-01: React project setup with Vite, Tailwind, shadcn/ui
- [ ] 07-02: Landing page and authentication screens
- [ ] 07-03: Repository input and Quick/Custom mode selection
- [ ] 07-04: Intent conversation chat interface
- [ ] 07-05: Dashboard with story list and progress tracking
- [ ] 07-06: Audio player with chapters, waveform, and controls

**Validation Gate**: Playwright MCP performs full end-user journey through web UI - import URL, complete flow, play audio

---

### Phase 8: Expo Mobile
**Goal**: iOS/Android app with full story generation and playback
**Depends on**: Phase 6
**Plans**: 5 plans

Plans:
- [ ] 08-01: Expo project setup with NativeWind styling
- [ ] 08-02: Authentication screens and session management
- [ ] 08-03: Home screen and new story flow
- [ ] 08-04: Intent chat interface for mobile
- [ ] 08-05: Audio player with background playback and chapters

**Validation Gate**: Playwright MCP verifies mobile web view flow (Expo web export), validates core functionality

---

### Phase 9: Full Experience
**Goal**: All narrative styles, full customization, enhanced features
**Depends on**: Phases 7-8
**Plans**: 4 plans

Plans:
- [ ] 09-01: All 5 narrative style implementations with style-specific prompts
- [ ] 09-02: Full conversational onboarding with chapter editing
- [ ] 09-03: Voice selection and preview functionality
- [ ] 09-04: Story sharing, public links, and download options

**Validation Gate**: Playwright MCP tests all narrative styles, customization options, sharing flow

---

### Phase 10: API & Docs
**Goal**: Public API for external developers with documentation
**Depends on**: Phase 9
**Plans**: 4 plans

Plans:
- [ ] 10-01: API key generation and management endpoints
- [ ] 10-02: Rate limiting and quota enforcement
- [ ] 10-03: API documentation (OpenAPI/Swagger)
- [ ] 10-04: Developer portal and usage examples

**Validation Gate**: Playwright MCP creates API key, makes authenticated requests, verifies rate limits

---

### Phase 11: Admin Dashboard
**Goal**: Administrative interface for user and cost management
**Depends on**: Phase 10
**Plans**: 4 plans

Plans:
- [ ] 11-01: Admin authentication and authorization
- [ ] 11-02: User management (list, search, modify quotas)
- [ ] 11-03: Usage analytics and cost tracking dashboard
- [ ] 11-04: API key administration and audit logs

**Validation Gate**: Playwright MCP performs admin login, user management, views analytics

---

### Phase 12: Self-Hosting
**Goal**: Docker and Kubernetes deployment documentation
**Depends on**: Phase 11
**Plans**: 3 plans

Plans:
- [ ] 12-01: Docker Compose configuration for local development
- [ ] 12-02: Production Docker images and multi-stage builds
- [ ] 12-03: Kubernetes manifests and Helm charts

**Validation Gate**: Playwright MCP verifies Docker Compose stack starts and serves requests

---

### Phase 13: Enterprise
**Goal**: Team features and enterprise-ready capabilities
**Depends on**: Phase 12
**Plans**: 3 plans

Plans:
- [ ] 13-01: Team/organization data model and endpoints
- [ ] 13-02: Team collaboration features (shared stories, roles)
- [ ] 13-03: SSO integration preparation (SAML/OIDC hooks)

**Validation Gate**: Playwright MCP tests team creation, member management, shared access

---

## Progress

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 1. Foundation | 0/5 | Not started | - |
| 2. Intent Agent | 0/4 | Not started | - |
| 3. Repo Analyzer | 0/5 | Not started | - |
| 4. Story Architect | 0/5 | Not started | - |
| 5. Voice Director | 0/4 | Not started | - |
| 6. FastAPI Backend | 0/6 | Not started | - |
| 7. React Frontend | 0/6 | Not started | - |
| 8. Expo Mobile | 0/5 | Not started | - |
| 9. Full Experience | 0/4 | Not started | - |
| 10. API & Docs | 0/4 | Not started | - |
| 11. Admin Dashboard | 0/4 | Not started | - |
| 12. Self-Hosting | 0/3 | Not started | - |
| 13. Enterprise | 0/3 | Not started | - |

**Total**: 58 plans across 13 phases

## Validation Strategy

Each phase includes a **Validation Gate** using Playwright MCP:
- No test files are created
- Playwright simulates end-user behavior
- Backend + Frontend validated together where applicable
- Additional gates for APIs, SDKs, and individual components

## Opus 4.5 Configuration

All agent prompts use:
- Model: `claude-opus-4-5-20251101`
- Beta flag: `effort-2025-11-24`
- Effort: `"high"`
- No `CRITICAL` or `MUST` emphasis (per Opus 4.5 migration guidelines)
