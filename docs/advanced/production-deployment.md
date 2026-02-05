# Production Deployment Guide

## Overview

This guide covers deploying Hats in production environments, including server setup, automation, monitoring, and scaling considerations.

## Deployment Options

### 1. Local Server Deployment

#### System Requirements
- **OS**: Linux (Ubuntu 20.04+, RHEL 8+, Debian 11+)
- **Python**: 3.9+
- **Git**: 2.25+
- **Memory**: 4GB minimum, 8GB recommended
- **Storage**: 20GB available space
- **Network**: Stable internet for AI agent APIs

#### Installation Script
```bash
#!/bin/bash
# hats-install.sh

# Update system
sudo apt-get update && sudo apt-get upgrade -y

# Install dependencies
sudo apt-get install -y python3 python3-pip git nodejs npm

# Install AI agents
npm install -g @anthropic-ai/claude-code
npm install -g @google/gemini-cli
# Install Q following its documentation

# Clone Hats
git clone https://github.com/yourusername/hats.git
cd hats

# Set permissions
chmod +x hats_orchestrator.py hats

# Create systemd service
sudo cp hats.service /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl enable hats
```

### 2. Docker Deployment

#### Dockerfile
```dockerfile
FROM python:3.11-slim

# Install system dependencies
RUN apt-get update && apt-get install -y \
    git \
    nodejs \
    npm \
    && rm -rf /var/lib/apt/lists/*

# Install AI CLI tools
RUN npm install -g @anthropic-ai/claude-code @google/gemini-cli

# Create hats user
RUN useradd -m -s /bin/bash hats
WORKDIR /home/hats

# Copy application
COPY --chown=hats:hats . /home/hats/hats/
WORKDIR /home/hats/hats

# Set permissions
RUN chmod +x hats_orchestrator.py hats

# Switch to hats user
USER hats

# Default command
CMD ["./hats", "run"]
```

#### Docker Compose
```yaml
# docker-compose.yml
version: '3.8'

services:
  hats:
    build: .
    container_name: hats
    restart: unless-stopped
    volumes:
      - ./workspace:/home/hats/workspace
      - ./prompts:/home/hats/prompts
      - hats-agent:/home/hats/hats/.agent
    environment:
      - HATS_MAX_ITERATIONS=100
      - HATS_AGENT=auto
      - HATS_CHECKPOINT_INTERVAL=5
    logging:
      driver: "json-file"
      options:
        max-size: "10m"
        max-file: "3"

volumes:
  hats-agent:
```

### 3. Cloud Deployment

#### AWS EC2
```bash
# User data script for EC2 instance
#!/bin/bash
yum update -y
yum install -y python3 git nodejs

# Install Hats
cd /opt
git clone https://github.com/yourusername/hats.git
cd hats
chmod +x hats_orchestrator.py hats

# Configure as service
cat > /etc/systemd/system/hats.service << EOF
[Unit]
Description=Hats
After=network.target

[Service]
Type=simple
User=ec2-user
WorkingDirectory=/opt/hats
ExecStart=/opt/hats/hats run
Restart=on-failure
RestartSec=10

[Install]
WantedBy=multi-user.target
EOF

systemctl enable hats
systemctl start hats
```

#### Kubernetes Deployment
```yaml
# hats-deployment.yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: hats
spec:
  replicas: 1
  selector:
    matchLabels:
      app: hats
  template:
    metadata:
      labels:
        app: hats
    spec:
      containers:
      - name: hats
        image: hats:latest
        resources:
          requests:
            memory: "2Gi"
            cpu: "1"
          limits:
            memory: "4Gi"
            cpu: "2"
        volumeMounts:
        - name: workspace
          mountPath: /workspace
        - name: config
          mountPath: /config
      volumes:
      - name: workspace
        persistentVolumeClaim:
          claimName: hats-workspace
      - name: config
        configMap:
          name: hats-config
```

## Configuration Management

### Environment Variables
```bash
# /etc/environment or .env file
HATS_HOME=/opt/hats
HATS_WORKSPACE=/var/hats/workspace
HATS_LOG_LEVEL=INFO
HATS_MAX_ITERATIONS=100
HATS_MAX_RUNTIME=14400
HATS_AGENT=claude
HATS_CHECKPOINT_INTERVAL=5
HATS_RETRY_DELAY=2
HATS_GIT_ENABLED=true
HATS_ARCHIVE_ENABLED=true
```

### Configuration File
```json
{
  "production": {
    "agent": "claude",
    "max_iterations": 100,
    "max_runtime": 14400,
    "checkpoint_interval": 5,
    "retry_delay": 2,
    "retry_max": 5,
    "timeout_per_iteration": 300,
    "git_enabled": true,
    "archive_enabled": true,
    "monitoring": {
      "enabled": true,
      "metrics_endpoint": "http://metrics.example.com",
      "log_level": "INFO"
    },
    "security": {
      "sandbox_enabled": true,
      "allowed_directories": ["/workspace"],
      "forbidden_commands": ["rm -rf", "sudo", "su"],
      "max_file_size": 10485760
    }
  }
}
```

