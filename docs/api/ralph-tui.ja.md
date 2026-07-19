# ralph-tui

Ralph をリアルタイムに監視するためのターミナル UI です。

## 概要

`ralph-tui` は次を提供します。

- リアルタイムのイテレーション表示
- ハットとイベントの状態
- エージェント出力のストリーミング
- インタラクティブな操作

[ratatui](https://ratatui.rs/) で構築されています。

## 機能

### ヘッダー表示

現在のオーケストレーション状態を示します。

- イテレーション数: `[iter 3]`
- 経過時間: `00:02:15`
- アクティブなハットの絵文字と名前: `🔨 Builder`
- モードインジケータ

### コンテンツエリア

エージェント出力を次とともに表示します。

- リアルタイムストリーミング
- シンタックスハイライト
- スクロール対応

### フッター

活動状態を示します。

- 活動インジケータ: `◉`（活動中）、`◯`（アイドル）、`■`（停止）
- 現在のイベントトピック
- 検索表示（有効な場合）

## 使い方

TUI は `ralph run` で既定で有効になります。

```bash
# TUI モード（既定）
ralph run

# TUI を無効にする
ralph run --no-tui
```

## キーバインド

| キー | 動作 |
|-----|--------|
| `q` | 終了 |
| `↑`/`↓` | 出力をスクロール |
| `PgUp`/`PgDn` | ページスクロール |
| `Home`/`End` | 先頭/末尾へジャンプ |
| `/` | 検索 |
| `n` | 次の検索結果 |
| `N` | 前の検索結果 |

## プログラムからの利用

### TUI アプリケーション

```rust
use ralph_tui::TuiApp;

let app = TuiApp::new();
app.run().await?;
```

### TUI ストリームハンドラ

アダプタとの統合向けです。

```rust
use ralph_tui::TuiStreamHandler;
use tokio::sync::mpsc;

let (tx, rx) = mpsc::channel(100);
let handler = TuiStreamHandler::new(tx);

// ハンドラは UxEvent を TUI に送る
```

### UX イベント

オーケストレーターから TUI へのイベントです。

```rust
use ralph_proto::UxEvent;

enum UxEvent {
    TerminalWrite(String),
    Resize { width: u16, height: u16 },
    FrameCapture(Vec<u8>),
    IterationStart(usize),
    HatSelected(String),
    EventPublished(String),
}
```

## ウィジェット

### ヘッダーウィジェット

```rust
use ralph_tui::widgets::Header;

let header = Header::new()
    .iteration(3)
    .elapsed(Duration::from_secs(135))
    .hat("🔨 Builder")
    .mode("hat-based");
```

### フッターウィジェット

```rust
use ralph_tui::widgets::Footer;

let footer = Footer::new()
    .activity(Activity::Active)
    .event_topic("build.done")
    .search_query(None);
```

### コンテンツウィジェット

```rust
use ralph_tui::widgets::Content;

let content = Content::new()
    .text(&output)
    .scroll(scroll_position);
```

## カスタマイズ

### 色

```rust
use ralph_tui::theme::Theme;

let theme = Theme {
    header_bg: Color::Rgb(30, 30, 46),
    header_fg: Color::Rgb(205, 214, 244),
    active_indicator: Color::Green,
    // ...
};
```

### レイアウト

```rust
use ralph_tui::Layout;

let layout = Layout {
    header_height: 3,
    footer_height: 2,
    // コンテンツが残りの領域を埋める
};
```

## エラー処理

```rust
pub enum TuiError {
    IoError(std::io::Error),
    RenderError(String),
    EventError(String),
}
```

## 例: カスタム TUI 統合

```rust
use ralph_tui::{TuiApp, TuiStreamHandler};
use ralph_adapters::PtyExecutor;
use tokio::sync::mpsc;

#[tokio::main]
async fn main() -> Result<()> {
    // TUI 更新用のチャンネルを作成する
    let (tx, rx) = mpsc::channel(100);

    // アダプタ用の TUI ハンドラを作成する
    let handler = TuiStreamHandler::new(tx);

    // 別タスクで TUI を spawn する
    let tui_handle = tokio::spawn(async move {
        let app = TuiApp::with_receiver(rx);
        app.run().await
    });

    // TUI ハンドラを使ってバックエンドを実行する
    let executor = PtyExecutor::new();
    executor.execute(&backend, &prompt, Box::new(handler)).await?;

    // TUI の終了を待つ
    tui_handle.await??;

    Ok(())
}
```

## TUI 検証

TUI 描画をテストするには、`/tui-validate` スキルを使います。

```bash
/tui-validate file:output.txt criteria:ralph-header
```

詳細は[テストと検証](../advanced/testing.ja.md)を参照してください。
