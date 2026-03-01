@hooks @metadata-mutation
Feature: Hook metadata mutation
  # Optional metadata injection from hook output
  # Source: specs/add-hooks-to-ralph-orchestrator-lifecycle/design.md (AC-13..AC-15)

  @AC-13
  Scenario: AC-13 Mutation opt-in only
    Given a hook with "mutate.enabled: false" or not configured
    When the hook emits JSON on stdout
    Then the metadata is ignored
    And orchestration context remains unchanged

  @AC-14
  Scenario: AC-14 Metadata-only mutation surface
    Given a hook with "mutate.enabled: true"
    When the hook emits valid JSON metadata on stdout
    Then only the metadata namespace is updated
    And prompt, events, and config remain immutable

  @AC-15
  Scenario: AC-15 JSON-only mutation format
    Given a hook with "mutate.enabled: true"
    When the hook emits non-JSON output on stdout
    Then the output is treated as invalid
    And an error is recorded in telemetry
