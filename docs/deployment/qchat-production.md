# Q Chat Adapter Production Deployment Guide

!!! warning "Deprecated"
    The Q Chat CLI has been rebranded to **Kiro CLI**. This guide references the legacy Q Chat adapter.
    Please refer to the [Kiro Migration Guide](../guide/kiro-migration.md) for information on migrating to the new `kiro` adapter.

This guide provides comprehensive instructions for deploying the Q Chat adapter in production environments with Ralph Orchestrator.

## Overview

The Q Chat adapter has been thoroughly tested and validated for production use with the following capabilities:
- Thread-safe concurrent message processing
- Robust error handling and recovery
- Graceful shutdown and resource cleanup
- Non-blocking I/O to prevent deadlocks
- Automatic retry with exponential backoff
- Signal handling for clean termination

## Prerequisites

### System Requirements
- Python 3.8 or higher
- Q CLI installed and configured
- Sufficient memory for concurrent operations (minimum 2GB recommended)
- Unix-like operating system (Linux, macOS)

### Installation
```bash
# Install Q CLI
pip install q-cli

# Verify installation
qchat --version

# Install Ralph Orchestrator with Q adapter support
pip install ralph-orchestrator
```

## Configuration

### Environment Variables

Configure the Q Chat adapter behavior using these environment variables:

```bash
# Core Configuration
export QCHAT_TIMEOUT=300          # Request timeout in seconds (default: 120)
export QCHAT_MAX_RETRIES=5        # Maximum retry attempts (default: 3)
export QCHAT_RETRY_DELAY=2        # Initial retry delay in seconds (default: 1)
export QCHAT_VERBOSE=1            # Enable verbose logging (default: 0)

# Performance Tuning
export QCHAT_BUFFER_SIZE=8192     # Pipe buffer size in bytes (default: 4096)
export QCHAT_POLL_INTERVAL=0.1    # Message queue polling interval (default: 0.1)
export QCHAT_MAX_CONCURRENT=10    # Maximum concurrent requests (default: 5)

# Resource Limits
export QCHAT_MAX_MEMORY_MB=4096   # Maximum memory usage in MB
export QCHAT_MAX_OUTPUT_SIZE=10485760  # Maximum output size in bytes (10MB)
```

### Configuration File

Create a configuration file for persistent settings:

```yaml
# config/qchat.yaml
adapter:
  name: qchat
  timeout: 300
  max_retries: 5
  retry_delay: 2
  
performance:
  buffer_size: 8192
  poll_interval: 0.1
  max_concurrent: 10
  
logging:
  level: INFO
  format: "%(asctime)s - %(name)s - %(levelname)s - %(message)s"
  file: /var/log/ralph/qchat.log
  
monitoring:
  metrics_enabled: true
  metrics_interval: 60
  health_check_port: 8080
```

## Deployment Scenarios

### 1. Single Instance Deployment

For simple production deployments with moderate load:

```bash
#!/bin/bash
# deploy-qchat.sh

# Set production environment
export ENVIRONMENT=production
export QCHAT_TIMEOUT=300
export QCHAT_VERBOSE=1

# Start Ralph Orchestrator with Q Chat
python -m ralph_orchestrator \
  --agent q \
  --config config/qchat.yaml \
  --checkpoint-interval 10 \
  --max-iterations 1000 \
  --metrics-interval 60 \
  --log-file /var/log/ralph/orchestrator.log
```

### 2. High-Availability Deployment

For mission-critical applications requiring high availability:

```bash
#!/bin/bash
# ha-deploy-qchat.sh

# Configure for high availability
export QCHAT_MAX_RETRIES=10
export QCHAT_RETRY_DELAY=5
export QCHAT_MAX_CONCURRENT=20

# Enable health monitoring
export HEALTH_CHECK_ENABLED=true
export HEALTH_CHECK_INTERVAL=30

# Start with supervisor for automatic restart
supervisorctl start ralph-qchat

# Or use systemd
systemctl start ralph-qchat.service
```

### 3. Containerized Deployment

Docker configuration for container deployments:

```dockerfile
# Dockerfile
FROM python:3.11-slim

WORKDIR /app

# Install dependencies
RUN pip install ralph-orchestrator q-cli

# Copy configuration
COPY config/qchat.yaml /app/config/

# Set environment variables
ENV QCHAT_TIMEOUT=300
ENV QCHAT_VERBOSE=1
ENV PYTHONUNBUFFERED=1

# Health check
HEALTHCHECK --interval=30s --timeout=10s --retries=3 \
  CMD python -c "import requests; requests.get('http://localhost:8080/health')"

# Run the orchestrator
CMD ["python", "-m", "ralph_orchestrator", "--agent", "q", "--config", "config/qchat.yaml"]
```

Docker Compose configuration:

