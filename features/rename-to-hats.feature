Feature: Rename ralph-orchestrator to hats
  As a user of the orchestrator
  I want the tool to be called "hats"
  So that the branding matches the hat-system concept

  Scenario: CLI binary is named hats
    Given I have installed hats-cli
    When I run "hats --version"
    Then the output should contain "hats"
    And the output should contain a valid semver version

  Scenario: Config file is hats.yml
    Given I have a project directory
    When I run "hats init --backend claude"
    Then a file "hats.yml" should exist in the project directory
    And it should contain "backend: claude"

  Scenario: Backward compatibility with ralph.yml
    Given I have a project directory with "ralph.yml" containing valid config
    And no "hats.yml" exists
    When I run "hats run -p 'test'"
    Then hats should load config from "ralph.yml"
    And emit a deprecation warning mentioning "hats.yml"

  Scenario: HATS_ env vars override config
    Given I have a hats.yml with max_iterations set to 10
    And HATS_MAX_ITERATIONS is set to 5
    When I run "hats run -p 'test'"
    Then max_iterations should be 5

  Scenario: RALPH_ env vars work as deprecated fallback
    Given I have a hats.yml with max_iterations set to 10
    And RALPH_MAX_ITERATIONS is set to 5
    And HATS_MAX_ITERATIONS is not set
    When I run "hats run -p 'test'"
    Then max_iterations should be 5
    And a deprecation warning should be emitted

  Scenario: npm package is @hats/cli
    Given I run "npm info @hats/cli"
    Then the output should contain "hats"
    And the package should provide a "hats" binary

  Scenario: Old npm package shows deprecation
    Given I run "npm info @ralph-orchestrator/ralph-cli"
    Then the output should contain "deprecated"
    And it should mention "@hats/cli" as the replacement
