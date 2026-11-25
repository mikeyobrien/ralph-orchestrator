# Q Chat アダプタ本番デプロイガイド

!!! warning "非推奨"
    Q Chat CLI は **Kiro CLI** にリブランドされました。このガイドはレガシーの Q Chat
    アダプタを参照しています。新しい `kiro` アダプタへの移行については
    [Kiro 移行ガイド](../guide/kiro-migration.ja.md) を参照してください。

このガイドは、Ralph Orchestrator とともに本番環境で Q Chat アダプタをデプロイするための
包括的な手順を提供します。

## 概要

Q Chat アダプタは、次の能力を伴って本番利用向けに徹底的にテストおよび検証されています。
- スレッドセーフな並行メッセージ処理
- 堅牢なエラー処理と回復
- グレースフルシャットダウンとリソースのクリーンアップ
- デッドロックを防ぐノンブロッキング I/O
- 指数バックオフを伴う自動リトライ
- クリーンな終了のためのシグナル処理

## 前提条件

### システム要件
- Python 3.8 以上
- Q CLI をインストールし設定済み
- 並行操作のための十分なメモリ（最低 2GB 推奨）
- Unix 系オペレーティングシステム（Linux、macOS）

### インストール
```bash
# Q CLI をインストールする
pip install q-cli

# インストールを検証する
qchat --version

# Q アダプタ対応の Ralph Orchestrator をインストールする
pip install ralph-orchestrator
```

## 設定

### 環境変数

これらの環境変数を使って Q Chat アダプタの挙動を設定します。

```bash
# Core Configuration
export QCHAT_TIMEOUT=300          # リクエストのタイムアウト（秒）（既定: 120）
export QCHAT_MAX_RETRIES=5        # 最大リトライ回数（既定: 3）
export QCHAT_RETRY_DELAY=2        # 初期リトライ遅延（秒）（既定: 1）
export QCHAT_VERBOSE=1            # 詳細ログを有効にする（既定: 0）

# Performance Tuning
export QCHAT_BUFFER_SIZE=8192     # パイプバッファサイズ（バイト）（既定: 4096）
export QCHAT_POLL_INTERVAL=0.1    # メッセージキューのポーリング間隔（既定: 0.1）
export QCHAT_MAX_CONCURRENT=10    # 最大並行リクエスト（既定: 5）

# Resource Limits
export QCHAT_MAX_MEMORY_MB=4096   # 最大メモリ使用（MB）
export QCHAT_MAX_OUTPUT_SIZE=10485760  # 最大出力サイズ（バイト）（10MB）
```

### 設定ファイル

永続的な設定のための設定ファイルを作成します。

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

## デプロイのシナリオ

### 1. 単一インスタンスのデプロイ

中程度の負荷を伴う単純な本番デプロイのために:

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

### 2. 高可用性のデプロイ

高可用性を要するミッションクリティカルなアプリケーションのために:

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

### 3. コンテナ化されたデプロイ

コンテナデプロイのための Docker 設定:

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

Docker Compose の設定:

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

## 監視と可観測性

### ロギングの設定

本番向けに構造化ロギングを設定します。

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

### メトリクス収集

主要なパフォーマンス指標を監視します。

```python
# metrics.py
from prometheus_client import Counter, Histogram, Gauge

# Define metrics
request_count = Counter('qchat_requests_total', 'Total number of Q Chat requests')
request_duration = Histogram('qchat_request_duration_seconds', 'Request duration')
active_requests = Gauge('qchat_active_requests', 'Number of active requests')
error_count = Counter('qchat_errors_total', 'Total number of errors', ['error_type'])
```

### ヘルスチェック

ヘルスチェックのエンドポイントを実装します。

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

## パフォーマンスの最適化

### 1. コネクションプーリング

高並行性のシナリオに最適化します。

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

### 2. キャッシュ戦略

繰り返しのクエリに応答キャッシュを実装します。

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

### 3. リソース上限

本番の安定性のためにリソース上限を設定します。

```bash
# Set system limits
ulimit -n 4096          # ファイルディスクリプタの上限を増やす
ulimit -u 2048          # プロセスの上限を増やす
ulimit -m 4194304       # メモリ上限を設定する（4GB）

# Configure cgroups for container environments
echo "4G" > /sys/fs/cgroup/memory/ralph-qchat/memory.limit_in_bytes
echo "80" > /sys/fs/cgroup/cpu/ralph-qchat/cpu.shares
```

