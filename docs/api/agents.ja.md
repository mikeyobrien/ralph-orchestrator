# エージェント API リファレンス

## 概要

Ralph はエージェントを CLI バックエンドとして扱います。バックエンドの選択は `ralph_core::HatBackend`
と `ralph_core::CliConfig` にあり、実行は `ralph-adapters` が処理します。

主要な型:
- `ralph_adapters::detect_backend_default`、`detect_backend`、`is_backend_available`
- `ralph_adapters::CliBackend`、`CliExecutor`
- `ralph_core::HatBackend`、`CliConfig`

## バックエンドの検出

PATH 内で利用可能なバックエンド（Claude、Kiro、Gemini、Codex、Forge、Amp、Copilot、OpenCode）を
検出します。

```rust
use ralph_adapters::{detect_backend_default, is_backend_available};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    if is_backend_available("claude") {
        println!("Claude CLI available");
    }

    let backend = detect_backend_default()?;
    println!("Selected backend: {backend}");

    Ok(())
}
```

## ハット設定からバックエンドを構築する

`HatBackend` は、`ralph.yml` で使われるハットごとのバックエンド定義です。実行のために
`CliBackend` へ変換できます。

```rust
use ralph_adapters::{CliBackend, CliExecutor};
use ralph_core::HatBackend;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let hat_backend = HatBackend::NamedWithArgs {
        backend_type: "claude".to_string(),
        args: vec!["--model".to_string(), "claude-sonnet-4".to_string()],
    };

    let backend = CliBackend::from_hat_backend(&hat_backend)?;
    let executor = CliExecutor::new(backend);

    let result = executor.execute_capture("Summarize the task in 3 bullets.").await?;
    if result.success {
        println!("{}", result.output);
    }

    Ok(())
}
```

## CLI 設定からバックエンドを構築する

トップレベルの設定から始めたい場合は `CliConfig` を使います。

```rust
use ralph_adapters::CliBackend;
use ralph_core::CliConfig;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = CliConfig {
        backend: "gemini".to_string(),
        ..Default::default()
    };

    let backend = CliBackend::from_config(&config)?;
    let (cmd, args, _stdin, _temp) = backend.build_command("Hello", false);

    println!("Command: {cmd}");
    println!("Args: {args:?}");

    Ok(())
}
```

## 実行結果

`CliExecutor` は `ExecutionResult` を返し、集約された出力・終了コード・タイムアウト状態を
含みます。

```rust
use ralph_adapters::{CliBackend, CliExecutor};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let backend = CliBackend::claude();
    let executor = CliExecutor::new(backend);

    let result = executor.execute_capture("List 5 project risks.").await?;
    println!("Success: {}", result.success);
    println!("Exit code: {:?}", result.exit_code);
    println!("Timed out: {}", result.timed_out);

    Ok(())
}
```
