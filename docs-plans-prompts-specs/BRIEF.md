# Code Story

**One-liner**: An open-source, developer-first platform that transforms code repositories into tailored audio narratives using Claude Agent SDK multi-agent architecture.

## Problem

Developers spend significant time onboarding to new codebases, understanding unfamiliar architectures, and reviewing code. Traditional documentation is often outdated or incomplete. Unlike general-purpose tools like NotebookLM, there's no purpose-built solution for transforming code repositories into structured, intent-driven audio narratives that developers can consume during commutes, workouts, or while coding.

## Success Criteria

How we know it worked:

- [ ] Users can paste a GitHub URL and receive a generated audio narrative within 5 minutes
- [ ] Intent conversation flow correctly tailors story structure based on user goals
- [ ] Audio narratives are listenable, engaging, and technically accurate
- [ ] 4-agent pipeline (Intent → Analyzer → Architect → Voice) works end-to-end
- [ ] Web app and mobile app both provide full story generation and playback
- [ ] Public API available for external integrations
- [ ] Self-hosting documentation enables teams to deploy their own instance
- [ ] Playwright MCP functional tests pass at all validation gates

## Constraints

Technical constraints that must be followed:

- **Claude Agent SDK (Python)**: Multi-agent orchestration must use claude-agent-sdk
- **Opus 4.5**: All Claude API calls use claude-opus-4-5-20251101 with effort="high"
- **ElevenLabs**: Voice synthesis via ElevenLabs API
- **FastAPI**: Backend REST API framework
- **React 18+**: Web frontend with Tailwind CSS + shadcn/ui
- **Expo (React Native)**: Mobile app with NativeWind
- **PostgreSQL**: Primary data persistence
- **Playwright MCP**: Functional testing at validation gates (no test files created)

## Out of Scope

What we're NOT building in v1.0 (prevents scope creep):

- Real-time collaboration features
- Video output (audio only)
- Support for non-GitHub repositories (GitLab, Bitbucket)
- Custom voice training (ElevenLabs PVC) - deferred to v2.0
- Multi-language audio output
- Payment processing / subscription billing (admin-managed in v1.0)
- Mobile app push notifications

## Tech Stack Summary

| Layer | Technology | Purpose |
|-------|------------|---------|
| AI Engine | Claude Agent SDK (Python) + Opus 4.5 | Multi-agent orchestration |
| Backend | FastAPI + PostgreSQL + Redis/Celery | REST API, auth, job queue |
| Web Frontend | React 18 + Tailwind CSS + shadcn/ui | Browser interface |
| Mobile | Expo (React Native) + NativeWind | iOS/Android app |
| Voice | ElevenLabs API | Audio synthesis |
| Storage | S3/Cloud Storage | Audio files, cached data |
| Testing | Playwright MCP | Functional validation |

## Agent Architecture

```
Intent Agent → Repo Analyzer Agent → Story Architect Agent → Voice Director Agent
     ↓              ↓                      ↓                       ↓
  Onboard       Analyze             Create Scripts          Synthesize Audio
```

## Repository

- **Target**: github.com/codestory/codestory
- **License**: MIT
- **Hosted**: codestory.dev (free tier + pro plans)
