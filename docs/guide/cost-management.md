# Cost Management Guide

Effective cost management is crucial when running AI orchestration at scale. This guide helps you optimize spending while maintaining task quality.

## Understanding Costs

### Token Pricing

Current pricing per million tokens:

| Agent | Input Cost | Output Cost | Avg Cost/Task |
|-------|------------|-------------|---------------|
| **Claude** | $3.00 | $15.00 | $5-50 |
| **Q Chat** | $0.50 | $1.50 | $1-10 |
| **Gemini** | $0.50 | $1.50 | $1-10 |

### Cost Calculation

```python
total_cost = (input_tokens / 1_000_000 * input_price) + 
             (output_tokens / 1_000_000 * output_price)
```

**Example:**
- Task uses 100K input tokens, 50K output tokens
- With Claude: (0.1 × $3) + (0.05 × $15) = $1.05
- With Q Chat: (0.1 × $0.50) + (0.05 × $1.50) = $0.125

## Cost Control Mechanisms

### 1. Hard Limits

Set maximum spending caps:

```bash
# Strict $10 limit
python hats_orchestrator.py --max-cost 10.0

# Conservative token limit
python hats_orchestrator.py --max-tokens 100000
```

### 2. Context Management

Reduce token usage through smart context handling:

```bash
# Aggressive context management
python hats_orchestrator.py \
  --context-window 50000 \
  --context-threshold 0.6  # Summarize at 60% full
```

### 3. Agent Selection

Choose cost-effective agents:

```bash
# Development: Use cheaper agents
python hats_orchestrator.py --agent q --max-cost 5.0

# Production: Use quality agents with limits
python hats_orchestrator.py --agent claude --max-cost 50.0
```

## Optimization Strategies

### 1. Tiered Agent Strategy

Use different agents for different task phases:

```bash
# Phase 1: Research with Q (cheap)
echo "Research the problem" > research.md
python hats_orchestrator.py --agent q --prompt research.md --max-cost 2.0

# Phase 2: Implementation with Claude (quality)
echo "Implement the solution" > implement.md
python hats_orchestrator.py --agent claude --prompt implement.md --max-cost 20.0

# Phase 3: Testing with Q (cheap)
echo "Test the solution" > test.md
python hats_orchestrator.py --agent q --prompt test.md --max-cost 2.0
```

### 2. Prompt Optimization

Reduce token usage through efficient prompts:

#### Before (Expensive)
```markdown
Please create a comprehensive web application with the following features:
- User authentication system with registration, login, password reset
- Dashboard with charts and graphs
- API with full CRUD operations
- Complete test suite
- Detailed documentation
[... 5000 tokens of requirements ...]
```

#### After (Optimized)
```markdown
Build user auth API:
- Register/login endpoints
- JWT tokens
- PostgreSQL storage
- Basic tests
See spec.md for details.
```

### 3. Context Window Management

#### Automatic Summarization

```bash
# Trigger summarization early to save tokens
python hats_orchestrator.py \
  --context-window 100000 \
  --context-threshold 0.5  # Summarize at 50%
```

#### Manual Context Control

```markdown
## Context Management
When context reaches 50%, summarize:
- Keep only essential information
- Remove completed task details
- Compress verbose outputs
```

### 4. Iteration Optimization

Fewer, smarter iterations save money:

```bash
# Many quick iterations (expensive)
python hats_orchestrator.py --max-iterations 100  # ❌

# Fewer, focused iterations (economical)
python hats_orchestrator.py --max-iterations 20   # ✅
```

## Cost Monitoring

### Real-time Tracking

Monitor costs during execution:

```bash
# Verbose cost reporting
python hats_orchestrator.py \
  --verbose \
  --metrics-interval 1
```

**Output:**
```
[INFO] Iteration 5: Tokens: 25,000 | Cost: $1.25 | Remaining: $48.75
```

### Cost Reports

Access detailed cost breakdowns:

```python
import json
from pathlib import Path

# Load metrics
metrics_dir = Path('.agent/metrics')
total_cost = 0

for metric_file in metrics_dir.glob('metrics_*.json'):
    with open(metric_file) as f:
        data = json.load(f)
        total_cost += data.get('cost', 0)

print(f"Total cost: ${total_cost:.2f}")
```

### Cost Dashboards

Create monitoring dashboards:

```python
#!/usr/bin/env python3
import json
import matplotlib.pyplot as plt
from pathlib import Path

costs = []
iterations = []

for metric_file in sorted(Path('.agent/metrics').glob('*.json')):
    with open(metric_file) as f:
        data = json.load(f)
        costs.append(data.get('total_cost', 0))
        iterations.append(data.get('iteration', 0))

plt.plot(iterations, costs)
plt.xlabel('Iteration')
plt.ylabel('Cumulative Cost ($)')
plt.title('Hats Cost Progression')
plt.savefig('cost_report.png')
```

## Budget Planning

### Task Cost Estimation

| Task Type | Complexity | Recommended Budget | Agent |
|-----------|------------|-------------------|--------|
| Simple Script | Low | $0.50 - $2 | Q Chat |
| Web API | Medium | $5 - $20 | Gemini/Claude |
| Full Application | High | $20 - $100 | Claude |
| Data Analysis | Medium | $5 - $15 | Gemini |
| Documentation | Low-Medium | $2 - $10 | Q/Claude |
| Debugging | Variable | $5 - $50 | Claude |

### Monthly Budget Planning

