# 設定 API リファレンス

## 概要

設定は `ralph_core::RalphConfig` によって定義されます。YAML ファイルは次の両方をサポート
します。
- **v2 ネスト形式**（推奨）: `cli`、`event_loop`、`core`、`hats`、`events`
- **v1 フラット形式**（レガシー）: `agent`、`max_iterations`、`prompt_file` など

`RalphConfig::parse_yaml` / `RalphConfig::from_file` を使い、`normalize()` を呼んでレガシー
フィールドを v2 ネスト構造にマッピングします。

## YAML から設定を読み込む

```rust
use ralph_core::RalphConfig;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut config = RalphConfig::from_file("ralph.yml")?;
    config.normalize();

    println!("Backend: {}", config.cli.backend);
    println!("Max iterations: {}", config.event_loop.max_iterations);

    Ok(())
}
```

## メモリ上で YAML をパースする

```rust
use ralph_core::RalphConfig;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let yaml = r#"
cli:
  backend: claude
event_loop:
  max_iterations: 50
  max_runtime_seconds: 3600
hats:
  planner:
    name: "Planner"
    triggers: ["task.create"]
    publishes: ["plan.done"]
"#;

    let mut config = RalphConfig::parse_yaml(yaml)?;
    config.normalize();

    assert_eq!(config.cli.backend, "claude");
    assert_eq!(config.event_loop.max_iterations, 50);

    Ok(())
}
```

## プログラムによる上書き

読み込み後に特定のフィールドを上書きできます。

```rust
use ralph_core::RalphConfig;

fn main() {
    let mut config = RalphConfig::default();

    config.cli.backend = "gemini".to_string();
    config.event_loop.max_iterations = 25;
    config.event_loop.max_runtime_seconds = 900;

    // 任意: パス解決のためワークスペースルートを更新する
    config.core = config.core.with_workspace_root("/tmp/ralph-run");
}
```

## YAML でのハットバックエンド

バックエンドの選択は、`hats` 内の `HatBackend` で制御されます。

```yaml
hats:
  builder:
    name: "Builder"
    triggers: ["plan.done"]
    publishes: ["build.done"]
    backend:
      type: "kiro"
      agent: "builder"
      args: ["--verbose"]

  reviewer:
    name: "Reviewer"
    triggers: ["build.done"]
    publishes: ["review.done"]
    backend:
      type: "claude"
      args: ["--model", "claude-sonnet-4"]

  custom:
    name: "Custom"
    triggers: ["review.done"]
    backend:
      command: "/usr/local/bin/my-llm"
      args: ["--safe"]
```
