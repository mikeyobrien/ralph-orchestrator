@hooks @error-dispositions
Feature: Hook error dispositions
  # Error handling policies for hook failures
  # Source: specs/add-hooks-to-ralph-orchestrator-lifecycle/design.md (AC-08..AC-09)

  @AC-08
  Scenario: AC-08 Per-hook warn policy
    Given `on_error: warn`
    When the hook exits non-zero
    Then orchestration continues and warning telemetry is recorded

  @AC-09
  Scenario: AC-09 Per-hook block policy
    Given `on_error: block`
    When the hook fails
    Then orchestration step is blocked and reason is surfaced
