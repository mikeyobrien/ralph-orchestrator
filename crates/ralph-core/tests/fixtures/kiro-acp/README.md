# kiro-acp Test Fixtures

This directory contains JSONL session fixtures for smoke testing the Ralph orchestrator with the kiro-acp adapter (agent-client-protocol mode via `kiro-cli acp`).

## Fixture Format

Same `ux.terminal.write` JSONL format as other fixtures:

```json
{"ts": 1000, "event": "ux.terminal.write", "data": {"bytes": "<base64>", "stdout": true, "offset_ms": 0}}
```

## Available Fixtures

### basic_kiro_acp_session.jsonl

Minimal kiro-acp session demonstrating:
- ACP session initialization
- Event parsing (`build.task`, `build.done`)
- Completion detection (`LOOP_COMPLETE`)

### kiro_acp_tool_use.jsonl

kiro-acp session with tool invocations demonstrating:
- `execute_command` and `write_file` tool usage
- `read_file` tool usage
- Event parsing for tool-heavy workflows

## kiro-acp CLI Command

```bash
kiro-cli acp --trust-all-tools
```

Prompt is delivered via stdin using the agent-client-protocol (ACP) session/prompt flow.

## Recording New Fixtures

```bash
cargo run --bin ralph -- run -c ralph.kiro-acp.yml --record-session session.jsonl -p "your prompt"
```

## See Also

- `../kiro/README.md` — Kiro fixtures (pty mode)
