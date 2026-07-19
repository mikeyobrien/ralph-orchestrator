# Docker デプロイガイド

一貫した再現可能な環境のために、Docker を使って Ralph Orchestrator をデプロイします。

## 前提条件

- Docker Engine 20.10 以降をインストール済み
- Docker Compose 2.0 以降（任意、マルチコンテナ構成用）
- 少なくとも 1 つの AI CLI ツールの API キーを設定済み
- 最低 2GB RAM、推奨 4GB
- イメージとデータ用に 10GB のディスク容量

## クイックスタート

### ビルド済みイメージを使う

```bash
# 最新のイメージを取得する
docker pull ghcr.io/mikeyobrien/ralph-orchestrator:latest

# 既定の設定で実行する
docker run -it \
  -v $(pwd):/workspace \
  -e CLAUDE_API_KEY=$CLAUDE_API_KEY \
  ghcr.io/mikeyobrien/ralph-orchestrator:latest
```

### ソースからビルドする

プロジェクトルートに `Dockerfile` を作成します。

```dockerfile
# Multi-stage build for optimal size
FROM python:3.11-slim as builder

# Install build dependencies
RUN apt-get update && apt-get install -y \
    gcc \
    git \
    && rm -rf /var/lib/apt/lists/*

# Copy requirements
WORKDIR /build
COPY pyproject.toml uv.lock ./
RUN pip install uv && uv sync --frozen

# Runtime stage
FROM python:3.11-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    git \
    nodejs \
    npm \
    && rm -rf /var/lib/apt/lists/*

# Install AI CLI tools
RUN npm install -g @anthropic-ai/claude-code
RUN npm install -g @google/gemini-cli

# Copy application
WORKDIR /app
COPY --from=builder /build/.venv /app/.venv
COPY . /app/

# Set environment
ENV PATH="/app/.venv/bin:$PATH"
ENV PYTHONUNBUFFERED=1

# Create workspace directory
RUN mkdir -p /workspace
WORKDIR /workspace

# Entry point
ENTRYPOINT ["python", "/app/ralph_orchestrator.py"]
CMD ["--help"]
```

ビルドして実行します。

```bash
# イメージをビルドする
docker build -t ralph-orchestrator:local .

# プロンプトで実行する
docker run -it \
  -v $(pwd):/workspace \
  -e CLAUDE_API_KEY=$CLAUDE_API_KEY \
  ralph-orchestrator:local \
  --agent claude \
  --prompt PROMPT.md
```

## Docker Compose のセットアップ

複数サービスを伴う複雑なデプロイのために:

```yaml
# docker-compose.yml
version: '3.8'

services:
  ralph:
    image: ghcr.io/mikeyobrien/ralph-orchestrator:latest
    container_name: ralph-orchestrator
    environment:
      - CLAUDE_API_KEY=${CLAUDE_API_KEY}
      - GEMINI_API_KEY=${GEMINI_API_KEY}
      - Q_API_KEY=${Q_API_KEY}
      - RALPH_MAX_ITERATIONS=100
      - RALPH_MAX_RUNTIME=14400
    volumes:
      - ./workspace:/workspace
      - ./prompts:/prompts:ro
      - ralph-cache:/app/.cache
    networks:
      - ralph-network
    restart: unless-stopped
    command: 
      - --agent=auto
      - --prompt=/prompts/PROMPT.md
      - --verbose

  # Optional: Monitoring with Prometheus
  prometheus:
    image: prom/prometheus:latest
    container_name: ralph-prometheus
    volumes:
      - ./monitoring/prometheus.yml:/etc/prometheus/prometheus.yml
      - prometheus-data:/prometheus
    networks:
      - ralph-network
    ports:
      - "9090:9090"

  # Optional: Grafana for visualization
  grafana:
    image: grafana/grafana:latest
    container_name: ralph-grafana
    environment:
      - GF_SECURITY_ADMIN_PASSWORD=admin
    volumes:
      - grafana-data:/var/lib/grafana
      - ./monitoring/dashboards:/etc/grafana/provisioning/dashboards
    networks:
      - ralph-network
    ports:
      - "3000:3000"

volumes:
  ralph-cache:
  prometheus-data:
  grafana-data:

networks:
  ralph-network:
    driver: bridge
```

