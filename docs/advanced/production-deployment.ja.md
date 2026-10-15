# 本番デプロイガイド

## 概要

このガイドでは、サーバーセットアップ、自動化、監視、スケーリングの考慮事項を含め、本番環境に
Ralph Orchestrator をデプロイする方法を解説します。

## デプロイオプション

### 1. ローカルサーバーデプロイ

#### システム要件
- **OS**: Linux（Ubuntu 20.04+、RHEL 8+、Debian 11+）
- **Python**: 3.9+
- **Git**: 2.25+
- **メモリ**: 最小 4GB、推奨 8GB
- **ストレージ**: 20GB の空き容量
- **ネットワーク**: AI エージェント API への安定したインターネット接続

#### インストールスクリプト
```bash
#!/bin/bash
# ralph-install.sh

# システムを更新する
sudo apt-get update && sudo apt-get upgrade -y

# 依存をインストールする
sudo apt-get install -y python3 python3-pip git nodejs npm

# AI エージェントをインストールする
npm install -g @anthropic-ai/claude-code
npm install -g @google/gemini-cli
# Q はドキュメントに従ってインストールする

# Ralph をクローンする
git clone https://github.com/yourusername/ralph-orchestrator.git
cd ralph-orchestrator

# 権限を設定する
chmod +x ralph_orchestrator.py ralph

# systemd サービスを作成する
sudo cp ralph.service /etc/systemd/system/
sudo systemctl daemon-reload
sudo systemctl enable ralph
```

### 2. Docker デプロイ

#### Dockerfile
```dockerfile
FROM python:3.11-slim

# システム依存をインストールする
RUN apt-get update && apt-get install -y \
    git \
    nodejs \
    npm \
    && rm -rf /var/lib/apt/lists/*

# AI CLI ツールをインストールする
RUN npm install -g @anthropic-ai/claude-code @google/gemini-cli

# ralph ユーザーを作成する
RUN useradd -m -s /bin/bash ralph
WORKDIR /home/ralph

# アプリケーションをコピーする
COPY --chown=ralph:ralph . /home/ralph/ralph-orchestrator/
WORKDIR /home/ralph/ralph-orchestrator

# 権限を設定する
RUN chmod +x ralph_orchestrator.py ralph

# ralph ユーザーに切り替える
USER ralph

# 既定のコマンド
CMD ["./ralph", "run"]
```

#### Docker Compose
```yaml
# docker-compose.yml
version: '3.8'

services:
  ralph:
    build: .
    container_name: ralph-orchestrator
    restart: unless-stopped
    volumes:
      - ./workspace:/home/ralph/workspace
      - ./prompts:/home/ralph/prompts
      - ralph-agent:/home/ralph/ralph-orchestrator/.agent
    environment:
      - RALPH_MAX_ITERATIONS=100
      - RALPH_AGENT=auto
      - RALPH_CHECKPOINT_INTERVAL=5
    logging:
      driver: "json-file"
      options:
        max-size: "10m"
        max-file: "3"

volumes:
  ralph-agent:
```

### 3. クラウドデプロイ

#### AWS EC2
```bash
# EC2 インスタンス用のユーザーデータスクリプト
#!/bin/bash
yum update -y
yum install -y python3 git nodejs

# Ralph をインストールする
cd /opt
git clone https://github.com/yourusername/ralph-orchestrator.git
cd ralph-orchestrator
chmod +x ralph_orchestrator.py ralph

# サービスとして構成する
cat > /etc/systemd/system/ralph.service << EOF
[Unit]
Description=Ralph Orchestrator
After=network.target

[Service]
Type=simple
User=ec2-user
WorkingDirectory=/opt/ralph-orchestrator
ExecStart=/opt/ralph-orchestrator/ralph run
Restart=on-failure
RestartSec=10

[Install]
WantedBy=multi-user.target
EOF

systemctl enable ralph
systemctl start ralph
```

#### Kubernetes デプロイ
```yaml
# ralph-deployment.yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: ralph-orchestrator
spec:
  replicas: 1
  selector:
    matchLabels:
      app: ralph
  template:
    metadata:
      labels:
        app: ralph
    spec:
      containers:
      - name: ralph
        image: ralph-orchestrator:latest
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
          claimName: ralph-workspace
      - name: config
        configMap:
          name: ralph-config
```

## 構成管理

### 環境変数
```bash
# /etc/environment または .env ファイル
RALPH_HOME=/opt/ralph-orchestrator
RALPH_WORKSPACE=/var/ralph/workspace
RALPH_LOG_LEVEL=INFO
RALPH_MAX_ITERATIONS=100
RALPH_MAX_RUNTIME=14400
RALPH_AGENT=claude
RALPH_CHECKPOINT_INTERVAL=5
RALPH_RETRY_DELAY=2
RALPH_GIT_ENABLED=true
RALPH_ARCHIVE_ENABLED=true
```

