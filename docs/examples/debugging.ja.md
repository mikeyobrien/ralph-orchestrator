# デバッグの例

!!! note "ドキュメント作成中"
    このページは作成中です。完全なデバッグワークフローの例は、近日中にご確認ください。

## 概要

この例は、調査、仮説検証、修正の検証のための専門ハットを伴って、Ralph を使って問題を
デバッグする方法を示します。

## 診断を有効にする

```bash
RALPH_DIAGNOSTICS=1 ralph run -p "fix the authentication bug"
```

## ログの確認

```bash
# すべてのエージェント出力を表示する
jq 'select(.type == "text")' .ralph/diagnostics/*/agent-output.jsonl

# ハット選択の決定を表示する
jq 'select(.event.type == "hat_selected")' .ralph/diagnostics/*/orchestration.jsonl

# エラーを表示する
jq '.' .ralph/diagnostics/*/errors.jsonl
```

## 関連項目

- [診断](../advanced/diagnostics.ja.md) - 完全な診断リファレンス
- [トラブルシューティング](../reference/troubleshooting.ja.md) - よくある問題
- [シンプルなタスク](simple-task.ja.md) - 基本的な例