スタックを起動します。

```bash
# すべてのサービスを起動する
docker-compose up -d

# ログを表示する
docker-compose logs -f ralph

# すべてのサービスを停止する
docker-compose down
```

## 環境変数

環境変数を通じて Ralph を設定します。

| 変数 | 説明 | 既定 |
|----------|-------------|---------|
| `CLAUDE_API_KEY` | Anthropic Claude の API キー | Claude に必須 |
| `GEMINI_API_KEY` | Google Gemini の API キー | Gemini に必須 |
| `Q_API_KEY` | Q Chat の API キー | Q に必須 |
| `RALPH_AGENT` | 既定のエージェント（claude/gemini/q/auto） | auto |
| `RALPH_MAX_ITERATIONS` | 最大ループイテレーション | 100 |
| `RALPH_MAX_RUNTIME` | 最大実行時間（秒） | 14400 |
| `RALPH_MAX_TOKENS` | 最大合計トークン | 1000000 |
| `RALPH_MAX_COST` | 最大コスト（USD） | 50.0 |
| `RALPH_CHECKPOINT_INTERVAL` | Git チェックポイントの頻度 | 5 |
| `RALPH_VERBOSE` | 詳細ログを有効にする | false |
| `RALPH_DRY_RUN` | 実行せずのテストモード | false |

## ボリュームマウント

マウントすべき必須のディレクトリ:

```bash
docker run -it \
  -v $(pwd)/workspace:/workspace \           # 作業ディレクトリ
  -v $(pwd)/prompts:/prompts:ro \           # プロンプトファイル（読み取り専用）
  -v $(pwd)/.agent:/app/.agent \            # エージェントの状態
  -v $(pwd)/.git:/workspace/.git \          # Git リポジトリ
  -v ~/.ssh:/root/.ssh:ro \                 # SSH キー（必要な場合）
  ralph-orchestrator:latest
```

## セキュリティ上の考慮事項

### 非 root ユーザーとして実行する

```dockerfile
# Add to Dockerfile
RUN useradd -m -u 1000 ralph
USER ralph
```

```bash
# ユーザーマッピングで実行する
docker run -it \
  --user $(id -u):$(id -g) \
  -v $(pwd):/workspace \
  ralph-orchestrator:latest
```

### シークレットの管理

API キーを決してハードコードしないでください。Docker シークレットまたは環境ファイルを
使います。

```bash
# .env ファイル（.gitignore に追加すること！）
CLAUDE_API_KEY=sk-ant-...
GEMINI_API_KEY=AIza...
Q_API_KEY=...

# 環境ファイルで実行する
docker run -it \
  --env-file .env \
  -v $(pwd):/workspace \
  ralph-orchestrator:latest
```

### ネットワークの分離

```bash
# 分離されたネットワークを作成する
docker network create ralph-isolated

# ネットワーク分離で実行する
docker run -it \
  --network ralph-isolated \
  --network-alias ralph \
  -v $(pwd):/workspace \
  ralph-orchestrator:latest
```

## リソース上限

暴走するコンテナを防ぎます。

```bash
docker run -it \
  --memory="4g" \
  --memory-swap="4g" \
  --cpu-shares=512 \
  --pids-limit=100 \
  -v $(pwd):/workspace \
  ralph-orchestrator:latest
```

## ヘルスチェック

ヘルス監視を追加します。

```dockerfile
# Add to Dockerfile
HEALTHCHECK --interval=30s --timeout=3s --retries=3 \
  CMD python -c "import sys; sys.exit(0)" || exit 1
```

## デバッグ

### 対話的なシェル

