# セキュリティ API リファレンス

## 概要

Ralph のセキュリティ関連ユーティリティは、複数のクレートに分散しています。一般的な保護策は
次のとおりです。

- `ralph_adapters::CliExecutor` による**安全な CLI 実行**（シェル呼び出しなし）
- `ralph_telegram::TelegramService::bot_token_masked` による**秘密情報のマスキング**
- `ralph_telegram::escape_html` による**出力のエスケープ**

## 安全な CLI 実行

`CliExecutor` は明示的な引数ベクトルを伴う `tokio::process::Command` を使うため、シェルの
展開を回避し、プロンプト内容によるインジェクションのリスクを減らします。

```rust
use ralph_adapters::{CliBackend, CliExecutor};
use ralph_core::CliConfig;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // バックエンドを明示的に構成する（シェルコマンドは介在しない）。
    let config = CliConfig {
        backend: "codex".to_string(),
        ..Default::default()
    };

    let backend = CliBackend::from_config(&config)?;
    let executor = CliExecutor::new(backend);

    let result = executor.execute_capture("Summarize this task.").await?;
    println!("success={} exit_code={:?}", result.success, result.exit_code);

    Ok(())
}
```

## ログ内の秘密情報のマスキング

Telegram と統合する際、`TelegramService::bot_token_masked` はトークンの先頭/末尾のみを
公開することで、ログを安全に保ちます。

```rust
use ralph_telegram::TelegramService;
use std::path::PathBuf;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let service = TelegramService::new(
        PathBuf::from("."),
        Some("1234567890:abcdefg_hijklmnop".to_string()),
        None, // api_url
        300,
        "loop-1".to_string(),
    )?;

    println!("token={}", service.bot_token_masked());
    Ok(())
}
```

## Telegram 出力用の HTML エスケープ

Telegram の HTML パースモードでは、特殊文字のエスケープが必要です。

```rust
use ralph_telegram::escape_html;

fn main() {
    let raw = "<task> & details";
    let safe = escape_html(raw);
    assert_eq!(safe, "&lt;task&gt; &amp; details");
}
```