```python
# Calculate monthly budget needs
tasks_per_month = 50
avg_cost_per_task = 10.0
safety_margin = 1.5

monthly_budget = tasks_per_month * avg_cost_per_task * safety_margin
print(f"Recommended monthly budget: ${monthly_budget}")
```

## Cost Optimization Profiles

### Minimal Cost Profile

Maximum savings, acceptable quality:

```bash
python hats_orchestrator.py \
  --agent q \
  --max-tokens 50000 \
  --max-cost 2.0 \
  --context-window 30000 \
  --context-threshold 0.5 \
  --checkpoint-interval 10
```

### Balanced Profile

Good quality, reasonable cost:

```bash
python hats_orchestrator.py \
  --agent gemini \
  --max-tokens 200000 \
  --max-cost 10.0 \
  --context-window 100000 \
  --context-threshold 0.7 \
  --checkpoint-interval 5
```

### Quality Profile

Best results, controlled spending:

```bash
python hats_orchestrator.py \
  --agent claude \
  --max-tokens 500000 \
  --max-cost 50.0 \
  --context-window 200000 \
  --context-threshold 0.8 \
  --checkpoint-interval 3
```

## Advanced Cost Management

### Dynamic Agent Switching

Switch agents based on budget remaining:

```python
# Pseudo-code for dynamic switching
if remaining_budget > 20:
    agent = "claude"
elif remaining_budget > 5:
    agent = "gemini"
else:
    agent = "q"
```

### Cost-Aware Prompts

Include cost considerations in prompts:

```markdown
## Budget Constraints
- Maximum budget: $10
- Optimize for efficiency
- Skip non-essential features if approaching limit
- Prioritize core functionality
```

### Batch Processing

Combine multiple small tasks:

```bash
# Inefficient: Multiple orchestrations
python hats_orchestrator.py --prompt task1.md  # $5
python hats_orchestrator.py --prompt task2.md  # $5
python hats_orchestrator.py --prompt task3.md  # $5
# Total: $15

# Efficient: Batched orchestration
cat task1.md task2.md task3.md > batch.md
python hats_orchestrator.py --prompt batch.md  # $10
# Total: $10 (33% savings)
```

## Cost Alerts

### Setting Up Alerts

```bash
#!/bin/bash
# cost_monitor.sh

COST_LIMIT=25.0
CURRENT_COST=$(python -c "
import json
with open('.agent/metrics/state_latest.json') as f:
    print(json.load(f)['total_cost'])
")

if (( $(echo "$CURRENT_COST > $COST_LIMIT" | bc -l) )); then
    echo "ALERT: Cost exceeded $COST_LIMIT" | mail -s "Hats Cost Alert" admin@example.com
fi
```

### Automated Stops

Implement circuit breakers:

```python
# cost_breaker.py
import json
import sys

with open('.agent/metrics/state_latest.json') as f:
    state = json.load(f)
    
if state['total_cost'] > state['max_cost'] * 0.9:
    print("WARNING: 90% of budget consumed")
    sys.exit(1)
```

## ROI Analysis

### Calculating ROI

```python
# ROI calculation
hours_saved = 10  # Hours of manual work saved
hourly_rate = 50  # Developer hourly rate
ai_cost = 25  # Cost of AI orchestration

value_created = hours_saved * hourly_rate
roi = (value_created - ai_cost) / ai_cost * 100

print(f"Value created: ${value_created}")
print(f"AI cost: ${ai_cost}")
print(f"ROI: {roi:.1f}%")
```

### Cost-Benefit Matrix

| Task | Manual Hours | Manual Cost | AI Cost | Savings |
|------|-------------|-------------|---------|---------|
| API Development | 40h | $2000 | $50 | $1950 |
| Documentation | 20h | $1000 | $20 | $980 |
| Testing Suite | 30h | $1500 | $30 | $1470 |
| Bug Fixing | 10h | $500 | $25 | $475 |

## Best Practices

### 1. Start Small

Test with minimal budgets first:

```bash
# Test run
python hats_orchestrator.py --max-cost 1.0 --max-iterations 5

# Scale up if successful
python hats_orchestrator.py --max-cost 10.0 --max-iterations 50
```

### 2. Monitor Continuously

Track costs in real-time:

```bash
# Terminal 1: Run orchestration
python hats_orchestrator.py --verbose

# Terminal 2: Monitor costs
watch -n 5 'tail -n 20 .agent/metrics/state_latest.json'
```

### 3. Optimize Iteratively

- Analyze cost reports
- Identify expensive operations
- Refine prompts and settings
- Test optimizations

### 4. Set Realistic Budgets

- Development: 50% of production budget
- Testing: 25% of production budget
- Production: Full budget with safety margin

### 5. Document Costs

Keep records for analysis:

```bash
# Save cost report after each run
python hats_orchestrator.py && \
  cp .agent/metrics/state_latest.json "reports/run_$(date +%Y%m%d_%H%M%S).json"
```

## Troubleshooting

### Common Issues

1. **Unexpected high costs**
   - Check token usage in metrics
   - Review prompt efficiency
   - Verify context settings

2. **Budget exceeded quickly**
   - Lower context window
   - Increase summarization threshold
   - Use cheaper agent

3. **Poor results with budget constraints**
   - Increase budget slightly
   - Optimize prompts
   - Consider phased approach

## Next Steps

- Review [Agent Selection](agents.md) for cost-effective choices
- Optimize [Prompts](prompts.md) for efficiency
- Explore [Examples](../examples/index.md) for cost-optimized patterns