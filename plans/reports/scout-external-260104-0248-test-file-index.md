# Test Files Complete Index
**Generated:** 2026-01-04 | **Total Files:** 50 | **Total Tests:** 280+

---

## Quick Reference: All Test Files

### Root-Level Tests (42 files)

#### ACP Protocol & Adapter Tests (9 files, 80 tests)
1. **test_acp_adapter.py** (33 KB)
   - TestACPAdapterInitialization (5 tests)
   - TestACPAdapterAvailability (2 tests)
   - TestACPAdapterInitialize (1 test)
   - TestACPAdapterExecute (1 test) - most complex
   - TestACPAdapterSignalHandling (1 test)
   - TestACPAdapterMetadata (1 test)
   - TestACPAdapterPromptExecution (N/A)
   - TestACPAdapterPromptEnhancement (N/A)

2. **test_acp_cli.py** (11 KB) - 8 tests
   - TestACPAgentChoice (1)
   - TestACPCLIArguments (1)
   - TestACPAdapterMap (1)
   - TestOrchestratorACPAdapter (1)
   - TestACPAutoDetection (1)
   - TestACPCLIConfigIntegration (1)
   - TestACPMainEntryPoint (1)
   - TestACPInitTemplate (1)

3. **test_acp_client.py** (14 KB) - 11 tests
   - TestACPClientInit (1)
   - TestACPClientStart (1)
   - TestACPClientStop (1)
   - TestACPClientWriteMessage (1)
   - TestACPClientSendRequest (1)
   - TestACPClientSendNotification (1)
   - TestACPClientResponseRouting (1)
   - TestACPClientNotificationHandler (1)
   - TestACPClientRequestHandler (1)
   - TestACPClientTimeout (1)
   - TestACPClientThreadSafety (1)

4. **test_acp_config.py** (15 KB) - 6 tests
   - TestACPAdapterConfigParsing (1)
   - TestACPAdapterConfigEnvironmentOverrides (1)
   - TestACPAdapterConfigDefaults (1)
   - TestACPAdapterConfigFromAdapterConfig (1)
   - TestACPConfigInitTemplate (1)
   - TestACPConfigValidation (1)

5. **test_acp_handlers.py** (43 KB) - 19 tests [LARGEST in ACP]
   - TestPermissionRequest (N/A)
   - TestPermissionResult (N/A)
   - TestACPHandlersInitialization (1)
   - TestACPHandlersAutoApprove (6)
   - TestACPHandlersDenyAll (2)
   - TestACPHandlersAllowlist (4)
   - TestACPHandlersInteractive (6+)

6. **test_acp_integration.py** (20 KB) - 6 tests
   - Integration tests requiring GOOGLE_API_KEY
   - @pytest.mark.integration marked

7. **test_acp_models.py** (16 KB) - 10 tests
   - Data model validation

8. **test_acp_orchestrator.py** (14 KB) - 6 tests
   - Orchestrator integration

9. **test_acp_protocol.py** (13 KB) - 6 tests
   - Protocol message handling

#### Onboarding Feature Tests (8 files, 59 tests)
10. **test_onboarding_agent_analyzer.py** (12 KB) - 7 tests
    - Agent analysis logic

11. **test_onboarding_cli.py** (14 KB) - 11 tests
    - Onboarding CLI commands
    - Private: -rw------- (mode 600)

12. **test_onboarding_config_generator.py** (23 KB) - 8 tests
    - Configuration generation
    - Private: -rw------- (mode 600)

13. **test_onboarding_history_analyzer.py** (17 KB) - 7 tests
    - History analysis and patterns
    - Private: -rw------- (mode 600)

14. **test_onboarding_integration.py** (19 KB) - 5 tests
    - End-to-end workflows
    - Private: -rw------- (mode 600)

15. **test_onboarding_pattern_extractor.py** (19 KB) - 8 tests
    - Pattern extraction logic
    - Private: -rw------- (mode 600)

16. **test_onboarding_scanner.py** (12 KB) - 7 tests
    - File scanning and discovery
    - Private: -rw------- (mode 600)

17. **test_onboarding_settings.py** (11 KB) - 6 tests
    - Settings management
    - Private: -rw------- (mode 600)

#### Output & Formatting Tests (3 files, 24 tests)
18. **test_output_formatters.py** (58 KB) [LARGEST OVERALL] - 14 tests
    - Rich text formatting
    - Colors and tables
    - Complex formatting scenarios

19. **test_error_formatter.py** (13 KB) - 6 tests
    - Error message formatting

20. **test_output.py** (6 KB) - 4 tests
    - Output stream handling

#### Logging Tests (2 files, 25 tests)
21. **test_async_logger.py** (35 KB) - 14 tests
    - Async logging operations
    - Queue management
    - Performance considerations

22. **test_verbose_logger.py** (16 KB) - 11 tests
    - Verbose logging modes
    - Output formatting

#### Configuration Tests (2 files, 11 tests)
23. **test_config.py** (22 KB) - 9 tests
    - Config parsing
    - Merging strategies
    - Validation

24. **test_logging_config.py** (11 KB) - 2 tests
    - Logging configuration

#### Core/Utilities Tests (9 files, 37 tests)
25. **test_orchestrator.py** (16 KB) - 6 tests
    - Core orchestrator execution