## Automation

### Systemd Service
```ini
# /etc/systemd/system/hats.service
[Unit]
Description=Hats Service
Documentation=https://github.com/yourusername/hats
After=network.target

[Service]
Type=simple
User=hats
Group=hats
WorkingDirectory=/opt/hats
ExecStart=/opt/hats/hats run --config production.json
ExecReload=/bin/kill -HUP $MAINPID
Restart=on-failure
RestartSec=30
StandardOutput=journal
StandardError=journal
SyslogIdentifier=hats
Environment="PYTHONUNBUFFERED=1"

# Security
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=/opt/hats /var/hats

[Install]
WantedBy=multi-user.target
```

### Cron Jobs
```bash
# /etc/cron.d/hats
# Clean old logs weekly
0 2 * * 0 hats /opt/hats/scripts/cleanup.sh

# Backup state daily
0 3 * * * hats tar -czf /backup/hats-$(date +\%Y\%m\%d).tar.gz /opt/hats/.agent

# Health check every 5 minutes
*/5 * * * * hats /opt/hats/scripts/health-check.sh || systemctl restart hats
```

### CI/CD Pipeline
```yaml
# .github/workflows/deploy.yml
name: Deploy Hats

on:
  push:
    branches: [main]
    paths:
      - 'hats_orchestrator.py'
      - 'hats'
      - 'requirements.txt'

jobs:
  deploy:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      
      - name: Run tests
        run: python test_comprehensive.py
      
      - name: Build Docker image
        run: docker build -t hats:${{ github.sha }} .
      
      - name: Push to registry
        run: |
          docker tag hats:${{ github.sha }} ${{ secrets.REGISTRY }}/hats:latest
          docker push ${{ secrets.REGISTRY }}/hats:latest
      
      - name: Deploy to server
        uses: appleboy/ssh-action@v0.1.5
        with:
          host: ${{ secrets.HOST }}
          username: ${{ secrets.USERNAME }}
          key: ${{ secrets.SSH_KEY }}
          script: |
            cd /opt/hats
            git pull
            systemctl restart hats
```

## Monitoring in Production

### Prometheus Metrics
```python
# metrics_exporter.py
from prometheus_client import Counter, Histogram, Gauge, start_http_server
import json
import glob

# Define metrics
iteration_counter = Counter('hats_iterations_total', 'Total iterations')
error_counter = Counter('hats_errors_total', 'Total errors')
runtime_gauge = Gauge('hats_runtime_seconds', 'Current runtime')
iteration_duration = Histogram('hats_iteration_duration_seconds', 'Iteration duration')

def collect_metrics():
    """Collect metrics from Hats state files"""
    state_files = glob.glob('.agent/metrics/state_*.json')
    if state_files:
        latest = max(state_files)
        with open(latest) as f:
            state = json.load(f)
            
        iteration_counter.inc(state.get('iteration_count', 0))
        runtime_gauge.set(state.get('runtime', 0))
        
        if state.get('errors'):
            error_counter.inc(len(state['errors']))

if __name__ == '__main__':
    # Start metrics server
    start_http_server(8000)
    
    # Collect metrics periodically
    while True:
        collect_metrics()
        time.sleep(30)
```

### Logging Setup
```python
# logging_config.py
import logging
import logging.handlers
import json

def setup_production_logging():
    """Configure production logging"""
    
    # JSON formatter for structured logging
    class JSONFormatter(logging.Formatter):
        def format(self, record):
            log_obj = {
                'timestamp': self.formatTime(record),
                'level': record.levelname,
                'logger': record.name,
                'message': record.getMessage(),
                'module': record.module,
                'function': record.funcName,
                'line': record.lineno
            }
            if record.exc_info:
                log_obj['exception'] = self.formatException(record.exc_info)
            return json.dumps(log_obj)
    
    # Configure root logger
    logger = logging.getLogger()
    logger.setLevel(logging.INFO)
    
    # File handler with rotation
    file_handler = logging.handlers.RotatingFileHandler(
        '/var/log/hats/hats.log',
        maxBytes=100*1024*1024,  # 100MB
        backupCount=10
    )
    file_handler.setFormatter(JSONFormatter())
    
    # Syslog handler
    syslog_handler = logging.handlers.SysLogHandler(address='/dev/log')
    syslog_handler.setFormatter(JSONFormatter())
    
    logger.addHandler(file_handler)
    logger.addHandler(syslog_handler)
```

## Security Hardening

### User Isolation
```bash
# Create dedicated user
sudo useradd -r -s /bin/bash -m -d /opt/hats hats
sudo chown -R hats:hats /opt/hats

# Set restrictive permissions
chmod 750 /opt/hats
chmod 640 /opt/hats/*.py
chmod 750 /opt/hats/hats
```

