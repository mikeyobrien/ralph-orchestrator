# Production Deployment Guide

This guide covers deployment patterns and operational checklists for running Hats in production environments. Hats is alpha-quality and evolving, so treat this as a solid starting point and validate/tune it for your infrastructure, risk profile, and compliance needs.

## Pre-Production Checklist

### Infrastructure Requirements

- [ ] **Compute Resources**
  - Minimum: 2 vCPUs, 4GB RAM per instance
  - Recommended: 4 vCPUs, 8GB RAM per instance
  - Auto-scaling configured (2-10 instances)

- [ ] **Storage**
  - 100GB SSD for application and logs
  - Separate persistent volumes for data
  - Backup storage configured

- [ ] **Network**
  - Load balancer configured
  - SSL/TLS certificates installed
  - Firewall rules configured
  - DDoS protection enabled

- [ ] **High Availability**
  - Multi-AZ deployment
  - Database replication
  - Redis cluster for caching
  - Message queue redundancy

### Security Requirements

- [ ] **Access Control**
  - RBAC configured
  - Service accounts created
  - API keys rotated
  - MFA enforced for admin access

- [ ] **Secrets Management**
  - HashiCorp Vault or AWS Secrets Manager configured
  - All secrets encrypted at rest
  - Secrets rotation policy implemented

- [ ] **Compliance**
  - Data encryption in transit and at rest
  - Audit logging enabled
  - GDPR/CCPA compliance verified
  - Security scanning completed

## Production Architecture

```mermaid
graph TB
    subgraph "Internet"
        U[Users]
        API[API Clients]
    end
    
    subgraph "Edge Layer"
        CDN[CloudFlare CDN]
        WAF[Web Application Firewall]
    end
    
    subgraph "Load Balancing"
        ALB[Application Load Balancer]
        NLB[Network Load Balancer]
    end
    
    subgraph "Application Layer"
        R1[Hats Instance 1]
        R2[Hats Instance 2]
        R3[Hats Instance N]
    end
    
    subgraph "Data Layer"
        PG[(PostgreSQL Primary)]
        PGR[(PostgreSQL Replica)]
        REDIS[(Redis Cluster)]
        S3[S3 Object Storage]
    end
    
    subgraph "Monitoring"
        PROM[Prometheus]
        GRAF[Grafana]
        ELK[ELK Stack]
    end
    
    U --> CDN
    API --> WAF
    CDN --> ALB
    WAF --> NLB
    ALB --> R1
    ALB --> R2
    ALB --> R3
    NLB --> R1
    R1 --> PG
    R2 --> PGR
    R3 --> REDIS
    R1 --> S3
    R1 --> PROM
    PROM --> GRAF
    R1 --> ELK
```

## Deployment Steps

### 1. Infrastructure Setup

#### AWS Production Setup

```bash
# Create VPC and subnets
aws ec2 create-vpc --cidr-block 10.0.0.0/16

# Create EKS cluster
eksctl create cluster \
  --name hats-production \
  --version 1.28 \
  --region us-west-2 \
  --nodegroup-name workers \
  --node-type m5.xlarge \
  --nodes 3 \
  --nodes-min 2 \
  --nodes-max 10 \
  --managed

# Create RDS instance
aws rds create-db-instance \
  --db-instance-identifier hats-prod-db \
  --db-instance-class db.t3.large \
  --engine postgres \
  --engine-version 15.4 \
  --master-username hats \
  --master-user-password $DB_PASSWORD \
  --allocated-storage 100 \
  --backup-retention-period 30 \
  --multi-az

# Create ElastiCache Redis cluster
aws elasticache create-cache-cluster \
  --cache-cluster-id hats-prod-cache \
  --engine redis \
  --cache-node-type cache.r6g.large \
  --num-cache-nodes 3 \
  --cache-subnet-group-name hats-cache-subnet
```

#### GCP Production Setup

```bash
# Create GKE cluster
gcloud container clusters create hats-production \
  --zone us-central1-a \
  --machine-type n2-standard-4 \
  --num-nodes 3 \
  --enable-autoscaling \
  --min-nodes 2 \
  --max-nodes 10 \
  --enable-autorepair \
  --enable-autoupgrade

# Create Cloud SQL instance
gcloud sql instances create hats-prod-db \
  --database-version=POSTGRES_15 \
  --cpu=4 \
  --memory=16GB \
  --region=us-central1 \
  --availability-type=REGIONAL \
  --backup \
  --backup-start-time=03:00

# Create Memorystore Redis
gcloud redis instances create hats-prod-cache \
  --size=5 \
  --region=us-central1 \
  --redis-version=redis_6_x \
  --tier=STANDARD_HA
```