```yaml
# docker-compose.yml
version: '3.8'

services:
  ralph-qchat:
    build: .
    container_name: ralph-qchat
    restart: unless-stopped
    environment:
      - QCHAT_TIMEOUT=300
      - QCHAT_MAX_RETRIES=5
      - QCHAT_VERBOSE=1
    volumes:
      - ./prompts:/app/prompts
      - ./checkpoints:/app/checkpoints
      - ./logs:/app/logs
    ports:
      - "8080:8080"  # Health check endpoint
    logging:
      driver: json-file
      options:
        max-size: "10m"
        max-file: "3"
```

## Monitoring and Observability

### Logging Configuration

Configure structured logging for production:

```python
# logging_config.py
import logging
import logging.handlers

def setup_logging():
    logger = logging.getLogger('ralph.qchat')
    logger.setLevel(logging.INFO)
    
    # File handler with rotation
    file_handler = logging.handlers.RotatingFileHandler(
        '/var/log/ralph/qchat.log',
        maxBytes=10485760,  # 10MB
        backupCount=5
    )
    
    # Structured log format
    formatter = logging.Formatter(
        '{"time": "%(asctime)s", "level": "%(levelname)s", '
        '"module": "%(module)s", "message": "%(message)s"}'
    )
    file_handler.setFormatter(formatter)
    
    logger.addHandler(file_handler)
    return logger
```

### Metrics Collection

Monitor key performance indicators:

```python
# metrics.py
from prometheus_client import Counter, Histogram, Gauge

# Define metrics
request_count = Counter('qchat_requests_total', 'Total number of Q Chat requests')
request_duration = Histogram('qchat_request_duration_seconds', 'Request duration')
active_requests = Gauge('qchat_active_requests', 'Number of active requests')
error_count = Counter('qchat_errors_total', 'Total number of errors', ['error_type'])
```

### Health Checks

Implement health check endpoints:

```python
# health_check.py
from flask import Flask, jsonify
import psutil

app = Flask(__name__)

@app.route('/health')
def health():
    """Basic health check endpoint"""
    return jsonify({
        'status': 'healthy',
        'adapter': 'qchat',
        'version': '1.0.0'
    })

@app.route('/health/detailed')
def health_detailed():
    """Detailed health check with system metrics"""
    return jsonify({
        'status': 'healthy',
        'adapter': 'qchat',
        'version': '1.0.0',
        'system': {
            'cpu_percent': psutil.cpu_percent(),
            'memory_percent': psutil.virtual_memory().percent,
            'disk_usage': psutil.disk_usage('/').percent
        }
    })

if __name__ == '__main__':
    app.run(host='0.0.0.0', port=8080)
```

## Performance Optimization

### 1. Connection Pooling

Optimize for high-concurrency scenarios:

```python
# connection_pool.py
from concurrent.futures import ThreadPoolExecutor
import queue

class QChatConnectionPool:
    def __init__(self, max_connections=10):
        self.executor = ThreadPoolExecutor(max_workers=max_connections)
        self.semaphore = threading.Semaphore(max_connections)
    
    def execute(self, prompt):
        with self.semaphore:
            future = self.executor.submit(self._execute_qchat, prompt)
            return future.result()
```

### 2. Caching Strategy

Implement response caching for repeated queries:

```python
# cache.py
from functools import lru_cache
import hashlib

class QChatCache:
    def __init__(self, max_size=1000):
        self.cache = {}
        self.max_size = max_size
    
    def get_cache_key(self, prompt):
        return hashlib.sha256(prompt.encode()).hexdigest()
    
    def get(self, prompt):
        key = self.get_cache_key(prompt)
        return self.cache.get(key)
    
    def set(self, prompt, response):
        if len(self.cache) >= self.max_size:
            # Remove oldest entry
            self.cache.pop(next(iter(self.cache)))
        key = self.get_cache_key(prompt)
        self.cache[key] = response
```

### 3. Resource Limits

Configure resource limits for production stability:

```bash
# Set system limits
ulimit -n 4096          # Increase file descriptor limit
ulimit -u 2048          # Increase process limit
ulimit -m 4194304       # Set memory limit (4GB)

# Configure cgroups for container environments
echo "4G" > /sys/fs/cgroup/memory/ralph-qchat/memory.limit_in_bytes
echo "80" > /sys/fs/cgroup/cpu/ralph-qchat/cpu.shares
```

## Troubleshooting

### Common Issues and Solutions

#### 1. Deadlock Prevention
```bash
# Check for pipe buffer issues
strace -p <PID> -e read,write

# Increase buffer size if needed
export QCHAT_BUFFER_SIZE=16384
```

#### 2. Memory Leaks
```bash
# Monitor memory usage
watch -n 1 'ps aux | grep qchat'

# Enable memory profiling
export PYTHONTRACEMALLOC=1
```

#### 3. Process Hanging
```bash
# Check process state
ps -eLf | grep qchat

# Send diagnostic signal
kill -USR1 <PID>  # Trigger diagnostic dump
```

