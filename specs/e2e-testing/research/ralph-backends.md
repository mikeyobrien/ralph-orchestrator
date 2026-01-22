# Ralph Backend Implementations

## Supported Backends

| Backend | Command | Headless Flag | Interactive Flag | Output Format |
|---------|---------|---------------|------------------|---------------|
| **Claude** | `claude` | `-p <prompt>` | positional arg | NDJSON Stream |
| **Kiro** | `kiro-cli chat` | `--no-interactive` | (remove flag) | Text |
| **Gemini** | `gemini` | `-p <prompt>` | `-i <prompt>` | Text |
| **Codex** | `codex exec` | `--full-auto` | (remove subcommand) | Text |
| **Amp** | `amp` | `--dangerously-allow-all -x` | (remove flag) | Text |
| **Copilot** | `copilot` | `--allow-all-tools -p` | (remove flag) | Text |
| **OpenCode** | `opencode run` | positional arg | positional arg | Text |
| **Custom** | configurable | configurable | configurable | configurable |

---

## Backend Capabilities Matrix

| Capability | Claude | Kiro | Gemini | Codex | Amp | Copilot | OpenCode |
|------------|--------|------|--------|-------|-----|---------|----------|
| **Autonomous Mode** | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| **Interactive Mode** | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |
| **NDJSON Streaming** | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ | ❌ |
| **PTY/TUI Support** | ✅ | ⚠️ | ⚠️ | ⚠️ | ⚠️ | ✅ | ⚠️ |
| **Custom Agents** | ❌ | ✅ | ❌ | ❌ | ❌ | ❌ | ❌ |
| **MCP Servers** | ❌ | ✅ | Limited | ✅ | ✅ | ❌ | ❌ |
| **Auto-Detection** | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ |

---

## Auto-Approval Flags

| Backend | Flag |
|---------|------|
| Claude | `--dangerously-skip-permissions` |
| Kiro | `--trust-all-tools` |
| Gemini | `--yolo` |
| Codex | `--full-auto` |
| Amp | `--dangerously-allow-all` |
| Copilot | `--allow-all-tools` |

---

## E2E Testing Implications

### Must Test Per Backend:
1. **Basic connectivity** - Can we invoke the CLI and get a response?
2. **Prompt passing** - Does the prompt reach the backend correctly?
3. **Output parsing** - Can Ralph parse the backend's output?
4. **Event extraction** - Are events extracted from agent output?
5. **Tool use** - Does tool invocation work end-to-end?
6. **Error handling** - Do errors propagate correctly?

### Backend-Specific Considerations:
- **Claude**: NDJSON streaming requires special parsing
- **Kiro**: Custom agent support adds complexity
- **OpenCode**: Simpler interface, good baseline test