### Network Security
```bash
# Firewall rules (iptables)
iptables -A OUTPUT -p tcp --dport 443 -j ACCEPT  # HTTPS for AI agents
iptables -A OUTPUT -p tcp --dport 22 -j ACCEPT   # Git SSH
iptables -A OUTPUT -j DROP                       # Block other outbound

# Or using ufw
ufw allow out 443/tcp
ufw allow out 22/tcp
ufw default deny outgoing
```

### API Key Management
```bash
# Use system keyring
pip install keyring

# Store API keys securely
python -c "import keyring; keyring.set_password('hats', 'claude_api_key', 'your-key')"

# Or use environment variables from secure store
source /etc/hats/secrets.env
```

## Scaling Considerations

### Horizontal Scaling
```python
# job_queue.py
import redis
import json

class HatsJobQueue:
    def __init__(self):
        self.redis = redis.Redis(host='localhost', port=6379)
    
    def add_job(self, prompt_file, config):
        """Add job to queue"""
        job = {
            'id': str(uuid.uuid4()),
            'prompt_file': prompt_file,
            'config': config,
            'status': 'pending',
            'created': time.time()
        }
        self.redis.lpush('hats:jobs', json.dumps(job))
        return job['id']
    
    def get_job(self):
        """Get next job from queue"""
        job_data = self.redis.rpop('hats:jobs')
        if job_data:
            return json.loads(job_data)
        return None
```

### Resource Limits
```python
# resource_limits.py
import resource

def set_production_limits():
    """Set resource limits for production"""
    
    # Memory limit (4GB)
    resource.setrlimit(
        resource.RLIMIT_AS,
        (4 * 1024 * 1024 * 1024, -1)
    )
    
    # CPU time limit (1 hour)
    resource.setrlimit(
        resource.RLIMIT_CPU,
        (3600, 3600)
    )
    
    # File size limit (100MB)
    resource.setrlimit(
        resource.RLIMIT_FSIZE,
        (100 * 1024 * 1024, -1)
    )
    
    # Process limit
    resource.setrlimit(
        resource.RLIMIT_NPROC,
        (100, 100)
    )
```

## Backup and Recovery

### Automated Backups
```bash
#!/bin/bash
# backup.sh

BACKUP_DIR="/backup/hats"
TIMESTAMP=$(date +%Y%m%d_%H%M%S)

# Create backup
tar -czf $BACKUP_DIR/hats_$TIMESTAMP.tar.gz \
    /opt/hats/.agent \
    /opt/hats/*.json \
    /opt/hats/PROMPT.md

# Keep only last 30 days
find $BACKUP_DIR -name "hats_*.tar.gz" -mtime +30 -delete

# Sync to S3 (optional)
aws s3 sync $BACKUP_DIR s3://my-bucket/hats-backups/
```

### Disaster Recovery
```bash
#!/bin/bash
# restore.sh

BACKUP_FILE=$1
RESTORE_DIR="/opt/hats"

# Stop service
systemctl stop hats

# Restore backup
tar -xzf $BACKUP_FILE -C /

# Reset Git repository
cd $RESTORE_DIR
git reset --hard HEAD

# Restart service
systemctl start hats
```

## Health Checks

### HTTP Health Endpoint
```python
# health_server.py
from flask import Flask, jsonify
import os
import json

app = Flask(__name__)

@app.route('/health')
def health():
    """Health check endpoint"""
    try:
        # Check Hats process
        pid_file = '/var/run/hats.pid'
        if os.path.exists(pid_file):
            with open(pid_file) as f:
                pid = int(f.read())
            os.kill(pid, 0)  # Check if process exists
            status = 'healthy'
        else:
            status = 'unhealthy'
        
        # Check last state
        state_files = glob.glob('.agent/metrics/state_*.json')
        if state_files:
            latest = max(state_files)
            with open(latest) as f:
                state = json.load(f)
        else:
            state = {}
        
        return jsonify({
            'status': status,
            'iteration': state.get('iteration_count', 0),
            'runtime': state.get('runtime', 0),
            'errors': len(state.get('errors', []))
        })
    except Exception as e:
        return jsonify({'status': 'error', 'message': str(e)}), 500

if __name__ == '__main__':
    app.run(host='0.0.0.0', port=8080)
```

## Production Checklist

### Pre-Deployment
- [ ] All tests passing
- [ ] Configuration reviewed
- [ ] API keys secured
- [ ] Backup strategy in place
- [ ] Monitoring configured
- [ ] Resource limits set
- [ ] Security hardening applied

### Deployment
- [ ] Service installed
- [ ] Permissions set correctly
- [ ] Logging configured
- [ ] Health checks working
- [ ] Metrics collection active
- [ ] Backup job scheduled

### Post-Deployment
- [ ] Service running
- [ ] Logs being generated
- [ ] Metrics visible
- [ ] Test job successful
- [ ] Alerts configured
- [ ] Documentation updated