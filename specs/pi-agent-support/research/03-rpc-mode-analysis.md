# Research: Pi's RPC Mode for Ralph Integration

## Summary

Pi's RPC mode offers rich bidirectional control (steering, follow-ups, abort, model switching, compaction) that could enhance Ralph's orchestration. However, it's significantly more complex than the CLI execution model and likely overkill for v1.

## RPC Capabilities

| Feature | CLI mode | RPC mode |
|---------|----------|----------|
| Send prompt | ✅ (args) | ✅ (JSON command) |
| Stream output | ✅ (NDJSON) | ✅ (events) |
| Abort | ✅ (SIGTERM) | ✅ (abort command) |
| Steer mid-run | ❌ | ✅ (steer command) |
| Follow-up queue | ❌ | ✅ (follow_up command) |
| Model switching | ❌ | ✅ (set_model) |
| Compaction | ❌ | ✅ (compact command) |
| Session management | ❌ | ✅ (new_session, switch) |
| Cost/usage stats | ✅ (in events) | ✅ (get_session_stats) |
| Extension UI | ❌ | ✅ (request/response) |

## RPC Protocol

Bidirectional JSON over stdin/stdout:
- **Commands** (to stdin): `{"type": "prompt", "message": "..."}` 
- **Responses**: `{"type": "response", "command": "prompt", "success": true}`
- **Events** (from stdout): Same event types as `--mode json`

Process lifecycle:
```bash
pi --mode rpc --no-session
# Process stays alive, accepts multiple prompts
```

## How Ralph Could Use RPC

### Scenario: Multi-iteration without process restart

Currently Ralph spawns a new CLI process per iteration. With RPC:
1. Spawn `pi --mode rpc` once
2. Send prompts via stdin between iterations
3. Keep session context across iterations (pi handles compaction)
4. Use `steer` to inject guidance mid-run (RObot integration)

### Scenario: Human-in-the-loop via RPC

Ralph's RObot system could use `steer` instead of injecting guidance into the next prompt:
- Human sends message → Ralph sends `{"type": "steer", "message": "..."}` to pi
- Pi interrupts current work and processes the steering message

## Complexity Analysis

RPC integration would require:
1. **Process lifecycle management**: Keep pi alive across iterations (vs spawn/kill per iteration)
2. **Bidirectional I/O**: Read events from stdout while writing commands to stdin (concurrent)
3. **Response correlation**: Match `id` fields between commands and responses
4. **State tracking**: Know when agent is streaming vs idle to decide prompt/steer/followUp
5. **Error recovery**: Handle process crashes, restart logic
6. **Extension UI handling**: Respond to extension_ui_request events or ignore them

This is a fundamentally different execution model from Ralph's current "spawn process, read output, kill process" approach.

## Recommendation

### v1: CLI mode (--mode json + -p)
- Use `pi -p --mode json --no-session` for headless execution
- Parse NDJSON with `PiStreamParser`
- Fits cleanly into existing `PtyExecutor` / `CliExecutor` model
- No architectural changes to Ralph's core loop

### v2 (future): RPC mode
- Worth exploring for long-running orchestration
- Could enable session persistence across iterations (token savings)
- Could enable real-time steering from RObot
- Requires new executor type (`RpcExecutor`) alongside `PtyExecutor` / `CliExecutor`
- Consider when Ralph adds persistent agent sessions

## Conclusion

RPC is powerful but premature for initial pi support. The CLI mode gives Ralph everything it needs for orchestration loops. RPC should be a follow-up feature when there's a concrete use case (e.g., persistent sessions across iterations, real-time steering).
