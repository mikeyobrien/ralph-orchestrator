# 開発環境のセットアップ

Ralph の開発に向けて環境をセットアップします。

## 前提条件

### 必須

- **Rust 1.75 以降** — [rustup](https://rustup.rs/) でインストールする
- **Git** — バージョン管理のため

### 任意

- **少なくとも 1 つの AI CLI** — 統合テストのため（Claude、Kiro など）
- **tmux** — TUI テストのため
- **freeze** — TUI のスクリーンショット取得のため

## クローンとビルド

```bash
# クローンする
git clone https://github.com/mikeyobrien/ralph-orchestrator.git
cd ralph-orchestrator

# ビルドする
cargo build

# リリースビルド
cargo build --release
```

## git フックのインストール

```bash
./scripts/setup-hooks.sh
```

これは、CI の Rust チェックを反映した pre-commit フックをインストールします。

- `./scripts/sync-embedded-files.sh check`
- `cargo fmt --all -- --check`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test`

## セットアップの確認

```bash
# テストを実行する
cargo test

# スモークテストを実行する
cargo test -p ralph-core smoke_runner

# 整形を確認する
cargo fmt --check

# clippy を実行する
cargo clippy --all-targets --all-features
```

## プロジェクト構成

```
ralph-orchestrator/
├── crates/                    # Cargo ワークスペースのクレート
│   ├── ralph-proto/           # プロトコル型
│   ├── ralph-core/            # オーケストレーションエンジン
│   ├── ralph-adapters/        # CLI バックエンド
│   ├── ralph-tui/             # ターミナル UI
│   ├── ralph-cli/             # バイナリのエントリポイント
│   ├── ralph-e2e/             # E2E テスト
│   └── ralph-bench/           # ベンチマーク
├── .ralph/
│   ├── specs/                 # コミットされる開発スペックと設計
│   └── tasks/                 # コミットされるコードタスクファイル
├── presets/                   # ハットコレクションのプリセット
├── docs/                      # ドキュメント
├── scripts/                   # ユーティリティスクリプト
├── Cargo.toml                 # ワークスペースの設定
├── CLAUDE.md                  # AI エージェントの指示
└── README.md                  # プロジェクトの概要
```

## 開発ワークフロー

### 1. ブランチを作成する

```bash
git checkout -b feature/my-feature
```

### 2. 変更する

`crates/` のコードを編集します。

### 3. テストを実行する

```bash
cargo test
```

### 4. 整形と lint

```bash
cargo fmt
cargo clippy --all-targets --all-features
```

### 5. コミットする

```bash
git add .
git commit -m "feat: add my feature"
```

### 6. プッシュして PR

```bash
git push origin feature/my-feature
# GitHub で PR を開く
```

## Ralph をローカルで実行する

```bash
# ソースから
cargo run --bin ralph -- run -p "test prompt"

# リリースビルドで
cargo run --release --bin ralph -- run -p "test prompt"

# バイナリを直接
./target/release/ralph run -p "test prompt"
```

## フィクスチャでのテスト

スモークテストは JSONL フィクスチャを使います。

```bash
# スモークテストを実行する
cargo test -p ralph-core smoke_runner

# 新しいフィクスチャを記録する
cargo run --bin ralph -- run --record-session fixture.jsonl -p "your prompt"
```

## E2E テスト

稼働中の AI バックエンドが必要です。

```bash
# E2E テストを実行する
cargo run -p ralph-e2e -- claude

# デバッグモード
cargo run -p ralph-e2e -- claude --keep-workspace --verbose
```

## デバッグ

### 診断を有効にする

```bash
RALPH_DIAGNOSTICS=1 cargo run --bin ralph -- run -p "test"
```

### デバッグログ

```bash
RUST_LOG=debug cargo run --bin ralph -- run -p "test"
```

### GDB/LLDB

```bash
# デバッグ情報付きでビルドする
cargo build

# デバッグする
lldb ./target/debug/ralph -- run -p "test"
```

## IDE のセットアップ

### VS Code

拡張機能をインストールします。

- rust-analyzer
- Even Better TOML
- crates

### IntelliJ IDEA

プラグインをインストールします。

- Rust
- TOML

## よくある問題

### cargo build が失敗する

```bash
# Rust を更新する
rustup update

# クリーンして再ビルドする
cargo clean
cargo build
```

### テストが失敗する

```bash
# 出力付きで実行する
cargo test -- --nocapture

# 特定のテストを実行する
cargo test test_name
```

### clippy のエラー

```bash
# すべての警告を見る
cargo clippy --all-targets --all-features 2>&1 | less

# 自動修正する
cargo clippy --fix
```

## 次のステップ

- [コードスタイル](style.ja.md) ガイドを読む
- [テスト](testing.ja.md) について学ぶ
- [PR の提出](pull-requests.ja.md) を見る
