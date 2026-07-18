# ユーザーガイド

Ralph Orchestrator を効果的に使うための実践的なガイドです。

## このセクションの内容

| ガイド | 説明 |
|-------|-------------|
| [設定](configuration.ja.md) | 完全なコア設定リファレンス |
| [プリセット](presets.ja.md) | 組み込みのハットコレクション |
| [CLI リファレンス](cli-reference.ja.md) | コマンドラインインターフェース |
| [バックエンド](backends.ja.md) | サポートされる AI バックエンド |
| [プロンプトの書き方](prompts.ja.md) | プロンプトエンジニアリングのコツ |
| [コスト管理](cost-management.ja.md) | API 費用の制御 |
| [Telegram 連携](telegram.ja.md) | Telegram によるヒューマンインザループ |

## クイックリンク

### はじめに

- コア設定を初期化する: `ralph init --backend claude`
- 組み込みのハットコレクションを一覧する: `ralph init --list-presets`
- ハットで実行する: `ralph run -c ralph.yml -H builtin:code-assist`

### Ralph の実行

- 基本的な実行（コアのみ）: `ralph run -c ralph.yml`
- ハット付き: `ralph run -c ralph.yml -H builtin:debug`
- インラインプロンプト付き: `ralph run -c ralph.yml -H builtin:code-assist -p "Implement feature X"`
- ヘッドレスモード: `ralph run --no-tui`
- セッションを再開する: `ralph run --continue`

### 監視

- イベント履歴を表示する: `ralph events`
- メモリを確認する: `ralph tools memory list`
- タスクを確認する: `ralph tools task list`

## ワークフローの選び方

| あなたの状況 | 推奨アプローチ |
|----------------|---------------------|
| 単純なタスク | コアのみ（ハットなし） |
| 実装作業 | `-H builtin:code-assist` |
| バグ調査 | `-H builtin:debug` |
| コードレビュー | `-H builtin:review` |
| 探索とアーキテクチャの把握 | `-H builtin:research` |

## よくあるタスク

### 新しい機能を始める

```bash
ralph init --backend claude
ralph run -c ralph.yml -H builtin:code-assist -p "Add OAuth login"
```

### 問題をデバッグする

```bash
ralph run -c ralph.yml -H builtin:debug -p "Investigate why user authentication fails on mobile"
```

### コードをレビューする

```bash
ralph run -c ralph.yml -H builtin:review -p "Review the changes in src/api/"
```

## 次のステップ

まず [設定](configuration.ja.md) から始めて、すべてのオプションを理解してください。
