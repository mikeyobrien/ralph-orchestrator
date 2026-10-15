# 診断

診断システムは、デバッグと分析のために Ralph の動作への完全な可視性を捕捉します。

## 診断を有効にする

環境変数でオプトインします。

```bash
RALPH_DIAGNOSTICS=1 ralph run -p "your prompt"
```

**無効時はオーバーヘッドゼロ** — 診断コードは完全に迂回されます。

## 出力の場所

診断は、タイムスタンプ付きのセッションディレクトリに書き込まれます。

```
.ralph/diagnostics/
└── 2024-01-21T08-45-30/           # ISO 8601 timestamp
    ├── agent-output.jsonl          # Agent text, tool calls, results
    ├── orchestration.jsonl         # Hat selection, events, backpressure
    ├── trace.jsonl                 # All tracing logs with metadata
    ├── performance.jsonl           # Timing, latency, token counts
    └── errors.jsonl                # Parse errors, validation failures
```

## ファイルの内容

### agent-output.jsonl

AI エージェントが出力するすべて:

```json
{"timestamp":"2024-01-21T08:45:35Z","type":"text","content":"Let me analyze..."}
{"timestamp":"2024-01-21T08:45:40Z","type":"tool_call","tool":"read_file","args":{"path":"src/lib.rs"}}
{"timestamp":"2024-01-21T08:45:42Z","type":"tool_result","tool":"read_file","result":"..."}
```

### orchestration.jsonl

ハット選択とイベントフロー:

```json
{"timestamp":"2024-01-21T08:45:30Z","event":{"type":"hat_selected","hat":"builder"}}
{"timestamp":"2024-01-21T08:46:00Z","event":{"type":"event_published","topic":"build.done"}}
{"timestamp":"2024-01-21T08:46:01Z","event":{"type":"event_routed","topic":"build.done","target":"reviewer"}}
```

### trace.jsonl

メタデータ付きのすべてのトレースログ:

```json
{"timestamp":"2024-01-21T08:45:30Z","level":"INFO","target":"ralph_core","message":"Starting iteration 1"}
{"timestamp":"2024-01-21T08:45:31Z","level":"DEBUG","target":"ralph_adapters","message":"Spawning claude process"}
{"timestamp":"2024-01-21T08:46:00Z","level":"WARN","target":"ralph_core","message":"Approaching context limit"}
```

### performance.jsonl

タイミングとリソース使用:

```json
{"timestamp":"2024-01-21T08:45:30Z","iteration":1,"duration_ms":30000,"tokens_in":1500,"tokens_out":2000}
{"timestamp":"2024-01-21T08:46:30Z","iteration":2,"duration_ms":25000,"tokens_in":1800,"tokens_out":1500}
```

### errors.jsonl

エラーと失敗:

```json
{"timestamp":"2024-01-21T08:45:50Z","type":"parse_error","message":"Failed to parse event","raw":"invalid json"}
{"timestamp":"2024-01-21T08:46:10Z","type":"validation_error","message":"Hat 'unknown' not found"}
```

## 診断のレビュー

### jq を使う

```bash
# すべてのエージェントテキスト出力
jq 'select(.type == "text")' .ralph/diagnostics/*/agent-output.jsonl

# すべてのツール呼び出し
jq 'select(.type == "tool_call")' .ralph/diagnostics/*/agent-output.jsonl

# ハット選択の決定
jq 'select(.event.type == "hat_selected")' .ralph/diagnostics/*/orchestration.jsonl

# すべてのイベント
jq '.event' .ralph/diagnostics/*/orchestration.jsonl

# すべてのエラー
jq '.' .ralph/diagnostics/*/errors.jsonl

# ERROR レベルのトレース
jq 'select(.level == "ERROR")' .ralph/diagnostics/*/trace.jsonl

# イテレーションごとのパフォーマンス
jq '{iteration, duration_ms, tokens_in, tokens_out}' .ralph/diagnostics/*/performance.jsonl
```

### よくあるクエリ

**なぜこのハットが選ばれたのか？**

```bash
jq 'select(.event.type == "hat_selected" and .event.hat == "builder")' \
  .ralph/diagnostics/*/orchestration.jsonl
```

**どのイベントが公開されたか？**

```bash
jq 'select(.event.type == "event_published") | .event.topic' \
  .ralph/diagnostics/*/orchestration.jsonl
```

**各イテレーションにどれくらいかかったか？**

```bash
jq '{iteration, duration_ms}' .ralph/diagnostics/*/performance.jsonl
```

**パースエラーはあったか？**

```bash
jq 'select(.type == "parse_error")' .ralph/diagnostics/*/errors.jsonl
```

## クリーンアップ

診断ファイルを削除します。

```bash
ralph clean --diagnostics
```

または手動で:

```bash
rm -rf .ralph/diagnostics/
```

## いつ使うか

次のときに診断を有効にします。

- 特定のハットがなぜ選ばれたのかをデバッグする
- エージェント出力のフローを理解する
- バックプレッシャーの発動を調査する
- パフォーマンスのボトルネックを分析する
- 失敗した実行の事後分析
- カスタムハットの開発

## ベストプラクティス

1. **デバッグには有効に、本番では無効に** — 診断は I/O オーバーヘッドを加える
2. **古いセッションをクリーンアップする** — 大きくなり得る
3. **分析には jq を使う** — JSONL はストリーミングクエリ向けに設計されている
4. **問題のあるセッションを保存する** — 後の分析のためにクリーンする前にコピーする

## TUI との統合

TUI は要約情報を表示します。詳細は診断を確認します。

| TUI が表示するもの | 診断が提供するもの |
|-----------|---------------------|
| 現在のハット | 完全な選択履歴 |
| 最近の出力 | 完全な出力ログ |
| イテレーション数 | イテレーションごとのタイミング |
| イベントトピック | 完全なイベントペイロード |

## デバッグセッションの例

```bash
# 1. 診断付きで実行する
RALPH_DIAGNOSTICS=1 ralph run -p "implement feature X"

# 2. セッションを見つける
ls -la .ralph/diagnostics/
# 2024-01-21T08-45-30/

# 3. まずエラーを確認する
jq '.' .ralph/diagnostics/2024-01-21T08-45-30/errors.jsonl

# 4. ハット選択をレビューする
jq '.event' .ralph/diagnostics/2024-01-21T08-45-30/orchestration.jsonl

# 5. エージェントが何をしたか確認する
jq 'select(.type == "tool_call")' .ralph/diagnostics/2024-01-21T08-45-30/agent-output.jsonl

# 6. パフォーマンスをレビューする
jq '{iteration, duration_ms}' .ralph/diagnostics/2024-01-21T08-45-30/performance.jsonl
```

## 次のステップ

- [テストと検証](testing.ja.md) について学ぶ
- [カスタムハットの作成](custom-hats.ja.md) を探る
- [イベントシステム](event-system.ja.md) を理解する
