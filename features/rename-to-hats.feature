Feature: Rename ralph-orchestrator to hats
  As a user of the orchestrator
  I want the tool to be called "hats"
  So that the branding matches the hat-system concept

  # ---------------------------------------------------------------------------
  # CLI binary
  # ---------------------------------------------------------------------------

  Scenario: CLI binary is named "hats"
    Given I have installed hats-cli
    When I run "hats --version"
    Then the output should match the pattern "hats \d+\.\d+\.\d+"
    And the exit code should be 0

  Scenario: "hats --help" references hats branding
    When I run "hats --help"
    Then the output should contain "hats"
    And the output should NOT contain "ralph" (case-insensitive)
    And the output should list available subcommands including "run", "init", "loops"

  # ---------------------------------------------------------------------------
  # Config file: hats.yml
  # ---------------------------------------------------------------------------

  Scenario: "hats init --backend" creates hats.yml
    Given an empty project directory
    When I run "hats init --backend claude"
    Then a file "hats.yml" should exist
    And its contents should include 'backend: "claude"'
    And its contents should include "LOOP_COMPLETE"
    And no file "ralph.yml" should exist

  Scenario: "hats init --preset" creates hats.yml from preset
    Given an empty project directory
    When I run "hats init --preset spec-driven"
    Then a file "hats.yml" should exist
    And its contents should define a "spec_writer" hat

  Scenario: "hats init" without arguments shows usage
    Given an empty project directory
    When I run "hats init"
    Then the output should contain "Usage" and list backends and presets
    And the exit code should be 0

  Scenario: "hats init" refuses to overwrite existing hats.yml
    Given a project directory with an existing "hats.yml"
    When I run "hats init --backend claude"
    Then the exit code should be non-zero
    And stderr should contain "--force"

  Scenario: "hats init --force" overwrites existing hats.yml
    Given a project directory with an existing "hats.yml"
    When I run "hats init --backend claude --force"
    Then the exit code should be 0
    And "hats.yml" should contain the new config

  Scenario: "hats init --backend" rejects unknown backends
    When I run "hats init --backend foobar"
    Then the exit code should be non-zero
    And stderr should contain "Unknown backend" and list valid backends

  # ---------------------------------------------------------------------------
  # Backward compatibility: ralph.yml
  # ---------------------------------------------------------------------------

  Scenario: hats loads config from ralph.yml when hats.yml is absent
    Given a project directory with "ralph.yml" containing:
      """
      cli:
        backend: "claude"
      event_loop:
        max_iterations: 50
      """
    And no "hats.yml" exists
    When I run "hats run -p 'test' --dry-run"
    Then hats should load config from "ralph.yml"
    And stderr should contain "ralph.yml" and "deprecated"
    And stderr should suggest renaming to "hats.yml"

  Scenario: hats.yml takes priority over ralph.yml
    Given a project directory with both "hats.yml" and "ralph.yml"
    And "hats.yml" sets max_iterations to 20
    And "ralph.yml" sets max_iterations to 50
    When I run "hats run -p 'test' --dry-run"
    Then hats should use max_iterations 20 (from hats.yml)
    And no deprecation warning should appear for ralph.yml

  # ---------------------------------------------------------------------------
  # Environment variables
  # ---------------------------------------------------------------------------

  Scenario: HATS_ environment variables are recognized
    Given a "hats.yml" in the project directory
    When I set HATS_VERBOSE to "1"
    And I run "hats run -p 'test' --dry-run"
    Then hats should run in verbose mode

  Scenario: HATS_TELEGRAM_BOT_TOKEN overrides config
    Given a "hats.yml" with telegram bot_token set to "config-token"
    When I set HATS_TELEGRAM_BOT_TOKEN to "env-token"
    And I run "hats bot status"
    Then the effective bot token should be "env-token"

  Scenario: RALPH_ environment variables work as deprecated fallback
    Given a "hats.yml" in the project directory
    When I set RALPH_VERBOSE to "1" and HATS_VERBOSE is NOT set
    And I run "hats run -p 'test' --dry-run"
    Then hats should run in verbose mode
    And stderr should contain "RALPH_VERBOSE is deprecated" and suggest "HATS_VERBOSE"

  Scenario: HATS_ takes precedence over RALPH_ for the same suffix
    When I set HATS_VERBOSE to "0" and RALPH_VERBOSE to "1"
    And I run "hats run -p 'test' --dry-run"
    Then the effective value should be "0" (from HATS_VERBOSE)
    And no deprecation warning should appear

  # ---------------------------------------------------------------------------
  # Internal directory structure
  # ---------------------------------------------------------------------------

  Scenario: Agent directory is .hats/
    Given a project with "hats.yml"
    When hats creates its working directory
    Then the directory should be ".hats/"
    And it should NOT create a ".ralph/" directory
    And ".hats/agent/" should contain the scratchpad and task files

  # ---------------------------------------------------------------------------
  # Crate and package names
  # ---------------------------------------------------------------------------

  Scenario: Cargo crates use hats- prefix
    Given the ralph-orchestrator source code
    Then the following crates should exist in Cargo.toml workspace members:
      | crate           |
      | hats-cli        |
      | hats-core       |
      | hats-adapters   |
      | hats-tui        |
      | hats-telegram   |
      | hats-e2e        |
      | hats-proto      |
      | hats-bench      |
    And no crate should use the "ralph-" prefix
