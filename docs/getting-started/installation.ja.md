# インストール

このガイドは、Ralph Orchestrator のすべてのインストール方法を説明します。

## 前提条件

### AI CLI ツール

Ralph が機能するには、少なくとも 1 つの AI CLI ツールが必要です。次のいずれかを
インストールしてください。

=== "Claude Code（推奨）"

    ```bash
    # npm 経由
    npm install -g @anthropic-ai/claude-code

    # またはセットアップ手順は https://claude.ai/code を参照
    ```

=== "Kiro"

    ```bash
    # インストールは https://kiro.dev/ を参照
    ```

=== "Gemini CLI"

    ```bash
    npm install -g @google/gemini-cli
    ```

=== "Codex"

    ```bash
    # https://github.com/openai/codex を参照
    ```

=== "Forge"

    ```bash
    curl -fsSL https://forgecode.dev/cli | sh
    ```

=== "Amp"

    ```bash
    # https://github.com/sourcegraph/amp を参照
    ```

=== "Copilot CLI"

    ```bash
    npm install -g @github/copilot
    ```

=== "OpenCode"

    ```bash
    curl -fsSL https://opencode.ai/install | bash
    ```

## Ralph のインストール

### npm 経由（推奨）

Ralph をインストールする最も簡単な方法です。

```bash
# グローバルにインストールする
npm install -g @ralph-orchestrator/ralph-cli

# または npx で直接実行する
npx @ralph-orchestrator/ralph-cli --version
```

### GitHub Releases インストーラ経由

```bash
curl --proto '=https' --tlsv1.2 -LsSf \
  https://github.com/mikeyobrien/ralph-orchestrator/releases/latest/download/ralph-cli-installer.sh | sh
```

### Cargo 経由

Rust をインストール済みの場合:

```bash
cargo install ralph-cli
```

### ソースから

最新の開発版が欲しい場合:

```bash
# リポジトリをクローンする
git clone https://github.com/mikeyobrien/ralph-orchestrator.git
cd ralph-orchestrator

# リリースバイナリをビルドする
cargo build --release

# PATH に追加する
export PATH="$PATH:$(pwd)/target/release"

# またはシンボリックリンクを作成する
sudo ln -s $(pwd)/target/release/ralph /usr/local/bin/ralph
```

## インストールの確認

```bash
# バージョンを確認する
ralph --version

# ヘルプを表示する
ralph --help

# 利用可能なプリセットを一覧する
ralph init --list-presets
```

## v1（レガシー）からの移行

レガシーの Ralph v1 をインストール済みの場合は、先にアンインストールしてください。

```bash
# pip でインストールした場合
pip uninstall ralph-orchestrator

# pipx でインストールした場合
pipx uninstall ralph-orchestrator

# uv でインストールした場合
uv tool uninstall ralph-orchestrator

# 削除を確認する
which ralph  # 何も返らないか、新しい Rust 版を指すはず
```

v1 リリースはもう保守されていません。詳細は
[v1 からの移行](../reference/migration-v1.ja.md) を参照してください。

## トラブルシューティング

### コマンドが見つからない

インストール後に `ralph` が見つからない場合:

```bash
# npm のグローバルインストールでは、npm の bin が PATH にあることを確認する
export PATH="$PATH:$(npm config get prefix)/bin"

# cargo のインストールでは
export PATH="$PATH:$HOME/.cargo/bin"
```

### AI エージェントが検出されない

Ralph は利用可能な AI CLI ツールを自動検出します。見つからない場合:

1. サポートされている AI CLI ツールのいずれかをインストールする（前提条件を参照）
2. そのツールが PATH にあることを確認する
3. AI CLI を直接実行して、動作することを確認する

### 権限が拒否される

権限エラーが出る場合:

```bash
# npm の場合
sudo npm install -g @ralph-orchestrator/ralph-cli

# シンボリックリンクの場合
sudo ln -s $(pwd)/target/release/ralph /usr/local/bin/ralph
```

## 次のステップ

Ralph がインストールできたので、[クイックスタート](quick-start.ja.md) ガイドに進んで
ください。