#### 4. High CPU Usage
```bash
# Profile CPU usage
py-spy top --pid <PID>

# Adjust polling interval
export QCHAT_POLL_INTERVAL=0.5
```

### Debug Mode

Enable debug mode for detailed diagnostics:

```bash
# Enable all debug features
export QCHAT_DEBUG=1
export QCHAT_VERBOSE=1
export PYTHONVERBOSE=1
export RUST_LOG=debug  # If using Rust-based components

# Run with debug logging
python -m ralph_orchestrator \
  --agent q \
  --verbose \
  --debug \
  --log-level DEBUG
```

## Security Considerations

### 1. Input Validation

Always validate and sanitize inputs:

```python
def validate_prompt(prompt):
    # Check prompt length
    if len(prompt) > MAX_PROMPT_LENGTH:
        raise ValueError("Prompt exceeds maximum length")
    
    # Sanitize special characters
    prompt = prompt.replace('\0', '')
    
    # Check for injection attempts
    if any(pattern in prompt for pattern in BLOCKED_PATTERNS):
        raise SecurityError("Potentially malicious prompt detected")
    
    return prompt
```

### 2. Process Isolation

Run Q Chat processes with limited privileges:

```bash
# Create dedicated user
useradd -r -s /bin/false qchat-user

# Run with limited privileges
sudo -u qchat-user python -m ralph_orchestrator --agent q
```

### 3. Network Security

Configure firewall rules for the health check endpoint:

```bash
# Allow health check port only from monitoring systems
iptables -A INPUT -p tcp --dport 8080 -s 10.0.0.0/8 -j ACCEPT
iptables -A INPUT -p tcp --dport 8080 -j DROP
```

## Maintenance and Updates

### Rolling Updates

Perform zero-downtime updates:

```bash
#!/bin/bash
# rolling-update.sh

# Start new version
docker-compose up -d ralph-qchat-new

# Wait for health check
while ! curl -f http://localhost:8081/health; do
  sleep 5
done

# Switch traffic (update load balancer/proxy)
nginx -s reload

# Stop old version
docker-compose stop ralph-qchat-old
```

### Backup and Recovery

Regular checkpoint backups:

```bash
# Backup checkpoints
tar -czf checkpoints-$(date +%Y%m%d).tar.gz checkpoints/

# Backup configuration
cp -r config/ backup/config-$(date +%Y%m%d)/

# Restore from backup
tar -xzf checkpoints-20240101.tar.gz
cp -r backup/config-20240101/* config/
```

## Performance Benchmarks

Expected performance metrics in production:

| Metric | Value | Notes |
|--------|-------|-------|
| **Latency (p50)** | < 500ms | For simple prompts |
| **Latency (p99)** | < 2000ms | For complex prompts |
| **Throughput** | 100 req/min | Single instance |
| **Concurrency** | 10-20 | Concurrent requests |
| **Memory Usage** | < 500MB | Per instance |
| **CPU Usage** | < 50% | Average utilization |
| **Error Rate** | < 0.1% | Production target |
| **Availability** | > 99.9% | With proper monitoring |

## Best Practices

1. **Always use checkpointing** for long-running tasks
2. **Monitor resource usage** continuously
3. **Implement rate limiting** to prevent overload
4. **Use connection pooling** for better performance
5. **Enable structured logging** for easier debugging
6. **Set appropriate timeouts** based on workload
7. **Implement circuit breakers** for fault tolerance
8. **Regular backup** of checkpoints and configuration
9. **Test disaster recovery** procedures regularly
10. **Keep Q CLI updated** to latest stable version

## Support and Resources

- **Documentation**: [Ralph Orchestrator Docs](https://ralph-orchestrator.readthedocs.io)
- **Issues**: [GitHub Issues](https://github.com/your-org/ralph-orchestrator/issues)
- **Community**: [Discord Server](https://discord.gg/ralph-orchestrator)
- **Emergency Support**: support@ralph-orchestrator.com

## Appendix: Systemd Service

```ini
# /etc/systemd/system/ralph-qchat.service
[Unit]
Description=Ralph Orchestrator with Q Chat Adapter
After=network.target

[Service]
Type=simple
User=qchat-user
Group=qchat-group
WorkingDirectory=/opt/ralph-orchestrator
Environment="QCHAT_TIMEOUT=300"
Environment="QCHAT_VERBOSE=1"
ExecStart=/usr/bin/python3 -m ralph_orchestrator --agent q --config /etc/ralph/qchat.yaml
Restart=always
RestartSec=10
StandardOutput=append:/var/log/ralph/qchat.log
StandardError=append:/var/log/ralph/qchat-error.log

[Install]
WantedBy=multi-user.target
```

Enable and start the service:

```bash
systemctl daemon-reload
systemctl enable ralph-qchat.service
systemctl start ralph-qchat.service
systemctl status ralph-qchat.service
```