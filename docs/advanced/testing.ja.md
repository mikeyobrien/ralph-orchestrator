# テストと検証

Ralph の開発と検証のための包括的なテストアプローチです。

## テストの種類

| 種類 | 用途 | 速度 | コスト |
|------|---------|-------|------|
| ユニットテスト | 個々の関数をテストする | 速い | 無料 |
| スモークテスト | 記録されたセッションをリプレイする | 速い | 無料 |
| E2E テスト | 実際のバックエンドに対して検証する | 遅い | API コスト |
| TUI 検証 | ターミナルの描画を検証する | 中 | 無料 |

## テストの実行

### すべてのテスト

```bash
cargo test
```

これはユニットテストとスモークテストを含みます（合計 344 以上のテスト）。

### スモークテストのみ

```bash
cargo test -p ralph-core smoke_runner
```

### Kiro 固有のテスト

```bash
cargo test -p ralph-core kiro
```

### E2E テスト

```bash
# すべてのバックエンド
cargo run -p ralph-e2e -- all

# 特定のバックエンド
cargo run -p ralph-e2e -- claude

# シナリオを一覧する
cargo run -p ralph-e2e -- --list
```

## スモークテスト

スモークテストは、ライブの API 呼び出しの代わりに記録された JSONL フィクスチャを使います。
速く、無料で、決定的です。

### 仕組み

1. セッションを JSONL に記録する
2. テスト中にリプレイする
3. 期待される挙動を検証する

### フィクスチャの場所

```
crates/ralph-core/tests/fixtures/
├── basic_session.jsonl          # Claude CLI session
└── kiro/                         # Kiro sessions
    ├── basic.jsonl
    ├── tool_use.jsonl
    └── autonomous.jsonl
```

### 新しいフィクスチャの記録

```bash
# セッションを記録する
ralph run -c ralph.yml --record-session session.jsonl -p "your prompt"

# または生の CLI 出力を捕捉する
claude -p "your prompt" 2>&1 | tee output.txt
```

### フィクスチャの形式

1 行に 1 イベントの JSONL:

```json
{"type":"output","content":"Starting task...","timestamp":"2024-01-21T10:00:00Z"}
{"type":"tool_call","tool":"read_file","args":{"path":"src/lib.rs"}}
{"type":"tool_result","result":"...contents..."}
{"type":"output","content":"LOOP_COMPLETE"}
```

## E2E テスト

エンドツーエンドのテストは、実際の AI バックエンドに対して検証します。

### テストの階層

| 階層 | 焦点 | シナリオ |
|------|-------|-----------|
| 1 | 接続性 | バックエンドの可用性、認証 |
| 2 | オーケストレーション | 単一/複数イテレーション |
| 3 | イベント | 解析、ルーティング |
| 4 | 能力 | ツール使用、ストリーミング |
| 5 | ハットコレクション | ワークフロー、ルーティング |
| 6 | メモリ | 追加、検索、注入 |
| 7 | エラー処理 | タイムアウト、上限 |

### E2E テストの実行

```bash
# Claude のすべてのテスト
cargo run -p ralph-e2e -- claude

# 利用可能なすべてのバックエンド
cargo run -p ralph-e2e -- all

# 高速モード（分析をスキップ）
cargo run -p ralph-e2e -- claude --skip-analysis

# デバッグモード（ワークスペースを保持）
cargo run -p ralph-e2e -- claude --keep-workspace --verbose
```

### E2E レポート

`.e2e-tests/` に生成されます。

```
.e2e-tests/
├── report.md      # Human-readable Markdown
├── report.json    # Machine-readable JSON
└── claude-connect/  # Test workspace (with --keep-workspace)
```

### E2E オーケストレーション

E2E テストの開発には、分離された設定を使います。

```bash
# E2E テストの開発
ralph run -c ralph.e2e.yml -p "fix e2e tests"
```

これは、汚染を避けるため別のスクラッチパッドを使います。

## TUI 検証

LLM-as-judge を使ってターミナル UI の描画を検証します。

### クイックスタート