## トラブルシューティング

### よくある問題と解決策

#### 1. デッドロックの防止
```bash
# Check for pipe buffer issues
strace -p <PID> -e read,write

# Increase buffer size if needed
export QCHAT_BUFFER_SIZE=16384
```

#### 2. メモリリーク
```bash
# Monitor memory usage
watch -n 1 'ps aux | grep qchat'

# Enable memory profiling
export PYTHONTRACEMALLOC=1
```

#### 3. プロセスのハング
```bash
# Check process state
ps -eLf | grep qchat

# Send diagnostic signal
kill -USR1 <PID>  # Trigger diagnostic dump
```

#### 4. 高い CPU 使用
```bash
# Profile CPU usage
py-spy top --pid <PID>

# Adjust polling interval
export QCHAT_POLL_INTERVAL=0.5
```

### デバッグモード

詳細な診断のためにデバッグモードを有効にします。

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

## セキュリティ上の考慮事項

### 1. 入力検証

常に入力を検証しサニタイズします。

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

### 2. プロセスの分離

Q Chat プロセスを限られた権限で実行します。

```bash
# Create dedicated user
useradd -r -s /bin/false qchat-user

# Run with limited privileges
sudo -u qchat-user python -m ralph_orchestrator --agent q
```

### 3. ネットワークセキュリティ

ヘルスチェックのエンドポイントにファイアウォールルールを設定します。

```bash
# Allow health check port only from monitoring systems
iptables -A INPUT -p tcp --dport 8080 -s 10.0.0.0/8 -j ACCEPT
iptables -A INPUT -p tcp --dport 8080 -j DROP
```

## 保守と更新

### ローリングアップデート

ゼロダウンタイムの更新を行います。

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

### バックアップと回復

定期的なチェックポイントのバックアップ:

```bash
# Backup checkpoints
tar -czf checkpoints-$(date +%Y%m%d).tar.gz checkpoints/

# Backup configuration
cp -r config/ backup/config-$(date +%Y%m%d)/

# Restore from backup
tar -xzf checkpoints-20240101.tar.gz
cp -r backup/config-20240101/* config/
```

## パフォーマンスベンチマーク

本番での期待されるパフォーマンスメトリクス:

| メトリクス | 値 | メモ |
|--------|-------|-------|
| **レイテンシ（p50）** | < 500ms | 単純なプロンプトの場合 |
| **レイテンシ（p99）** | < 2000ms | 複雑なプロンプトの場合 |
| **スループット** | 100 req/min | 単一インスタンス |
| **並行性** | 10-20 | 並行リクエスト |
| **メモリ使用** | < 500MB | インスタンスあたり |
| **CPU 使用** | < 50% | 平均使用率 |
| **エラー率** | < 0.1% | 本番の目標 |
| **可用性** | > 99.9% | 適切な監視を伴う |

## ベストプラクティス

1. 長時間実行タスクには**常にチェックポイントを使う**
2. リソース使用を**継続的に監視する**
3. 過負荷を防ぐため**レート制限を実装する**
4. より良いパフォーマンスのため**コネクションプーリングを使う**
5. デバッグを容易にするため**構造化ロギングを有効にする**
6. ワークロードに基づいて**適切なタイムアウトを設定する**
7. 耐障害性のため**サーキットブレーカーを実装する**
8. チェックポイントと設定の**定期的なバックアップ**
9. 災害復旧の手順を**定期的にテストする**
10. Q CLI を最新の安定版に**更新し続ける**

## サポートとリソース

- **ドキュメント**: [Ralph Orchestrator Docs](https://ralph-orchestrator.readthedocs.io)
- **Issues**: [GitHub Issues](https://github.com/your-org/ralph-orchestrator/issues)
- **コミュニティ**: [Discord サーバー](https://discord.gg/ralph-orchestrator)
- **緊急サポート**: support@ralph-orchestrator.com

## 付録: Systemd サービス

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

サービスを有効にして起動します。

```bash
systemctl daemon-reload
systemctl enable ralph-qchat.service
systemctl start ralph-qchat.service
systemctl status ralph-qchat.service
```
