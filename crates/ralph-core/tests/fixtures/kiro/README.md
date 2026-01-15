# Kiro CLI Test Fixtures

This directory contains JSONL session fixtures for smoke testing the Ralph orchestrator with the Kiro CLI adapter (AWS Kiro, formerly Amazon Q Developer CLI).

## Fixture Format

Each fixture is a JSONL file with terminal write events recorded from Kiro CLI sessions.
The format matches the `SessionRecorder` output:

```json
{"ts": 1000, "event": "ux.terminal.write", "data": {"bytes": "<base64>", "stdout": true, "offset_ms": 0}}
```

- `ts`: Timestamp in milliseconds
- `event`: Event type (use `ux.terminal.write` for terminal output)
- `data.bytes`: Base64-encoded raw terminal output bytes
- `data.stdout`: `true` for stdout, `false` for stderr
- `data.offset_ms`: Offset from session start in milliseconds

## Available Fixtures

### basic_kiro_session.jsonl

Minimal Kiro session demonstrating:
- Kiro startup output
- Event parsing (`build.task`, `build.done`)
- Completion promise detection (`LOOP_COMPLETE`)

Contains 3 terminal write chunks and 2 parsed events.

### kiro_tool_use.jsonl

Kiro session with tool invocations demonstrating:
- Shell tool execution (`shell`)
- File write operations (`write`)
- File read operations (`read`)
- Event parsing for tool-heavy workflows

Contains multiple tool invocation outputs with proper event parsing.

### kiro_autonomous.jsonl

Autonomous mode session (`--no-interactive`) demonstrating:
- No user confirmation prompts
- Trusted tool execution (`--trust-all-tools`)
- Direct completion without interaction

## Kiro CLI Command Reference

### Autonomous Mode (default for Ralph)

```bash
kiro-cli chat --no-interactive --trust-all-tools "your prompt"
```

- `--no-interactive`: Disables confirmation prompts, exits on Ctrl+C
- `--trust-all-tools`: Enables autonomous tool use without confirmation

### Interactive Mode

```bash
kiro-cli chat --trust-all-tools "your prompt"
```

- Omits `--no-interactive` to allow user interaction
- Ctrl+C cancels current operation instead of exiting

## Recording New Fixtures

### Option 1: Using Ralph Session Recording

```bash
cargo run --bin ralph -- run -c ralph.kiro.yml --record-session session.jsonl -p "your prompt"
```

### Option 2: Manual Capture

```bash
# Capture raw Kiro output
kiro-cli chat --no-interactive --trust-all-tools "your prompt" 2>&1 | tee kiro_output.txt

# Convert to fixture format using the test helpers
```

### Option 3: Programmatic Creation

```rust
use ralph_proto::TerminalWrite;
use ralph_core::Record;

let text = "Kiro output with <event topic=\"build.task\">Task</event>";
let write = TerminalWrite::new(text.as_bytes(), true, 0);
let record = Record {
    ts: 1000,
    event: "ux.terminal.write".to_string(),
    data: serde_json::to_value(&write).unwrap(),
};
println!("{}", serde_json::to_string(&record).unwrap());
```

## Kiro Built-in Tools

Kiro includes these built-in tools that may appear in recordings:

| Tool | Description |
|------|-------------|
| `read` | Read file contents |
| `write` | Write/create files |
| `shell` | Execute shell commands |
| `aws` | AWS CLI operations |
| `report` | Generate reports |

## Usage in Tests

```rust
use ralph_core::testing::{SmokeRunner, SmokeTestConfig};

let config = SmokeTestConfig::new("tests/fixtures/kiro/basic_kiro_session.jsonl");
let result = SmokeRunner::run(&config)?;

assert!(result.completed_successfully());
assert!(result.event_count() >= 2);
```

## See Also

- `specs/adapters/kiro.spec.md` - Kiro adapter specification
- `ralph.kiro.yml` - Example Kiro configuration
- `../README.md` - Parent fixture documentation
