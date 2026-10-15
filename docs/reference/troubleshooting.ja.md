# トラブルシューティングガイド

## よくある問題と解決策

### インストールの問題

#### エージェントが見つからない

**問題**: `ralph: command 'claude' not found`

**解決策**:

1. エージェントのインストールを検証する:

   ```bash
   which claude
   which gemini
   which q
   ```

2. 欠けているエージェントをインストールする:

   ```bash
   # Claude
   npm install -g @anthropic-ai/claude-code

   # Gemini
   npm install -g @google/gemini-cli
   ```

3. PATH に追加する:

   ```bash
   export PATH=$PATH:/usr/local/bin
   ```

#### 権限が拒否される

**問題**: `Permission denied: './ralph'`

**解決策**:

```bash
chmod +x ralph
```

### 設定の問題

#### 設定ファイルが既に存在する

**問題**: `ralph.yml already exists. Use --force to overwrite.`

**解決策**:

1. 既存のファイルを上書きする:

   ```bash
   ralph init --backend claude --force
   ```

2. 既存の設定を移動またはリネームする:

   ```bash
   mv ralph.yml ralph.yml.bak
   ```

3. 別の設定ファイルを使う:

   ```bash
   ralph run -c path/to/other.yml
   ```

#### 設定が見つからない

**問題**: `Config file not found: ralph.yml`

**解決策**:

1. パスを検証する:

   ```bash
   ls -la ralph.yml
   ```

2. 設定を生成する:

   ```bash
   ralph init --backend claude
   ```

3. 設定フラグを省いて既定を使う:

   ```bash
   ralph run
   ```

#### 不明なバックエンド

**問題**: `Unknown backend 'foo'`

**解決策**:

1. サポートされるバックエンドを使う:

   ```bash
   ralph init --backend claude
   ralph init --backend gemini
   ralph init --backend codex
   ```

2. プリセットを一覧する（バックエンドのヒントを含む）:

   ```bash
   ralph init --list-presets
   ```

#### 不明なプリセット

**問題**: `Unknown preset 'foo'`

**解決策**:

1. プリセットを一覧する:

   ```bash
   ralph init --list-presets
   ```

2. 既知の組み込みハットコレクションを使う:

   ```bash
   ralph init --backend claude
   ralph run -c ralph.yml -H builtin:code-assist
   ```

#### カスタムバックエンドのコマンド

**問題**: `Custom backend requires a command`

**解決策**:

1. 設定にコマンドを追加する:

   ```yaml
   cli:
     backend: "custom"
     command: "my-agent"
     prompt_mode: "stdin" # または "arg"
   ```

2. テンプレートを生成する:

   ```bash
   ralph init --backend custom
   ```

#### 曖昧なルーティング

**問題**: `Ambiguous routing: trigger 'build.done' is claimed by both 'builder' and 'reviewer'`

**解決策**:

1. 各トリガーを主張するハットが 1 つだけであることを確認する:

   ```yaml
   hats:
     builder:
       triggers: ["build.task"]
       publishes: ["build.done"]
     reviewer:
       triggers: ["review.request"]
       publishes: ["review.done"]
   ```

2. コアイベントを再利用する代わりに、委譲イベント（例: `work.start`）を使う。

#### 予約トリガー

**問題**: `Reserved trigger 'task.start' used by hat 'builder'`

**解決策**:

1. 予約トリガーをカスタムイベントに置き換える:

   ```yaml
   hats:
     builder:
       triggers: ["work.start"]
       publishes: ["work.done"]
   ```

#### ハットの description が欠けている

**問題**: `Hat 'builder' is missing required 'description' field`

**解決策**:

```yaml
hats:
  builder:
    description: "Implements code changes for assigned tasks"
```

#### 相互排他のフィールド

**問題**: `Mutually exclusive fields: 'prompt' and 'prompt_file' cannot both be specified`

**解決策**:

- `event_loop` では `prompt` **または** `prompt_file` のどちらかを使い、両方は使わない。

#### RObot の設定

**問題**: `RObot config error: RObot.timeout_seconds - timeout_seconds is required when RObot is enabled`

**解決策**:

1. 必須のフィールドを設定する:

   ```yaml
   RObot:
     enabled: true
     timeout_seconds: 300
     telegram:
       bot_token: "..." # または RALPH_TELEGRAM_BOT_TOKEN を設定
   ```

2. またはヒューマンインザループが不要なら RObot を無効にする:

   ```yaml
   RObot:
     enabled: false
   ```

### 実行の問題

#### タスクが長く走りすぎる

**問題**: Ralph が目標を達成せずに最大イテレーションを走る

**考えられる原因**:

1. 不明瞭または過度に複雑なタスクの説明
2. エージェントが目標に向けて進捗していない
3. タスクの範囲がイテレーション上限には大きすぎる

**解決策**:

1. イテレーションの進捗とログを確認する:

   ```bash
   ralph status
   ```

2. 複雑なタスクを分解する:

   ```markdown
   # 次の代わりに:

   Build a complete web application

   # こう試す:

   Create a Flask app with one endpoint that returns "Hello World"
   ```

3. イテレーション上限を増やすか、別のエージェントを試す:

   ```bash
   ralph run --max-iterations 200
   ralph run --agent gemini
   ```

#### エージェントのタイムアウト

**問題**: `Agent execution timed out`

**解決策**:

1. アダプタの非活動タイムアウトを増やす:

   ```yaml
   # ralph.yml 内
   adapters:
     claude:
       timeout: 600
   ```

2. プロンプトの複雑さを減らす:
   - 大きなタスクを小さく分ける
   - 不要なコンテキストを削除する

3. システムリソースを確認する:

   ```bash
   htop
   free -h
   ```

#### 繰り返しのエラー

**問題**: 複数のイテレーションで同じエラーが起きる

**解決策**:

1. エラーパターンを確認する:

   ```bash
   cat .agent/metrics/state_*.json | jq '.errors'
   ```

2. ワークスペースをクリアして再試行する:

   ```bash
   ralph clean
   ralph run
   ```

3. 手作業の介入:
   - 特定の問題を修正する
   - PROMPT.md に明確化を加える
   - 実行を再開する

#### ループ検出の問題

**問題**: `Loop detected: XX% similarity to previous output`

Ralph のループ検出は、エージェント出力が最後の 5 つの出力のいずれかと 90% 以上類似したとき
に発動します。

**考えられる原因**:

1. エージェントが同じサブタスクで行き詰まっている
2. エージェントが似た「作業中」メッセージを生成している
3. API エラーが同一のリトライメッセージを引き起こしている
4. タスクが同じアクションを繰り返し必要とする（誤検出）

**解決策**:

1. **正当なループか確認する**:

   ```bash
   # 最近の出力を見直す
   ls -lt .agent/prompts/ | head -10
   diff .agent/prompts/prompt_N.md .agent/prompts/prompt_N-1.md
   ```

2. **多様性を促すようプロンプトを改善する**:

   ```markdown
   # 明示的な進捗追跡を追加する

   ## Current Status

   Document what step you're on and what has changed since last iteration.
   ```

3. **タスクを分解する**:
   - エージェントが同じことを繰り返すなら、タスクの再構成が必要かもしれない
   - より小さく、より明確なサブタスクに分ける

4. **根本的な問題を確認する**:
   - リトライを引き起こす API エラー
   - 進捗をブロックする権限の問題
   - 欠けている依存

#### 完了マーカーが検出されない

**問題**: `TASK_COMPLETE` マーカーがあるのに Ralph が実行を続ける

**考えられる原因**:

1. マーカーの形式が誤っている
2. 不可視文字やエンコーディングの問題
3. マーカーがコードブロックに埋もれている

**解決策**:

1. **正確な形式を使う**:

   ```markdown
   # 正しい形式:

   - [x] TASK_COMPLETE
         [x] TASK_COMPLETE

   # 誤り（発動しない）:

   - [ ] TASK_COMPLETE # チェックされていない
         TASK_COMPLETE # チェックボックスなし
   - [x] TASK_COMPLETE # 大文字の X
   ```

2. **隠れた文字を確認する**:

   ```bash
   cat -A PROMPT.md | grep TASK_COMPLETE
   ```