### 設定ファイル
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

## 自動化

### Systemd サービス
```ini
# /etc/systemd/system/ralph.service
[Unit]
Description=Ralph Orchestrator Service
Documentation=https://github.com/yourusername/ralph-orchestrator
After=network.target

[Service]
Type=simple
User=ralph
Group=ralph
WorkingDirectory=/opt/ralph-orchestrator
ExecStart=/opt/ralph-orchestrator/ralph run --config production.json
ExecReload=/bin/kill -HUP $MAINPID
Restart=on-failure
RestartSec=30
StandardOutput=journal
StandardError=journal
SyslogIdentifier=ralph
Environment="PYTHONUNBUFFERED=1"

# セキュリティ
NoNewPrivileges=true
PrivateTmp=true
ProtectSystem=strict
ProtectHome=true
ReadWritePaths=/opt/ralph-orchestrator /var/ralph

[Install]
WantedBy=multi-user.target
```

### Cron ジョブ
```bash
# /etc/cron.d/ralph
# 古いログを毎週削除する
0 2 * * 0 ralph /opt/ralph-orchestrator/scripts/cleanup.sh

# 状態を毎日バックアップする
0 3 * * * ralph tar -czf /backup/ralph-$(date +\%Y\%m\%d).tar.gz /opt/ralph-orchestrator/.agent

# 5分ごとにヘルスチェックする
*/5 * * * * ralph /opt/ralph-orchestrator/scripts/health-check.sh || systemctl restart ralph
```

### CI/CD パイプライン
```yaml
# .github/workflows/deploy.yml
name: Deploy Ralph

on:
  push:
    branches: [main]
    paths:
      - 'ralph_orchestrator.py'
      - 'ralph'
      - 'requirements.txt'

jobs:
  deploy:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3

      - name: Run tests
        run: python test_comprehensive.py

      - name: Build Docker image
        run: docker build -t ralph-orchestrator:${{ github.sha }} .

      - name: Push to registry
        run: |
          docker tag ralph-orchestrator:${{ github.sha }} ${{ secrets.REGISTRY }}/ralph:latest
          docker push ${{ secrets.REGISTRY }}/ralph:latest

      - name: Deploy to server
        uses: appleboy/ssh-action@v0.1.5
        with:
          host: ${{ secrets.HOST }}
          username: ${{ secrets.USERNAME }}
          key: ${{ secrets.SSH_KEY }}
          script: |
            cd /opt/ralph-orchestrator
            git pull
            systemctl restart ralph
```

## 本番環境での監視

### Prometheus メトリクス
```python
# metrics_exporter.py
from prometheus_client import Counter, Histogram, Gauge, start_http_server
import json
import glob

# メトリクスを定義する
iteration_counter = Counter('ralph_iterations_total', 'Total iterations')
error_counter = Counter('ralph_errors_total', 'Total errors')
runtime_gauge = Gauge('ralph_runtime_seconds', 'Current runtime')
iteration_duration = Histogram('ralph_iteration_duration_seconds', 'Iteration duration')

def collect_metrics():
    """Ralph の状態ファイルからメトリクスを収集する"""
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
    # メトリクスサーバーを起動する
    start_http_server(8000)

    # 定期的にメトリクスを収集する
    while True:
        collect_metrics()
        time.sleep(30)
```

### ロギングのセットアップ
```python
# logging_config.py
import logging
import logging.handlers
import json

def setup_production_logging():
    """本番ロギングを構成する"""

    # 構造化ロギング用の JSON フォーマッタ
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

    # ルートロガーを構成する
    logger = logging.getLogger()
    logger.setLevel(logging.INFO)

    # ローテーション付きのファイルハンドラ
    file_handler = logging.handlers.RotatingFileHandler(
        '/var/log/ralph/ralph.log',
        maxBytes=100*1024*1024,  # 100MB
        backupCount=10
    )
    file_handler.setFormatter(JSONFormatter())

    # Syslog ハンドラ
    syslog_handler = logging.handlers.SysLogHandler(address='/dev/log')
    syslog_handler.setFormatter(JSONFormatter())

    logger.addHandler(file_handler)
    logger.addHandler(syslog_handler)
```

## セキュリティ強化

### ユーザーの分離
```bash
# 専用ユーザーを作成する
sudo useradd -r -s /bin/bash -m -d /opt/ralph ralph
sudo chown -R ralph:ralph /opt/ralph-orchestrator

# 制限的な権限を設定する
chmod 750 /opt/ralph-orchestrator
chmod 640 /opt/ralph-orchestrator/*.py
chmod 750 /opt/ralph-orchestrator/ralph
```

