# PDD Summary: ACP Support for Ralph Orchestrator

## Project Overview

This document summarizes the Prompt-Driven Development process for adding Agent Client Protocol (ACP) support to Ralph Orchestrator.

---

## Artifacts Created

```
.sop/planning/
├── rough-idea.md              # Original concept
├── idea-honing.md             # Requirements Q&A (7 questions)
├── summary.md                 # This document
├── research/
│   ├── acp-protocol.md        # ACP protocol research
│   ├── agent-shell-analysis.md # xenodium/agent-shell analysis
│   └── ralph-adapter-architecture.md # Existing adapter patterns
├── design/
│   └── detailed-design.md     # Complete design document
└── implementation/
    └── plan.md                # 12-step implementation plan
```

---

## Key Design Decisions

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Protocol | Agent Client Protocol (JSON-RPC 2.0) | Active standard, matches xenodium reference |
| Role | ACP Client | Ralph spawns agents as subprocesses |
| Architecture | New ACPAdapter | Keeps existing adapters unchanged |
| Capabilities | Full (permissions, files, terminal) | Maximum flexibility |
| Permissions | Configurable | Supports autonomous and interactive modes |
| Test targets | Gemini CLI + Claude Code | Major ACP-compatible agents |

---

## Requirements Summary

1. **ACP Client**: Ralph connects to ACP agents via subprocess stdin/stdout
2. **New Adapter**: `ACPAdapter` class alongside Claude, QChat, Gemini
3. **Full Capabilities**: Permission handling, file ops, terminal ops
4. **Configurable Permissions**: auto_approve, allowlist, interactive, deny_all
5. **Verbose Updates**: Capture all session/update types
6. **Primary Targets**: Gemini CLI and Claude Code

---

## Architecture

```
Ralph Orchestrator
    └── ACPAdapter
        ├── ACPProtocol (JSON-RPC 2.0)
        ├── ACPClient (subprocess management)
        ├── ACPHandlers (permission, file, terminal)
        └── ACPSession (state accumulation)
            │
            └──stdin/stdout──▶ ACP Agent (Gemini/Claude/etc.)
```

---

## Implementation Plan Overview

| Phase | Steps | Description |
|-------|-------|-------------|
| Core Protocol | 1-3 | JSON-RPC 2.0, data models, subprocess client |
| Basic Adapter | 4-5 | Initialize, session, prompt execution |
| Handlers | 6-8 | Permissions, files, terminal |
| Integration | 9-10 | Config (ralph.yml), CLI |
| Testing | 11-12 | Gemini integration, orchestrator loop |

**Total**: 12 incremental steps, each producing working code.

---

## Next Steps

1. **Review** the detailed design at `.sop/planning/design/detailed-design.md`
2. **Review** the implementation plan at `.sop/planning/implementation/plan.md`
3. **Begin implementation** following the plan checklist
4. **Test** with Gemini CLI (requires Google API key)

---

## Configuration Preview

```yaml
# ralph.yml
adapters:
  acp:
    enabled: true
    agent_command: gemini
    timeout: 300
    permission_mode: auto_approve
```

## CLI Preview

```bash
# Run with ACP adapter using Gemini
ralph run -a acp --acp-agent gemini -p "Build a calculator"

# Run with Claude via ACP
ralph run -a acp --acp-agent claude
```

---

## Research Sources

- https://agentclientprotocol.com (Official ACP docs)
- https://github.com/xenodium/agent-shell (Reference implementation)
- https://github.com/xenodium/acp.el (Emacs ACP library)
