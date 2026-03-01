@hooks @metadata-mutation
Feature: Hook metadata mutation
  # Optional metadata injection from hook output
  # Source: specs/add-hooks-to-ralph-orchestrator-lifecycle/design.md (AC-13..AC-15)

  @AC-13
  Scenario: AC-13 Mutation opt-in only
    Given mutation is not enabled for a hook
    When hook emits JSON metadata
    Then metadata is ignored and orchestration context is unchanged

  @AC-14
  Scenario: AC-14 Metadata-only mutation surface
    Given mutation is enabled
    When hook emits valid JSON metadata
    Then only metadata namespace is updated; prompt/events/config remain immutable

  @AC-15
  Scenario: AC-15 JSON-only mutation format
    Given mutation output is non-JSON
    When mutation parsing occurs
    Then output is treated as invalid mutation output error
