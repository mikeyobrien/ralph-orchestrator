@hooks @telemetry-and-validation
Feature: Hook telemetry and validation
  # Telemetry recording and CLI validation
  # Source: specs/add-hooks-to-ralph-orchestrator-lifecycle/design.md (AC-16..AC-18)

  @AC-16
  Scenario: AC-16 Hook telemetry completeness
    Given any hook invocation
    When it completes (or times out)
    Then telemetry includes event/phase, timestamps, duration, exit code, timeout, outputs, disposition

  @AC-17
  Scenario: AC-17 Validation command
    Given malformed hooks config
    When "ralph hooks validate" runs
    Then it returns actionable failures without starting loop execution

  @AC-18
  Scenario: AC-18 Preflight integration
    Given preflight is enabled
    When "ralph run" starts
    Then hooks validation executes as part of preflight and can fail the run