### 2. Application Configuration

#### Production Environment Variables

```bash
# .env.production
# API Configuration
HATS_AGENT=auto
HATS_MAX_ITERATIONS=1000
HATS_MAX_RUNTIME=28800  # 8 hours
HATS_MAX_TOKENS=5000000
HATS_MAX_COST=500.0

# Database
DATABASE_URL=postgresql://user:pass@db.example.com:5432/hats_prod
DATABASE_POOL_SIZE=20
DATABASE_MAX_OVERFLOW=40

# Redis Cache
REDIS_URL=redis://redis.example.com:6379/0
REDIS_POOL_SIZE=50
REDIS_TTL=3600

# Monitoring
ENABLE_METRICS=true
METRICS_PORT=8080
LOG_LEVEL=INFO
SENTRY_DSN=https://xxx@sentry.io/xxx

# Security
SECRET_KEY=$(openssl rand -hex 32)
ENCRYPTION_KEY=$(openssl rand -base64 32)
API_RATE_LIMIT=100/minute
ALLOWED_HOSTS=hats.example.com
CORS_ORIGINS=https://app.example.com

# AI Service Keys (from secrets manager)
CLAUDE_API_KEY_SECRET=arn:aws:secretsmanager:region:account:secret:claude-key
GEMINI_API_KEY_SECRET=arn:aws:secretsmanager:region:account:secret:gemini-key
```

#### Production Settings

```python
# config/production.py
import os
from typing import Dict, Any

class ProductionConfig:
    """Production configuration"""
    
    # Application
    DEBUG = False
    TESTING = False
    ENV = 'production'
    
    # Security
    SECRET_KEY = os.environ['SECRET_KEY']
    SESSION_COOKIE_SECURE = True
    SESSION_COOKIE_HTTPONLY = True
    SESSION_COOKIE_SAMESITE = 'Strict'
    PERMANENT_SESSION_LIFETIME = 3600
    
    # Database
    SQLALCHEMY_DATABASE_URI = os.environ['DATABASE_URL']
    SQLALCHEMY_ENGINE_OPTIONS = {
        'pool_size': 20,
        'pool_recycle': 3600,
        'pool_pre_ping': True,
        'max_overflow': 40,
        'connect_args': {
            'connect_timeout': 10,
            'application_name': 'hats'
        }
    }
    
    # Caching
    CACHE_TYPE = 'redis'
    CACHE_REDIS_URL = os.environ['REDIS_URL']
    CACHE_DEFAULT_TIMEOUT = 3600
    
    # Rate Limiting
    RATELIMIT_STORAGE_URL = os.environ['REDIS_URL']
    RATELIMIT_STRATEGY = 'fixed-window'
    RATELIMIT_DEFAULT = "100/hour"
    
    # Monitoring
    PROMETHEUS_METRICS = True
    LOGGING_CONFIG = {
        'version': 1,
        'disable_existing_loggers': False,
        'formatters': {
            'json': {
                'class': 'pythonjsonlogger.jsonlogger.JsonFormatter',
                'format': '%(asctime)s %(name)s %(levelname)s %(message)s'
            }
        },
        'handlers': {
            'console': {
                'class': 'logging.StreamHandler',
                'formatter': 'json',
                'level': 'INFO'
            },
            'file': {
                'class': 'logging.handlers.RotatingFileHandler',
                'filename': '/var/log/hats/app.log',
                'maxBytes': 104857600,  # 100MB
                'backupCount': 10,
                'formatter': 'json'
            }
        },
        'root': {
            'level': 'INFO',
            'handlers': ['console', 'file']
        }
    }
```

### 3. Deployment Configuration

#### Kubernetes Production Manifests

