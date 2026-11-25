# インストールガイド

Ralph Orchestrator の包括的なインストール手順です。

## 前提条件

- **OS**: macOS、Linux、または Windows
- **Node.js**: 18 以降（npm インストールに必要）
- **Rust**: 1.70 以降（cargo インストールに必要）

## インストール方法

### 方法 1: npm（推奨）

```bash
npm install -g @ralph-orchestrator/ralph-cli
```

### 方法 2: GitHub Releases インストーラ

```bash
curl --proto '=https' --tlsv1.2 -LsSf \
  https://github.com/mikeyobrien/ralph-orchestrator/releases/latest/download/ralph-cli-installer.sh | sh
```

### 方法 3: Cargo

```bash
cargo install ralph-cli
```

### 方法 4: ビルド済みバイナリ（cargo-dist）

GitHub Releases から最新の `ralph-cli-<target>.tar.xz` 成果物をダウンロードし、展開して、
`ralph` を PATH に配置します。

```bash
# 例（プラットフォームに合った正しいアーカイブに置き換える）
mkdir -p ~/bin
curl -L -o ralph.tar.xz "<release-archive-url>"
tar -xJf ralph.tar.xz
mv ralph ~/bin/
export PATH="$HOME/bin:$PATH"
```

> Homebrew は現在、このリポジトリの自動リリースフローからは公開されていません。

## インストールの確認

```bash
ralph --version
```

## 次のステップ

- サポートされる AI バックエンド CLI を少なくとも 1 つインストールする（Claude Code、
  Gemini CLI、Forge、Copilot CLI など）
- バックエンドの API キーまたは認証を設定する
- クイックスタートガイドに従う: `getting-started/quick-start.ja.md`
