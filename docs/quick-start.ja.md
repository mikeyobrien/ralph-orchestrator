# クイックスタートガイド

Ralph Orchestrator を 5 分で使い始めましょう！

## 前提条件

始める前に、次を用意していることを確認してください。

- Python 3.8 以上
- Git（チェックポイント機能のため）
- 少なくとも 1 つの AI CLI ツールをインストール済み

## ステップ 1: AI エージェントをインストールする

Ralph は複数の AI エージェントで動作します。少なくとも 1 つをインストールします。

=== "Claude（推奨）"

    ```bash
    npm install -g @anthropic-ai/claude-code
    # またはセットアップ手順は https://claude.ai/code を参照
    ```

=== "Q Chat"

    ```bash
    pip install q-cli
    # または https://github.com/qchat/qchat の手順に従う
    ```

=== "Gemini"

    ```bash
    npm install -g @google/gemini-cli
    # API キーで設定する
    ```

=== "ACP エージェント"

    ```bash
    # 任意の ACP 準拠エージェントを使える
    # 例: ACP モードの Gemini CLI
    npm install -g @google/gemini-cli
    # 実行: ralph run -a acp --acp-agent gemini
    ```

## ステップ 2: Ralph Orchestrator をクローンする

```bash
# リポジトリをクローンする
git clone https://github.com/mikeyobrien/ralph-orchestrator.git
cd ralph-orchestrator

# 監視用の任意の依存をインストールする
pip install psutil  # システムメトリクスに推奨
```

## ステップ 3: 最初のタスクを作成する

タスクを記した `PROMPT.md` ファイルを作成します。

```markdown
# Task: Create a Todo List CLI

Build a Python command-line todo list application with:

- Add tasks
- List tasks
- Mark tasks as complete
- Save tasks to a JSON file

Include proper error handling and a help command.

The orchestrator will continue iterations until all requirements are met or limits reached.
```

## ステップ 4: Ralph を実行する

```bash
# 基本的な実行（利用可能なエージェントを自動検出する）
python ralph_orchestrator.py --prompt PROMPT.md

# またはエージェントを明示的に指定する
python ralph_orchestrator.py --agent claude --prompt PROMPT.md

# または ACP 準拠のエージェントを使う
python ralph_orchestrator.py --agent acp --acp-agent gemini --prompt PROMPT.md
```

## ステップ 5: 進捗を監視する

Ralph は次を行います。

1. プロンプトファイルを読む
2. AI エージェントを実行する
3. 完了を確認する
4. 完了するか上限に達するまで反復する

次のような出力が表示されます。

```
2025-09-08 10:30:45 - INFO - Starting Ralph Orchestrator v1.0.0
2025-09-08 10:30:45 - INFO - Using agent: claude
2025-09-08 10:30:45 - INFO - Starting iteration 1/100
2025-09-08 10:30:52 - INFO - Iteration 1 complete
2025-09-08 10:30:52 - INFO - Task not complete, continuing...
```

## 次に何が起こるのか？

Ralph は、次のいずれかの条件が満たされるまで反復を続けます。

- 🎯 すべての要件が満たされたように見える
- ⏱️ 最大イテレーションに到達（既定: 100）
- ⏰ 最大実行時間を超過（既定: 4 時間）
- 💰 トークンまたはコストの上限に到達
- ❌ 回復不能なエラーが発生
- ✅ プロンプトファイルで完了マーカーを検出
- 🔄 ループ検出が発動（反復的な出力）

## 完了を知らせる

タスクが完了したら、PROMPT.md に完了マーカーを追加します。

```markdown
## Status

- [x] Created todo.py with CLI interface
- [x] Implemented add, list, complete commands
- [x] Added JSON persistence
- [x] Wrote unit tests
- [x] TASK_COMPLETE
```

Ralph は `- [x] TASK_COMPLETE` マーカーを検出し、直ちにオーケストレーションを停止します。
これにより、AI エージェントはイテレーション上限だけに頼るのではなく、「完了した」と知らせる
ことができます。

## 基本的な設定

コマンドラインオプションで Ralph の挙動を制御します。

```bash
# イテレーションを制限する
python ralph_orchestrator.py --prompt PROMPT.md --max-iterations 50

# コスト上限を設定する
python ralph_orchestrator.py --prompt PROMPT.md --max-cost 10.0

# 詳細ログを有効にする
python ralph_orchestrator.py --prompt PROMPT.md --verbose

# ドライラン（実行せずにテストする）
python ralph_orchestrator.py --prompt PROMPT.md --dry-run
```

## タスクの例

### シンプルな関数

```markdown
Write a Python function that validates email addresses using regex.
Include comprehensive unit tests.
```

### Web スクレイパー

```markdown
Create a web scraper that:

1. Fetches the HackerNews homepage
2. Extracts the top 10 stories
3. Saves them to a JSON file
   Use requests and BeautifulSoup.
```

### CLI ツール

```markdown
Build a markdown to HTML converter CLI tool:

- Accept input/output file arguments
- Support basic markdown syntax
- Add --watch mode for auto-conversion
```

## 次のステップ

最初の Ralph タスクを実行できたので、次に進みましょう。

- 📖 詳しい設定は [ユーザーガイド](guide/overview.ja.md) を読む
- 🔒 [セキュリティ機能](advanced/security.ja.md) について学ぶ
- 💰 [コスト管理](guide/cost-management.ja.md) を理解する
- 📊 [監視](advanced/monitoring.ja.md) をセットアップする
- 🚀 [本番](advanced/production-deployment.ja.md) にデプロイする

## トラブルシューティング

### エージェントが見つからない

Ralph が AI エージェントを見つけられない場合:

```bash
ERROR: No AI agents detected. Please install claude, q, gemini, or an ACP-compliant agent.
```

**解決策**: サポートされるエージェントのいずれかをインストールする（ステップ 1 を参照）

### 権限が拒否される

権限エラーが出る場合:

```bash
chmod +x ralph_orchestrator.py
```

### タスクが完了しない

タスクが無限に実行される場合:

- プロンプトに明確な完了基準が含まれているか確認する
- エージェントがファイルを変更し完了に向けて作業できることを確認する
- `.agent/metrics/` のイテレーションログを見直す

## 助けを得る

- [FAQ](reference/faq.ja.md) を確認する
- [トラブルシューティングガイド](reference/troubleshooting.ja.md) を読む
- [GitHub で issue を開く](https://github.com/mikeyobrien/ralph-orchestrator/issues)
- [ディスカッション](https://github.com/mikeyobrien/ralph-orchestrator/discussions) に参加する

---

🎉 **おめでとうございます！** 最初の Ralph オーケストレーションを実行できました！
