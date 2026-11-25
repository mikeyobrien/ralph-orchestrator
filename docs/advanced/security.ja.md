# セキュリティの考慮事項

## 概要

Ralph Orchestrator は、大きなシステムアクセスを伴って AI エージェントを実行します。この
ドキュメントは、安全な運用のためのセキュリティの考慮事項とベストプラクティスを概説します。

## 脅威モデル

### 潜在的なリスク

1. **意図しないコード実行**
   - AI エージェントが有害なコードを生成・実行し得る
   - プロジェクトの範囲を超えたファイルシステムの変更
   - システムコマンドの実行

2. **データの露出**
   - プロンプトやコード内の API キー
   - Git 履歴内の機微なデータ
   - 状態ファイル内の認証情報

3. **リソースの枯渇**
   - 生成されたコード内の無限ループ
   - 過剰な API 呼び出し
   - ディスク容量の消費

4. **サプライチェーン**
   - 侵害された AI CLI ツール
   - 悪意あるプロンプトインジェクション
   - 依存の脆弱性

## セキュリティ制御

Ralph は、脅威から保護するために複数のセキュリティレイヤーを実装しています。

```
 🔒 Security Defense Layers

   ╭───────────────────╮
   │    User Input     │
   ╰───────────────────╯
     │
     │
     ∨
   ┌───────────────────┐
   │ Input Validation  │
   └───────────────────┘
     │
     │
     ∨
   ┌───────────────────┐
   │ Process Isolation │
   └───────────────────┘
     │
     │
     ∨
   ┌───────────────────┐
   │  File Boundaries  │
   └───────────────────┘
     │
     │
     ∨
   ┌───────────────────┐
   │    Git Safety     │
   └───────────────────┘
     │
     │
     ∨
   ┌───────────────────┐
   │ Env Sanitization  │
   └───────────────────┘
     │
     │
     ∨
   ╭───────────────────╮
   │     AI Agent      │
   ╰───────────────────╯
```

<details>
<summary>graph-easy のソース</summary>

```
graph { label: "🔒 Security Defense Layers"; flow: south; }
[ User Input ] { shape: rounded; } -> [ Input Validation ]
[ Input Validation ] -> [ Process Isolation ]
[ Process Isolation ] -> [ File Boundaries ]
[ File Boundaries ] -> [ Git Safety ]
[ Git Safety ] -> [ Env Sanitization ]
[ Env Sanitization ] -> [ AI Agent ] { shape: rounded; }
```

</details>

### プロセスの分離

Ralph は AI エージェントを、次を伴うサブプロセスで実行します。

- タイムアウト保護（既定 5 分）
- 出力サイズの上限
- エラー境界

```python
result = subprocess.run(
    cmd,
    capture_output=True,
    text=True,
    timeout=300,  # 5-minute timeout
    env=filtered_env  # Sanitized environment
)
```

### ファイルシステムの境界

#### 制限されたパス

- プロジェクトディレクトリ内で作業する
- システムファイルにアクセスしない
- .git の整合性を保つ

#### 安全な既定

```python
# Validate paths stay within project
def validate_path(path):
    abs_path = os.path.abspath(path)
    project_path = os.path.abspath('.')
    return abs_path.startswith(project_path)
```

### Git の安全性

#### 保護される操作

- フォースプッシュなし
- ブランチの削除なし
- 履歴の書き換えなし

#### チェックポイントのみのコミット

```bash
# Ralph only creates checkpoint commits
git add .
git commit -m "Ralph checkpoint: iteration N"
```

### 環境のサニタイズ

#### フィルタされた変数

```python
SAFE_ENV_VARS = [
    'PATH', 'HOME', 'USER',
    'LANG', 'LC_ALL', 'TERM'
]

def get_safe_env():
    return {k: v for k, v in os.environ.items()
            if k in SAFE_ENV_VARS}
```

#### 認証情報の露出なし

- API キーを決して環境経由で渡さない
- エージェントは自身の認証情報ストアを使うべき
- プロンプトやログに秘密情報を入れない

## ベストプラクティス

### 1. プロンプトのセキュリティ

#### する

- 実行前にプロンプトをレビューする
- 具体的で境界のある指示を使う
- 安全上の制約を含める

#### しない

- プロンプトに認証情報を含める
- システムレベルの変更を要求する
- 無制限のイテレーションを使う

### 2. エージェントの設定

#### Claude

```bash
# Use restricted mode if available
claude --safe-mode PROMPT.md
```

#### Gemini

```bash
# Limit context and capabilities
gemini --no-web --no-exec PROMPT.md
```

### 3. リポジトリのセットアップ

#### .gitignore

```gitignore
# Security-sensitive files
*.key
*.pem
.env
.env.*
secrets/
credentials/

# Ralph workspace
.agent/metrics/
.agent/logs/
```

#### pre-commit フック