```yaml
# k8s/production/deployment.yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: hats
  namespace: production
spec:
  replicas: 3
  strategy:
    type: RollingUpdate
    rollingUpdate:
      maxSurge: 1
      maxUnavailable: 0
  selector:
    matchLabels:
      app: hats
  template:
    metadata:
      labels:
        app: hats
        version: v1.0.0
    spec:
      affinity:
        podAntiAffinity:
          requiredDuringSchedulingIgnoredDuringExecution:
          - labelSelector:
              matchExpressions:
              - key: app
                operator: In
                values:
                - hats
            topologyKey: kubernetes.io/hostname
      containers:
      - name: hats
        image: ghcr.io/mikeyobrien/hats:v1.0.0
        ports:
        - containerPort: 8080
          name: metrics
        envFrom:
        - secretRef:
            name: hats-secrets
        - configMapRef:
            name: hats-config
        resources:
          requests:
            memory: "4Gi"
            cpu: "2"
          limits:
            memory: "8Gi"
            cpu: "4"
        livenessProbe:
          httpGet:
            path: /health
            port: 8080
          initialDelaySeconds: 30
          periodSeconds: 30
          timeoutSeconds: 5
          failureThreshold: 3
        readinessProbe:
          httpGet:
            path: /ready
            port: 8080
          initialDelaySeconds: 10
          periodSeconds: 10
          timeoutSeconds: 5
          failureThreshold: 3
        volumeMounts:
        - name: config
          mountPath: /app/config
          readOnly: true
        - name: secrets
          mountPath: /app/secrets
          readOnly: true
      volumes:
      - name: config
        configMap:
          name: hats-config
      - name: secrets
        secret:
          secretName: hats-secrets
          defaultMode: 0400
```

### 4. Load Balancing and SSL

#### Ingress Configuration

```yaml
# k8s/production/ingress.yaml
apiVersion: networking.k8s.io/v1
kind: Ingress
metadata:
  name: hats-ingress
  namespace: production
  annotations:
    kubernetes.io/ingress.class: nginx
    cert-manager.io/cluster-issuer: letsencrypt-prod
    nginx.ingress.kubernetes.io/rate-limit: "100"
    nginx.ingress.kubernetes.io/ssl-redirect: "true"
    nginx.ingress.kubernetes.io/force-ssl-redirect: "true"
spec:
  tls:
  - hosts:
    - hats.example.com
    secretName: hats-tls
  rules:
  - host: hats.example.com
    http:
      paths:
      - path: /
        pathType: Prefix
        backend:
          service:
            name: hats-service
            port:
              number: 80
```

### 5. Database Setup

#### Production Database Initialization

```sql
-- Create production database
CREATE DATABASE hats_production;
CREATE USER hats_app WITH ENCRYPTED PASSWORD 'secure_password';
GRANT ALL PRIVILEGES ON DATABASE hats_production TO hats_app;

-- Enable extensions
\c hats_production
CREATE EXTENSION IF NOT EXISTS "uuid-ossp";
CREATE EXTENSION IF NOT EXISTS "pg_stat_statements";

-- Create schemas
CREATE SCHEMA IF NOT EXISTS hats;
CREATE SCHEMA IF NOT EXISTS audit;

-- Set search path
ALTER USER hats_app SET search_path TO hats, public;

-- Create tables
CREATE TABLE hats.orchestrations (
    id UUID PRIMARY KEY DEFAULT uuid_generate_v4(),
    task_id VARCHAR(255) UNIQUE NOT NULL,
    agent_type VARCHAR(50) NOT NULL,
    status VARCHAR(50) NOT NULL,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    completed_at TIMESTAMP WITH TIME ZONE,
    metadata JSONB,
    metrics JSONB
);

-- Create indexes
CREATE INDEX idx_orchestrations_status ON hats.orchestrations(status);
CREATE INDEX idx_orchestrations_created ON hats.orchestrations(created_at);
CREATE INDEX idx_orchestrations_agent ON hats.orchestrations(agent_type);

-- Audit table
CREATE TABLE audit.activity_log (
    id BIGSERIAL PRIMARY KEY,
    timestamp TIMESTAMP WITH TIME ZONE DEFAULT CURRENT_TIMESTAMP,
    user_id VARCHAR(255),
    action VARCHAR(100),
    resource VARCHAR(255),
    details JSONB,
    ip_address INET
);
```

### 6. Monitoring Setup

#### Prometheus Configuration

