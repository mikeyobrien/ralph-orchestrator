# Migrating from Q Chat to Kiro CLI

The Amazon Q Developer CLI has been rebranded to **Kiro CLI** (v1.20+). Ralph Orchestrator v1.2.3+ fully supports this transition with the new `KiroAdapter`.

This guide helps you migrate your existing Q Chat configurations and workflows to Kiro CLI.

## Quick Summary

- **New Command:** `kiro-cli` (replaces `q`)
- **Adapter Flag:** `-a kiro` (replaces `-a q` or `-a qchat`)
- **Config Section:** `adapters.kiro` (replaces `adapters.qchat`)
- **Environment Vars:** `RALPH_KIRO_*` (replaces `RALPH_QCHAT_*`)

## Command Line Changes

To run Ralph with Kiro CLI:

```bash
# New way
ralph run -a kiro

# Legacy way (still works but deprecated)
ralph run -a q
ralph run -a qchat
```

If `kiro-cli` is not found, Ralph will automatically fall back to the `q` command if available, preserving backward compatibility.

## Configuration Changes

### ralph.yml

Update your `ralph.yml` configuration to use the new `kiro` section. The `q` and `qchat` sections are deprecated but still supported.

```yaml
# New Configuration
adapters:
  kiro:
    enabled: true
    timeout: 600
    args: []
    env: {}

# Deprecated Configuration
# adapters:
#   q:
#     enabled: true
#     timeout: 600
```

### Environment Variables

Update your environment variables to the new namespace:

| Legacy Variable | New Variable | Default |
|----------------|--------------|---------|
| `RALPH_QCHAT_COMMAND` | `RALPH_KIRO_COMMAND` | `kiro-cli` |
| `RALPH_QCHAT_TIMEOUT` | `RALPH_KIRO_TIMEOUT` | `600` |
| `RALPH_QCHAT_PROMPT_FILE` | `RALPH_KIRO_PROMPT_FILE` | `PROMPT.md` |
| `RALPH_QCHAT_TRUST_TOOLS` | `RALPH_KIRO_TRUST_TOOLS` | `true` |
| `RALPH_QCHAT_NO_INTERACTIVE` | `RALPH_KIRO_NO_INTERACTIVE` | `true` |

## System Paths

The Kiro CLI uses new directory paths for configuration and data. Ralph's adapter is aware of these changes, but you should update any manual setups or scripts.

| Component | Legacy Path (Q Chat) | New Path (Kiro) |
|-----------|----------------------|-----------------|
| **MCP Servers** | `~/.aws/amazonq/mcp.json` | `~/.kiro/settings/mcp.json` |
| **Prompts** | `~/.aws/amazonq/prompts` | `~/.kiro/prompts` |
| **Project Config** | `.amazonq/` | `.kiro/` |
| **Global Config** | `~/.aws/amazonq/` | `~/.kiro/` |
| **Logs** | `$TMPDIR/qchat-log` | `$TMPDIR/kiro-log` |

## Migration Steps

1.  **Install Kiro CLI**: Ensure you have installed the new Kiro CLI (version 1.20 or later).
2.  **Update Config**: Update your `ralph.yml` to replace `q` adapter config with `kiro`.
3.  **Update Scripts**: Change any CI/CD or startup scripts to use `ralph run -a kiro`.
4.  **Move MCP Config**: If you use custom MCP servers, move your `mcp.json` to the new location:
    ```bash
    mkdir -p ~/.kiro/settings
    cp ~/.aws/amazonq/mcp.json ~/.kiro/settings/mcp.json
    ```

## Backward Compatibility

Ralph maintains full backward compatibility:
- Running `-a q` still works (uses `KiroAdapter` internally with legacy settings).
- If `kiro-cli` is missing, it falls back to `q`.
- Old environment variables (`RALPH_QCHAT_*`) are NOT automatically read by the `KiroAdapter` to strictly separate configurations, but the legacy `QChatAdapter` (which reads them) redirects to `KiroAdapter` logic where possible or operates as a fallback.

> **Note:** The `QChatAdapter` class is now deprecated and emits a warning when initialized.
