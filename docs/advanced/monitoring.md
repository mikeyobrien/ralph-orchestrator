# Monitoring and Observability

## Overview

Ralph Orchestrator provides comprehensive monitoring capabilities to track execution, performance, and system health. This guide covers monitoring tools, metrics, and best practices.

## Built-in Monitoring

Ralph's monitoring system collects and routes execution data through multiple channels:

```
                           📊 Metrics Collection Flow

                                                                        ┌────────────────────┐
                                                                   ┌──> │ .agent/metrics/    │
                                                                   │    └────────────────────┘
┌─────────────────┐     ┌──────────────────┐     ┌───────────────┐ │    ┌────────────────────┐
│   Orchestrator  │ ──> │ Iteration Events │ ──> │    Metrics    │ ┼──> │   .agent/logs/     │
└─────────────────┘     └──────────────────┘     │   Collector   │ │    └────────────────────┘
                                                 └───────────────┘ │    ┌────────────────────┐
                                                                   └──> │     Console        │
                                                                        └────────────────────┘
```

<details>
<summary>graph-easy source</summary>

```
graph { label: "📊 Metrics Collection Flow"; flow: east; }
[ Orchestrator ] -> [ Iteration Events ] -> [ Metrics Collector ]
[ Metrics Collector ] -> [ .agent/metrics/ ]
[ Metrics Collector ] -> [ .agent/logs/ ]
[ Metrics Collector ] -> [ Console ]
```

</details>

### State Files

Ralph automatically generates state files in `.agent/metrics/`:

```json
{
  "iteration_count": 15,
  "runtime": 234.5,
  "start_time": "2025-09-07T15:44:35",
  "agent": "claude",
  "prompt_file": "PROMPT.md",
  "status": "running",
  "errors": [],
  "checkpoints": [5, 10, 15],
  "last_output_size": 2048
}
```

### Real-time Status

```bash
# Check current status
./ralph status

# Output:
Ralph Orchestrator Status
=========================
Status: RUNNING
Current Iteration: 15
Runtime: 3m 54s
Agent: claude
Last Checkpoint: iteration 15
Errors: 0
```

### Execution Logs

#### Verbose Mode

```bash
# Enable detailed logging
./ralph run --verbose

# Output includes:
# - Agent commands
# - Execution times
# - Output summaries
# - Error details
```

#### Log Levels

```python
import logging

# Configure log level
logging.basicConfig(
    level=logging.DEBUG,  # DEBUG, INFO, WARNING, ERROR
    format='%(asctime)s - %(name)s - %(levelname)s - %(message)s',
    handlers=[
        logging.FileHandler('.agent/logs/ralph.log'),
        logging.StreamHandler()
    ]
)
```

## Metrics Collection

### Performance Metrics

```python
# Automatically collected metrics
metrics = {
    'iteration_times': [],      # Time per iteration
    'agent_response_times': [], # Agent execution duration
    'output_sizes': [],         # Response size per iteration
    'error_rate': 0.0,         # Errors per iteration
    'checkpoint_times': [],     # Checkpoint creation duration
    'total_api_calls': 0       # Total agent invocations
}
```

### Custom Metrics

```python
# Add custom metrics collection
class MetricsCollector:
    def record_metric(self, name: str, value: float):
        """Record custom metric"""
        timestamp = time.time()
        self.metrics.append({
            'name': name,
            'value': value,
            'timestamp': timestamp
        })

    def export_metrics(self):
        """Export metrics to JSON"""
        with open('.agent/metrics/custom.json', 'w') as f:
            json.dump(self.metrics, f, indent=2)
```

## Monitoring Tools

### 1. Ralph Monitor (Built-in)

```bash
# Continuous monitoring
watch -n 5 './ralph status'

# Tail logs
tail -f .agent/logs/ralph.log

# Monitor metrics
watch -n 10 'cat .agent/metrics/state_*.json | jq .'
```

### 2. Git History Monitoring