```yaml
# monitoring/prometheus.yaml
global:
  scrape_interval: 15s
  evaluation_interval: 15s

alerting:
  alertmanagers:
  - static_configs:
    - targets:
      - alertmanager:9093

rule_files:
  - "alerts.yml"

scrape_configs:
  - job_name: 'hats'
    kubernetes_sd_configs:
    - role: pod
      namespaces:
        names:
        - production
    relabel_configs:
    - source_labels: [__meta_kubernetes_pod_label_app]
      action: keep
      regex: hats
    - source_labels: [__meta_kubernetes_pod_name]
      target_label: instance
```

#### Alert Rules

```yaml
# monitoring/alerts.yml
groups:
- name: hats_alerts
  interval: 30s
  rules:
  - alert: HighErrorRate
    expr: rate(hats_errors_total[5m]) > 0.1
    for: 5m
    labels:
      severity: critical
    annotations:
      summary: High error rate detected
      description: "Error rate is {{ $value }} errors/sec"
  
  - alert: HighMemoryUsage
    expr: container_memory_usage_bytes{pod=~"hats-.*"} / container_spec_memory_limit_bytes > 0.9
    for: 5m
    labels:
      severity: warning
    annotations:
      summary: High memory usage
      description: "Memory usage is above 90%"
  
  - alert: LongRunningTask
    expr: hats_task_duration_seconds > 14400
    for: 1m
    labels:
      severity: warning
    annotations:
      summary: Task running longer than 4 hours
      description: "Task {{ $labels.task_id }} has been running for {{ $value }} seconds"
```

### 7. Backup and Recovery

#### Automated Backup Script

```bash
#!/bin/bash
# backup.sh

# Configuration
BACKUP_DIR="/backups"
S3_BUCKET="s3://hats-backups"
DATE=$(date +%Y%m%d_%H%M%S)
RETENTION_DAYS=30

# Database backup
pg_dump $DATABASE_URL | gzip > $BACKUP_DIR/db_$DATE.sql.gz

# Application data backup
tar -czf $BACKUP_DIR/data_$DATE.tar.gz /app/data

# Upload to S3
aws s3 cp $BACKUP_DIR/db_$DATE.sql.gz $S3_BUCKET/db/
aws s3 cp $BACKUP_DIR/data_$DATE.tar.gz $S3_BUCKET/data/

# Clean old backups
find $BACKUP_DIR -name "*.gz" -mtime +$RETENTION_DAYS -delete
aws s3 ls $S3_BUCKET/db/ | while read -r line; do
  createDate=$(echo $line | awk '{print $1" "$2}')
  createDate=$(date -d "$createDate" +%s)
  olderThan=$(date -d "$RETENTION_DAYS days ago" +%s)
  if [[ $createDate -lt $olderThan ]]; then
    fileName=$(echo $line | awk '{print $4}')
    aws s3 rm $S3_BUCKET/db/$fileName
  fi
done
```

### 8. Security Hardening

#### Security Policies

```yaml
# k8s/production/security.yaml
apiVersion: policy/v1beta1
kind: PodSecurityPolicy
metadata:
  name: hats-psp
spec:
  privileged: false
  allowPrivilegeEscalation: false
  requiredDropCapabilities:
    - ALL
  volumes:
    - 'configMap'
    - 'emptyDir'
    - 'projected'
    - 'secret'
    - 'persistentVolumeClaim'
  hostNetwork: false
  hostIPC: false
  hostPID: false
  runAsUser:
    rule: 'MustRunAsNonRoot'
  seLinux:
    rule: 'RunAsAny'
  supplementalGroups:
    rule: 'RunAsAny'
  fsGroup:
    rule: 'RunAsAny'
  readOnlyRootFilesystem: true
---
apiVersion: networking.k8s.io/v1
kind: NetworkPolicy
metadata:
  name: hats-network-policy
  namespace: production
spec:
  podSelector:
    matchLabels:
      app: hats
  policyTypes:
  - Ingress
  - Egress
  ingress:
  - from:
    - namespaceSelector:
        matchLabels:
          name: ingress-nginx
    ports:
    - protocol: TCP
      port: 8080
  egress:
  - to:
    - namespaceSelector: {}
    ports:
    - protocol: TCP
      port: 443
    - protocol: TCP
      port: 5432  # PostgreSQL
    - protocol: TCP
      port: 6379  # Redis
```

## Performance Optimization

### Caching Strategy

