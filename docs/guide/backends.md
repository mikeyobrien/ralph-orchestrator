# Backends

Ralph supports multiple AI CLI backends. This guide covers setup and selection.

## Supported Backends

| Backend | CLI Tool | Notes |
|---------|----------|-------|
| Claude Code | `claude` | Recommended, primary support |
| Kiro | `kiro-cli` | Amazon/AWS |
| Gemini CLI | `gemini` | Google |
| Codex | `codex` | OpenAI |
| Forge | `forge` | Multi-provider terminal agent |
| Amp | `amp` | Sourcegraph |
| Copilot CLI | `copilot` | GitHub |
| OpenCode | `opencode` | Community |
| Pi | `pi` | Multi-provider |
| Roo | `roo` | Roo Code |
| OMP | `omp` | oh-my-pi, Pi-family |

## Auto-Detection

Ralph automatically detects installed backends:

```bash
ralph init
# Auto-detects available backend
```

Detection order (first available wins):
1. Claude
2. Kiro
3. Gemini
4. Codex
5. Forge
6. Amp
7. Copilot
8. OpenCode
9. Pi
10. Roo
11. OMP

OMP is last because it is the newest catalogued backend; this preserves the
detection order existing installations rely on.

## Explicit Selection

Override auto-detection:

```bash
# Via CLI
ralph init --backend kiro
ralph run --backend gemini

# Via config
# ralph.yml
cli:
  backend: "claude"
```

## Backend Setup

Each backend below includes:
- **Install** instructions
- **Auth & env vars** (API keys or login)
- **Hat YAML** configuration
- **`ralph doctor`** validation notes

Backend names (used in YAML and CLI flags): `claude`, `kiro`, `gemini`, `codex`, `forge`, `amp`, `copilot`, `opencode`, `pi`, `roo`, `omp`.

### Claude Code (`claude`)

The recommended backend with full feature support.

```bash
# Install
npm install -g @anthropic-ai/claude-code

# Authenticate
claude login

# Verify
claude --version
```

**Auth & env vars:**
- `claude login` (preferred)
- `ANTHROPIC_API_KEY` (used by `ralph doctor` auth hints)

**Hat YAML:**
```yaml
hats:
  planner:
    backend: "claude"
```

**Doctor checks:**
- `claude --version` must succeed
- Warns if `ANTHROPIC_API_KEY` is missing

**Features:**
- Full streaming support
- All hat features
- Memory integration

### Kiro (`kiro`)

Amazon/AWS AI assistant.

```bash
# Install
# Visit https://kiro.dev/

# Verify
kiro-cli --version
```

**Auth & env vars:**
- Complete Kiro CLI authentication (AWS/SSO) per Kiro docs
- `KIRO_API_KEY` (optional; used by `ralph doctor` auth hints)

**Hat YAML:**
```yaml
hats:
  coder:
    backend: "kiro"
```

**Kiro agent selection (optional):**
```yaml
hats:
  reviewer:
    backend:
      type: "kiro"
      agent: "codex"
```

**Doctor checks:**
- `kiro-cli --version` must succeed
- Warns if `KIRO_API_KEY` is missing (OK if you authenticated via CLI)

### Gemini CLI (`gemini`)

Google's AI CLI.

```bash
# Install
npm install -g @google/gemini-cli

# Configure API key
export GEMINI_API_KEY=your-key

# Verify
gemini --version
```

**Auth & env vars:**
- `GEMINI_API_KEY` (used by `ralph doctor` auth hints)

**Hat YAML:**
```yaml
hats:
  analyst:
    backend: "gemini"
```

**Doctor checks:**
- `gemini --version` must succeed
- Warns if `GEMINI_API_KEY` is missing

### Codex (`codex`)

OpenAI's code-focused model.

```bash
# Install
# Visit https://github.com/openai/codex

# Configure
export OPENAI_API_KEY=your-key

# Verify
codex --version
```

**Auth & env vars:**
- `OPENAI_API_KEY` or `CODEX_API_KEY` (either satisfies `ralph doctor` auth hints)

**Hat YAML:**
```yaml
hats:
  coder:
    backend: "codex"
```

**Doctor checks:**
- `codex --version` must succeed
- Warns if neither `OPENAI_API_KEY` nor `CODEX_API_KEY` is set

### Forge (`forge`)

Multi-provider terminal AI agent.

```bash
# Install
curl -fsSL https://forgecode.dev/cli | sh

# Authenticate
forge provider login

# Verify
forge --version
```

**Auth & env vars:**
- Configure providers through `forge provider login` or Forge's provider configuration
- No auth env vars are checked by `ralph doctor` for Forge

