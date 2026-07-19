# ralph-core

オーケストレーションエンジン — Ralph の中核です。

## 概要

`ralph-core` は次を提供します。

- 設定の読み込みと検証
- メインのイベントループ
- メモリとタスクのストレージ
- イベントのパース
- 指示の組み立て

## 主要コンポーネント

### Config

YAML からの設定読み込みです。

```rust
use ralph_core::config::Config;

// ファイルから読み込む
let config = Config::load("ralph.yml")?;

// 既定値で読み込む
let config = Config::default();
```

**設定の構造:**

```rust
pub struct Config {
    pub cli: CliConfig,
    pub event_loop: EventLoopConfig,
    pub core: CoreConfig,
    pub memories: MemoryConfig,
    pub tasks: TaskConfig,
    pub hats: HashMap<String, HatConfig>,
}

pub struct EventLoopConfig {
    pub completion_promise: String,
    pub max_iterations: usize,
    pub max_runtime_seconds: u64,
    pub idle_timeout_secs: u64,
    pub starting_event: Option<String>,
    pub checkpoint_interval: usize,
    pub prompt_file: Option<String>,
}

pub struct CliConfig {
    pub backend: String,
    pub prompt_mode: PromptMode,
}

pub struct MemoryConfig {
    pub enabled: bool,
    pub inject: InjectMode,
    pub budget: usize,
    pub filter: MemoryFilter,
}

pub struct TaskConfig {
    pub enabled: bool,
}
```

### EventLoop

メインのオーケストレーションループです。

```rust
use ralph_core::EventLoop;

// 設定を使って作成する
let event_loop = EventLoop::new(config);

// オーケストレーションを実行する
let result = event_loop.run().await?;
```

**EventLoop のライフサイクル:**

1. 設定を読み込む
2. ハットを使って EventBus を初期化する
3. 開始イベントを発行する（構成されていれば）
4. ループ:
   - 次のイベントを取得する
   - 一致するハットを見つける
   - 指示を注入する
   - バックエンドを実行する
   - 出力をパースしてイベントを取り出す
   - 完了を確認する
5. 結果を返す

### MemoryStore

永続メモリ管理です。

```rust
use ralph_core::memory_store::MemoryStore;

let store = MemoryStore::new(".agent/memories.md");

// メモリを追加する
store.add(Memory {
    content: "Uses barrel exports".to_string(),
    memory_type: MemoryType::Pattern,
    tags: vec!["structure".to_string()],
})?;

// 検索する
let results = store.search("exports")?;

// 種類ごとに一覧する
let patterns = store.list_by_type(MemoryType::Pattern)?;
```

### TaskStore

ランタイムのタスク追跡です。

```rust
use ralph_core::task_store::TaskStore;

let store = TaskStore::new(".agent/tasks.jsonl");

// タスクを追加する
let id = store.add(Task {
    title: "Implement auth".to_string(),
    priority: 2,
    blocked_by: vec![],
})?;

// 準備できているタスクを取得する
let ready = store.ready()?;

// タスクを閉じる
store.close(&id)?;
```

### EventParser

エージェント出力からイベントをパースします。

```rust
use ralph_core::event_parser::EventParser;

let parser = EventParser::new();

// 出力をパースする
let events = parser.parse(agent_output)?;

// 完了を確認する
let complete = parser.is_complete(agent_output, "LOOP_COMPLETE");
```

**認識されるイベント形式:**

```bash
# CLI コマンド
ralph emit "build.done" "tests: pass, lint: pass, typecheck: pass, audit: pass, coverage: pass"

# JSON
{"event": "build.done", "payload": "tests: pass, lint: pass, typecheck: pass, audit: pass, coverage: pass"}
```

### Instructions

ハットの指示の組み立てです。

```rust
use ralph_core::instructions::InstructionBuilder;

let builder = InstructionBuilder::new();

// ハット向けの指示を組み立てる
let instructions = builder
    .with_base_prompt(&config.prompt_file)
    .with_guardrails(&config.guardrails)
    .with_memories(&memories)
    .with_hat_instructions(&hat.instructions)
    .build()?;
```

## テストサポート

### Smoke Runner

JSONL フィクスチャによるリプレイベースのテストです。

```rust
use ralph_core::testing::smoke_runner::SmokeRunner;

let runner = SmokeRunner::new("tests/fixtures/basic.jsonl");
let result = runner.run().await?;
assert!(result.completed);
```

### Session Recorder

リプレイ用にセッションを記録します。

```rust
use ralph_core::session_recorder::SessionRecorder;

let recorder = SessionRecorder::new("session.jsonl");
recorder.record_output("Hello")?;
recorder.record_tool_call("read_file", args)?;
recorder.finish()?;
```

## エラー型

```rust
pub enum CoreError {
    ConfigError(String),
    IoError(std::io::Error),
    ParseError(String),
    MemoryError(String),
    TaskError(String),
}
```

## フィーチャーフラグ

| フラグ | 説明 |
|------|-------------|
| `default` | 標準機能 |
| `testing` | テストユーティリティ |

## 例: カスタムイベントループ

```rust
use ralph_core::{Config, EventLoop};
use ralph_proto::{EventBus, Event};

#[tokio::main]
async fn main() -> Result<()> {
    // 設定を読み込む
    let config = Config::load("ralph.yml")?;

    // イベントループを作成する
    let mut event_loop = EventLoop::new(config);

    // 任意: カスタムイベントリスナーを追加する
    event_loop.on_event(|event| {
        println!("Event: {:?}", event.topic);
    });

    // 実行する
    let result = event_loop.run().await?;

    println!("Completed in {} iterations", result.iterations);
    Ok(())
}
```
