# ralph-adapters

各種 AI ツール向けの CLI バックエンド統合です。

## 概要

`ralph-adapters` は次を提供します。

- Claude、Kiro、Gemini などのバックエンド定義
- リアルタイム出力のための PTY ベース実行
- 異なる出力モード向けのストリームハンドラ
- 利用可能なバックエンドの自動検出

## サポートされるバックエンド

| バックエンド | CLI | 状態 |
|---------|-----|--------|
| Claude Code | `claude` | フルサポート |
| Kiro | `kiro` | フルサポート |
| Gemini CLI | `gemini` | フルサポート |
| Codex | `codex` | フルサポート |
| Forge | `forge` | フルサポート |
| Amp | `amp` | フルサポート |
| Copilot CLI | `copilot` | フルサポート |
| OpenCode | `opencode` | フルサポート |

## 主要コンポーネント

### CliBackend

バックエンド定義です。

```rust
pub struct CliBackend {
    pub name: String,
    pub command: String,
    pub prompt_mode: PromptMode,
    pub output_format: OutputFormat,
}

pub enum PromptMode {
    Arg,    // cli -p "prompt"
    Stdin,  // echo "prompt" | cli
    NoPrompt, // プロンプト注入なしの対話的 CLI
}

pub enum OutputFormat {
    Text,
    Ndjson,
    Custom(Box<dyn Parser>),
}
```

**組み込みバックエンド:**

```rust
use ralph_adapters::backends;

let claude = backends::claude();
let kiro = backends::kiro();
let gemini = backends::gemini();
```

### 自動検出

利用可能なバックエンドを検出します。

```rust
use ralph_adapters::auto_detect;

// 最初に見つかった利用可能なバックエンドを取得する
let backend = auto_detect::detect()?;

// すべての利用可能なバックエンドを取得する
let backends = auto_detect::detect_all();

// 特定のバックエンドを確認する
let available = auto_detect::is_available("claude");
```

**検出順序:**

1. Claude
2. Kiro
3. Gemini
4. Codex
5. Forge
6. Amp
7. Copilot
8. OpenCode

### PtyExecutor

リアルタイム出力のための PTY ベース実行です。

```rust
use ralph_adapters::pty_executor::PtyExecutor;

let executor = PtyExecutor::new();

// ストリームハンドラを使って実行する
let result = executor.execute(
    &backend,
    &prompt,
    Box::new(ConsoleStreamHandler::new()),
).await?;
```

### StreamHandler

バックエンドからの出力を処理します。

```rust
pub trait StreamHandler: Send {
    fn on_output(&mut self, chunk: &str);
    fn on_complete(&mut self);
    fn on_error(&mut self, error: &str);
}
```

**組み込みハンドラ:**

```rust
use ralph_adapters::stream_handler::*;

// コンソール出力（プレーン）
let handler = ConsoleStreamHandler::new();

// 整形された出力
let handler = PrettyStreamHandler::new();

// TUI モード
let handler = TuiStreamHandler::new(tx);

// 静音（CI モード）
let handler = QuietStreamHandler::new();
```

### Claude ストリームパーサー

Claude の NDJSON ストリーミング出力をパースします。

```rust
use ralph_adapters::claude_stream::ClaudeStreamParser;

let parser = ClaudeStreamParser::new();

// チャンクをパースする
let events = parser.parse_chunk(chunk)?;

for event in events {
    match event {
        ClaudeEvent::Text(text) => println!("{}", text),
        ClaudeEvent::ToolCall(call) => println!("Tool: {}", call.name),
        ClaudeEvent::ToolResult(result) => println!("Result: {}", result),
        ClaudeEvent::Complete => break,
    }
}
```

## カスタムバックエンド

カスタムのバックエンド定義を作成します。

```rust
use ralph_adapters::{CliBackend, PromptMode, OutputFormat};

let my_backend = CliBackend {
    name: "my-ai".to_string(),
    command: "my-ai-cli".to_string(),
    prompt_mode: PromptMode::Arg,
    output_format: OutputFormat::Text,
};
```

## カスタムストリームハンドラ

`StreamHandler` トレイトを実装します。

```rust
use ralph_adapters::StreamHandler;

struct MyHandler {
    buffer: String,
}

impl StreamHandler for MyHandler {
    fn on_output(&mut self, chunk: &str) {
        self.buffer.push_str(chunk);
        // カスタム処理
    }

    fn on_complete(&mut self) {
        println!("Done: {}", self.buffer);
    }

    fn on_error(&mut self, error: &str) {
        eprintln!("Error: {}", error);
    }
}
```

## エラー型

```rust
pub enum AdapterError {
    BackendNotFound(String),
    ExecutionError(String),
    ParseError(String),
    IoError(std::io::Error),
}
```

## フィーチャーフラグ

| フラグ | 説明 |
|------|-------------|
| `default` | すべてのバックエンド |
| `claude` | Claude サポートのみ |
| `kiro` | Kiro サポートのみ |

## 例: バックエンドを実行する

```rust
use ralph_adapters::{backends, PtyExecutor, ConsoleStreamHandler};

#[tokio::main]
async fn main() -> Result<()> {
    // Claude バックエンドを取得する
    let backend = backends::claude();

    // エグゼキュータを作成する
    let executor = PtyExecutor::new();

    // プロンプトを使って実行する
    let result = executor.execute(
        &backend,
        "Write a hello world function",
        Box::new(ConsoleStreamHandler::new()),
    ).await?;

    println!("Exit code: {}", result.exit_code);
    println!("Output: {}", result.output);

    Ok(())
}
```