**Hat YAML:**
```yaml
hats:
  coder:
    backend: "forge"
```

**Forge agent selection (optional):**
```yaml
hats:
  reviewer:
    backend:
      type: "forge"
      args: ["--agent", "sage"]
```

**Doctor checks:**
- `forge --version` must succeed

**Execution modes:**
- Headless Ralph runs call `forge -p "<prompt>"`
- Interactive Forge is launched as `forge`; Forge does not support initial prompt injection in no-arg interactive mode

### Amp (`amp`)

Sourcegraph's AI assistant.

```bash
# Install
# Visit https://github.com/sourcegraph/amp

# Verify
amp --version
```

**Auth & env vars:**
- Authenticate via `amp` CLI per Sourcegraph docs
- No auth env vars are checked by `ralph doctor` for Amp

**Hat YAML:**
```yaml
hats:
  helper:
    backend: "amp"
```

**Doctor checks:**
- `amp --version` must succeed

### Copilot CLI (`copilot`)

GitHub's AI assistant.

```bash
# Install
npm install -g @github/copilot

# Authenticate
copilot auth login

# Verify
copilot --version
```

**Auth & env vars:**
- Authenticate via Copilot CLI (`copilot auth login` or `gh auth login`)
- No auth env vars are checked by `ralph doctor` for Copilot

**Hat YAML:**
```yaml
hats:
  reviewer:
    backend: "copilot"
```

**Doctor checks:**
- `copilot --version` must succeed

### OpenCode (`opencode`)

Community AI CLI.

```bash
# Install
curl -fsSL https://opencode.ai/install | bash

# Verify
opencode --version
```

**Auth & env vars:**
- Set one of: `OPENCODE_API_KEY`, `ORCAROUTER_API_KEY`, `ANTHROPIC_API_KEY`, `OPENAI_API_KEY`
- OpenCode can proxy multiple providers; use the env var matching your provider

**OrcaRouter (optional):**

