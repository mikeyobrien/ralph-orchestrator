# File Inventory - Ralph Orchestrator Gap Analysis
Generated: 2026-01-04 16:15

## Summary Statistics

| Category | Count |
|----------|-------|
| PROMPT.md files | 2 |
| ACCEPTANCE_CRITERIA.yaml files | 2 (1 standard, 1 comprehensive) |
| Test files | 62 |
| Orchestration source files | 5 |
| Mobile app screens | 5 |
| Mobile components | 1 |
| Mobile hooks | 2 |
| Documentation files | 55 |
| Validation evidence directories | 2 (with 52 evidence files total) |
| Plan reports | 14 |

---

## 1. Prompts Directory Tree

```
prompts/
├── .logs/
│   └── ralph_orchestrator.log
├── archive/
│   ├── completed/
│   │   ├── ONBOARDING_PROMPT.md
│   │   ├── VALIDATION_FEATURE_PROMPT.md
│   │   ├── VALIDATION_PROPOSAL_PROMPT.md
│   │   └── WEB_PROMPT.md
│   ├── old-iterations/
│   │   └── prompt_20260102_*.md (80+ archived versions)
│   ├── BOOTSTRAP_SELF_IMPROVEMENT.md
│   ├── TUI_PROMPT.md
│   ├── TUI_PROMPT_VALIDATED.md
│   └── prompt_20260104_*.md (27 recent archives)
├── orchestration/
│   ├── COMPREHENSIVE_ACCEPTANCE_CRITERIA.yaml
│   ├── PROMPT.md
│   └── validation-evidence/
│       └── .gitkeep (EMPTY - no evidence collected)
├── self-improvement/
│   ├── ACCEPTANCE_CRITERIA.yaml
│   ├── PROMPT.md
│   └── validation-evidence/
│       ├── cli/
│       │   ├── cli-output.txt
│       │   └── ralph_validator_cli.py
│       ├── final/
│       │   └── summary.md
│       ├── ios/ (screenshots + Swift files)
│       ├── phase-00/ through phase-06/ (evidence)
│       └── web/ (screenshots + test files)
├── test-tui.md
└── VALIDATION_PROPOSAL_PROMPT.md
```

---

## 2. Orchestration Source Files

```
src/ralph_orchestrator/orchestration/
├── __init__.py
├── config.py      - Profile/MCP configuration management
├── coordinator.py - Multi-instance coordination
├── discovery.py   - MCP server discovery
└── manager.py     - Run management integration
```

**Total: 5 files**

---

## 3. Complete Source Tree

```
src/ralph_orchestrator/
├── __init__.py
├── __main__.py (53KB - main CLI entry point)
├── adapters/
│   ├── __init__.py
│   ├── acp.py
│   ├── acp_client.py
│   ├── acp_handlers.py
│   ├── acp_models.py
│   ├── acp_protocol.py
│   ├── base.py
│   ├── claude.py
│   ├── gemini.py
│   └── qchat.py
├── async_logger.py
├── context.py
├── daemon/
│   ├── __init__.py
│   ├── cli.py
│   ├── ipc.py
│   ├── log_forwarder.py
│   └── manager.py
├── error_formatter.py
├── instance.py
├── logging_config.py
├── main.py (23KB)
├── metrics.py
├── onboarding/
│   ├── __init__.py
│   ├── agent_analyzer.py
│   ├── config_generator.py
│   ├── history_analyzer.py
│   ├── pattern_extractor.py
│   ├── scanner.py
│   └── settings_loader.py
├── orchestration/
│   ├── __init__.py
│   ├── config.py
│   ├── coordinator.py
│   ├── discovery.py
│   └── manager.py
├── orchestrator.py (69KB - core orchestration logic)
├── output/
│   ├── __init__.py
│   ├── base.py
│   ├── console.py
│   ├── content_detector.py
│   ├── json_formatter.py
│   ├── plain.py
│   └── rich_formatter.py
├── run_manager.py
├── safety.py
├── security.py
├── tui/
│   ├── __init__.py
│   ├── app.py
│   ├── connection.py
│   ├── screens/
│   │   ├── __init__.py
│   │   ├── help.py
│   │   └── history.py
│   └── widgets/
│       ├── __init__.py
│       ├── metrics.py
│       ├── output.py
│       ├── progress.py
│       ├── tasks.py
│       └── validation.py
├── verbose_logger.py
└── web/
    ├── __init__.py
    ├── __main__.py
    ├── auth.py
    ├── database.py
    ├── rate_limit.py
    └── server.py
```

**Total: 67 Python source files**

---

## 4. Mobile App Files (ralph-mobile/)

```
ralph-mobile/
├── app/
│   ├── _layout.tsx
│   └── (tabs)/
│       ├── _layout.tsx
│       ├── history.tsx
│       ├── index.tsx
│       └── settings.tsx
├── components/
│   └── OrchestratorCard.tsx
└── hooks/
    ├── useAuth.tsx
    └── useOrchestrators.ts
```