3. **マーカーが独立した行にあることを確認する**:

   ````markdown
   # 良い - 独立した行

   - [x] TASK_COMPLETE

   # 悪い - コードブロック内

   ```markdown
   - [x] TASK_COMPLETE # コードブロック内 - 機能しない
   ```
   ````

   ```

   ```

4. **エンコーディングを検証する**:

   ```bash
   file PROMPT.md
   # 次を表示するはず: UTF-8 Unicode text
   ```

### Git の問題

#### チェックポイントが失敗する

**問題**: `Failed to create checkpoint`

**解決策**:

1. Git リポジトリを初期化する:

   ```bash
   git init
   git add .
   git commit -m "Initial commit"
   ```

2. Git のステータスを確認する:

   ```bash
   git status
   ```

3. Git の設定を修正する:

   ```bash
   git config user.email "you@example.com"
   git config user.name "Your Name"
   ```

#### 未コミットの変更の警告

**問題**: `Uncommitted changes detected`

**解決策**:

1. 変更をコミットする:

   ```bash
   git add .
   git commit -m "Save work"
   ```

2. 変更を stash する:

   ```bash
   git stash
   ralph run
   git stash pop
   ```

3. Git 操作を無効にする:

   ```bash
   ralph run --no-git
   ```

### コンテキストの問題

#### コンテキストウィンドウの超過

**問題**: `Context window limit exceeded`

**症状**:

- エージェントが前の指示を忘れる
- 不完全な応答
- 情報が欠けているというエラー

**解決策**:

1. ファイルサイズを減らす:

   ```bash
   # 大きなファイルを分割する
   split -l 500 large_file.py part_
   ```

2. より簡潔なプロンプトを使う:

   ```markdown
   # 不要な詳細を削除する

   # 現在のタスクに焦点を当てる
   ```

3. より大きなコンテキストのエージェントに切り替える:

   ```bash
   # Claude は 200K のコンテキストを持つ
   ralph run --agent claude
   ```

4. イテレーション履歴をクリアする:

   ```bash
   rm .agent/prompts/prompt_*.md
   ```

### パフォーマンスの問題

#### 実行が遅い

**問題**: イテレーションに時間がかかりすぎる

**解決策**:

1. システムリソースを確認する:

   ```bash
   top
   df -h
   iostat
   ```

2. 並列操作を減らす:
   - 他のアプリケーションを閉じる
   - バックグラウンドプロセスを制限する

3. より速いエージェントを使う:

   ```bash
   # Q は通常より速い
   ralph run --agent q
   ```

#### メモリ使用が多い

**問題**: Ralph が過剰なメモリを消費する

**解決策**:

1. リソース上限を設定する:

   ```python
   # ralph.json 内
   {
     "resource_limits": {
       "memory_mb": 2048
     }
   }
   ```

2. 古い状態ファイルをクリーンにする:

   ```bash
   find .agent -name "*.json" -mtime +7 -delete
   ```

3. Ralph を再起動する:

   ```bash
   pkill -f ralph_orchestrator
   ralph run
   ```

### 状態とメトリクスの問題

#### 破損した状態ファイル

**問題**: `Invalid state file`

**解決策**:

1. 破損したファイルを削除する:

   ```bash
   rm .agent/metrics/state_latest.json
   ```

2. バックアップから復元する:

   ```bash
   cp .agent/metrics/state_*.json .agent/metrics/state_latest.json
   ```

3. 状態をリセットする:

   ```bash
   ralph clean
   ```

#### メトリクスが欠けている

**問題**: メトリクスが収集されない

**解決策**:

1. メトリクスディレクトリを確認する:

   ```bash
   ls -la .agent/metrics/
   ```

2. 欠けていればディレクトリを作成する:

   ```bash
   mkdir -p .agent/metrics
   ```

3. 権限を確認する:

   ```bash
   chmod 755 .agent/metrics
   ```

## エラーメッセージ

### よくあるエラーコード

| エラー           | 意味                | 解決策               |
| --------------- | ---------------------- | ---------------------- |
| `Exit code 1`   | 一般的な失敗        | 詳細はログを確認する |
| `Exit code 130` | 割り込み（Ctrl+C）   | 通常の割り込み    |
| `Exit code 137` | 強制終了（メモリ不足） | メモリ上限を増やす |
| `Exit code 124` | タイムアウト                | タイムアウト値を増やす |

