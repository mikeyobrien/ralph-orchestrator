---
status: pending
created: 2026-01-26
---
# Task: Implement `ralph hats` Command for Hat Topology Visualization and Validation

## Description
Add a `ralph hats` subcommand that validates hat configurations and visualizes the event flow topology. This helps users catch configuration errors before running and understand their hat workflows.

## Background
Hat configurations define event-driven workflows where hats subscribe to and publish events. Common issues include:
- **Unreachable hats**: Hats whose triggers are never published by any other hat
- **Orphan events**: Events that are published but no hat subscribes to them
- **Dead ends**: Hats that don't publish any events (unless intentionally terminal)
- **Missing starting_event subscribers**: The configured `starting_event` has no hat listening

Currently, `validate_topology_reachability()` in `hatless_ralph.rs` logs warnings at runtime, but users have no way to validate configurations before running or visualize the topology.

## Reference Documentation
**Required:**
- Hat config parsing: `crates/ralph-core/src/config.rs` (HatConfig struct ~line 967)
- Hat registry: `crates/ralph-core/src/hat_registry.rs`
- Existing validation: `crates/ralph-core/src/hatless_ralph.rs` (validate_topology_reachability ~line 505)
- CLI structure: `crates/ralph-cli/src/main.rs` (Commands enum)

**Additional References:**
- Similar commands for patterns: `ralph events`, `ralph loops`, `ralph tools`
- Hat topology table generation: `hatless_ralph.rs` hats_section() ~line 437

## Technical Requirements

### Subcommands
1. `ralph hats validate` - Validate hat topology and report issues
2. `ralph hats graph` - Display ASCII topology graph
3. `ralph hats list` - List all configured hats with details

### Core Validation Checks
1. **Unreachable hats** - Hats whose triggers are never published
2. **Orphan events** - Published events with no subscribers
3. **Dead ends** - Non-terminal hats that don't publish events
4. **Cycles** - Detect and report event cycles (informational, not error)
5. **Starting event validation** - If `starting_event` configured, verify subscriber exists
6. **Duplicate triggers** - Multiple hats triggered by same event (warning)

### Graph Visualization
- ASCII-based topology diagram showing event flow
- Support `--format mermaid` for Mermaid markdown output
- Show entry point (task.start), hats, and terminal states

## Dependencies
- `clap` - Already in use for CLI argument parsing
- `RalphConfig` from `ralph-core` - Config loading
- `HatRegistry` from `ralph-core` - Hat lookup

## Implementation Approach

### 1. Define HatsArgs and subcommands
```rust
#[derive(Parser, Debug)]
pub struct HatsArgs {
    #[command(subcommand)]
    pub command: HatsCommand,
}

#[derive(Subcommand, Debug)]
pub enum HatsCommand {
    /// Validate hat topology and report issues
    Validate,
    /// Display hat topology graph
    Graph {
        /// Output format (ascii, mermaid)
        #[arg(long, default_value = "ascii")]
        format: GraphFormat,
    },
    /// List all configured hats
    List {
        /// Output format (table, json)
        #[arg(long, default_value = "table")]
        format: ListFormat,
    },
}
```

### 2. Add to Commands enum
```rust
enum Commands {
    // ... existing variants
    /// Inspect and validate hat configurations
    Hats(HatsArgs),
}
```

### 3. Create hats.rs module
```rust
// crates/ralph-cli/src/hats.rs

pub struct TopologyValidator {
    hats: Vec<HatInfo>,
    starting_event: Option<String>,
}

pub struct ValidationResult {
    pub errors: Vec<ValidationError>,
    pub warnings: Vec<ValidationWarning>,
    pub info: Vec<ValidationInfo>,
}

impl TopologyValidator {
    pub fn validate(&self) -> ValidationResult { ... }
    pub fn render_ascii_graph(&self) -> String { ... }
    pub fn render_mermaid_graph(&self) -> String { ... }
}
```

### 4. Validation logic
```rust
fn find_unreachable_hats(&self) -> Vec<&HatInfo> {
    // Hats whose triggers are never published by Ralph or other hats
}

fn find_orphan_events(&self) -> Vec<String> {
    // Events published but not subscribed to (except terminal events)
}

fn find_dead_ends(&self) -> Vec<&HatInfo> {
    // Hats that don't publish and aren't explicitly terminal
}

fn detect_cycles(&self) -> Vec<Vec<String>> {
    // Find event cycles (hat A -> event -> hat B -> event -> hat A)
}
```

