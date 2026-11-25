# Telegram 連携

Ralph は、Telegram を通じたヒューマンインザループのコミュニケーションをサポートします。
エージェントはオーケストレーション中に質問でき、人間はいつでも能動的なガイダンスを
送れます。すべて Telegram ボットを通じて行われます。

## セットアップ

### 1. Telegram ボットを作成する

1. Telegram を開き、[@BotFather](https://t.me/BotFather) にメッセージを送る
2. `/newbot` を送り、プロンプトに従う
3. ボットトークンをコピーする（形式: `123456:ABC-DEF1234ghIkl-zyx57W2v1u123ew11`）

### 2. Ralph を設定する

**オプション A: 環境変数（推奨）**

```bash
export RALPH_TELEGRAM_BOT_TOKEN="your-bot-token"
```

**オプション B: 設定ファイル**

```yaml
# ralph.yml
RObot:
  enabled: true
  timeout_seconds: 300
  telegram:
    bot_token: "your-bot-token"
```

環境変数は設定ファイルより優先されます。

### 3. ループを開始する

```bash
ralph run -p "your prompt"
```

ボットは起動時に挨拶メッセージを送ります。チャット ID は、あなたがボットに最初に送った
メッセージから自動検出されます。何かメッセージを送るだけで始められます。

## 設定リファレンス

```yaml
RObot:
  enabled: true                    # ヒューマンインザループを有効にする（既定: false）
  timeout_seconds: 300             # 応答を待ってブロックする長さ
  checkin_interval_seconds: 120    # 定期的なステータス更新（任意）
  telegram:
    bot_token: "your-bot-token"    # または RALPH_TELEGRAM_BOT_TOKEN 環境変数を使う
    api_url: "http://localhost:8081"  # 任意: カスタム Bot API URL（テスト用）
```

| フィールド | 必須 | 説明 |
|-------|----------|-------------|
| `enabled` | はい | Telegram を有効化するには `true` にする |
| `timeout_seconds` | はい | 続行前に人間の返信を待つ秒数 |
| `checkin_interval_seconds` | いいえ | 定期的な「まだ作業中」のステータス更新を送る |
| `telegram.bot_token` | はい* | BotFather からのボットトークン（*または環境変数で設定） |
| `telegram.api_url` | いいえ | カスタム Telegram Bot API URL（または `RALPH_TELEGRAM_API_URL` 環境変数） |

長時間実行するループでは、`timeout_seconds` を増やし `checkin_interval_seconds` を
設定します。

```yaml
RObot:
  enabled: true
  timeout_seconds: 43200            # 12 時間
  checkin_interval_seconds: 900     # 15 分ごとにチェックイン
```

## 仕組み

### エージェントが質問する（`human.interact`）

エージェントがオーケストレーション中に `human.interact` イベントを発行すると:

1. ボットが質問をコンテキスト（ハット名、イテレーション、ループ ID）とともに整形して
   Telegram に送る
2. イベントループが**ブロック**し、返信を待つ
3. あなたが Telegram で返信する
4. あなたの返信が `human.response` イベントとして公開される
5. 次のイテレーションが、そのコンテキストであなたの応答を受け取る

`timeout_seconds` 以内に返信が届かない場合、ループは応答なしで続行します。

### 能動的なガイダンスを送る（`human.guidance`）

いつでもメッセージを送れます（質問への返信としてではなく）。

1. あなたのメッセージが `human.guidance` イベントとして `events.jsonl` に書き込まれる
2. 次のイテレーションで、すべてのガイダンスイベントが収集され、番号付きリストに squash される
3. `## ROBOT GUIDANCE` セクションがエージェントのプロンプトに注入される

これにより、エージェントが尋ねてくるのを待たずに誘導できます。

### イベントの要約

| イベント | 方向 | 挙動 |
|-------|-----------|----------|
| `human.interact` | エージェント → 人間 | エージェントが質問する。返信またはタイムアウトまでループがブロックする |
| `human.response` | 人間 → エージェント | `human.interact` の質問へのあなたの返信 |
| `human.guidance` | 人間 → エージェント | エージェントの次のプロンプトに注入される能動的なメッセージ |

## 並列ループのルーティング

（ワークツリーを通じて）複数のループを並列に実行するとき、メッセージは優先順位で
ルーティングされます。

1. **Reply-to**: ボットの質問に返信すると、それを尋ねたループにルーティングされる
2. **@プレフィックス**: メッセージを `@loop-id` で始めると、その特定のループにルーティング
   される
3. **既定**: ルーティングのないメッセージは主ループに行く

例:

- 質問メッセージに直接返信する → 尋ねたループにルーティング
- `@feature-auth check the edge cases` を送る → `feature-auth` ループにルーティング
- `focus on tests` を送る → 主（メイン）ループにルーティング

各ループは独自の `events.jsonl` を持ちます。
- 主ループ: `.ralph/events.jsonl`
- ワークツリーループ: `.worktrees/<loop-id>/.ralph/events.jsonl`

## マルチメディアのサポート

Telegram 連携は、ファイルと画像の送信をサポートします。

- **ドキュメント**: 任意のファイル種別（ログ、レポートなど）
- **写真**: 任意の HTML 整形されたキャプション付きの画像ファイル

どちらもテキストメッセージと同様に、指数バックオフでのリトライをサポートします。

## ボットの挙動

### ライフサイクル

- **起動**: チャット ID が既知であれば挨拶メッセージを送る
- **実行中**: ロングポーリング（`getUpdates`）で受信メッセージをポーリングする
- **シャットダウン**: 別れのメッセージを送り、ポーリングタスクを停止する

### リアクション

ボットは絵文字であなたのメッセージにリアクションします。
- **質問への返信**: サムズアップでリアクション
- **能動的なガイダンス**: 目のリアクションと、短いテキストの確認

### 主ループのみ

Telegram ボットは、**主ループ**（`.ralph/loop.lock` を保持するもの）でのみ起動します。
ワークツリーループは、主ループのボットを通じてメッセージをルーティングします。

## エラー処理

| シナリオ | 挙動 |
|----------|----------|
| 送信失敗 | 指数バックオフでリトライ: 1s, 2s, 4s（3 回試行） |
| すべてのリトライが失敗 | 診断に記録し、タイムアウトとして扱う（ループは続行） |
| ボットトークンの欠落 | 設定と環境変数の両オプションを挙げる明確なエラー |
| 応答タイムアウト | `timeout_seconds` で設定可能。ループは応答なしで続行 |
| チャット ID なし | 質問は記録されるが送信されない。ボットにメッセージを送ると解決 |

## 状態ファイル

ボットは状態を `.ralph/telegram-state.json` に永続化します。

```json
{
  "chat_id": 123456789,
  "last_seen": "2026-01-29T10:00:00Z",
  "pending_questions": {
    "main": {
      "asked_at": "2026-01-29T10:05:00Z",
      "message_id": 42
    }
  }
}
```

- `chat_id`: ボットへのあなたの最初のメッセージから自動検出される
- `pending_questions`: どのループに未回答の質問があるかを追跡し、返信のルーティングに使う

## アーキテクチャ

```
TelegramService (ライフサイクル管理)
├── BotApi / TelegramBot (teloxide のラッパー、メッセージ/ドキュメント/写真を送る)
├── StateManager (チャット ID、保留中の質問、返信ルーティング)
├── MessageHandler (受信メッセージ → events.jsonl)
└── retry_with_backoff (すべての送信の指数リトライ)
```

クレートは `crates/ralph-telegram/` にあり、次のモジュールを持ちます。

| モジュール | 用途 |
|--------|---------|
| `lib.rs` | 公開 API のエクスポート |
| `bot.rs` | `BotApi` トレイト + `TelegramBot` 実装、メッセージ整形 |
| `service.rs` | `TelegramService` のライフサイクル、送受信、ポーリング |
| `handler.rs` | 受信メッセージをイベントにルーティングする `MessageHandler` |
| `state.rs` | `StateManager` + `TelegramState` の永続化 |
| `error.rs` | 型付きエラーバリアントを持つ `TelegramError` enum |

## テスト

```bash
cargo test -p ralph-telegram          # 33 個のユニットテスト（モック、ネットワークなし）
cargo test -p ralph-core human        # ralph-core の 11 個の統合テスト
```

すべてのテストは `BotApi` の `MockBot` 実装を使います。テスト中に Telegram API 呼び出しは
行われません。

## モック Telegram サーバーでのテスト

`human.interact` を使うカスタムハットを開発するとき、Ralph をモックの Telegram Bot API
サーバーに向けることで、実際の Telegram ボットなしでヒューマンインザループの全フローを
ローカルでテストできます。

### 1. モックサーバーを起動する

[telegram-test-api](https://github.com/nickolay/telegram-test-api) は、Telegram Bot API を
実装した Docker ベースのモックです。

```bash
docker run -d --name telegram-mock -p 8081:8081 \
  ghcr.io/nickolay/telegram-test-api:latest
```

### 2. Ralph をそれに向ける

**オプション A: 環境変数**

```bash
export RALPH_TELEGRAM_API_URL="http://localhost:8081"
export RALPH_TELEGRAM_BOT_TOKEN="test-token"
```

**オプション B: 設定ファイル**

```yaml
# ralph.yml
RObot:
  enabled: true
  timeout_seconds: 30
  telegram:
    bot_token: "test-token"
    api_url: "http://localhost:8081"
```

環境変数は設定ファイルの値より優先されます。

### 3. ループを実行する

```bash
ralph run -p "your prompt" --max-iterations 5
```

ボットは、すべての API リクエストを `https://api.telegram.org` ではなくモックサーバーに
送ります。リクエストを検査し、返信をシミュレートし、ハットが正しい `human.interact`
イベントを発行するかを検証できます。すべて実際の Telegram に触れずに行えます。

### ユースケース

- **カスタムハットの開発**: ハットが適切なときに適切な質問をするか検証する
- **CI/CD パイプライン**: ネットワークアクセスやボットトークンなしで HIL 統合テストを実行する
- **デバッグ**: Ralph が Telegram API に送る正確なペイロードを検査する

## トラブルシューティング

### ボットが応答しない

- ボットトークンを検証する: `curl https://api.telegram.org/bot<TOKEN>/getMe`
- ボットに少なくとも 1 通のメッセージを送ったか確認する（チャット ID の自動検出のため）
- 設定に `RObot.enabled: true` が設定されているか確認する

### メッセージが誤ったループに行く

- 質問を尋ねたループにルーティングするには reply-to を使う
- 特定のループを対象にするには `@loop-id` プレフィックスを使う
- ルーティングされないメッセージは既定で主ループに行く

### 応答できる前にタイムアウトする

- 設定の `timeout_seconds` を増やす
- 長いタスクでは、ループがまだアクティブだと分かるよう `checkin_interval_seconds` を設定する

### 「No chat ID configured」の警告

- ボットは、あなたが送った最初のメッセージからチャット ID を自動検出する
- ボットに何かメッセージを送って接続を確立する
- チャット ID は `.ralph/telegram-state.json` に永続化される
