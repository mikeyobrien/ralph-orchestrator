# Backends

Ralph supports multiple AI CLI backends. This guide covers setup and selection.

## Supported Backends

| Backend | CLI Tool | Notes |
|---------|----------|-------|
| Claude Code | `claude` | Recommended, primary support |
| Kiro | `kiro-cli` | Amazon/AWS |
| Gemini CLI | `gemini` | Google |
| Codex | `codex` | OpenAI |
| Amp | `amp` | Sourcegraph |
| Copilot CLI | `copilot` | GitHub |
| OpenCode | `opencode` | Community |

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
5. Amp
6. Copilot
7. OpenCode

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

Backend names (used in YAML and CLI flags): `claude`, `kiro`, `gemini`, `codex`, `amp`, `copilot`, `opencode`.

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
- Set one of: `OPENCODE_API_KEY`, `ANTHROPIC_API_KEY`, `OPENAI_API_KEY`
- OpenCode can proxy multiple providers; use the env var matching your provider

**Hat YAML:**
```yaml
hats:
  strategist:
    backend: "opencode"
```

**Doctor checks:**
- `opencode --version` must succeed
- Warns if none of `OPENCODE_API_KEY`, `ANTHROPIC_API_KEY`, `OPENAI_API_KEY` are set

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

| Feature | Claude | Kiro | Gemini | Codex |
|---------|--------|------|--------|-------|
| Streaming | Yes | Yes | Yes | Yes |
| Tool use | Full | Full | Partial | Partial |
| Context size | Large | Large | Large | Medium |
| Speed | Fast | Fast | Fast | Medium |
| Cost | $$ | $ | $ | $$ |

## Troubleshooting

### Backend Not Found

```
ERROR: No AI agents detected
```

**Solution:**
1. Install a supported backend
2. Ensure it's in your PATH
3. Test directly: `claude -p "test"`

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