26. **test_adapters.py** (12 KB) - 6 tests
    - Adapter base classes
    - Lifecycle management

27. **test_integration.py** (13 KB) - 4 tests
    - Core integration workflows

28. **test_security.py** (6 KB) - 6 tests
    - Security and permissions

29. **test_validation_feature.py** (22 KB) - 6 tests
    - Feature validation

30. **test_signal_handling.py** (7 KB) - 4 tests
    - Signal handling
    - Cleanup operations

31. **test_context.py** (5 KB) - 2 tests
    - Execution context

32. **test_loop_detection.py** (7 KB) - 2 tests
    - Infinite loop detection

33. **test_completion_detection.py** (7 KB) - 1 test
    - Task completion detection

#### QChat Adapter Tests (3 files, 12 tests)
34. **test_qchat_adapter.py** (18 KB) - 9 tests
    - QChat adapter implementation

35. **test_qchat_integration.py** (6 KB) - 2 tests
    - Integration with orchestrator

36. **test_qchat_message_queue.py** (18 KB) - 1 test
    - Message queue operations

#### Web Server Tests (4 files, 10 tests)
37. **test_web_server.py** (18 KB) - 2 tests
    - Server startup
    - Routing

38. **test_web_auth.py** (5 KB) - 2 tests
    - Authentication

39. **test_web_rate_limit.py** (11 KB) - 5 tests
    - Rate limiting

40. **test_web_database.py** (14 KB) - 1 test
    - Database operations

#### Metrics & Performance Tests (2 files, 5 tests)
41. **test_metrics.py** (20 KB) - 5 tests
    - Metrics collection
    - Reporting

42. **test_performance_simple.py** (5 KB) - 0 active tests
    - Performance benchmarks (placeholder)

#### Root Fixtures & Configuration (1 file)
43. **conftest.py** (1.5 KB)
    - Root pytest fixtures
    - Custom markers: @pytest.mark.integration, @pytest.mark.slow
    - Fixtures: temp_workspace, google_api_key

---

### TUI Tests (5 files in /tests/tui/)

#### TUI Test Support Files
**conftest.py** (1.8 KB)
- Fixtures: app, mock_events, mock_connection
- MockConnection class for simulating connections
- Event types: ITERATION_START, OUTPUT, TOOL_CALL, METRICS, COMPLETE

#### TUI Test Modules
1. **test_connection.py** (6 KB)
   - TUI event connection
   - Event communication

2. **test_integration.py** (5 KB)
   - TUI end-to-end workflows
   - Full lifecycle testing

3. **test_widgets.py** (4 KB)
   - Widget rendering
   - Widget behavior

4. **test_widgets_mounted.py** (54 KB) [LARGEST TUI FILE]
   - Complex widget mounting
   - Widget state management
   - Interaction scenarios

5. **test_screens.py** (2 KB)
   - Screen lifecycle
   - Screen transitions

6. **__init__.py** (24 bytes)
   - Package marker

---

## Statistics

### By Category
| Category | Files | Tests | Avg Size |
|----------|-------|-------|----------|
| ACP | 9 | 80 | 16 KB |
| Onboarding | 8 | 59 | 13 KB |
| Output | 3 | 24 | 25 KB |
| Logging | 2 | 25 | 25 KB |
| Core | 9 | 37 | 9 KB |
| Config | 2 | 11 | 16 KB |
| QChat | 3 | 12 | 14 KB |
| Web | 4 | 10 | 12 KB |
| Metrics | 2 | 5 | 12 KB |
| TUI | 5 | ~20 | 14 KB |

### File Size Distribution
- **Under 10 KB:** 15 files
- **10-20 KB:** 18 files
- **20-30 KB:** 8 files
- **30-60 KB:** 5 files
- **Over 60 KB:** 4 files

### Test Type Distribution
- **Unit Tests:** ~240 (86%)
- **Integration Tests:** ~40 (14%)
- **Class-based Groups:** ~95 test classes
- **Function-based Tests:** ~170 functions

---

## Key Insights

### Test Concentration
- ACP system: 30% of tests
- Onboarding feature: 21% of tests
- Supporting infrastructure: 49% of tests

### File Organization
- Flat structure for main tests (no subdirectories except /tui)
- Naming convention: `test_{component}.py` or `test_{module}_{feature}.py`
- Private files (mode 600) in onboarding tests (7 files)

### Largest Test Files (Complexity Indicators)
1. test_output_formatters.py (58 KB, 14 tests) - Rich formatting
2. test_acp_handlers.py (43 KB, 19 tests) - Permission handling
3. test_async_logger.py (35 KB, 14 tests) - Logging infrastructure
4. test_acp_adapter.py (33 KB, 8 tests) - Adapter protocol
5. test_widgets_mounted.py (54 KB, TUI) - UI component complexity

### Quick Path to Test Files by Feature
- **To test ACP:** `/tests/test_acp_*.py` (9 files)
- **To test Onboarding:** `/tests/test_onboarding_*.py` (8 files)
- **To test TUI:** `/tests/tui/test_*.py` (5 files)
- **To test Output:** `/tests/test_output*.py` (3 files)
- **To test Web:** `/tests/test_web_*.py` (4 files)

