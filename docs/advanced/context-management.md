# Context Management

## Overview

Managing context windows effectively is crucial for Hats's success. AI agents have limited context windows, and exceeding them can cause failures or degraded performance.

## Context Window Limits

### Current Agent Limits

| Agent | Context Window | Token Limit | Approximate Characters |
|-------|---------------|-------------|----------------------|
| Claude | 200K tokens | 200,000 | ~800,000 chars |
| Gemini | 32K tokens | 32,768 | ~130,000 chars |
| Q Chat | 8K tokens | 8,192 | ~32,000 chars |

## Context Components

### What Consumes Context

1. **PROMPT.md file** - The task description
2. **Previous outputs** - Agent responses
3. **File contents** - Code being modified
4. **System messages** - Instructions to agent
5. **Error messages** - Debugging information

### Context Calculation

```python
def estimate_context_usage(prompt_file, workspace_files):
    """Estimate total context usage"""
    total_chars = 0
    
    # Prompt file
    with open(prompt_file) as f:
        total_chars += len(f.read())
    
    # Workspace files
    for file in workspace_files:
        if os.path.exists(file):
            with open(file) as f:
                total_chars += len(f.read())
    
    # Estimate tokens (rough: 4 chars = 1 token)
    estimated_tokens = total_chars / 4
    
    return {
        'characters': total_chars,
        'estimated_tokens': estimated_tokens,
        'percentage_used': {
            'claude': (estimated_tokens / 200000) * 100,
            'gemini': (estimated_tokens / 32768) * 100,
            'q': (estimated_tokens / 8192) * 100
        }
    }
```

## Context Optimization Strategies

### 1. Prompt Optimization

#### Keep Prompts Concise
```markdown
# Bad - Too verbose
Create a comprehensive Python application that implements a calculator
with extensive error handling, logging capabilities, user-friendly
interface, and support for basic arithmetic operations including
addition, subtraction, multiplication, and division...

# Good - Concise and clear
Create a Python calculator with:
- Basic operations: +, -, *, /
- Error handling for division by zero
- Simple CLI interface
```

#### Use Structured Format
```markdown
# Task: Calculator Module

## Requirements:
- [ ] Basic operations (add, subtract, multiply, divide)
- [ ] Input validation
- [ ] Unit tests

## Constraints:
- Python 3.11+
- No external dependencies
- 100% test coverage
```

### 2. File Management

#### Split Large Files
```python
# Instead of one large file
# calculator.py (5000 lines)

# Use modular structure
# calculator/
#   ├── __init__.py
#   ├── operations.py (500 lines)
#   ├── validators.py (300 lines)
#   ├── interface.py (400 lines)
#   └── utils.py (200 lines)
```

#### Exclude Unnecessary Files
```python
# .agent/config.json
{
  "exclude_patterns": [
    "*.pyc",
    "__pycache__",
    "*.log",
    "test_*.py",  # Exclude during implementation
    "docs/",      # Exclude documentation
    ".git/"       # Never include git directory
  ]
}
```

### 3. Incremental Processing

#### Task Decomposition
```markdown
# Instead of one large task
"Build a complete web application"

# Break into phases
Phase 1: Create project structure
Phase 2: Implement data models
Phase 3: Add API endpoints
Phase 4: Build frontend
Phase 5: Add tests
```

#### Checkpoint Strategy
```python
def create_context_aware_checkpoint(iteration, context_usage):
    """Create checkpoint when context is getting full"""
    if context_usage['percentage_used']['current_agent'] > 70:
        # Reset context by creating checkpoint
        create_checkpoint(iteration)
        # Clear working memory
        clear_agent_memory()
        # Summarize progress
        create_progress_summary()
```

### 4. Context Window Sliding

#### Maintain Rolling Context
```python
class ContextManager:
    def __init__(self, max_history=5):
        self.history = []
        self.max_history = max_history
    
    def add_iteration(self, prompt, response):
        """Add iteration to history with sliding window"""
        self.history.append({
            'prompt': prompt,
            'response': response,
            'timestamp': time.time()
        })
        
        # Keep only recent history
        if len(self.history) > self.max_history:
            self.history.pop(0)
    
    def get_context(self):
        """Get current context for agent"""
        # Include only recent iterations
        return '\n'.join([
            f"Iteration {i+1}:\n{h['response'][:500]}..."
            for i, h in enumerate(self.history[-3:])
        ])
```

## Advanced Techniques

### 1. Context Compression

```python
def compress_context(text, max_length=1000):
    """Compress text while preserving key information"""
    if len(text) <= max_length:
        return text
    
    # Extract key sections
    lines = text.split('\n')
    important_lines = []
    
    for line in lines:
        # Keep headers, errors, and key code
        if any(marker in line for marker in 
               ['#', 'def ', 'class ', 'ERROR', 'TODO']):
            important_lines.append(line)
    
    compressed = '\n'.join(important_lines)
    
    # If still too long, truncate with summary
    if len(compressed) > max_length:
        return compressed[:max_length-20] + "\n... (truncated)"
    
    return compressed
```

### 2. Semantic Chunking