**Total: 8 TypeScript files**

---

## 5. Test Files

```
tests/
├── conftest.py
├── test_acp_adapter.py
├── test_acp_cli.py
├── test_acp_client.py
├── test_acp_config.py
├── test_acp_handlers.py
├── test_acp_integration.py
├── test_acp_models.py
├── test_acp_orchestrator.py
├── test_acp_protocol.py
├── test_adapters.py
├── test_api_orchestrators.py
├── test_async_logger.py
├── test_cli_daemon.py
├── test_completion_detection.py
├── test_config.py
├── test_context.py
├── test_coordinator.py
├── test_daemon.py
├── test_discovery.py
├── test_error_formatter.py
├── test_instance.py
├── test_integration.py
├── test_ipc.py
├── test_log_forwarder.py
├── test_logging_config.py
├── test_loop_detection.py
├── test_metrics.py
├── test_onboarding_*.py (7 files)
├── test_orchestration_config.py
├── test_orchestration_integration.py
├── test_orchestrator.py
├── test_output.py
├── test_output_formatters.py
├── test_performance_simple.py
├── test_qchat_*.py (3 files)
├── test_run_manager.py
├── test_security.py
├── test_signal_handling.py
├── test_tui_app.py
├── test_tui_widgets.py
├── test_validation_*.py (2 files)
├── test_verbose_logger.py
├── test_web_*.py (4 files)
└── tui/
    ├── __init__.py
    ├── conftest.py
    ├── test_connection.py
    ├── test_integration.py
    ├── test_screens.py
    ├── test_widgets.py
    └── test_widgets_mounted.py
```

**Total: 62 test files**

---

## 6. Validation Evidence

### Root Level (validation-evidence/)
```
validation-evidence/
├── orchestration-00/
│   ├── run-manager-create.txt
│   └── run-manager-tests.txt
├── orchestration-01/
│   ├── profiles.txt
│   └── tests.txt
├── orchestration-02/
│   ├── discovery.txt
│   └── tests.txt
├── orchestration-03/
│   ├── mcps.txt
│   └── tests.txt
├── orchestration-04/
│   ├── coordination.txt
│   └── tests.txt
└── orchestration-05/
    ├── integration.txt
    └── tests.txt
```

### Self-Improvement Evidence
```
prompts/self-improvement/validation-evidence/
├── cli/ (2 files)
├── final/summary.md
├── ios/ (7 files - screenshots + Swift)
├── phase-00/ (2 files)
├── phase-01/ (2 files)
├── phase-02/ (5 files)
├── phase-03/ (3 files)
├── phase-04/ (5 files)
├── phase-05/ (4 files)
├── phase-06/ (2 files)
└── web/ (8 files)
```

### Orchestration Evidence
```
prompts/orchestration/validation-evidence/
└── .gitkeep (EMPTY - GAP IDENTIFIED)
```

---

## 7. Documentation Files

### Key Design Documents
- docs/designs/2026-01-04-onboarding-architecture.md
- docs/designs/2026-01-04-agent-harness.md
- docs/plans/260104-subagent-orchestration-architecture.md
- docs/codebase-summary.md
- docs/project-overview-pdr.md

### API Documentation
- docs/api/agents.md
- docs/api/cli.md
- docs/api/config.md
- docs/api/metrics.md
- docs/api/orchestrator.md

### Guides
- docs/guide/onboarding.md
- docs/guide/validation.md
- docs/guide/web-monitoring.md
- docs/quick-start.md

**Total: 55 documentation files**

---

## 8. Planning Documents

```
plans/
├── 260102-1510-validation-redesign/ (empty)
├── 260104-1614-orchestration-gap-analysis/ (this analysis)
├── 260104-ralph-comprehensive-improvement/
│   ├── BRIEF.md
│   └── ROADMAP.md
└── reports/
    ├── debugger-260104-0956-*.md (4 reports)
    ├── elevenlabs-260103-1404-*.md
    ├── researcher-260104-1552-*.md
    └── scout-external-260104-0248-*.md (6 reports)
```

**Total: 14 plan reports**

---

## 9. Key Gaps Identified

1. **Orchestration validation-evidence/ is EMPTY** - only .gitkeep
2. **COMPREHENSIVE_ACCEPTANCE_CRITERIA.yaml** exists but no evidence against it
3. **Mobile app has no tests** - ralph-mobile/tests/ does not exist
4. **No services directory** - ralph-mobile/services/ empty (API calls likely in hooks)

---

## File Counts by Type

| Extension | Count |
|-----------|-------|
| .py (src) | 67 |
| .py (tests) | 62 |
| .tsx/.ts (mobile) | 8 |
| .md (docs) | 55 |
| .yaml | 2 |
| .txt (evidence) | 32 |
| .png (screenshots) | 12 |