```python
# cache_config.py
from functools import lru_cache
import redis
import json

class CacheManager:
    def __init__(self, redis_url):
        self.redis = redis.from_url(redis_url)
    
    def cache_result(self, key, value, ttl=3600):
        """Cache result with TTL"""
        self.redis.setex(
            key,
            ttl,
            json.dumps(value)
        )
    
    @lru_cache(maxsize=1000)
    def get_cached(self, key):
        """Get cached value"""
        value = self.redis.get(key)
        return json.loads(value) if value else None
```

### Database Optimization

```sql
-- Optimize queries
CREATE INDEX CONCURRENTLY idx_orchestrations_composite 
ON hats.orchestrations(agent_type, status, created_at);

-- Partition large tables
CREATE TABLE hats.orchestrations_2024 PARTITION OF hats.orchestrations
FOR VALUES FROM ('2024-01-01') TO ('2025-01-01');

-- Vacuum and analyze
VACUUM ANALYZE hats.orchestrations;
```

## Disaster Recovery

### Recovery Plan

1. **RTO (Recovery Time Objective)**: 4 hours
2. **RPO (Recovery Point Objective)**: 1 hour

### Failover Procedure

```bash
#!/bin/bash
# failover.sh

# Check primary health
if ! curl -f https://hats.example.com/health; then
  echo "Primary unhealthy, initiating failover"
  
  # Update DNS to point to standby
  aws route53 change-resource-record-sets \
    --hosted-zone-id Z123456 \
    --change-batch file://failover-dns.json
  
  # Promote standby database
  aws rds promote-read-replica \
    --db-instance-identifier hats-standby-db
  
  # Scale up standby region
  kubectl scale deployment hats \
    --replicas=5 \
    --context=standby-cluster
  
  # Notify team
  aws sns publish \
    --topic-arn arn:aws:sns:region:account:alerts \
    --message "Failover initiated to standby region"
fi
```

## Maintenance

### Rolling Updates

```bash
# Zero-downtime deployment
kubectl set image deployment/hats \
  hats=ghcr.io/mikeyobrien/hats:v1.0.1 \
  --record

# Monitor rollout
kubectl rollout status deployment/hats

# Rollback if needed
kubectl rollout undo deployment/hats
```

### Health Checks

```python
# health.py
from flask import Flask, jsonify
import psutil
import redis

app = Flask(__name__)

@app.route('/health')
def health():
    """Health check endpoint"""
    checks = {
        'database': check_database(),
        'redis': check_redis(),
        'disk': check_disk_space(),
        'memory': check_memory()
    }
    
    status = 'healthy' if all(checks.values()) else 'unhealthy'
    return jsonify({
        'status': status,
        'checks': checks
    }), 200 if status == 'healthy' else 503

def check_database():
    try:
        db.session.execute('SELECT 1')
        return True
    except:
        return False

def check_redis():
    try:
        r = redis.from_url(REDIS_URL)
        r.ping()
        return True
    except:
        return False

def check_disk_space():
    usage = psutil.disk_usage('/')
    return usage.percent < 90

def check_memory():
    usage = psutil.virtual_memory()
    return usage.percent < 90
```

## Production Checklist

### Pre-Deployment
- [ ] All tests passing
- [ ] Security scan completed
- [ ] Performance testing done
- [ ] Documentation updated
- [ ] Runbook created
- [ ] Rollback plan ready

### Deployment
- [ ] Database migrations run
- [ ] Secrets configured
- [ ] Monitoring enabled
- [ ] Alerts configured
- [ ] Health checks passing
- [ ] Load balancer configured

### Post-Deployment
- [ ] Smoke tests passed
- [ ] Performance metrics normal
- [ ] Error rates acceptable
- [ ] User acceptance testing
- [ ] Documentation published
- [ ] Team notified

## Support and Maintenance

### SLA Targets
- **Uptime**: 99.9% (43.8 minutes downtime/month)
- **Response Time**: < 500ms p95
- **Error Rate**: < 0.1%
- **Recovery Time**: < 4 hours

### On-Call Procedures
1. Alert received via PagerDuty
2. Check runbook for known issues
3. Assess severity and impact
4. Implement fix or workaround
5. Document incident
6. Post-mortem if needed

## Next Steps

- [Monitoring Guide](../advanced/monitoring.md) - Set up comprehensive monitoring
- [Security Best Practices](../advanced/security.md) - Security hardening
- [Troubleshooting Guide](../reference/troubleshooting.md) - Common issues and solutions
