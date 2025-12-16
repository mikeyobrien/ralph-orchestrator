# Security Considerations

## Overview

Ralph Orchestrator executes AI agents with significant system access. This document outlines security considerations and best practices for safe operation.

## Threat Model

### Potential Risks

1. **Unintended Code Execution**
   - AI agents may generate and execute harmful code
   - File system modifications beyond project scope
   - System command execution

2. **Data Exposure**
   - API keys in prompts or code
   - Sensitive data in Git history
   - Credentials in state files

3. **Resource Exhaustion**
   - Infinite loops in generated code
   - Excessive API calls
   - Disk space consumption

4. **Supply Chain**
   - Compromised AI CLI tools
   - Malicious prompt injection
   - Dependency vulnerabilities

## Security Controls

Ralph implements multiple security layers to protect against threats:

```
 🔒 Security Defense Layers

   ╭───────────────────╮
   │    User Input     │
   ╰───────────────────╯
     │
     │
     ∨
   ┌───────────────────┐
   │ Input Validation  │
   └───────────────────┘
     │
     │
     ∨
   ┌───────────────────┐
   │ Process Isolation │
   └───────────────────┘
     │
     │
     ∨
   ┌───────────────────┐
   │  File Boundaries  │
   └───────────────────┘
     │
     │
     ∨
   ┌───────────────────┐
   │    Git Safety     │
   └───────────────────┘
     │
     │
     ∨
   ┌───────────────────┐
   │ Env Sanitization  │
   └───────────────────┘
     │
     │
     ∨
   ╭───────────────────╮
   │     AI Agent      │
   ╰───────────────────╯
```

<details>
<summary>graph-easy source</summary>

```
graph { label: "🔒 Security Defense Layers"; flow: south; }
[ User Input ] { shape: rounded; } -> [ Input Validation ]
[ Input Validation ] -> [ Process Isolation ]
[ Process Isolation ] -> [ File Boundaries ]
[ File Boundaries ] -> [ Git Safety ]
[ Git Safety ] -> [ Env Sanitization ]
[ Env Sanitization ] -> [ AI Agent ] { shape: rounded; }
```

</details>

### Process Isolation

Ralph runs AI agents in subprocesses with:

- Timeout protection (5 minutes default)
- Output size limits
- Error boundaries

```python
result = subprocess.run(
    cmd,
    capture_output=True,
    text=True,
    timeout=300,  # 5-minute timeout
    env=filtered_env  # Sanitized environment
)
```

### File System Boundaries

#### Restricted Paths

- Work within project directory
- No access to system files
- Preserve .git integrity

#### Safe Defaults

```python
# Validate paths stay within project
def validate_path(path):
    abs_path = os.path.abspath(path)
    project_path = os.path.abspath('.')
    return abs_path.startswith(project_path)
```

### Git Safety

#### Protected Operations

- No force pushes
- No branch deletion
- No history rewriting

#### Checkpoint-Only Commits

```bash
# Ralph only creates checkpoint commits
git add .
git commit -m "Ralph checkpoint: iteration N"
```

### Environment Sanitization

#### Filtered Variables

```python
SAFE_ENV_VARS = [
    'PATH', 'HOME', 'USER',
    'LANG', 'LC_ALL', 'TERM'
]

def get_safe_env():
    return {k: v for k, v in os.environ.items()
            if k in SAFE_ENV_VARS}
```

#### No Credential Exposure

- Never pass API keys through environment
- Agents should use their own credential stores
- No secrets in prompts or logs

## Best Practices

### 1. Prompt Security

#### DO

- Review prompts before execution
- Use specific, bounded instructions
- Include safety constraints

#### DON'T

- Include credentials in prompts
- Request system-level changes
- Use unbounded iterations

### 2. Agent Configuration

#### Claude

```bash
# Use restricted mode if available
claude --safe-mode PROMPT.md
```

#### Gemini

```bash
# Limit context and capabilities
gemini --no-web --no-exec PROMPT.md
```

### 3. Repository Setup

#### .gitignore

```gitignore
# Security-sensitive files
*.key
*.pem
.env
.env.*
secrets/
credentials/

# Ralph workspace
.agent/metrics/
.agent/logs/
```

#### Pre-commit Hooks