```yaml
# .pre-commit-config.yaml
repos:
  - repo: https://github.com/Yelp/detect-secrets
    hooks:
      - id: detect-secrets
        args: ["--baseline", ".secrets.baseline"]
```

### 4. ランタイムの監視

#### リソース上限

```python
# Set resource limits
import resource

# Limit memory usage to 1GB
resource.setrlimit(
    resource.RLIMIT_AS,
    (1024 * 1024 * 1024, -1)
)

# Limit CPU time to 1 hour
resource.setrlimit(
    resource.RLIMIT_CPU,
    (3600, -1)
)
```

#### 監査ロギング

```python
import logging
import json

# Log all agent executions
logging.info(json.dumps({
    'event': 'agent_execution',
    'agent': agent_name,
    'timestamp': time.time(),
    'user': os.getenv('USER'),
    'prompt_hash': hashlib.sha256(prompt.encode()).hexdigest()
}))
```

## セキュリティチェックリスト

### Ralph を実行する前に

- [ ] 安全でない指示がないか PROMPT.md をレビューする
- [ ] プロンプトに認証情報がないことを確認する
- [ ] 作業ディレクトリが正しいことを検証する
- [ ] Git リポジトリがバックアップされていることを確認する
- [ ] エージェントツールが最新であることを確認する

### 実行中

- [ ] リソース使用を監視する
- [ ] 予期しないファイル変更に注意する
- [ ] エージェント出力に異常がないか確認する
- [ ] チェックポイントが作成されていることを検証する
- [ ] ログに機微なデータがないことを確認する

### 完了後

- [ ] 生成されたコードにセキュリティ問題がないかレビューする
- [ ] Git 履歴に露出した秘密情報がないか確認する
- [ ] システムファイルが変更されていないことを検証する
- [ ] 一時ファイルをクリーンアップする
- [ ] 露出した可能性のある認証情報をローテーションする

## インシデント対応

### 侵害が疑われる場合

1. **即座の行動**

   ```bash
   # Stop Ralph
   pkill -f ralph_orchestrator

   # Preserve evidence
   cp -r .agent /tmp/ralph-incident-$(date +%s)

   # Check for modifications
   git status
   git diff
   ```

2. **調査**
   - .agent/metrics/state\_\*.json をレビューする
   - システムログを確認する
   - Git 履歴を調べる
   - エージェント出力を分析する

3. **回復**

   ```bash
   # Reset to last known good state
   git reset --hard <last-good-commit>

   # Clean workspace
   rm -rf .agent

   # Rotate credentials if needed
   # Update API keys for affected services
   ```

## サンドボックスの選択肢

### Docker コンテナ

```dockerfile
FROM python:3.11-slim
RUN useradd -m -s /bin/bash ralph
USER ralph
WORKDIR /home/ralph/project
COPY --chown=ralph:ralph . .
CMD ["./ralph", "run"]
```

### 仮想マシン

```bash
# Run in VM with snapshot
vagrant up
vagrant ssh -c "cd /project && ./ralph run"
vagrant snapshot restore clean
```

### 制限されたユーザー

```bash
# Create restricted user
sudo useradd -m -s /bin/bash ralph-runner
sudo usermod -L ralph-runner  # No password login

# Run as restricted user
sudo -u ralph-runner ./ralph run
```

## API キーの管理

### 安全な保管

#### キーを決して保管しない場所

- PROMPT.md ファイル
- Git リポジトリ
- スクリプト内の環境変数
- ログファイル

#### 推奨されるアプローチ

1. エージェント固有の認証情報ストア
2. システムのキーチェーン/キーリング
3. 暗号化されたボールト（例: HashiCorp Vault）
4. クラウドのシークレットマネージャ

### キーのローテーション

```bash
# Regular rotation schedule
# 1. Generate new keys
# 2. Update agent configurations
# 3. Test with new keys
# 4. Revoke old keys
```

## コンプライアンスの考慮事項

### データプライバシー

- プロンプトで PII を処理しない
- 共有する前に出力をサニタイズする
- データレジデンシーの要件を遵守する

### 監査証跡

- 実行ログを維持する
- プロンプトの変更を追跡する
- エージェントとのやり取りを文書化する

### アクセス制御

- 誰が Ralph を実行できるかを制限する
- エージェントの権限を制限する
- リポジトリのアクセスを制御する

## セキュリティ更新

次を最新に保ちます。

- AI CLI ツールの更新
- Python のセキュリティパッチ
- Git のセキュリティ勧告
- 依存の脆弱性

```bash
# Check for updates
npm update -g @anthropic-ai/claude-code
pip install --upgrade subprocess
git --version
```

## セキュリティ問題の報告

セキュリティの脆弱性を発見した場合:

1. 公開の issue を**開かない**
2. セキュリティレポートを次にメールする: <security@ralph-orchestrator.org>
3. 次を含める:
   - 脆弱性の説明
   - 再現手順
   - 潜在的な影響
   - 提案する修正（あれば）

48 時間以内に応答し、速やかに修正を提供することを目指します。
