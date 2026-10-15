# タスクシステム

!!! note "ドキュメント作成中"
    このページは作成中です。包括的なタスクシステムのドキュメントは、近日中にご確認ください。

## 概要

Ralph のタスクシステムは、`.agent/tasks.jsonl` を通じてランタイムの作業追跡を提供し、
レガシーのスクラッチパッドの仕組みを置き換えます。

## タスクのライフサイクル

1. **作成（Created）** - タスクがキューに追加される
2. **進行中（In Progress）** - エージェントが活発に作業中
3. **完了（Completed）** - タスクが正常に終了した
4. **ブロック（Blocked）** - 依存または入力を待っている

## 設定

```yaml
tasks:
  enabled: true  # Default
  path: .agent/tasks.jsonl
```

## CLI コマンド

```bash
ralph task list              # 現在のタスクを表示する
ralph task add "description" # 新しいタスクを追加する
ralph task complete <id>     # タスクを完了としてマークする
```

## 関連項目

- [メモリとタスク](../concepts/memories-and-tasks.ja.md) - 中核概念
- [メモリシステム](memory-system.ja.md) - 永続的な学習
- [CLI リファレンス](../guide/cli-reference.ja.md) - 完全な CLI ドキュメント