```yaml
# .pre-commit-config.yaml
repos:
  - repo: https://github.com/Yelp/detect-secrets
    hooks:
      - id: detect-secrets
        args: ["--baseline", ".secrets.baseline"]
```

### 4. Runtime Monitoring

#### Resource Limits

```python
# Set resource limits
import resource

# Limit memory usage to 1GB
resource.setrlimit(
    resource.RLIMIT_AS,
    (1024 * 1024 * 1024, -1)
)

# Limit CPU time to 1 hour
resource.setrlimit(
    resource.RLIMIT_CPU,
    (3600, -1)
)
```

#### Audit Logging

```python
import logging
import json

# Log all agent executions
logging.info(json.dumps({
    'event': 'agent_execution',
    'agent': agent_name,
    'timestamp': time.time(),
    'user': os.getenv('USER'),
    'prompt_hash': hashlib.sha256(prompt.encode()).hexdigest()
}))
```

## Security Checklist

### Before Running Ralph

- [ ] Review PROMPT.md for unsafe instructions
- [ ] Check no credentials in prompt
- [ ] Verify working directory is correct
- [ ] Ensure Git repository is backed up
- [ ] Confirm agent tools are up-to-date

### During Execution

- [ ] Monitor resource usage
- [ ] Watch for unexpected file changes
- [ ] Check agent output for anomalies
- [ ] Verify checkpoints are created
- [ ] Ensure no sensitive data in logs

### After Completion

- [ ] Review generated code for security issues
- [ ] Check Git history for exposed secrets
- [ ] Verify no system files were modified
- [ ] Clean up temporary files
- [ ] Rotate any potentially exposed credentials

## Incident Response

### If Compromise Suspected

1. **Immediate Actions**

   ```bash
   # Stop Ralph
   pkill -f ralph_orchestrator

   # Preserve evidence
   cp -r .agent /tmp/ralph-incident-$(date +%s)

   # Check for modifications
   git status
   git diff
   ```

2. **Investigation**
   - Review .agent/metrics/state\_\*.json
   - Check system logs
   - Examine Git history
   - Analyze agent outputs

3. **Recovery**

   ```bash
   # Reset to last known good state
   git reset --hard <last-good-commit>

   # Clean workspace
   rm -rf .agent

   # Rotate credentials if needed
   # Update API keys for affected services
   ```

## Sandboxing Options

### Docker Container

```dockerfile
FROM python:3.11-slim
RUN useradd -m -s /bin/bash ralph
USER ralph
WORKDIR /home/ralph/project
COPY --chown=ralph:ralph . .
CMD ["./ralph", "run"]
```

### Virtual Machine

```bash
# Run in VM with snapshot
vagrant up
vagrant ssh -c "cd /project && ./ralph run"
vagrant snapshot restore clean
```

### Restricted User

```bash
# Create restricted user
sudo useradd -m -s /bin/bash ralph-runner
sudo usermod -L ralph-runner  # No password login

# Run as restricted user
sudo -u ralph-runner ./ralph run
```

## API Key Management

### Secure Storage

#### Never Store Keys In

- PROMPT.md files
- Git repositories
- Environment variables in scripts
- Log files

#### Recommended Approaches

1. Agent-specific credential stores
2. System keychain/keyring
3. Encrypted vault (e.g., HashiCorp Vault)
4. Cloud secret managers

### Key Rotation

```bash
# Regular rotation schedule
# 1. Generate new keys
# 2. Update agent configurations
# 3. Test with new keys
# 4. Revoke old keys
```

## Compliance Considerations

### Data Privacy

- Don't process PII in prompts
- Sanitize outputs before sharing
- Comply with data residency requirements

### Audit Trail

- Maintain execution logs
- Track prompt modifications
- Document agent interactions

### Access Control

- Limit who can run Ralph
- Restrict agent permissions
- Control repository access

## Security Updates

Stay current with:

- AI CLI tool updates
- Python security patches
- Git security advisories
- Dependency vulnerabilities

```bash
# Check for updates
npm update -g @anthropic-ai/claude-code
pip install --upgrade subprocess
git --version
```

## Reporting Security Issues

If you discover a security vulnerability:

1. **Do NOT** open a public issue
2. Email security report to: <security@ralph-orchestrator.org>
3. Include:
   - Description of vulnerability
   - Steps to reproduce
   - Potential impact
   - Suggested fix (if any)

We aim to respond within 48 hours and provide fixes promptly.