### エージェント固有のエラー

#### Claude のエラー

```
"Rate limit exceeded"
```

**解決策**: イテレーション間に遅延を加えるか、API プランをアップグレードする

```
"Invalid API key"
```

**解決策**: Claude CLI の設定を確認する

#### Gemini のエラー

```
"Quota exceeded"
```

**解決策**: クォータのリセットを待つか、プランをアップグレードする

```
"Model not available"
```

**解決策**: Gemini CLI のバージョンを確認して更新する

#### Q Chat のエラー

```
"Connection refused"
```

**解決策**: Q サービスが実行されていることを確認する

## デバッグモード

### 詳細ログを有効にする

```bash
# 最大の詳細度
ralph run --verbose

# デバッグ環境で
DEBUG=1 ralph run

# ログを保存する
ralph run --verbose 2>&1 | tee debug.log
```

### 実行を検査する

```python
# PROMPT.md にデバッグポイントを追加する
print("DEBUG: Reached checkpoint 1")
```

### 実行をトレースする

```bash
# システムコールをトレースする
strace -o trace.log ralph run

# Python の実行をプロファイルする
python -m cProfile ralph_orchestrator.py
```

## 回復の手順

### 失敗状態から

1. **現在の状態を保存する**:

   ```bash
   cp -r .agent .agent.backup
   ```

2. **失敗を分析する**:

   ```bash
   tail -n 100 .agent/logs/ralph.log
   ```

3. **問題を修正する**:
   - PROMPT.md を更新する
   - コードのエラーを修正する
   - 問題のあるファイルをクリアする

4. **再開または再起動する**:

   ```bash
   # チェックポイントから再開する
   ralph run

   # または最初から始める
   ralph clean && ralph run
   ```

### Git チェックポイントから

```bash
# チェックポイントを一覧する
git log --oneline | grep checkpoint

# チェックポイントにリセットする
git reset --hard <commit-hash>

# 実行を再開する
ralph run
```

## 助けを得る

### 自己診断

診断スクリプトを実行します。

```bash
cat > diagnose.sh << 'EOF'
#!/bin/bash
echo "Ralph Orchestrator Diagnostic"
echo "============================"
echo "Agents available:"
which claude && echo "  ✓ Claude" || echo "  ✗ Claude"
which gemini && echo "  ✓ Gemini" || echo "  ✗ Gemini"
which q && echo "  ✓ Q" || echo "  ✗ Q"
echo ""
echo "Git status:"
git status --short
echo ""
echo "Ralph status:"
./ralph status
echo ""
echo "Recent errors:"
grep ERROR .agent/logs/*.log 2>/dev/null | tail -5
EOF
chmod +x diagnose.sh
./diagnose.sh
```

### コミュニティサポート

1. **GitHub Issues**: [バグを報告する](https://github.com/mikeyobrien/ralph-orchestrator/issues)
2. **Discussions**: [質問する](https://github.com/mikeyobrien/ralph-orchestrator/discussions)
3. **Discord**: コミュニティチャットに参加する

### バグの報告

バグ報告に含めるもの:

1. Ralph のバージョン: `ralph --version`
2. エージェントのバージョン
3. エラーメッセージ
4. PROMPT.md の内容
5. 診断の出力
6. 再現手順

## 予防のヒント

### ベストプラクティス

1. **単純に始める**: まず基本的なタスクでテストする
2. **定期的なチェックポイント**: 既定の 5 イテレーション間隔を使う
3. **進捗を監視する**: 頻繁に状況を確認する
4. **バージョン管理**: Ralph を実行する前にコミットする
5. **リソース上限**: 適切な上限を設定する
6. **明確な要件**: 具体的でテスト可能な基準を書く

### 事前確認チェックリスト

Ralph を実行する前に:

- [ ] PROMPT.md が明確で具体的である
- [ ] Git リポジトリがクリーンである
- [ ] エージェントがインストールされ機能している
- [ ] 十分なディスク容量がある
- [ ] プロンプトに機微なデータがない
- [ ] 重要なファイルをバックアップした