OpenCode reads provider definitions from the [models.dev](https://models.dev) registry, which includes OrcaRouter as a first-class provider (`ORCAROUTER_API_KEY`). Route any `orcarouter/*` model through the [OrcaRouter](https://www.orcarouter.ai) gateway:

```yaml
hats:
  strategist:
    backend:
      type: "opencode"
      args: ["-m", "orcarouter/auto"]
```

**Hat YAML:**
```yaml
hats:
  strategist:
    backend: "opencode"
```

**Doctor checks:**
- `opencode --version` must succeed
- Warns if none of `OPENCODE_API_KEY`, `ORCAROUTER_API_KEY`, `ANTHROPIC_API_KEY`, `OPENAI_API_KEY` are set

### Pi (`pi`)

Multi-provider AI coding assistant.

```bash
# Install
npm install -g @earendil-works/pi-coding-agent

# Verify
pi --version
```

**Auth & env vars:**
- Set one of: `ORCAROUTER_API_KEY`, `ANTHROPIC_API_KEY`, `OPENAI_API_KEY`, `GEMINI_API_KEY`, or any supported provider key
- Pi routes to the provider specified via `--provider` (default: google)
- Pass API key explicitly with `--api-key` or rely on provider-specific env vars

**OrcaRouter (optional):**

Pi supports [models.dev](https://models.dev) providers, including OrcaRouter as a named provider. Route through the [OrcaRouter](https://www.orcarouter.ai) gateway with `--provider orcarouter`:

```yaml
hats:
  coder:
    backend:
      type: "pi"
      args: ["--provider", "orcarouter", "--model", "orcarouter/auto"]
```

**Hat YAML:**
```yaml
hats:
  coder:
    backend: "pi"
```

**Pi provider selection (optional):**
```yaml
hats:
  coder:
    backend:
      type: "pi"
      args: ["--provider", "anthropic", "--model", "claude-sonnet-4"]
```

**Doctor checks:**
- `pi --version` must succeed
- Warns if no provider API key is set

### OMP (`omp`)

[OMP (oh-my-pi)](https://github.com/can1357/oh-my-pi) is a multi-provider coding
agent and the newest member of the Pi family. Ralph routes OMP through the same
Pi-family stream processor while keeping its output identity distinct, so a
command override preserves the correct `OMP` label.

```bash
# Install — visit the project for current method (Node/Bun)
# https://github.com/can1357/oh-my-pi

# Verify (tested baseline: omp/17.2.10)
omp --version
```

**Auth & env vars:**
- OMP owns its own providers, models, credentials, tools, extensions, skills,
  rules, and approval policy. Ralph only selects OMP and threads arguments
  through; it never interprets them and performs **no** Ralph-side OMP auth.
- Authenticate via OMP itself (its in-app login or the provider keys OMP reads).

**Exact argv (what Ralph launches):**
- Headless iteration: `omp -p --mode json --no-session --auto-approve [--model …] <prompt>`
- Interactive (`ralph plan --backend omp`): `omp --no-session <prompt>` — the
  autonomous flags (`-p`, `--mode json`, `--auto-approve`) are intentionally
  omitted so you keep OMP's TUI and approval prompts.

**`--auto-approve` vs OMP's yolo mode:** Ralph passes the documented
`--auto-approve` flag for autonomous iterations. This is distinct from OMP's own
`--approval-mode=yolo` alias; Ralph does not depend on OMP's mutable default
approval mode. To set an explicit OMP permission policy, pass the mode through
`cli.args` (e.g. `args: ["--approval-mode", "write"]`) — Ralph forwards it
verbatim and never interprets it.

**Hat YAML / per-hat args:**
```yaml
hats:
  coder:
    backend: "omp"
    args: ["--provider", "anthropic", "--model", "claude-sonnet-4"]
```

Or compose the repo-root overlay with a builtin hat collection (OMP defines no
hat collection of its own):
```bash
ralph run -c ralph.omp.yml -H builtin:code-assist -p "your task"
```

**Doctor checks:**
- `omp --version` must succeed
- No Ralph-side auth check (OMP authenticates itself)

**Notes & limits:**
- Tested baseline: `omp/17.2.10`. Ralph documents this version but does **not**
  enforce a hard minimum. A JSON protocol mismatch surfaces as a protocol error
  (see [Troubleshooting](../reference/troubleshooting.md)).
- Argument pass-through: `cli.args` are forwarded verbatim to every OMP
  invocation; OMP interprets them (`--provider`, `--model`, `--profile`,
  `--max-time`, …).
- `--no-session` makes each run ephemeral; Ralph does not persist or resume OMP
  sessions.
- ACP/RPC: not supported. Ralph uses the OMP JSON stream protocol only.

## Per-Hat Backend Override

Different hats can use different backends:

```yaml
hats:
  planner:
    backend: "claude"  # Use Claude for planning
    triggers: ["task.start"]
    instructions: "Create a plan..."

  coder:
    backend: "kiro"    # Use Kiro for coding
    triggers: ["plan.ready"]
    instructions: "Implement..."
```

## Custom Backends

For unsupported CLIs, use the custom backend:

```yaml
cli:
  backend: "custom"
  custom_command: "my-ai-cli"
  prompt_mode: "arg"  # or "stdin"
```

**Prompt modes:**

| Mode | How Prompt is Passed |
|------|---------------------|
| `arg` | `my-ai-cli -p "prompt"` |
| `stdin` | `echo "prompt" \| my-ai-cli` |

## Backend Comparison

| Feature | Claude | Kiro | Gemini | Codex | Pi | OMP |
|---------|--------|------|--------|-------|----|-----|
| Streaming | Yes | Yes | Yes | Yes | Yes | Yes |
| Tool use | Full | Full | Partial | Partial | Full | Full |
| Context size | Large | Large | Large | Medium | Large | Large |
| Speed | Fast | Fast | Fast | Medium | Fast | Medium |
| Cost | $$ | $ | $ | $$ | $ | $ |

## Troubleshooting

### Backend Not Found

```
ERROR: No AI agents detected
```

**Solution:**
1. Install a supported backend
2. Ensure it's in your PATH
3. Test directly: `claude -p "test"` or `pi -p "test"`

### Authentication Failed

```
ERROR: Authentication required
```

**Solution:**
```bash
# Claude
claude login

# Copilot
copilot auth login

# Gemini - set API key
export GEMINI_API_KEY=your-key

# Pi - set provider API key
export ANTHROPIC_API_KEY=your-key
```

If the CLI is already authenticated but `ralph doctor` still warns, ensure the
expected env vars above are set (doctor checks are hints, not hard failures).

### Wrong Backend Used

```bash
# Force specific backend
ralph run --backend claude

# Or set in config
cli:
  backend: "claude"
```

### Backend Hanging

Some backends need interactive authentication on first run:

```bash
# Run backend directly first
claude -p "test"

# Then use with Ralph
ralph run
```

## Best Practices

1. **Pick one primary backend** — Consistency helps
2. **Test backend directly** — Before using with Ralph
3. **Use per-hat overrides sparingly** — Can complicate debugging
4. **Keep backends updated** — New features, bug fixes

## Next Steps

- Configure [Presets](presets.md) for your workflow
- Learn about [Cost Management](cost-management.md)
- Explore [Writing Prompts](prompts.md)