```bash
# View checkpoint history
git log --oneline | grep "Ralph checkpoint"

# Analyze code changes over time
git diff --stat HEAD~10..HEAD

# Track file modifications
git log --follow -p PROMPT.md
```

### 3. System Resource Monitoring

```bash
# Monitor Ralph process
htop -p $(pgrep -f ralph_orchestrator)

# Track resource usage
pidstat -p $(pgrep -f ralph_orchestrator) 1

# Monitor file system changes
inotifywait -m -r . -e modify,create,delete
```

## Dashboard Setup

### Terminal Dashboard

Create `monitor.sh`:

```bash
#!/bin/bash
# Ralph Monitoring Dashboard

while true; do
    clear
    echo "=== RALPH ORCHESTRATOR MONITOR ==="
    echo ""

    # Status
    ./ralph status
    echo ""

    # Recent errors
    echo "Recent Errors:"
    tail -n 5 .agent/logs/ralph.log | grep ERROR || echo "No errors"
    echo ""

    # Resource usage
    echo "Resource Usage:"
    ps aux | grep ralph_orchestrator | grep -v grep
    echo ""

    # Latest checkpoint
    echo "Latest Checkpoint:"
    ls -lt .agent/checkpoints/ | head -2

    sleep 5
done
```

### Web Dashboard (Optional)

```python
# Simple Flask dashboard
from flask import Flask, jsonify, render_template_string
import json
import glob

app = Flask(__name__)

@app.route('/metrics')
def metrics():
    # Get latest state file
    state_files = glob.glob('.agent/metrics/state_*.json')
    if state_files:
        latest = max(state_files)
        with open(latest) as f:
            return jsonify(json.load(f))
    return jsonify({'status': 'no data'})

@app.route('/')
def dashboard():
    return render_template_string('''
    <html>
        <head>
            <title>Ralph Dashboard</title>
            <script>
                function updateMetrics() {
                    fetch('/metrics')
                        .then(response => response.json())
                        .then(data => {
                            document.getElementById('metrics').innerHTML =
                                JSON.stringify(data, null, 2);
                        });
                }
                setInterval(updateMetrics, 5000);
            </script>
        </head>
        <body onload="updateMetrics()">
            <h1>Ralph Orchestrator Dashboard</h1>
            <pre id="metrics"></pre>
        </body>
    </html>
    ''')

if __name__ == '__main__':
    app.run(debug=True, port=5000)
```

## Alerting

### Error Detection

```python
# Monitor for errors
def check_errors():
    with open('.agent/metrics/state_latest.json') as f:
        state = json.load(f)

    if state.get('errors'):
        send_alert(f"Ralph encountered errors: {state['errors']}")

    if state.get('iteration_count', 0) > 100:
        send_alert("Ralph exceeded 100 iterations")

    if state.get('runtime', 0) > 14400:  # 4 hours
        send_alert("Ralph runtime exceeded 4 hours")
```

### Notification Methods

```bash
# Desktop notification
notify-send "Ralph Alert" "Task completed successfully"

# Email alert
echo "Ralph task failed" | mail -s "Ralph Alert" admin@example.com

# Slack webhook
curl -X POST -H 'Content-type: application/json' \
    --data '{"text":"Ralph task completed"}' \
    YOUR_SLACK_WEBHOOK_URL
```

## Performance Analysis

### Iteration Analysis

```python
# Analyze iteration performance
import pandas as pd
import matplotlib.pyplot as plt

def analyze_iterations():
    # Load metrics
    metrics = []
    for file in glob.glob('.agent/metrics/state_*.json'):
        with open(file) as f:
            metrics.append(json.load(f))

    # Create DataFrame
    df = pd.DataFrame(metrics)

    # Plot iteration times
    plt.figure(figsize=(10, 6))
    plt.plot(df['iteration_count'], df['runtime'])
    plt.xlabel('Iteration')
    plt.ylabel('Cumulative Runtime (seconds)')
    plt.title('Ralph Execution Performance')
    plt.savefig('.agent/performance.png')

    # Statistics
    print(f"Average iteration time: {df['runtime'].diff().mean():.2f}s")
    print(f"Total iterations: {df['iteration_count'].max()}")
    print(f"Error rate: {len(df[df['errors'].notna()]) / len(df):.2%}")
```

