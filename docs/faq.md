# Frequently Asked Questions

## General Questions

### What is Ralph Orchestrator?

Ralph Orchestrator is an implementation of the Ralph Wiggum technique - a simple but effective pattern for autonomous task completion using AI agents. It continuously runs an AI agent against a prompt file until the task is marked complete or limits are reached.

### Why is it called "Ralph Wiggum"?

The technique is named after the Simpsons character Ralph Wiggum, whose quote "Me fail English? That's unpossible!" embodies the philosophy of deterministic failure in an unpredictable world. The system keeps trying until it succeeds, embracing the "unpossible."

### Who created Ralph Orchestrator?

The Ralph Wiggum technique was created by [Geoffrey Huntley](https://ghuntley.com/ralph/). This implementation builds on his concept with additional features like multiple agent support, checkpointing, and comprehensive testing.

### What AI agents does it support?

Ralph Orchestrator currently supports:

- **Claude** (Anthropic Claude Code CLI)
- **Gemini** (Google Gemini CLI)
- **Q Chat** (Q CLI tool)

The system auto-detects available agents and can automatically select the best one.

## Installation & Setup

### Do I need all three AI agents installed?

No, you only need at least one AI agent installed. Ralph will auto-detect which agents are available and use them accordingly.

### How do I install the AI agents?

```bash
# Claude
npm install -g @anthropic-ai/claude-code

# Gemini
npm install -g @google/gemini-cli

# Q Chat
# Follow instructions at https://github.com/qchat/qchat
```

### What are the system requirements?

- **OS**: Linux, macOS, or Windows (with WSL)
- **Python**: 3.9 or higher
- **Git**: 2.25 or higher
- **Memory**: 4GB minimum, 8GB recommended
- **Storage**: 20GB available space

### Can I run Ralph in Docker?

Yes! A Dockerfile is provided:

```bash
docker build -t ralph-orchestrator .
docker run -v $(pwd):/workspace ralph-orchestrator
```

## Usage Questions

### How do I know when Ralph is done?

Ralph stops when:

1. Maximum iterations are reached (default: 100)
2. Maximum runtime is exceeded (default: 4 hours)
3. Cost limits are reached (default: $50)
4. Too many consecutive errors occur
5. A completion marker is detected
6. Loop detection triggers (repetitive outputs)

### How do I signal task completion?

Add a checkbox marker to your PROMPT.md:

```markdown
- [x] TASK_COMPLETE
```

Ralph will detect this marker and stop orchestration immediately. This allows the AI agent to signal "I'm done" instead of relying solely on iteration limits.

**Important**: The marker must be in checkbox format (`- [x]` or `[x]`), not plain text.

### What triggers loop detection?

Loop detection triggers when the current agent output is ≥90% similar (using fuzzy string matching) to any of the last 5 outputs. This prevents infinite loops where an agent produces essentially the same response repeatedly.

Common triggers:

- Agent stuck on the same task
- Oscillating between similar approaches
- Consistent API error messages
- Placeholder "still working" responses

When triggered, you'll see: `WARNING - Loop detected: 92.3% similarity to previous output`

### Can I disable loop detection?

Loop detection cannot be disabled directly, but it only triggers on highly similar outputs (≥90% threshold). To avoid false positives:

1. Ensure agent outputs include iteration-specific details
2. Add progress indicators that change each iteration
3. Check if agent is stuck on the same subtask
4. Refine your prompt to encourage varied responses

See [Loop Detection](advanced/loop-detection.md) for detailed documentation.

### What should I put in PROMPT.md?

Write clear, specific requirements with measurable success criteria. Include:

- Task description
- Requirements list
- Success criteria
- Example inputs/outputs (if applicable)
- File structure (for complex projects)

### How many iterations does it typically take?

This varies by task complexity:

- Simple functions: 5-10 iterations
- Web APIs: 20-30 iterations
- Complex applications: 50-100 iterations

### Can I resume if Ralph stops?

Yes! Ralph saves state and can resume from where it left off:

```bash
# Ralph will automatically resume from last state
ralph run
```

### How do I monitor progress?

```bash
# Check status
ralph status

# Watch in real-time
watch -n 5 'ralph status'

# View logs
tail -f .agent/logs/ralph.log
```

## Configuration

### How do I change the default agent?

Edit `ralph.json`:

```json
{
  "agent": "claude" // or "gemini", "q", "auto"
}
```

Or use command line:

```bash
ralph run --agent claude
```

### Can I set custom iteration limits?

Yes, in multiple ways:

```bash
# Command line
ralph run --max-iterations 50

# Config file (ralph.json)
{
  "max_iterations": 50
}

# Environment variable
export RALPH_MAX_ITERATIONS=50
```

### What is checkpoint interval?

Checkpoint interval determines how often Ralph creates Git commits to save progress. Default is every 5 iterations.

### How do I disable Git operations?

```bash
ralph run --no-git
```

Or in config:

```json
{
  "git_enabled": false
}
```

## Troubleshooting

### Why isn't my task completing?

Common reasons:

1. Task description is unclear
2. Requirements are too complex for single prompt
3. Agent doesn't understand the format
4. Missing resources or dependencies

### Ralph keeps hitting the same error

Try:

1. Simplifying the task
2. Adding clarification to PROMPT.md
3. Using a different agent
4. Manually fixing the specific issue

### How do I reduce API costs?

1. Use more efficient agents (Q is free)
2. Reduce max iterations
3. Write clearer prompts to reduce iterations
4. Use checkpoint recovery instead of restarting

### Can I use Ralph offline?

No, Ralph requires internet access to communicate with AI agent APIs. However, you can use a local AI model if you create a compatible CLI wrapper.

## Advanced Usage

### Can I extend Ralph with custom agents?

Yes! Implement the Agent interface:

```python
class MyAgent(Agent):
    def __init__(self):
        super().__init__('myagent', 'myagent-cli')

    def execute(self, prompt_file):
        # Your implementation
        pass
```

### Can I run multiple Ralph instances?

Yes, but in different directories to avoid conflicts:

```bash
# Terminal 1
cd project1 && ralph run

# Terminal 2
cd project2 && ralph run
```

### How do I integrate Ralph into CI/CD?

```yaml
# GitHub Actions example
- name: Run Ralph
  run: |
    ralph run --max-iterations 50 --dry-run

- name: Check completion
  run: |
    ralph status
```

### Can Ralph modify files outside the project?

By default, Ralph works within the current directory. For safety, it's designed not to modify system files or files outside the project directory.

## Best Practices

### What makes a good prompt?

Good prompts are:

- **Specific**: Clear requirements and constraints
- **Measurable**: Defined success criteria
- **Structured**: Organized with sections
- **Complete**: All necessary information included

### Should I commit PROMPT.md to Git?

Yes! Version control your prompts to:

- Track requirement changes
- Share with team members
- Reproduce results
- Build a prompt library

### How often should I check on Ralph?

For typical tasks:

- First 5 iterations: Watch closely
- 5-20 iterations: Check every 5 minutes
- 20+ iterations: Check every 15 minutes

### When should I intervene manually?

Intervene when:

- Same error repeats 3+ times
- Progress stalls for 10+ iterations
- Output diverges from requirements
- Resource usage is excessive

## Cost & Performance

### How much does it cost to run Ralph?

Approximate costs per task:

- Simple function: $0.05-0.10
- Web API: $0.20-0.30
- Complex application: $0.50-1.00

(Varies by agent and API pricing)

### Which agent is fastest?

Generally:

1. **Q**: Fastest response time
2. **Gemini**: Balanced speed and capability
3. **Claude**: Most capable but slower

### How can I speed up execution?

1. Use simpler prompts
2. Reduce context size
3. Choose faster agents
4. Increase system resources
5. Disable unnecessary features

### Does Ralph work with rate limits?

Yes, Ralph handles rate limits with:

- Exponential backoff
- Retry logic
- Agent switching (if multiple available)

## Security & Privacy

### Is my code sent to AI providers?

Yes, the contents of PROMPT.md and relevant files are sent to the AI agent's API. Never include sensitive data like:

- API keys
- Passwords
- Personal information
- Proprietary code

### How do I protect sensitive information?

1. Use environment variables for secrets
2. Add sensitive files to .gitignore
3. Review prompts before running
4. Use local development credentials
5. Audit generated code

### Can Ralph access my system?

Ralph runs AI agents in subprocesses with:

- Timeout protection
- Resource limits
- Working directory restrictions

However, agents can execute code, so always review outputs.

### Is it safe to run Ralph on production servers?

Not recommended. Ralph is designed for development environments. For production, use Ralph locally and deploy tested code.

## Community & Support

### How do I report bugs?

1. Check existing issues on GitHub
2. Create detailed bug report with:
   - Ralph version
   - Error messages
   - Steps to reproduce
   - System information

### Can I contribute to Ralph?

Yes! We welcome contributions:

- Bug fixes
- New features
- Documentation improvements
- Agent integrations

See [CONTRIBUTING.md](contributing.md) for guidelines.

### Where can I get help?

- **GitHub Issues**: Bug reports and feature requests
- **GitHub Discussions**: Questions and community help
- **Discord**: Real-time chat with community

### Is there commercial support?

Currently, Ralph Orchestrator is community-supported open source software. Commercial support may be available in the future.
