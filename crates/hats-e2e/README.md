# hats-e2e

End-to-end test harness for the Hats. Validates Hats's behavior against real AI backends (Claude, Kiro, OpenCode) to ensure the orchestration loop works correctly.

## Quick Start

```bash
# Run all tests for all available backends
cargo run -p hats-e2e -- all

# Run tests for a specific backend
cargo run -p hats-e2e -- claude

# List available scenarios
cargo run -p hats-e2e -- --list

# Run with detailed output
cargo run -p hats-e2e -- claude --verbose

# Keep workspaces for debugging
cargo run -p hats-e2e -- claude --keep-workspace

# Skip meta-Hats analysis for faster runs
cargo run -p hats-e2e -- claude --skip-analysis
```

## Architecture

```text
┌─────────────┐     ┌─────────────┐     ┌─────────────┐
│  TestRunner │────▶│  Scenarios  │────▶│  Executor   │
└─────────────┘     └─────────────┘     └─────────────┘
       │                                       │
       ▼                                       ▼
┌─────────────┐                         ┌─────────────┐
│  Reporter   │                         │   Backend   │
└─────────────┘                         └─────────────┘
```

### Components

| Component | Description |
|-----------|-------------|
| **TestRunner** | Orchestrates scenario execution and collects results |
| **Scenarios** | Define test cases via the `TestScenario` trait |
| **Executor** | Spawns `hats run` processes and captures output |
| **Reporter** | Generates terminal, Markdown, and JSON reports |
| **Analyzer** | Uses meta-Hats for rich failure diagnosis |
| **WorkspaceManager** | Isolates tests in `.e2e-tests/` directories |

## Test Scenarios

Scenarios are organized into 7 tiers:

### Tier 1: Connectivity
Basic backend availability tests.
- `ClaudeConnectScenario` - Claude backend connectivity
- `KiroConnectScenario` - Kiro backend connectivity
- `OpenCodeConnectScenario` - OpenCode backend connectivity

### Tier 2: Orchestration Loop
Full Hats orchestration cycle validation.
- `ClaudeSingleIterScenario` - Single iteration completion
- `ClaudeMultiIterScenario` - Multi-iteration progression
- `ClaudeCompletionScenario` - `LOOP_COMPLETE` detection

### Tier 3: Events
Event parsing and routing.
- `ClaudeEventsScenario` - Event XML parsing
- `ClaudeBackpressureScenario` - `build.done` backpressure evidence

### Tier 4: Capabilities
Backend feature validation.
- `ClaudeToolUseScenario` - Tool invocation handling
- `ClaudeStreamingScenario` - NDJSON streaming output

### Tier 5: Hat Collections
Hat-based workflow testing.
- `HatSingleScenario` - Single hat execution
- `HatMultiWorkflowScenario` - Planner → Builder delegation
- `HatInstructionsScenario` - Hat instructions followed
- `HatEventRoutingScenario` - Events route to correct hat
- `HatBackendOverrideScenario` - Per-hat backend selection

### Tier 6: Memory System
Persistent memory validation.
- `MemoryAddScenario` - Memory creation via CLI
- `MemorySearchScenario` - Memory search functionality
- `MemoryInjectionScenario` - Auto-injection in prompts
- `MemoryPersistenceScenario` - Cross-run persistence

### Tier 7: Error Handling (RED phase)
Graceful failure modes.
- `TimeoutScenario` - Timeout termination
- `MaxIterationsScenario` - Max iterations limit
- `AuthFailureScenario` - Invalid credentials handling
- `BackendUnavailableScenario` - Missing CLI handling

## Reports

Reports are generated in `.e2e-tests/`:

```bash
.e2e-tests/
├── report.md      # Agent-readable Markdown report
├── report.json    # Machine-readable JSON report
└── claude-connect/  # Test workspace (if --keep-workspace)
    ├── hats.yml
    ├── prompt.md
    └── .agent/
```

### Report Formats

```bash
# Markdown only (default)
cargo run -p hats-e2e -- --report markdown

# JSON only
cargo run -p hats-e2e -- --report json

# Both formats
cargo run -p hats-e2e -- --report both
```

## Library Usage

The crate can be used as a library for programmatic testing:

```rust
use hats_e2e::{
    TestRunner, WorkspaceManager, RunConfig,
    ClaudeConnectScenario, TestScenario,
};

#[tokio::main]
async fn main() {
    let workspace = WorkspaceManager::new(".e2e-tests");
    let scenarios: Vec<Box<dyn TestScenario>> = vec![
        Box::new(ClaudeConnectScenario::new()),
    ];

    let runner = TestRunner::new(workspace, scenarios);
    let config = RunConfig::new();
    let results = runner.run(&config).await.unwrap();

    println!("Passed: {}", results.passed_count());
}
```

## Development

```bash
# Run unit tests
cargo test -p hats-e2e

# Run clippy
cargo clippy -p hats-e2e

# Generate docs
cargo doc -p hats-e2e --open
```

### Adding New Scenarios

1. Create a new file in `src/scenarios/` (e.g., `my_scenario.rs`)
2. Implement the `TestScenario` trait:

```rust
use crate::scenarios::{TestScenario, ScenarioError, Assertions};
use crate::{Backend, ScenarioConfig, ExecutionResult, TestResult};

pub struct MyScenario;

impl TestScenario for MyScenario {
    fn id(&self) -> &str { "my-scenario" }
    fn description(&self) -> &str { "Tests something important" }
    fn tier(&self) -> &str { "Tier N: Category" }
    fn backend(&self) -> Backend { Backend::Claude }

    fn setup(&self, workspace: &Path) -> Result<ScenarioConfig, ScenarioError> {
        // Create hats.yml and prompt
    }

    fn assertions(&self, result: &ExecutionResult) -> Vec<TestResult> {
        // Validate execution results
    }
}
```

3. Register in `src/scenarios/mod.rs` and `src/lib.rs`
4. Add to `get_all_scenarios()` in `src/main.rs`

## Environment Variables

| Variable | Description |
|----------|-------------|
| `ANTHROPIC_API_KEY` | Required for Claude backend |
| `KIRO_API_KEY` | Required for Kiro backend |
| `OPENCODE_API_KEY` | Required for OpenCode backend |

## License

Same as parent hats project.
