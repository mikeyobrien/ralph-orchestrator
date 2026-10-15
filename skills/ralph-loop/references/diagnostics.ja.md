# Ralph ループの診断

## 現在の診断ファイル

診断を有効にするには次のようにします。

```bash
RALPH_DIAGNOSTICS=1 ralph run -c ralph.yml -H .ralph/hats/my-workflow.yml -p "..."
```

セッションディレクトリは `.ralph/diagnostics/<timestamp>/` 配下に作られます。

主なファイル:

- `agent-output.jsonl`: エージェントのテキストとツール呼び出し
- `orchestration.jsonl`: ハット選択、イベント、バックプレッシャー
- `performance.jsonl`: 処理時間とトークンのメトリクス
- `errors.jsonl`: パースエラーと検証失敗
- `trace.jsonl`: より低レベルのトレース
- `prompt-log.md`: 各イテレーションでエージェントに送られた完全なプロンプト

便利なコマンド:

```bash
SESSION=".ralph/diagnostics/$(ls -t .ralph/diagnostics | head -1)"
jq 'select(.event.type == "hat_selected")' "$SESSION/orchestration.jsonl"
jq 'select(.type == "tool_call")' "$SESSION/agent-output.jsonl"
jq '.' "$SESSION/errors.jsonl"
jq '{iteration, duration_ms}' "$SESSION/performance.jsonl"

# 特定のイテレーションの完全なプロンプトを表示する
grep -A 1000 "^# Iteration 3" "$SESSION/prompt-log.md" | sed '/^---$/q'
```

## 一時停止と再開の成果物

フック駆動の一時停止では、次のオペレーター向けファイルを使います。

- `.ralph/suspend-state.json`
- `.ralph/resume-requested`

ループ動作中に現れることがある関連の制御信号ファイル:

- `.ralph/stop-requested`
- `.ralph/restart-requested`

通常のオペレーターフロー:

1. `.ralph/suspend-state.json` を確認する
2. `ralph loops resume <id>` を実行する
3. Ralph に `.ralph/resume-requested` を消費させる

CLI の経路が使えず、かつ復旧の仕組みをすでに確認済みでない限り、これらのファイルを手作業で
書き込むのは避けてください。

## 確認する価値のある状態ファイル

- `.ralph/loop.lock`: 主ループの pid とプロンプト
- `.ralph/loops.json`: 追跡中のループのメタデータ
- `.ralph/merge-queue.jsonl`: キュー待ち/マージ中/レビューのイベント

ユーザーが簡潔なオペレーター向けの要約を求めている場合は、ファイルを手作業で解析するより
`ralph loops list --json` を優先してください。