```python
def chunk_by_semantics(code_file):
    """Split code into semantic chunks"""
    chunks = []
    current_chunk = []
    
    with open(code_file) as f:
        lines = f.readlines()
    
    for line in lines:
        current_chunk.append(line)
        
        # End chunk at logical boundaries
        if line.strip().startswith('def ') or \
           line.strip().startswith('class '):
            if len(current_chunk) > 1:
                chunks.append(''.join(current_chunk[:-1]))
                current_chunk = [line]
    
    # Add remaining
    if current_chunk:
        chunks.append(''.join(current_chunk))
    
    return chunks
```

### 3. Progressive Disclosure

```python
class ProgressiveContext:
    """Gradually reveal context as needed"""
    
    def __init__(self):
        self.levels = {
            'summary': 100,      # Brief summary
            'outline': 500,      # Structure only
            'essential': 2000,   # Key components
            'detailed': 10000,   # Full details
        }
    
    def get_context_at_level(self, content, level='essential'):
        """Get context at specified detail level"""
        max_chars = self.levels[level]
        
        if level == 'summary':
            return self.create_summary(content, max_chars)
        elif level == 'outline':
            return self.extract_outline(content, max_chars)
        elif level == 'essential':
            return self.extract_essential(content, max_chars)
        else:
            return content[:max_chars]
```

## Context Monitoring

### Track Usage

```python
def monitor_context_usage():
    """Monitor and log context usage"""
    usage = estimate_context_usage('PROMPT.md', glob.glob('*.py'))
    
    # Log warning if approaching limits
    for agent, percentage in usage['percentage_used'].items():
        if percentage > 80:
            logging.warning(
                f"Context usage for {agent}: {percentage:.1f}% - "
                f"Consider optimization"
            )
    
    # Save metrics
    with open('.agent/metrics/context_usage.json', 'w') as f:
        json.dump(usage, f, indent=2)
    
    return usage
```

### Visualization

```python
import matplotlib.pyplot as plt

def visualize_context_usage(iterations_data):
    """Plot context usage over iterations"""
    iterations = [d['iteration'] for d in iterations_data]
    usage = [d['context_percentage'] for d in iterations_data]
    
    plt.figure(figsize=(10, 6))
    plt.plot(iterations, usage, marker='o')
    plt.axhline(y=80, color='orange', linestyle='--', label='Warning')
    plt.axhline(y=100, color='red', linestyle='--', label='Limit')
    plt.xlabel('Iteration')
    plt.ylabel('Context Usage (%)')
    plt.title('Context Window Usage Over Time')
    plt.legend()
    plt.savefig('.agent/context_usage.png')
```

## Best Practices

### 1. Start Small
- Begin with minimal context
- Add detail only when needed
- Remove completed sections

### 2. Use References
```markdown
# Instead of including full code
See `calculator.py` for implementation details

# Reference specific sections
Refer to lines 45-67 in `utils.py` for error handling
```

### 3. Summarize Periodically
```python
def create_iteration_summary(iteration_num):
    """Create summary every N iterations"""
    if iteration_num % 10 == 0:
        summary = {
            'completed': [],
            'in_progress': [],
            'pending': [],
            'issues': []
        }
        # ... populate summary
        
        with open(f'.agent/summaries/summary_{iteration_num}.md', 'w') as f:
            f.write(format_summary(summary))
```

### 4. Clean Working Directory
```bash
# Remove unnecessary files
rm -f *.pyc
rm -rf __pycache__
rm -f *.log

# Archive old iterations
tar -czf .agent/archive/iteration_1-50.tar.gz .agent/prompts/prompt_*.md
rm .agent/prompts/prompt_*.md
```

## Troubleshooting

### Context Overflow Symptoms

| Symptom | Likely Cause | Solution |
|---------|-------------|----------|
| Agent forgets earlier instructions | Context window full | Create checkpoint and reset |
| Incomplete responses | Hitting token limits | Reduce prompt size |
| Repeated work | Lost context | Use summaries |
| Errors about missing information | Context truncated | Split into smaller tasks |

### Recovery Strategies

```python
def recover_from_context_overflow():
    """Recover when context limits exceeded"""
    
    # 1. Save current state
    save_state()
    
    # 2. Create summary of work done
    summary = create_work_summary()
    
    # 3. Reset with minimal context
    new_prompt = f"""
    Continue from checkpoint. Previous work summary:
    {summary}
    
    Current task: {get_current_task()}
    """
    
    # 4. Resume with fresh context
    return new_prompt
```

## Agent-Specific Tips

### Claude (200K context)
- Can handle large codebases
- Include more context for better results
- Use for complex, multi-file tasks

### Gemini (32K context)
- Balance between context and detail
- Good for medium-sized projects
- Optimize file inclusion

### Q Chat (8K context)
- Minimize context aggressively
- Focus on single files/functions
- Use for targeted tasks

## Configuration

```json
{
  "context_management": {
    "max_prompt_size": 5000,
    "max_file_size": 10000,
    "max_files_included": 10,
    "compression_enabled": true,
    "sliding_window_size": 5,
    "checkpoint_on_high_usage": true,
    "usage_warning_threshold": 80,
    "usage_critical_threshold": 95
  }
}
```