# API リファレンス

Ralph のクレートに関する技術リファレンスドキュメントです。

## クレート概要

| クレート | 用途 | ドキュメント |
|-------|---------|---------------|
| [ralph-proto](ralph-proto.ja.md) | プロトコル型: Event, Hat, Topic | コアデータ構造 |
| [ralph-core](ralph-core.ja.md) | オーケストレーションエンジン | EventLoop, Config |
| [ralph-adapters](ralph-adapters.ja.md) | CLI バックエンド | バックエンド統合 |
| [ralph-tui](ralph-tui.ja.md) | ターミナル UI | TUI コンポーネント |
| [ralph-cli](ralph-cli.ja.md) | バイナリのエントリポイント | CLI コマンド |

## クイックリンク

### コア型

```rust
// イベント
use ralph_proto::{Event, Topic, EventBus};

// ハット
use ralph_proto::{Hat, HatId};

// 設定
use ralph_core::config::{Config, EventLoopConfig, CliConfig};
```

### よくある操作

```rust
// 設定を読み込む
let config = Config::load("ralph.yml")?;

// イベントループを作成する
let event_loop = EventLoop::new(config);

// オーケストレーションを実行する
event_loop.run().await?;
```

## Rust ドキュメント

Rust ドキュメントを生成して表示します。

```bash
# ドキュメントを生成する
cargo doc --no-deps --open

# 依存も含めて生成する
cargo doc --open
```

## 安定性

| クレート | 状態 |
|-------|--------|
| ralph-proto | 安定 |
| ralph-core | 安定 |
| ralph-adapters | 安定 |
| ralph-tui | 実験的 |
| ralph-cli | 安定 |
| ralph-e2e | 内部用 |
| ralph-bench | 内部用 |

「安定」は、公開 API が破壊的な変更を起こしにくいことを意味します。
「実験的」は、API が変わる可能性があることを意味します。
「内部用」は、そのクレートが外部利用を意図していないことを意味します。
