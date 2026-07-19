# Ralph Orchestrator へのコントリビューション

Ralph Orchestrator へのコントリビューションをご検討いただき、ありがとうございます！この
ドキュメントは、効果的に貢献するためのガイドラインと情報を提供します。

## 行動規範

このプロジェクトおよびその参加者すべては、[行動規範](CODE_OF_CONDUCT.md) によって
律せられます。参加することで、あなたはこの規範を守ることが期待されます。

## はじめに

### 前提条件

- [Rust](https://rustup.rs/) 1.75 以降
- 少なくとも 1 つの AI CLI バックエンド（[Claude Code](https://github.com/anthropics/claude-code)、[Kiro](https://kiro.dev/)、[Gemini CLI](https://github.com/google-gemini/gemini-cli) など）

### 開発環境のセットアップ

```bash
# リポジトリをクローンする
git clone https://github.com/mikeyobrien/ralph-orchestrator.git
cd ralph-orchestrator

# pre-commit と pre-push のチェック用に git フックをインストールする
./scripts/setup-hooks.sh

# プロジェクトをビルドする
cargo build

# テストを実行する
cargo test
```

## コントリビューションの方法

### バグの報告

バグ報告を作成する前に、重複を避けるため既存の issue を確認してください。バグ報告を
作成するときは、できるだけ多くの詳細を含めてください。

- **明確で説明的なタイトルを使う**
- **問題を再現する正確な手順を記述する**
- **具体的な例を提示する**（コードスニペット、設定ファイルなど）
- **観測した挙動と期待した挙動を記述する**
- **環境を含める**（OS、Rust のバージョン、バックエンド CLI のバージョン）

### 機能の提案

機能の提案を歓迎します！次をお願いします。

- その機能がすでに提案されていないか、**既存の issue を確認する**
- 機能とそのユースケースの**明確な説明を提示する**
- なぜこの機能がほとんどのユーザーにとって有用なのかを**説明する**

### プルリクエスト

1. **リポジトリをフォーク**し、`main` からブランチを作成する
2. 新機能の**テストを書く**
3. **コードスタイルに従う**（`cargo fmt` と `cargo clippy` を実行する）
4. 必要に応じて**ドキュメントを更新する**
5. 提出前に**すべてのテストが通ることを確認する**

#### プルリクエストのプロセス

1. リポジトリをフォークする
2. 機能ブランチを作成する（`git checkout -b feature/amazing-feature`）
3. 新機能のテストを書く
4. `cargo test` が通ることを確認する
5. `cargo clippy --all-targets --all-features` を実行する
6. `cargo fmt --check` を実行する
7. 変更をコミットする（`git commit -m 'Add amazing feature'`）
8. ブランチにプッシュする（`git push origin feature/amazing-feature`）
9. プルリクエストを開く

### コミットメッセージ

- 明確で説明的なコミットメッセージを使う
- 現在形の動詞で始める（"Added feature" ではなく "Add feature"）
- 該当する場合は issue を参照する（`Fixes #123`）

## 開発ガイドライン

### 哲学

完全な開発哲学については [AGENTS.md](AGENTS.md) を読んでください。主要な信条:

1. **フレッシュコンテキストは信頼性** - 各イテレーションはコンテキストをクリアする
2. **規定よりバックプレッシャー** - 悪い作業を拒否するゲートを作る
3. **計画は使い捨て** - 再生成は安い
4. **ディスクは状態、Git は記憶** - ファイルが引き継ぎの仕組みである

### コードスタイル

- コミット前に `cargo fmt` を実行する
- すべての `cargo clippy` 警告に対処する
- Rust のイディオムとベストプラクティスに従う
- 公開 API には doc コメントを付ける

### テスト

```bash
# すべてのテストを実行する
cargo test

# スモークテストを実行する（リプレイベース、API 呼び出しなし）
cargo test -p ralph-core smoke_runner

# カバレッジ付きで実行する（ローカルのみ — cargo-llvm-cov を使用）
just coverage          # 完全な HTML レポート → coverage/html/index.html
just coverage-summary  # 手早いターミナル要約
just coverage-badge-json  # README バッジで使う Shields ペイロードを生成
just coverage-open     # 生成してブラウザで開く
```

**重要**: コードを変更したら、必ずスモークテストを実行してください。スモークテストは記録
済みのフィクスチャを使い、速く・無料で・決定的です。

### カバレッジ

カバレッジはローカルのみで実行します。PR のフィードバックを速く安く保つため、意図的に CI
の一部にはしていません。`cargo-llvm-cov`（devenv シェルに含まれる）を使います。これは
`cargo test` がすでに行っているのと同じコンパイルを計装するため、別途のビルドペナルティは
ありません。

README バッジは、`main` へのプッシュ時に GitHub Actions から GitHub Pages 経由で自動的に
公開されます。ローカルでは、プッシュ前に確認するために同じ Shields ペイロードを生成
できます。

```bash
# devenv を使っていない場合は手動でインストールする
rustup component add llvm-tools-preview
cargo install cargo-llvm-cov

# カバレッジを生成する
just coverage

# CI で GitHub Pages が配信するのと同じローカルのバッジペイロードを生成する
just coverage-badge-json
```

### プロジェクト構成

```
ralph-orchestrator/
├── crates/
│   ├── ralph-cli/      # CLI アプリケーション
│   ├── ralph-core/     # コアライブラリ
│   ├── ralph-tui/      # ターミナル UI
│   ├── ralph-adapters/ # バックエンドアダプタ
│   └── ralph-e2e/      # エンドツーエンドのテスト
├── presets/            # 事前設定済みのハットコレクション
├── specs/              # 設計仕様
└── tasks/              # コードタスクファイル
```

### スペックの作成

重要な機能を実装する前に:

1. PDD 手法を用いて `specs/` にスペックを作成する
2. スペックのレビュー/承認を得る
3. スペックに従って実装する

基準: 新しいチームメンバーが、スペックとコードベースだけで実装できること。

### テストフィクスチャの記録

ライブセッションから新しいテストフィクスチャを作成するには:

```bash
# セッションを記録する
cargo run --bin ralph -- run -c ralph.claude.yml --record-session session.jsonl -p "your prompt"
```

フィクスチャの形式の詳細は `crates/ralph-core/tests/fixtures/` を参照してください。

## 避けるべきアンチパターン

- エージェントが処理できる機能をオーケストレーターに組み込む
- 複雑なリトライロジック（フレッシュコンテキストが復旧を担う）
- 詳細な逐次手順の指示（代わりにバックプレッシャーを使う）
- タスク選択時に作業のスコープを決める（計画作成時にスコープを決める）
- コードで確認せずに機能が欠けていると仮定する

## 助けが必要ですか？

- **Issues**: バグや機能要望には issue を開く
- **Discussions**: 質問には GitHub Discussions を使う
- **ドキュメント**: [ドキュメント](https://mikeyobrien.github.io/ralph-orchestrator/) を確認する

## ライセンス

コントリビュートすることで、あなたの貢献が MIT ライセンスの下でライセンスされることに
同意したものとみなされます。