### Cost Tracking

```python
# Estimate API costs
def calculate_costs():
    costs = {
        'claude': 0.01,    # $ per call
        'gemini': 0.005,   # $ per call
        'q': 0.0           # Free
    }

    total_cost = 0
    for file in glob.glob('.agent/metrics/state_*.json'):
        with open(file) as f:
            state = json.load(f)
            agent = state.get('agent', 'claude')
            total_cost += costs.get(agent, 0)

    print(f"Estimated cost: ${total_cost:.2f}")
    return total_cost
```

## Log Management

### Log Rotation

```python
# Configure log rotation
import logging.handlers

handler = logging.handlers.RotatingFileHandler(
    '.agent/logs/ralph.log',
    maxBytes=10*1024*1024,  # 10MB
    backupCount=5
)
```

### Log Aggregation

```bash
# Combine all logs
cat .agent/logs/*.log > combined.log

# Filter by date
grep "2025-09-07" .agent/logs/*.log

# Extract errors only
grep -E "ERROR|CRITICAL" .agent/logs/*.log > errors.log
```

### Log Analysis

```bash
# Count errors by type
grep ERROR .agent/logs/*.log | cut -d: -f4 | sort | uniq -c

# Find longest running iterations
grep "Iteration .* completed" .agent/logs/*.log | \
    awk '{print $NF}' | sort -rn | head -10

# Agent usage statistics
grep "Using agent:" .agent/logs/*.log | \
    cut -d: -f4 | sort | uniq -c
```

## Health Checks

### Automated Health Checks

```python
def health_check():
    """Comprehensive health check"""
    health = {
        'status': 'healthy',
        'checks': []
    }

    # Check prompt file exists
    if not os.path.exists('PROMPT.md'):
        health['status'] = 'unhealthy'
        health['checks'].append('PROMPT.md missing')

    # Check agent availability
    for agent in ['claude', 'q', 'gemini']:
        if shutil.which(agent):
            health['checks'].append(f'{agent}: available')
        else:
            health['checks'].append(f'{agent}: not found')

    # Check disk space
    stat = os.statvfs('.')
    free_space = stat.f_bavail * stat.f_frsize / (1024**3)  # GB
    if free_space < 1:
        health['status'] = 'warning'
        health['checks'].append(f'Low disk space: {free_space:.2f}GB')

    # Check Git status
    result = subprocess.run(['git', 'status', '--porcelain'],
                          capture_output=True, text=True)
    if result.stdout:
        health['checks'].append('Uncommitted changes present')

    return health
```

## Troubleshooting with Monitoring

### Common Issues

| Symptom              | Check                         | Solution                         |
| -------------------- | ----------------------------- | -------------------------------- |
| High iteration count | `.agent/metrics/state_*.json` | Review prompt clarity            |
| Slow performance     | Iteration times in logs       | Check agent response times       |
| Memory issues        | System monitor                | Increase limits or add swap      |
| Repeated errors      | Error patterns in logs        | Fix underlying issue             |
| No progress          | Git diff output               | Check if agent is making changes |

### Debug Mode

```bash
# Maximum verbosity
RALPH_DEBUG=1 ./ralph run --verbose

# Trace execution
python -m trace -t ralph_orchestrator.py

# Profile performance
python -m cProfile -o profile.stats ralph_orchestrator.py
```

## Best Practices

1. **Regular Monitoring**
   - Check status every 10-15 minutes
   - Review logs for anomalies
   - Monitor resource usage

2. **Metric Retention**
   - Archive old metrics weekly
   - Compress logs monthly
   - Maintain 30-day history

3. **Alert Fatigue**
   - Set reasonable thresholds
   - Group related alerts
   - Prioritize critical issues

4. **Documentation**
   - Document custom metrics
   - Track performance baselines
   - Note configuration changes