## Acceptance Criteria

### 1. Basic Validation - Pass
- Given a valid hat config with no issues
- When `ralph hats validate` is run
- Then output shows "All hats valid" with green checkmarks
- And exit code is 0

### 2. Unreachable Hat Detection
- Given a hat with trigger "never.published" that no hat publishes
- When `ralph hats validate` is run
- Then output shows error: "Hat 'X' is unreachable (trigger 'never.published' is never published)"
- And exit code is non-zero

### 3. Orphan Event Detection
- Given a hat that publishes "orphan.event" with no subscribers
- When `ralph hats validate` is run
- Then output shows warning: "Event 'orphan.event' is published but has no subscribers"
- And exit code is 0 (warning, not error)

### 4. Starting Event Validation
- Given `starting_event: "workflow.start"` but no hat subscribes to it
- When `ralph hats validate` is run
- Then output shows error: "starting_event 'workflow.start' has no subscribers"
- And exit code is non-zero

### 5. ASCII Graph Output
- Given a valid hat config
- When `ralph hats graph` is run
- Then output shows ASCII topology diagram with:
  - Entry point (task.start)
  - All hats as nodes
  - Events as labeled edges
  - Terminal states marked

### 6. Mermaid Graph Output
- Given a valid hat config
- When `ralph hats graph --format mermaid` is run
- Then output is valid Mermaid flowchart syntax
- And can be pasted into Mermaid-compatible renderer

### 7. List Hats Table
- Given a hat config with multiple hats
- When `ralph hats list` is run
- Then output shows table with columns: ID, Name, Triggers, Publishes, Description
- And all configured hats are listed

### 8. List Hats JSON
- Given a hat config
- When `ralph hats list --format json` is run
- Then output is valid JSON array of hat objects
- And includes all hat properties

### 9. Cycle Detection (Informational)
- Given hats that form a cycle (A publishes to B, B publishes to A)
- When `ralph hats validate` is run
- Then output shows info: "Cycle detected: A -> event -> B -> event -> A"
- And exit code is 0 (cycles are often intentional)

### 10. No Hats Configured
- Given a config with no hats section
- When `ralph hats validate` is run
- Then output shows "No hats configured (solo mode)"
- And exit code is 0

### 11. Config Flag Support
- Given `ralph hats validate -c custom.yml`
- When the command executes
- Then it validates the hats from custom.yml

### 12. Help Text
- Given `ralph hats --help`
- When executed
- Then shows usage with all subcommands documented

## Example Output

### `ralph hats validate`
```
Hat Topology Validation
=======================

Hats: 6 configured
Entry: task.start -> plan.start

Checks:
  [ok] All hats are reachable
  [ok] Starting event 'plan.start' has subscriber (Planner)
  [ok] No dead-end hats
  [warn] Event 'escalate.human' published by Handler has no subscribers
  [info] Cycle detected: Builder -> build.blocked -> Planner -> build.task -> Builder

Result: Valid (1 warning)
```

### `ralph hats graph`
```
task.start
    |
    v
[Planner] --build.task--> [Builder]
                              |
                    +---------+---------+
                    |                   |
              build.done          build.blocked
                    |                   |
                    v                   |
              [Validator]               |
                    |                   |
            validation.done             |
                    |                   |
                    v                   |
              [Confessor]               |
                    |                   |
          +---------+---------+         |
          |                   |         |
    confession.clean   confession.issues_found
          |                   |         |
          +-------------------+         |
                    |                   |
                    v                   |
               [Handler] <--------------+
                    |
            summary.request
                    |
                    v
             [Summarizer]
                    |
                    v
             LOOP_COMPLETE
```

### `ralph hats list`
```
ID                  NAME               TRIGGERS                           PUBLISHES
planner             Planner            plan.start                         build.task
builder             Builder            build.task                         build.done, build.blocked
validator           Validator          build.done                         validation.done
confessor           Confessor          validation.done                    confession.clean, confession.issues_found
confession_handler  Confession Handler confession.clean, confession.issues_found  build.task, summary.request
summarizer          Summarizer         summary.request                    (terminal)
```

## Metadata
- **Complexity**: Medium
- **Labels**: CLI, Hats, Validation, Visualization, DX
- **Required Skills**: Rust, clap, graph traversal, ASCII rendering
