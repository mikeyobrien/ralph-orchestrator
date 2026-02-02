# Configuration API Reference

## Overview

Configuration is defined by `ralph_core::RalphConfig`. The YAML file supports both:
- **v2 nested format** (preferred): `cli`, `event_loop`, `core`, `hats`, `events`
- **v1 flat format** (legacy): `agent`, `max_iterations`, `prompt_file`, etc.

Use `RalphConfig::parse_yaml` / `RalphConfig::from_file` and call `normalize()` to map
legacy fields into the v2 nested structure.

## Load Configuration From YAML

```rust
use ralph_core::RalphConfig;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut config = RalphConfig::from_file("ralph.yml")?;
    config.normalize();

    println!("Backend: {}", config.cli.backend);
    println!("Max iterations: {}", config.event_loop.max_iterations);

    Ok(())
}
```

## Parse YAML In Memory

```rust
use ralph_core::RalphConfig;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let yaml = r#"
cli:
  backend: claude
event_loop:
  max_iterations: 50
  max_runtime_seconds: 3600
hats:
  planner:
    name: "Planner"
    triggers: ["task.create"]
    publishes: ["plan.done"]
"#;

    let mut config = RalphConfig::parse_yaml(yaml)?;
    config.normalize();

    assert_eq!(config.cli.backend, "claude");
    assert_eq!(config.event_loop.max_iterations, 50);

    Ok(())
}
```

## Programmatic Overrides

You can override specific fields after loading:

```rust
use ralph_core::RalphConfig;

fn main() {
    let mut config = RalphConfig::default();

    config.cli.backend = "gemini".to_string();
    config.event_loop.max_iterations = 25;
    config.event_loop.max_runtime_seconds = 900;

    // Optional: update workspace root for path resolution
    config.core = config.core.with_workspace_root("/tmp/ralph-run");
}
```

## Hat Backends in YAML

Backend selection is controlled via `HatBackend` inside `hats`.

```yaml
hats:
  builder:
    name: "Builder"
    triggers: ["plan.done"]
    publishes: ["build.done"]
    backend:
      type: "kiro"
      agent: "builder"
      args: ["--verbose"]

  reviewer:
    name: "Reviewer"
    triggers: ["build.done"]
    publishes: ["review.done"]
    backend:
      type: "claude"
      args: ["--model", "claude-sonnet-4"]

  custom:
    name: "Custom"
    triggers: ["review.done"]
    backend:
      command: "/usr/local/bin/my-llm"
      args: ["--safe"]
```