```bash
# ralph の代わりにシェルで起動する
docker run -it \
  -v $(pwd):/workspace \
  --entrypoint /bin/bash \
  ralph-orchestrator:latest

# コンテナ内で
python /app/ralph_orchestrator.py --dry-run
```

### ログの表示

```bash
# コンテナのログをフォローする
docker logs -f <container-id>

# ログをファイルに保存する
docker logs <container-id> > ralph.log 2>&1
```

### 実行中のコンテナの検査

```bash
# 実行中のコンテナでコマンドを実行する
docker exec -it <container-id> /bin/bash

# プロセスの状況を確認する
docker exec <container-id> ps aux

# 環境を表示する
docker exec <container-id> env
```

## 本番デプロイ

### Docker Swarm を使う

```bash
# swarm を初期化する
docker swarm init

# シークレットを作成する
echo $CLAUDE_API_KEY | docker secret create claude_key -
echo $GEMINI_API_KEY | docker secret create gemini_key -

# スタックをデプロイする
docker stack deploy -c docker-compose.yml ralph-stack

# サービスをスケールする
docker service scale ralph-stack_ralph=3
```

### Kubernetes を使う

大規模なコンテナオーケストレーションは
[Kubernetes デプロイガイド](kubernetes.ja.md) を参照してください。

## 監視とメトリクス

### メトリクスのエクスポート

```python
# 設定でメトリクスを有効にする
docker run -it \
  -e RALPH_ENABLE_METRICS=true \
  -e RALPH_METRICS_PORT=8080 \
  -p 8080:8080 \
  ralph-orchestrator:latest
```

### Prometheus の設定

```yaml
# prometheus.yml
scrape_configs:
  - job_name: 'ralph'
    static_configs:
      - targets: ['ralph:8080']
    metrics_path: '/metrics'
```

## トラブルシューティング

### よくある問題

#### 権限が拒否される

```bash
# ボリュームの権限を修正する
docker run -it \
  --user $(id -u):$(id -g) \
  -v $(pwd):/workspace:Z \  # SELinux コンテキスト
  ralph-orchestrator:latest
```

#### メモリ不足

```bash
# メモリ上限を増やす
docker run -it \
  --memory="8g" \
  --memory-swap="8g" \
  ralph-orchestrator:latest
```

#### ネットワークのタイムアウト

```bash
# タイムアウト値を増やす
docker run -it \
  -e RALPH_RETRY_DELAY=5 \
  -e RALPH_MAX_RETRIES=10 \
  ralph-orchestrator:latest
```

### デバッグモード

```bash
# デバッグログを有効にする
docker run -it \
  -e LOG_LEVEL=DEBUG \
  -e RALPH_VERBOSE=true \
  ralph-orchestrator:latest \
  --verbose --dry-run
```

## ベストプラクティス

1. 本番では**常に具体的なイメージタグを使う**（`latest` ではなく）
2. 誤った変更を防ぐため**プロンプトを読み取り専用でマウントする**
3. 不要なファイルを除外するため **.dockerignore を使う**
4. 自動回復のため**ヘルスチェックを実装する**
5. リソースの枯渇を防ぐため**リソース上限を設定する**
6. イメージサイズを最小化するため**マルチステージビルドを使う**
7. Trivy のようなツールで**イメージの脆弱性をスキャンする**
8. **シークレットを決してバージョン管理にコミットしない**
9. 永続的なデータには**ボリュームマウントを使う**
10. **コンテナのログ**とメトリクスを監視する

## .dockerignore の例

```
# .dockerignore
.git
.github
*.pyc
__pycache__
.pytest_cache
.venv
site/
docs/
tests/
*.md
!README.md
.env
.env.*
```

## 次のステップ

- [Kubernetes デプロイ](kubernetes.ja.md) - コンテナオーケストレーション向け
- [CI/CD 統合](ci-cd.ja.md) - Docker ビルドの自動化
- [本番ガイド](production.ja.md) - 本番のベストプラクティス
