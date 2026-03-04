# Rough Idea: Add roo-cli as a Provider

Add `roo-cli` as a new provider/backend in ralph-orchestrator, following the same pattern as kiro and other existing backends.

This means:
- Auto-detection support (checking if `roo-cli` or equivalent binary is in PATH)
- CLI backend configuration (headless mode, interactive mode, prompt passing)
- A minimal preset file (`presets/minimal/roo.yml`)
- Integration into all the places where backends are registered (from_config, from_name, for_interactive_prompt, etc.)

The goal is to allow users to configure `cli.backend: "roo"` in their ralph.yml and have it work seamlessly with the roo-cli binary, similar to how kiro works with kiro-cli.
