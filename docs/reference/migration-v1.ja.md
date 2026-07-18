# v1 からの移行

Python ベースの Ralph v1 から Rust ベースの v2 への移行ガイドです。

## 概要

Ralph v2 は、大きな変更を伴う Rust での完全な書き直しです。

| 観点 | v1（Python） | v2（Rust） |
|--------|-------------|-----------|
| 言語 | Python | Rust |
| インストール | pip/pipx | npm/cargo |
| 設定形式 | Python の dict | YAML |
| ハットシステム | なし | 中核機能 |
| イベントシステム | なし | 中核機能 |
| メモリ | なし | 組み込み |
| タスク | なし | 組み込み |
| TUI | 基本的 | 完全な ratatui |

## v1 のアンインストール

まず古い Python 版を削除します。

```bash
# pip でインストールした場合
pip uninstall ralph-orchestrator

# pipx でインストールした場合
pipx uninstall ralph-orchestrator

# uv でインストールした場合
uv tool uninstall ralph-orchestrator

# 削除を確認する
which ralph  # 何も返らないはず
```

## v2 のインストール

```bash
# npm 経由（推奨）
npm install -g @ralph-orchestrator/ralph-cli

# GitHub Releases インストーラ経由
curl --proto '=https' --tlsv1.2 -LsSf \
  https://github.com/mikeyobrien/ralph-orchestrator/releases/latest/download/ralph-cli-installer.sh | sh

# Cargo 経由
cargo install ralph-cli
```

## 設定の変更

### v1 の設定（Python）

```python
# ralph_config.py
config = {
    "max_iterations": 100,
    "agent": "claude",
    "cost_limit": 10.0,
    "checkpoint_interval": 10,
}
```

### v2 の設定（YAML）

```yaml
# ralph.yml
cli:
  backend: "claude"

event_loop:
  completion_promise: "LOOP_COMPLETE"
  max_iterations: 100
  checkpoint_interval: 10
```

## コマンドの変更

| v1 コマンド | v2 コマンド |
|------------|------------|
| `python ralph_orchestrator.py --prompt PROMPT.md` | `ralph run` |
| `python ralph_orchestrator.py --agent claude` | `ralph run --backend claude` |
| `python ralph_orchestrator.py --max-iterations 50` | `ralph run --max-iterations 50` |
| `python ralph_orchestrator.py --dry-run` | `ralph run --dry-run` |

## v2 の新機能

### ハットシステム

v1 には存在しなかった専門ペルソナ:

```yaml
hats:
  planner:
    triggers: ["task.start"]
    publishes: ["plan.ready"]
    instructions: "Create a plan..."
```

### イベント

ハット間の型付き通信:

```bash
ralph emit "build.done" "tests: pass, lint: pass, typecheck: pass, audit: pass, coverage: pass"
ralph events  # 履歴を表示する
```

### メモリ

永続的な学習:

```bash
ralph tools memory add "Pattern discovered" -t pattern
ralph tools memory search "pattern"
```

### タスク

ランタイムの追跡:

```bash
ralph tools task add "Implement feature"
ralph tools task list
ralph tools task close task-123
```

### プリセット

事前設定済みのワークフロー:

```bash
ralph init --preset tdd-red-green
```

### TUI

リッチなターミナルインターフェース（既定で有効）:

```bash
ralph run  # TUI モード
ralph run --no-tui  # ヘッドレスモード
```

## 削除された機能

一部の v1 機能は、v2 では異なる方法で扱われます。

| v1 機能 | v2 の相当機能 |
|------------|---------------|
| コスト追跡 | 組み込みではない（バックエンドの追跡を使う） |
| ループ検出 | 簡素化（最大イテレーション） |
| ACP プロトコル | サポートされない（直接の CLI のみ） |
| メトリクスのエクスポート | 診断システム |

## PROMPT.md の互換性

プロンプトファイルの形式はおおむね互換性があります。

```markdown
# Task: My Task

Description here.

## Requirements
- Requirement 1
- Requirement 2
```

**変更点:**

- `- [x] TASK_COMPLETE` マーカーはもう使わない
- 代わりに出力で `LOOP_COMPLETE` を使う
- 受け入れ基準は引き続き同じように機能する

## 状態ディレクトリ

| v1 の場所 | v2 の場所 |
|-------------|-------------|
| `.agent/metrics/` | （削除） |
| `.agent/checkpoints/` | Git ベース |
| `.agent/prompts/` | （削除） |
| `.agent/plans/` | （削除） |
| （なし） | `.agent/memories.md` |
| （なし） | `.agent/tasks.jsonl` |
| （なし） | `.agent/event_history.jsonl` |

## 移行の手順

### 1. v1 をアンインストールする

```bash
pip uninstall ralph-orchestrator
```

### 2. v2 をインストールする

```bash
npm install -g @ralph-orchestrator/ralph-cli
```

### 3. 設定を変換する

古い設定から `ralph.yml` を作成します。

```yaml
cli:
  backend: "claude"  # 以前は "agent"

event_loop:
  completion_promise: "LOOP_COMPLETE"
  max_iterations: 100  # 以前と同じ
```

### 4. プロンプトを更新する

完了マーカーを変更します。

```markdown
# 変更前（v1）
- [x] TASK_COMPLETE

# 変更後（v2）
Output: LOOP_COMPLETE
```

### 5. 古い状態をクリーンにする

```bash
rm -rf .agent/metrics .agent/checkpoints .agent/prompts .agent/plans
```

### 6. テストする

```bash
ralph run --dry-run
ralph run
```

## 助けを得る

移行の問題に遭遇した場合:

- [トラブルシューティング](troubleshooting.ja.md) を確認する
- [issue を開く](https://github.com/mikeyobrien/ralph-orchestrator/issues)
- [v1.2.3](https://github.com/mikeyobrien/ralph-orchestrator/tree/v1.2.3) の v1 コードを参照する
