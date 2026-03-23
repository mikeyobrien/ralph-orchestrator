## Scope

- Tests-versus-implementation drift and UI/backend contract drift follow-up.
- Inspected files:
  - [frontend/ralph-web/src/components/tasks/TaskDetailHeader.test.tsx](/home/coe/scroll/agent-orchestrator/frontend/ralph-web/src/components/tasks/TaskDetailHeader.test.tsx)
  - [backend/ralph-web-server/src/services/TaskBridge.test.ts](/home/coe/scroll/agent-orchestrator/backend/ralph-web-server/src/services/TaskBridge.test.ts)
  - [backend/ralph-web-server/src/queue/Dispatcher.test.ts](/home/coe/scroll/agent-orchestrator/backend/ralph-web-server/src/queue/Dispatcher.test.ts)
  - [crates/ralph-cli/src/loops.rs](/home/coe/scroll/agent-orchestrator/crates/ralph-cli/src/loops.rs)
  - [crates/ralph-telegram/src/handler.rs](/home/coe/scroll/agent-orchestrator/crates/ralph-telegram/src/handler.rs)
  - [crates/ralph-telegram/src/service.rs](/home/coe/scroll/agent-orchestrator/crates/ralph-telegram/src/service.rs)
- Bug classes checked:
  - tests encoding the same wrong assumption as the implementation
  - UI/backend contract mismatch
  - cancellation and lifecycle regression coverage
  - restart/recovery regression coverage
  - authorization/routing regression coverage

## Report file

- [artifacts/bug-hunter-tests-drift-followup.md](/home/coe/scroll/agent-orchestrator/artifacts/bug-hunter-tests-drift-followup.md)

## Findings

- No new standalone P0-P2 findings in this pass.
- This follow-up confirmed that the remaining defects in scope are already captured by earlier findings, and the main residual risk here is missing regression coverage around those same defect families.

## Evidence

- The frontend status-contract drift is clearly encoded in tests:
  - [TaskDetailHeader.test.tsx](/home/coe/scroll/agent-orchestrator/frontend/ralph-web/src/components/tasks/TaskDetailHeader.test.tsx) only exercises `open`, `running`, `failed`, `completed`, and `closed`.
  - There is no `pending` coverage, which matches the already-documented `pending` crash path in the detail header/page.
- The web cancellation tests validate the dispatcher path, not the bridge path that the UI actually uses:
  - [Dispatcher.test.ts](/home/coe/scroll/agent-orchestrator/backend/ralph-web-server/src/queue/Dispatcher.test.ts) covers pending and running cancellation via `Dispatcher.cancelTask()`.
  - [TaskBridge.test.ts](/home/coe/scroll/agent-orchestrator/backend/ralph-web-server/src/services/TaskBridge.test.ts) covers `cancelTask()` only with mocked `ProcessSupervisor.stop()` return values and `recoverStuckTasks()`.
  - There is no end-to-end test spanning `TaskBridge.cancelTask()` plus detached runner exit plus queue-state reconciliation, which is exactly the already-documented cancelled-versus-completed divergence.
- The loop log tests still encode the obsolete log-path assumption:
  - [crates/ralph-cli/src/loops.rs](/home/coe/scroll/agent-orchestrator/crates/ralph-cli/src/loops.rs) includes `test_show_logs_falls_back_to_history`, which creates `.ralph/history.jsonl` and never exercises the `.ralph/current-events` marker path used by fresh runs.
  - That aligns with the already-documented `ralph loops logs` mismatch.
- Telegram routing tests cover only the happy path:
  - [crates/ralph-telegram/src/handler.rs](/home/coe/scroll/agent-orchestrator/crates/ralph-telegram/src/handler.rs) tests `@feature-auth ...` routing and auto-detected chat binding.
  - There is no regression coverage for foreign-chat rejection, `@../...` traversal attempts, or sender-chat mismatch handling, which matches the already-documented Telegram auth/routing defects.
- Telegram service tests focus on timeout/retry behavior:
  - [crates/ralph-telegram/src/service.rs](/home/coe/scroll/agent-orchestrator/crates/ralph-telegram/src/service.rs) has good coverage for timeout cleanup and retry backoff.
  - They do not cover concurrent `pending_questions` state updates or unauthorized inbound message handling.

## Areas inspected

- [frontend/ralph-web/src/components/tasks/TaskDetailHeader.test.tsx](/home/coe/scroll/agent-orchestrator/frontend/ralph-web/src/components/tasks/TaskDetailHeader.test.tsx)
  - Lower-risk conclusion: the suite is small and understandable; it does not conceal another distinct bug beyond the already-known `pending` status mismatch.
- [backend/ralph-web-server/src/services/TaskBridge.test.ts](/home/coe/scroll/agent-orchestrator/backend/ralph-web-server/src/services/TaskBridge.test.ts)
  - Lower-risk conclusion: the file mostly shows missing integration coverage, not a separate defect beyond the already-reported restart/cancellation families.
- [backend/ralph-web-server/src/queue/Dispatcher.test.ts](/home/coe/scroll/agent-orchestrator/backend/ralph-web-server/src/queue/Dispatcher.test.ts)
  - Lower-risk conclusion: dispatcher-native cancellation looks internally coherent; the defect is that web cancellation bypasses it.
- [crates/ralph-cli/src/loops.rs](/home/coe/scroll/agent-orchestrator/crates/ralph-cli/src/loops.rs)
  - Lower-risk conclusion: the inspected tests reinforce the existing logs-path finding but did not expose a second independent logs/control-plane bug in this pass.
- [crates/ralph-telegram/src/handler.rs](/home/coe/scroll/agent-orchestrator/crates/ralph-telegram/src/handler.rs)
  - Lower-risk conclusion: the tests are narrow but consistent with the code; they do not contradict the previously documented auth/routing defects.
- [crates/ralph-telegram/src/service.rs](/home/coe/scroll/agent-orchestrator/crates/ralph-telegram/src/service.rs)
  - Lower-risk conclusion: retry and timeout handling appear sound inside the inspected unit-tested paths.

## Recommended next search

- After fixes land, add focused regressions for:
  - `pending` task status rendering on the detail page/header.
  - Web `TaskBridge.cancelTask()` end-to-end cancellation reconciliation.
  - `ralph loops logs` with `.ralph/current-events`.
  - Telegram foreign-chat rejection, path traversal in `@loop-id`, and concurrent state persistence.
- I did not find another fresh P0-P2 bug in this pass, so the next highest-value work is remediation plus regression hardening rather than another test-drift-only sweep.
