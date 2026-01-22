# Ralph Features to Test (E2E)

## Core Features

### 1. Backend Connectivity
- [ ] Claude CLI invocation
- [ ] Kiro CLI invocation
- [ ] OpenCode CLI invocation
- [ ] Auto-detection of available backends

### 2. Prompt Handling
- [ ] Inline prompt via `-p`
- [ ] File prompt via `-P` or `prompt_file`
- [ ] Large prompt handling (>7000 chars for Claude)
- [ ] Prompt effectiveness (agent follows instructions)

### 3. Orchestration Loop
- [ ] Single iteration completes
- [ ] Multiple iterations progress
- [ ] Completion promise detection (LOOP_COMPLETE)
- [ ] Max iterations termination
- [ ] Max runtime termination

### 4. Event System
- [ ] Event XML tag parsing from output
- [ ] Event publication to EventBus
- [ ] Event consumption by hats
- [ ] Backpressure validation (build.done evidence)

### 5. Tool Use
- [ ] Tool invocation works
- [ ] Tool results returned correctly
- [ ] Multi-tool sequences work

### 6. Streaming & Output
- [ ] NDJSON parsing (Claude)
- [ ] Text output parsing (other backends)
- [ ] Real-time streaming works

### 7. Error Handling
- [ ] Backend unavailable
- [ ] Invalid credentials
- [ ] Timeout handling
- [ ] Malformed output handling

### 8. State Management
- [ ] Scratchpad persists between iterations
- [ ] Session recording works
- [ ] Resume from scratchpad (`--continue`)

---

## Backend-Specific Tests

### Claude
- [ ] Stream-json output format
- [ ] Tool use with NDJSON events
- [ ] Large prompt temp file handling

### Kiro
- [ ] Basic prompt/response
- [ ] Custom agent invocation (`--agent`)
- [ ] Trust-all-tools mode

### OpenCode
- [ ] Basic prompt/response
- [ ] `run` subcommand works

---

## Prompt Effectiveness Tests

Per the writing-skills principles, we need to validate:

1. **Agent follows core instructions** - Does it read specs, update scratchpad?
2. **Backpressure is respected** - Does it run tests before claiming done?
3. **Events are published correctly** - Does it emit proper event XML?
4. **Completion is accurate** - Does it only emit LOOP_COMPLETE when truly done?