### ネットワークセキュリティ
```bash
# ファイアウォールルール（iptables）
iptables -A OUTPUT -p tcp --dport 443 -j ACCEPT  # AI エージェント用の HTTPS
iptables -A OUTPUT -p tcp --dport 22 -j ACCEPT   # Git SSH
iptables -A OUTPUT -j DROP                       # その他のアウトバウンドをブロック

# または ufw を使う
ufw allow out 443/tcp
ufw allow out 22/tcp
ufw default deny outgoing
```

### API キー管理
```bash
# システムキーリングを使う
pip install keyring

# API キーを安全に保管する
python -c "import keyring; keyring.set_password('ralph', 'claude_api_key', 'your-key')"

# または安全なストアからの環境変数を使う
source /etc/ralph/secrets.env
```

## スケーリングの考慮事項

### 水平スケーリング
```python
# job_queue.py
import redis
import json

class RalphJobQueue:
    def __init__(self):
        self.redis = redis.Redis(host='localhost', port=6379)

    def add_job(self, prompt_file, config):
        """ジョブをキューに追加する"""
        job = {
            'id': str(uuid.uuid4()),
            'prompt_file': prompt_file,
            'config': config,
            'status': 'pending',
            'created': time.time()
        }
        self.redis.lpush('ralph:jobs', json.dumps(job))
        return job['id']

    def get_job(self):
        """キューから次のジョブを取得する"""
        job_data = self.redis.rpop('ralph:jobs')
        if job_data:
            return json.loads(job_data)
        return None
```

### リソース上限
```python
# resource_limits.py
import resource

def set_production_limits():
    """本番用のリソース上限を設定する"""

    # メモリ上限（4GB）
    resource.setrlimit(
        resource.RLIMIT_AS,
        (4 * 1024 * 1024 * 1024, -1)
    )

    # CPU 時間上限（1時間）
    resource.setrlimit(
        resource.RLIMIT_CPU,
        (3600, 3600)
    )

    # ファイルサイズ上限（100MB）
    resource.setrlimit(
        resource.RLIMIT_FSIZE,
        (100 * 1024 * 1024, -1)
    )

    # プロセス数上限
    resource.setrlimit(
        resource.RLIMIT_NPROC,
        (100, 100)
    )
```

## バックアップと復旧

### 自動バックアップ
```bash
#!/bin/bash
# backup.sh

BACKUP_DIR="/backup/ralph"
TIMESTAMP=$(date +%Y%m%d_%H%M%S)

# バックアップを作成する
tar -czf $BACKUP_DIR/ralph_$TIMESTAMP.tar.gz \
    /opt/ralph-orchestrator/.agent \
    /opt/ralph-orchestrator/*.json \
    /opt/ralph-orchestrator/PROMPT.md

# 直近30日分のみ保持する
find $BACKUP_DIR -name "ralph_*.tar.gz" -mtime +30 -delete

# S3 に同期する（任意）
aws s3 sync $BACKUP_DIR s3://my-bucket/ralph-backups/
```

### 災害復旧
```bash
#!/bin/bash
# restore.sh

BACKUP_FILE=$1
RESTORE_DIR="/opt/ralph-orchestrator"

# サービスを停止する
systemctl stop ralph

# バックアップを復元する
tar -xzf $BACKUP_FILE -C /

# Git リポジトリをリセットする
cd $RESTORE_DIR
git reset --hard HEAD

# サービスを再起動する
systemctl start ralph
```

## ヘルスチェック

### HTTP ヘルスエンドポイント
```python
# health_server.py
from flask import Flask, jsonify
import os
import json

app = Flask(__name__)

@app.route('/health')
def health():
    """ヘルスチェックエンドポイント"""
    try:
        # Ralph プロセスを確認する
        pid_file = '/var/run/ralph.pid'
        if os.path.exists(pid_file):
            with open(pid_file) as f:
                pid = int(f.read())
            os.kill(pid, 0)  # プロセスが存在するか確認する
            status = 'healthy'
        else:
            status = 'unhealthy'

        # 最後の状態を確認する
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

## 本番チェックリスト

### デプロイ前
- [ ] すべてのテストが通過している
- [ ] 構成をレビュー済み
- [ ] API キーが保護されている
- [ ] バックアップ戦略が整っている
- [ ] 監視が構成されている
- [ ] リソース上限が設定されている
- [ ] セキュリティ強化を適用済み

### デプロイ
- [ ] サービスがインストールされている
- [ ] 権限が正しく設定されている
- [ ] ロギングが構成されている
- [ ] ヘルスチェックが機能している
- [ ] メトリクス収集が有効になっている
- [ ] バックアップジョブがスケジュールされている

### デプロイ後
- [ ] サービスが実行中である
- [ ] ログが生成されている
- [ ] メトリクスが可視化されている
- [ ] テストジョブが成功している
- [ ] アラートが構成されている
- [ ] ドキュメントが更新されている