```bash
# 捕捉した出力から検証する
/tui-validate file:output.txt criteria:ralph-header

# tmux 経由でライブ TUI を検証する
/tui-validate tmux:ralph-session criteria:ralph-full

# カスタム基準
/tui-validate command:"cargo run --example tui" criteria:"Shows header"
```

### 組み込みの基準

| 基準 | 検証するもの |
|----------|-----------|
| `ralph-header` | イテレーション数、経過時間、ハット表示 |
| `ralph-footer` | 活動インジケータ、イベントトピック |
| `ralph-full` | 完全なレイアウトと階層 |
| `tui-basic` | コンテンツがあり、アーティファクトがない |

### ライブ TUI の捕捉

```bash
# 1. tmux で TUI を起動する
tmux new-session -d -s ralph-test -x 100 -y 30
tmux send-keys -t ralph-test "ralph run -p 'test'" Enter

# 2. 描画を待つ
sleep 3

# 3. 捕捉する
tmux capture-pane -t ralph-test -p -e > tui-capture.txt

# 4. 検証する
/tui-validate file:tui-capture.txt criteria:ralph-header
```

### 前提条件

```bash
brew install charmbracelet/tap/freeze  # Screenshot tool
brew install tmux                       # Live capture
```

## Lint

```bash
# 整形を確認する
cargo fmt --check

# clippy を実行する
cargo clippy --all-targets --all-features
```

## pre-commit フック

フックをインストールします。

```bash
./scripts/setup-hooks.sh
```

フックは、各コミットの前に CI と同等の Rust チェックを実行します。

- `./scripts/sync-embedded-files.sh check`
- `cargo fmt --all -- --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test`

## ヒューマンインザループのテスト（Telegram）

実際の Telegram ボットなしで `human.interact` を使うカスタムハットをテストするには、Ralph を
モックの Telegram Bot API サーバーに向けます。

```bash
# モックサーバーを起動する
docker run -d -p 8081:8081 ghcr.io/nickolay/telegram-test-api:latest

# Ralph をそれに向ける
export RALPH_TELEGRAM_API_URL="http://localhost:8081"
export RALPH_TELEGRAM_BOT_TOKEN="test-token"

# ループを実行する
ralph run -p "your prompt" --max-iterations 5
```

または `ralph.yml` に `RObot.telegram.api_url` を設定します。詳細は完全な
[Telegram テストガイド](../guide/telegram.ja.md#testing-with-a-mock-telegram-server)
を参照してください。

## テストのベストプラクティス

### 1. 変更後にテストを実行する

```bash
cargo test  # 完了を宣言する前に必ず実行する
```

### 2. スモークテストを優先する

新機能には、ライブ API に頼るのではなくリプレイフィクスチャを作成します。

### 3. 統合には E2E を使う

E2E テストは高価ですが、統合の問題を捕捉します。

### 4. TUI の変更を検証する

`ralph-tui` を変更した後は、TUI 検証を使います。

### 5. フィクスチャを最新に保つ

挙動が変わったら、対応するフィクスチャを更新します。

## 新しいテストの作成

### ユニットテスト

```rust
#[test]
fn test_event_parsing() {
    let input = r#"ralph emit "build.done" "tests pass""#;
    let event = parse_event(input).unwrap();
    assert_eq!(event.topic, "build.done");
}
```

### スモークテスト

1. セッションを記録する: `--record-session fixture.jsonl`
2. `tests/fixtures/` に置く
3. フィクスチャを参照するテストケースを追加する

### E2E シナリオ

```rust
pub struct MyScenario;

impl E2EScenario for MyScenario {
    fn name(&self) -> &str { "my-scenario" }
    fn tier(&self) -> u8 { 3 }

    async fn run(&self, ctx: &E2EContext) -> E2EResult {
        // Test implementation
    }
}
```

## 次のステップ

- デバッグには [診断](diagnostics.ja.md) を探る
- [アーキテクチャ](architecture.ja.md) について学ぶ
- [コントリビューションガイド](../contributing/index.ja.md) を見る
